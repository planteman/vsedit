//! Async utilities, throttle, debounce.
//!
//! Equivalent to VS Code's `vs/base/common/async.ts`.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::sleep;

use vsedit_cancellation::{CancellationToken, CancellationTokenSource};

/// A throttled function that limits execution rate.
pub struct Throttle<F, T>
where
    F: Fn() -> T + Send + 'static,
    T: Send + 'static,
{
    func: Arc<F>,
    delay: Duration,
    last_run: Arc<Mutex<Option<std::time::Instant>>>,
}

impl<F, T> Throttle<F, T>
where
    F: Fn() -> T + Send + 'static,
    T: Send + 'static,
{
    /// Create a new throttle with the given delay.
    pub fn new(delay: Duration, func: F) -> Self {
        Self {
            func: Arc::new(func),
            delay,
            last_run: Arc::new(Mutex::new(None)),
        }
    }

    /// Execute the function if enough time has passed since the last execution.
    pub fn call(&self) -> Option<T> {
        let mut last = self.last_run.lock().unwrap();
        let now = std::time::Instant::now();

        if let Some(last_time) = *last {
            if now.duration_since(last_time) < self.delay {
                return None;
            }
        }

        *last = Some(now);
        drop(last);
        Some((self.func)())
    }
}

/// A debounced async function that delays execution until input stops.
pub struct Debounce {
    delay: Duration,
    cancel: Arc<Mutex<Option<CancellationTokenSource>>>,
}

impl Debounce {
    /// Create a new debounce with the given delay.
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            cancel: Arc::new(Mutex::new(None)),
        }
    }

    /// Schedule a debounced execution. Cancels any pending execution.
    pub async fn run<F, Fut>(&self, func: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        // Cancel previous
        {
            let mut cancel = self.cancel.lock().unwrap();
            if let Some(old) = cancel.take() {
                old.cancel();
            }
            let new_source = CancellationTokenSource::new();
            *cancel = Some(new_source);
        }

        let delay = self.delay;
        let cancel = self.cancel.clone();

        tokio::spawn(async move {
            sleep(delay).await;

            let should_run = {
                let cancel = cancel.lock().unwrap();
                cancel
                    .as_ref()
                    .map(|s| !s.is_cancelled())
                    .unwrap_or(false)
            };

            if should_run {
                func().await;
            }
        });
    }

    /// Cancel any pending debounced execution.
    pub fn cancel(&self) {
        let mut cancel = self.cancel.lock().unwrap();
        if let Some(source) = cancel.take() {
            source.cancel();
        }
    }
}

/// Run an async task with a timeout.
pub async fn with_timeout<F, T>(
    duration: Duration,
    future: F,
) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| TimeoutError)
}

/// Error returned when an operation times out.
#[derive(Debug, Clone)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for TimeoutError {}

/// A barrier that can be signaled once and awaited by multiple waiters.
pub struct Barrier {
    notify: Arc<Notify>,
    is_open: Arc<Mutex<bool>>,
}

impl Barrier {
    /// Create a new closed barrier.
    pub fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
            is_open: Arc::new(Mutex::new(false)),
        }
    }

    /// Open the barrier, waking all waiters.
    pub fn open(&self) {
        let mut is_open = self.is_open.lock().unwrap();
        *is_open = true;
        self.notify.notify_waiters();
    }

    /// Wait until the barrier is opened.
    pub async fn wait(&self) {
        loop {
            {
                if *self.is_open.lock().unwrap() {
                    return;
                }
            }
            self.notify.notified().await;
        }
    }

    /// Check if the barrier is open.
    pub fn is_open(&self) -> bool {
        *self.is_open.lock().unwrap()
    }
}

impl Default for Barrier {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a sequence of async tasks, collecting results.
pub async fn sequence<T, E>(
    tasks: Vec<Pin<Box<dyn Future<Output = Result<T, E>> + Send>>>,
) -> Result<Vec<T>, E> {
    let mut results = Vec::with_capacity(tasks.len());
    for task in tasks {
        results.push(task.await?);
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur in async utilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncError {
    /// The operation timed out.
    Timeout,
    /// The operation was cancelled.
    Cancelled,
    /// A retry budget was exhausted.
    RetriesExhausted {
        /// Number of attempts that were made.
        attempts: u32,
    },
    /// A generic async error with a message.
    Other(String),
}

impl std::fmt::Display for AsyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AsyncError::Timeout => write!(f, "operation timed out"),
            AsyncError::Cancelled => write!(f, "operation was cancelled"),
            AsyncError::RetriesExhausted { attempts } => {
                write!(f, "retries exhausted after {attempts} attempts")
            }
            AsyncError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AsyncError {}

impl From<TimeoutError> for AsyncError {
    fn from(_: TimeoutError) -> Self {
        AsyncError::Timeout
    }
}

// ---------------------------------------------------------------------------
// RetryPolicy – configurable retry with exponential back-off
// ---------------------------------------------------------------------------

/// Configuration for retrying a fallible async operation.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    /// Maximum number of attempts (must be ≥ 1).
    pub max_attempts: u32,
    /// Initial delay between retries.
    pub initial_delay: Duration,
    /// Multiplicative back-off factor applied after each attempt.
    pub backoff_factor: f64,
    /// Upper bound for any single delay.
    pub max_delay: Duration,
}

impl RetryPolicy {
    /// Create a policy with sensible defaults: 3 attempts, 100 ms initial,
    /// factor 2.0, max delay 5 s.
    pub fn new() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(5),
        }
    }

    /// Set the maximum number of attempts.
    pub fn with_max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n.max(1);
        self
    }

    /// Set the initial delay.
    pub fn with_initial_delay(mut self, d: Duration) -> Self {
        self.initial_delay = d;
        self
    }

    /// Set the back-off factor.
    pub fn with_backoff_factor(mut self, f: f64) -> Self {
        self.backoff_factor = f.max(1.0);
        self
    }

    /// Set the maximum per-attempt delay.
    pub fn with_max_delay(mut self, d: Duration) -> Self {
        self.max_delay = d;
        self
    }

    /// Compute the delay for a given zero-based attempt index.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let millis = self.initial_delay.as_millis() as f64
            * self.backoff_factor.powi(attempt as i32);
        let capped = Duration::from_millis(millis.min(self.max_delay.as_millis() as f64) as u64);
        capped
    }

    /// Validate that the policy is well-formed.
    pub fn validate(&self) -> Result<(), AsyncError> {
        if self.max_attempts == 0 {
            return Err(AsyncError::Other(
                "max_attempts must be at least 1".into(),
            ));
        }
        if self.backoff_factor < 1.0 {
            return Err(AsyncError::Other(
                "backoff_factor must be >= 1.0".into(),
            ));
        }
        Ok(())
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute `op` according to `policy`, retrying on `Err`.
pub async fn retry<F, Fut, T, E>(policy: &RetryPolicy, mut op: F) -> Result<T, AsyncError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0u32;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(_) if attempt + 1 >= policy.max_attempts => {
                return Err(AsyncError::RetriesExhausted {
                    attempts: attempt + 1,
                });
            }
            Err(_) => {
                let delay = policy.delay_for_attempt(attempt);
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CancellableFuture – run a future with a cancellation token
// ---------------------------------------------------------------------------

/// Run `future` until it completes or `token` is cancelled.
pub async fn with_cancellation<F, T>(
    mut token: CancellationToken,
    future: F,
) -> Result<T, AsyncError>
where
    F: Future<Output = T> + Send,
{
    tokio::select! {
        value = future => Ok(value),
        _ = token.cancelled() => Err(AsyncError::Cancelled),
    }
}

// ---------------------------------------------------------------------------
// AsyncQueue – bounded MPSC-style queue
// ---------------------------------------------------------------------------

/// A bounded async queue backed by a `tokio::sync::mpsc` channel.
pub struct AsyncQueue<T> {
    tx: tokio::sync::mpsc::Sender<T>,
    rx: Arc<Mutex<tokio::sync::mpsc::Receiver<T>>>,
    capacity: usize,
}

impl<T: Send + 'static> AsyncQueue<T> {
    /// Create a new queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let (tx, rx) = tokio::sync::mpsc::channel(cap);
        Self {
            tx,
            rx: Arc::new(Mutex::new(rx)),
            capacity: cap,
        }
    }

    /// The capacity of the queue.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Send a value into the queue, waiting if full.
    pub async fn send(&self, value: T) -> Result<(), AsyncError> {
        self.tx
            .send(value)
            .await
            .map_err(|_| AsyncError::Other("queue closed".into()))
    }

    /// Receive the next value, waiting if empty. Returns `None` when the
    /// sender half is dropped and the queue is empty.
    pub async fn recv(&self) -> Option<T> {
        let mut rx = self.rx.lock().unwrap();
        rx.recv().await
    }
}

impl<T: Send + 'static> std::fmt::Debug for AsyncQueue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncQueue")
            .field("capacity", &self.capacity)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Display impls for existing types
// ---------------------------------------------------------------------------

impl<F, T> std::fmt::Debug for Throttle<F, T>
where
    F: Fn() -> T + Send + 'static,
    T: Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Throttle")
            .field("delay", &self.delay)
            .finish()
    }
}

impl std::fmt::Debug for Debounce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Debounce")
            .field("delay", &self.delay)
            .finish()
    }
}

impl std::fmt::Debug for Barrier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Barrier")
            .field("is_open", &self.is_open())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// RunOnceScheduler – delayed single execution
// ---------------------------------------------------------------------------

/// A scheduler that delays a single execution, cancelling any previous pending one.
pub struct RunOnceScheduler {
    delay: Duration,
    pending: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl RunOnceScheduler {
    /// Create a new scheduler with the given delay.
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            pending: Arc::new(Mutex::new(None)),
        }
    }

    /// Schedule a function to execute after the delay, cancelling any pending execution.
    pub fn schedule<F>(&self, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut pending = self.pending.lock().unwrap();
        if let Some(handle) = pending.take() {
            handle.abort();
        }
        let delay = self.delay;
        *pending = Some(tokio::spawn(async move {
            sleep(delay).await;
            func();
        }));
    }

    /// Cancel any pending execution.
    pub fn cancel(&self) {
        let mut pending = self.pending.lock().unwrap();
        if let Some(handle) = pending.take() {
            handle.abort();
        }
    }

    /// Check if there is a pending execution.
    pub fn is_pending(&self) -> bool {
        let pending = self.pending.lock().unwrap();
        pending
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }
}

impl fmt::Debug for RunOnceScheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunOnceScheduler")
            .field("delay", &self.delay)
            .field("is_pending", &self.is_pending())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// IntervalRunner – periodic task execution
// ---------------------------------------------------------------------------

/// Runs a function repeatedly at a configurable interval.
pub struct IntervalRunner {
    interval: Arc<Mutex<Duration>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl IntervalRunner {
    /// Create a new interval runner with the given interval.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval: Arc::new(Mutex::new(interval)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Start running `func` repeatedly at the configured interval.
    pub fn start<F>(&self, func: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.stop();
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let running = self.running.clone();
        let interval = self.interval.clone();
        let handle = tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                let dur = { *interval.lock().unwrap() };
                sleep(dur).await;
                if running.load(std::sync::atomic::Ordering::SeqCst) {
                    func();
                }
            }
        });
        let mut h = self.handle.lock().unwrap();
        *h = Some(handle);
    }

    /// Stop the runner.
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let mut h = self.handle.lock().unwrap();
        if let Some(handle) = h.take() {
            handle.abort();
        }
    }

    /// Check if the runner is currently active.
    pub fn is_running(&self) -> bool {
        self.running
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Update the interval for the next tick.
    pub fn set_interval(&self, duration: Duration) {
        let mut interval = self.interval.lock().unwrap();
        *interval = duration;
    }
}

impl fmt::Debug for IntervalRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntervalRunner")
            .field("interval", &*self.interval.lock().unwrap())
            .field("is_running", &self.is_running())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// RateLimiter – token-bucket rate limiter
// ---------------------------------------------------------------------------

/// A simple token-bucket rate limiter.
pub struct RateLimiter {
    capacity: u32,
    tokens: Arc<Mutex<u32>>,
    refill_rate: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter with the given capacity and refill rate.
    pub fn new(capacity: u32, refill_rate: Duration) -> Self {
        Self {
            capacity,
            tokens: Arc::new(Mutex::new(capacity)),
            refill_rate,
        }
    }

    /// Try to acquire a token. Returns `true` if a token was available.
    pub fn try_acquire(&self) -> bool {
        let mut tokens = self.tokens.lock().unwrap();
        if *tokens > 0 {
            *tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Return the number of available tokens.
    pub fn available(&self) -> u32 {
        *self.tokens.lock().unwrap()
    }

    /// Reset tokens to full capacity.
    pub fn reset(&self) {
        let mut tokens = self.tokens.lock().unwrap();
        *tokens = self.capacity;
    }
}

impl fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimiter")
            .field("capacity", &self.capacity)
            .field("available", &self.available())
            .field("refill_rate", &self.refill_rate)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for async operations.
#[derive(Debug, Clone, PartialEq)]
pub struct AsyncStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl AsyncStats {
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
    pub fn merge(&mut self, other: &AsyncStats) {
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

impl Default for AsyncStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AsyncStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AsyncStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for async.
#[derive(Debug, Clone)]
pub struct AsyncValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl AsyncValidator {
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

impl Default for AsyncValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TaskPriority – priority levels for async tasks
// ---------------------------------------------------------------------------

/// Priority levels for scheduling async tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskPriority {
    /// Background work that can be deferred.
    Low = 0,
    /// Normal priority (default).
    Normal = 1,
    /// User-facing work that should complete promptly.
    High = 2,
    /// Critical work that must run before anything else.
    Critical = 3,
}

impl TaskPriority {
    /// Return all priority levels from lowest to highest.
    pub fn all() -> &'static [TaskPriority] {
        &[
            TaskPriority::Low,
            TaskPriority::Normal,
            TaskPriority::High,
            TaskPriority::Critical,
        ]
    }

    /// Return a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            TaskPriority::Low => "low",
            TaskPriority::Normal => "normal",
            TaskPriority::High => "high",
            TaskPriority::Critical => "critical",
        }
    }

    /// Return `true` if this priority is at least `High`.
    pub fn is_elevated(&self) -> bool {
        *self >= TaskPriority::High
    }
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// ConcurrencyLimiter – semaphore-based concurrency control
// ---------------------------------------------------------------------------

/// Limits the number of concurrently executing async operations using a
/// `tokio::sync::Semaphore`.
pub struct ConcurrencyLimiter {
    semaphore: Arc<tokio::sync::Semaphore>,
    max_permits: usize,
}

impl ConcurrencyLimiter {
    /// Create a limiter that allows at most `max_concurrent` tasks at once.
    pub fn new(max_concurrent: usize) -> Self {
        let max = max_concurrent.max(1);
        Self {
            semaphore: Arc::new(tokio::sync::Semaphore::new(max)),
            max_permits: max,
        }
    }

    /// Run `future` once a permit is available.  The permit is held for the
    /// duration of the future and released automatically.
    pub async fn run<F, T>(&self, future: F) -> T
    where
        F: Future<Output = T>,
    {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .expect("semaphore closed unexpectedly");
        future.await
    }

    /// Return the maximum number of concurrent permits.
    pub fn max_concurrent(&self) -> usize {
        self.max_permits
    }

    /// Return the number of permits currently available (not held).
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

impl fmt::Debug for ConcurrencyLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConcurrencyLimiter")
            .field("max_permits", &self.max_permits)
            .field("available", &self.available_permits())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// AsyncBatcher – collect items and flush as a batch
// ---------------------------------------------------------------------------

/// Collects items and flushes them in batches once the batch size is reached.
pub struct AsyncBatcher<T> {
    batch_size: usize,
    items: Arc<Mutex<Vec<T>>>,
    total_flushed: Arc<Mutex<u64>>,
}

impl<T: Send + 'static> AsyncBatcher<T> {
    /// Create a batcher that flushes every `batch_size` items.
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size: batch_size.max(1),
            items: Arc::new(Mutex::new(Vec::new())),
            total_flushed: Arc::new(Mutex::new(0)),
        }
    }

    /// Add an item. Returns `Some(batch)` if the batch is full and was
    /// drained, otherwise `None`.
    pub fn add(&self, item: T) -> Option<Vec<T>> {
        let mut items = self.items.lock().unwrap();
        items.push(item);
        if items.len() >= self.batch_size {
            let batch: Vec<T> = items.drain(..).collect();
            let mut total = self.total_flushed.lock().unwrap();
            *total += batch.len() as u64;
            Some(batch)
        } else {
            None
        }
    }

    /// Drain any remaining items regardless of batch size.
    pub fn flush(&self) -> Vec<T> {
        let mut items = self.items.lock().unwrap();
        let batch: Vec<T> = items.drain(..).collect();
        let mut total = self.total_flushed.lock().unwrap();
        *total += batch.len() as u64;
        batch
    }

    /// Return the number of items currently buffered.
    pub fn pending(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    /// Return the total number of items flushed so far.
    pub fn total_flushed(&self) -> u64 {
        *self.total_flushed.lock().unwrap()
    }

    /// Return the configured batch size.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

impl<T: Send + 'static> fmt::Debug for AsyncBatcher<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncBatcher")
            .field("batch_size", &self.batch_size)
            .field("pending", &self.pending())
            .field("total_flushed", &self.total_flushed())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// CircuitBreaker – circuit breaker pattern state machine
// ---------------------------------------------------------------------------

/// States of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation – requests flow through.
    Closed,
    /// Too many failures – requests are rejected immediately.
    Open,
    /// Tentatively allowing a single probe request.
    HalfOpen,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => f.write_str("closed"),
            CircuitState::Open => f.write_str("open"),
            CircuitState::HalfOpen => f.write_str("half-open"),
        }
    }
}

/// A circuit breaker that tracks consecutive failures and transitions between
/// [`CircuitState`] variants.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    failure_threshold: u32,
    success_threshold: u32,
    total_trips: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// * `failure_threshold` – consecutive failures before opening.
    /// * `success_threshold` – consecutive successes in half-open before closing.
    pub fn new(failure_threshold: u32, success_threshold: u32) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold: failure_threshold.max(1),
            success_threshold: success_threshold.max(1),
            total_trips: 0,
        }
    }

    /// Current state.
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Whether the breaker currently allows requests.
    pub fn is_call_permitted(&self) -> bool {
        match self.state {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => false,
        }
    }

    /// Record a successful call.
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    self.state = CircuitState::Closed;
                    self.success_count = 0;
                }
            }
            CircuitState::Closed => {}
            CircuitState::Open => {}
        }
    }

    /// Record a failed call.
    pub fn record_failure(&mut self) {
        self.success_count = 0;
        self.failure_count += 1;
        match self.state {
            CircuitState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitState::Open;
                    self.total_trips += 1;
                    self.failure_count = 0;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.total_trips += 1;
                self.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Manually transition from [`CircuitState::Open`] to
    /// [`CircuitState::HalfOpen`] (e.g. after a cooldown timer).
    pub fn attempt_reset(&mut self) {
        if self.state == CircuitState::Open {
            self.state = CircuitState::HalfOpen;
            self.failure_count = 0;
            self.success_count = 0;
        }
    }

    /// Manually force the breaker closed.
    pub fn force_close(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
    }

    /// Number of times the breaker has tripped open.
    pub fn total_trips(&self) -> u64 {
        self.total_trips
    }
}

// ---------------------------------------------------------------------------
// TaskDependencyGraph – directed acyclic dependency tracking
// ---------------------------------------------------------------------------

/// Tracks dependencies between named tasks and determines which tasks are
/// ready to execute (all dependencies satisfied).
#[derive(Debug, Clone)]
pub struct TaskDependencyGraph {
    /// Map from task id to its set of dependency ids.
    deps: std::collections::HashMap<String, Vec<String>>,
    /// Set of completed task ids.
    completed: std::collections::HashSet<String>,
}

impl TaskDependencyGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            deps: std::collections::HashMap::new(),
            completed: std::collections::HashSet::new(),
        }
    }

    /// Register a task with no dependencies.
    pub fn add_task(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.deps.entry(id).or_default();
    }

    /// Register that `task` depends on `dependency`.
    pub fn add_dependency(&mut self, task: impl Into<String>, dependency: impl Into<String>) {
        let task = task.into();
        let dep = dependency.into();
        self.deps.entry(dep.clone()).or_default();
        self.deps.entry(task.clone()).or_default().push(dep);
    }

    /// Mark a task as completed.
    pub fn complete(&mut self, id: &str) {
        self.completed.insert(id.to_string());
    }

    /// Return `true` if the given task has all its dependencies satisfied.
    pub fn is_ready(&self, id: &str) -> bool {
        if self.completed.contains(id) {
            return false; // already done
        }
        match self.deps.get(id) {
            Some(deps) => deps.iter().all(|d| self.completed.contains(d)),
            None => false, // unknown task
        }
    }

    /// Return all task ids that are ready to execute (dependencies met, not
    /// yet completed).
    pub fn ready_tasks(&self) -> Vec<String> {
        self.deps
            .keys()
            .filter(|id| self.is_ready(id))
            .cloned()
            .collect()
    }

    /// Return true if every registered task is completed.
    pub fn all_complete(&self) -> bool {
        self.deps.keys().all(|id| self.completed.contains(id))
    }

    /// Number of registered tasks.
    pub fn task_count(&self) -> usize {
        self.deps.len()
    }

    /// Number of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Detect whether adding `dependency` as a requirement of `task` would
    /// create a cycle. Uses a simple DFS.
    pub fn would_cycle(&self, task: &str, dependency: &str) -> bool {
        // A cycle exists if `task` is reachable from `dependency`.
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![dependency.to_string()];
        while let Some(current) = stack.pop() {
            if current == task {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(deps) = self.deps.get(&current) {
                    stack.extend(deps.iter().cloned());
                }
            }
        }
        false
    }
}

impl Default for TaskDependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TaskProgress – progress tracking for long-running tasks
// ---------------------------------------------------------------------------

/// Tracks progress of a long-running task.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskProgress {
    current: u64,
    total: u64,
    message: String,
}

impl TaskProgress {
    /// Create a new progress tracker with the given total work units.
    pub fn new(total: u64) -> Self {
        Self {
            current: 0,
            total: total.max(1),
            message: String::new(),
        }
    }

    /// Increment progress by `amount`.
    pub fn advance(&mut self, amount: u64) {
        self.current = self.current.saturating_add(amount).min(self.total);
    }

    /// Set progress to an absolute value.
    pub fn set(&mut self, value: u64) {
        self.current = value.min(self.total);
    }

    /// Update the human-readable status message.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
    }

    /// Percentage complete as a value in `[0.0, 100.0]`.
    pub fn percentage(&self) -> f64 {
        (self.current as f64 / self.total as f64) * 100.0
    }

    /// Whether the task is finished (`current >= total`).
    pub fn is_complete(&self) -> bool {
        self.current >= self.total
    }

    /// Current work units completed.
    pub fn current(&self) -> u64 {
        self.current
    }

    /// Total work units.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Current status message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Remaining work units.
    pub fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.current)
    }
}

impl fmt::Display for TaskProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}/{}] {:.1}%{}",
            self.current,
            self.total,
            self.percentage(),
            if self.message.is_empty() {
                String::new()
            } else {
                format!(" – {}", self.message)
            }
        )
    }
}

// ---------------------------------------------------------------------------
// AdaptiveTimeout – compute timeouts based on observed latencies
// ---------------------------------------------------------------------------

/// Computes adaptive timeouts based on a sliding window of observed durations.
#[derive(Debug, Clone)]
pub struct AdaptiveTimeout {
    samples: Vec<u64>,
    max_samples: usize,
    multiplier: f64,
    floor: Duration,
    ceiling: Duration,
}

impl AdaptiveTimeout {
    /// Create a new adaptive timeout calculator.
    ///
    /// * `max_samples` – how many recent observations to keep.
    /// * `multiplier`  – scale factor applied to the mean (e.g. 2.0 = 2× mean).
    /// * `floor`       – minimum timeout.
    /// * `ceiling`     – maximum timeout.
    pub fn new(max_samples: usize, multiplier: f64, floor: Duration, ceiling: Duration) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples: max_samples.max(1),
            multiplier: multiplier.max(1.0),
            floor,
            ceiling,
        }
    }

    /// Record an observed duration in milliseconds.
    pub fn record(&mut self, duration_ms: u64) {
        if self.samples.len() >= self.max_samples {
            self.samples.remove(0);
        }
        self.samples.push(duration_ms);
    }

    /// Compute the recommended timeout.  Returns `floor` if no samples exist.
    pub fn timeout(&self) -> Duration {
        if self.samples.is_empty() {
            return self.floor;
        }
        let sum: u64 = self.samples.iter().sum();
        let mean = sum as f64 / self.samples.len() as f64;
        let computed = Duration::from_millis((mean * self.multiplier) as u64);
        computed.max(self.floor).min(self.ceiling)
    }

    /// Number of observations recorded.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// The p95 estimate from the current samples (simple: sort and pick 95th
    /// percentile index).  Returns `None` if fewer than 2 samples.
    pub fn p95(&self) -> Option<Duration> {
        if self.samples.len() < 2 {
            return None;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let idx = ((sorted.len() as f64) * 0.95).ceil() as usize - 1;
        Some(Duration::from_millis(sorted[idx.min(sorted.len() - 1)]))
    }
}

// ---------------------------------------------------------------------------
// TaskResultCache – simple bounded cache keyed by string
// ---------------------------------------------------------------------------

/// A bounded cache for task results, evicting the oldest entry when full.
#[derive(Debug, Clone)]
pub struct TaskResultCache<V> {
    capacity: usize,
    /// Insertion-ordered entries.
    entries: Vec<(String, V)>,
}

impl<V: Clone> TaskResultCache<V> {
    /// Create a cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    /// Insert a key-value pair, evicting the oldest entry if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: V) {
        let key = key.into();
        // Update existing entry if present.
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries[pos].1 = value;
            return;
        }
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    /// Look up a cached value.
    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Remove a cached entry.
    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The configured capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Return all currently cached keys.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// race – return the first successful result from multiple futures
// ---------------------------------------------------------------------------

/// Run two futures concurrently and return the result of whichever completes
/// first.
pub async fn race<F1, F2, T>(a: F1, b: F2) -> T
where
    F1: Future<Output = T> + Send,
    F2: Future<Output = T> + Send,
    T: Send,
{
    tokio::select! {
        v = a => v,
        v = b => v,
    }
}


// ---------------------------------------------------------------------------
// AsyncTaskPool
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AsyncTaskPool {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl AsyncTaskPool {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for AsyncTaskPool {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for AsyncTaskPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "AsyncTaskPool({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// AsyncPriorityScheduler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AsyncPriorityScheduler {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl AsyncPriorityScheduler {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for AsyncPriorityScheduler {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for AsyncPriorityScheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "AsyncPriorityScheduler({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// AsyncTaskPoolSnapshot — point-in-time snapshot of AsyncTaskPool state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AsyncTaskPoolSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl AsyncTaskPoolSnapshot {
    pub fn capture(source: &AsyncTaskPool, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for AsyncTaskPoolSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// AsyncPrioritySchedulerStats — aggregate statistics for AsyncPriorityScheduler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AsyncPrioritySchedulerStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl AsyncPrioritySchedulerStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for AsyncPrioritySchedulerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// AsyncTaskPoolConfig — configuration for AsyncTaskPool
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AsyncTaskPoolConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl AsyncTaskPoolConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for AsyncTaskPoolConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for AsyncTaskPoolConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// BatchProgress — track progress of a batch operation
// ---------------------------------------------------------------------------

/// Tracks progress of a multi-item batch operation.
#[derive(Debug, Clone)]
pub struct BatchProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl BatchProgress {
    pub fn new(total: usize) -> Self {
        Self { total, completed: 0, failed: 0, skipped: 0 }
    }

    pub fn mark_completed(&mut self) { self.completed += 1; }
    pub fn mark_failed(&mut self) { self.failed += 1; }
    pub fn mark_skipped(&mut self) { self.skipped += 1; }

    /// Percentage of items processed (completed + failed + skipped) vs total.
    pub fn percentage(&self) -> f64 {
        if self.total == 0 { return 100.0; }
        let processed = self.completed + self.failed + self.skipped;
        (processed as f64 / self.total as f64) * 100.0
    }

    /// Returns true when all items are accounted for.
    pub fn is_done(&self) -> bool {
        self.completed + self.failed + self.skipped >= self.total
    }

    /// Fraction of completed items among processed items.
    pub fn success_rate(&self) -> f64 {
        let processed = self.completed + self.failed;
        if processed == 0 { return 1.0; }
        self.completed as f64 / processed as f64
    }

    /// How many items are still pending.
    pub fn remaining(&self) -> usize {
        self.total.saturating_sub(self.completed + self.failed + self.skipped)
    }
}

impl fmt::Display for BatchProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}/{}] {:.1}% (ok={}, fail={}, skip={})",
            self.completed + self.failed + self.skipped,
            self.total,
            self.percentage(),
            self.completed,
            self.failed,
            self.skipped,
        )
    }
}

// ---------------------------------------------------------------------------
// SlidingWindowLimiter — time-window rate limiting
// ---------------------------------------------------------------------------

/// Limits requests within a sliding time window.
#[derive(Debug, Clone)]
pub struct SlidingWindowLimiter {
    window_ms: u64,
    max_requests: usize,
    timestamps: Vec<u64>,
}

impl SlidingWindowLimiter {
    pub fn new(window_ms: u64, max_requests: usize) -> Self {
        Self {
            window_ms,
            max_requests,
            timestamps: Vec::new(),
        }
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    /// Try to acquire a slot. Returns `true` if allowed.
    pub fn try_acquire(&mut self, now_ms: u64) -> bool {
        self.prune(now_ms);
        if self.timestamps.len() < self.max_requests {
            self.timestamps.push(now_ms);
            true
        } else {
            false
        }
    }

    /// How many more requests can be made right now.
    pub fn remaining_at(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.max_requests.saturating_sub(self.timestamps.len())
    }

    /// Milliseconds until the next slot becomes available (0 if one is free now).
    pub fn next_available_in(&mut self, now_ms: u64) -> u64 {
        self.prune(now_ms);
        if self.timestamps.len() < self.max_requests {
            return 0;
        }
        // oldest timestamp in window determines when a slot opens
        if let Some(&oldest) = self.timestamps.first() {
            let opens_at = oldest + self.window_ms;
            opens_at.saturating_sub(now_ms)
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// RetryPolicy extra helpers
// ---------------------------------------------------------------------------

impl RetryPolicy {
    /// Whether another attempt should be made given current attempt index.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt + 1 < self.max_attempts
    }

    /// Upper bound on the total wait across all retry delays.
    pub fn total_max_wait(&self) -> Duration {
        let mut total = Duration::ZERO;
        for i in 0..self.max_attempts.saturating_sub(1) {
            total += self.delay_for_attempt(i);
        }
        total
    }
}


/// A coalescing timer that resets on each trigger, firing only after quiet period.
pub struct CoalescingTimer {
    quiet_period: Duration,
    last_trigger: Arc<Mutex<Option<std::time::Instant>>>,
    fire_count: Arc<Mutex<u64>>,
}

impl CoalescingTimer {
    pub fn new(quiet_period: Duration) -> Self {
        Self {
            quiet_period,
            last_trigger: Arc::new(Mutex::new(None)),
            fire_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn trigger(&self) {
        let mut last = self.last_trigger.lock().unwrap();
        *last = Some(std::time::Instant::now());
    }

    pub fn should_fire(&self) -> bool {
        let last = self.last_trigger.lock().unwrap();
        match *last {
            Some(t) => t.elapsed() >= self.quiet_period,
            None => false,
        }
    }

    pub fn fire(&self) {
        let mut count = self.fire_count.lock().unwrap();
        *count += 1;
        let mut last = self.last_trigger.lock().unwrap();
        *last = None;
    }

    pub fn fire_count(&self) -> u64 {
        *self.fire_count.lock().unwrap()
    }

    pub fn reset(&self) {
        let mut last = self.last_trigger.lock().unwrap();
        *last = None;
        let mut count = self.fire_count.lock().unwrap();
        *count = 0;
    }
}

/// Tracks execution statistics for async operations.
pub struct AsyncStatsTracker {
    completed: u64,
    failed: u64,
    total_duration_ms: u64,
}

impl AsyncStatsTracker {
    pub fn new() -> Self {
        Self { completed: 0, failed: 0, total_duration_ms: 0 }
    }

    pub fn record_success(&mut self, duration_ms: u64) {
        self.completed += 1;
        self.total_duration_ms += duration_ms;
    }

    pub fn record_failure(&mut self) {
        self.failed += 1;
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.completed + self.failed;
        if total == 0 { return 0.0; }
        self.completed as f64 / total as f64
    }

    pub fn average_duration_ms(&self) -> f64 {
        if self.completed == 0 { return 0.0; }
        self.total_duration_ms as f64 / self.completed as f64
    }

    pub fn total_operations(&self) -> u64 {
        self.completed + self.failed
    }
}

/// Priority-based task queue for ordered async execution.
pub struct PriorityTaskQueue {
    tasks: Vec<(u32, String)>,
}

impl PriorityTaskQueue {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn enqueue(&mut self, priority: u32, label: String) {
        self.tasks.push((priority, label));
        self.tasks.sort_by(|a, b| a.0.cmp(&b.0));
    }

    pub fn dequeue(&mut self) -> Option<(u32, String)> {
        if self.tasks.is_empty() { None } else { Some(self.tasks.remove(0)) }
    }

    pub fn peek(&self) -> Option<&(u32, String)> {
        self.tasks.first()
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    pub fn drain_by_priority(&mut self, max_priority: u32) -> Vec<(u32, String)> {
        let mut drained = Vec::new();
        self.tasks.retain(|(p, l)| {
            if *p <= max_priority {
                drained.push((*p, l.clone()));
                false
            } else {
                true
            }
        });
        drained
    }
}



// ---------------------------------------------------------------------------
// async – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for async task utilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YAsyncAsyncTaskState {
    Pending,
    Running,
    Completed,
    Cancelled,
}

impl YAsyncAsyncTaskState {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Pending => 0,
            Self::Running => 1,
            Self::Completed => 2,
            Self::Cancelled => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YAsyncAsyncTaskState] {
        &[
            YAsyncAsyncTaskState::Pending,
            YAsyncAsyncTaskState::Running,
            YAsyncAsyncTaskState::Completed,
            YAsyncAsyncTaskState::Cancelled,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YAsyncAsyncTaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks async barrier data.
#[derive(Debug, Clone)]
pub struct YAsyncAsyncBarrier {
    pub count: usize,
    pub waiting: usize,
    pub released: bool,
}

impl YAsyncAsyncBarrier {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            count: 0,
            waiting: 0,
            released: false,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YAsyncAsyncBarrier({}: {:?})", "count", self.count)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_async_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_async_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_async_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_async_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_async_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_async_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_async_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_async_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// async – Extended async semaphore helpers
// ---------------------------------------------------------------------------

/// Priority levels for async semaphore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZAsyncPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZAsyncPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZAsyncPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZAsyncPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks async semaphore data.
#[derive(Debug, Clone)]
pub struct ZAsyncAsyncSemaphore {
    pub waiters: Vec<u64>,
    pub permits: usize,
    pub max_permits: usize,
}

impl ZAsyncAsyncSemaphore {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            waiters: Vec::new(),
            permits: 0,
            max_permits: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.waiters.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.waiters.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.waiters.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZAsyncAsyncSemaphore[permits={:?}, max_permits={:?}]", self.permits, self.max_permits)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for async semaphore.
pub fn z_async_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_async_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_async_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_async_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_async_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_async_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_async_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 102
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer102 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer102 {
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
pub fn xb_fnv1a_102(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_102<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_102<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_102(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_102(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 5
// ---------------------------------------------------------------------------

/// Generic object pool `Xc5Pool<T>`.
pub struct Xc5Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc5Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc5PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc5Pool<T> {
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
    pub fn stats(&self) -> Xc5PoolStats {
        Xc5PoolStats {
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

impl<T> Default for Xc5Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc5Scheduler`.
pub struct Xc5Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc5Scheduler {
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

impl Default for Xc5Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_5 hash for the given byte slice.
pub fn xc_5_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_5 convention.
pub fn xc_5_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe115 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe115Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe115PipelineError {
    pub stage: Xe115Stage,
    pub message: String,
}

impl std::fmt::Display for Xe115PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe115Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe115Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError>>>,
    stage_names: Vec<Xe115Stage>,
}

impl Xe115Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe115Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe115Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe115Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe115Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> {
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

    pub fn compose(mut self, other: Xe115Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe115CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe115CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe115Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe115CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe115CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe115Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe115CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_115_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe115CacheEntry {
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

    fn xe_115_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe115CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_115_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> {
    Ok(data)
}

pub fn xe_115_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_115_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_115_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_115_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe115PipelineError> {
    Err(Xe115PipelineError {
        stage: Xe115Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_113: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg113Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg113Graph {
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

impl Default for Xg113Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_113: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg113Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg113Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg113Heap<T>) {
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

impl<T: Ord> Default for Xg113Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 4).
pub struct Xh4SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh4SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 46 as u64,
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

/// A compact bit set supporting boolean operations (variant 4).
pub struct Xh4BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh4BitSet {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_limits_rate() {
        let throttle = Throttle::new(Duration::from_millis(100), || 42);
        assert_eq!(throttle.call(), Some(42));
        assert_eq!(throttle.call(), None); // too soon
    }

    #[tokio::test]
    async fn barrier_works() {
        let barrier = Barrier::new();
        assert!(!barrier.is_open());
        barrier.open();
        assert!(barrier.is_open());
        barrier.wait().await; // should return immediately
    }

    #[tokio::test]
    async fn timeout_succeeds() {
        let result = with_timeout(Duration::from_secs(1), async { 42 }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn timeout_fails() {
        let result = with_timeout(Duration::from_millis(1), async {
            sleep(Duration::from_secs(10)).await;
            42
        })
        .await;
        assert!(result.is_err());
    }

    // ---- New tests ----

    #[test]
    fn async_error_display() {
        assert_eq!(AsyncError::Timeout.to_string(), "operation timed out");
        assert_eq!(AsyncError::Cancelled.to_string(), "operation was cancelled");
        assert_eq!(
            AsyncError::RetriesExhausted { attempts: 5 }.to_string(),
            "retries exhausted after 5 attempts"
        );
        assert_eq!(
            AsyncError::Other("boom".into()).to_string(),
            "boom"
        );
    }

    #[test]
    fn async_error_from_timeout() {
        let err: AsyncError = TimeoutError.into();
        assert_eq!(err, AsyncError::Timeout);
    }

    #[test]
    fn retry_policy_builder() {
        let policy = RetryPolicy::new()
            .with_max_attempts(5)
            .with_initial_delay(Duration::from_millis(200))
            .with_backoff_factor(3.0)
            .with_max_delay(Duration::from_secs(10));

        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.initial_delay, Duration::from_millis(200));
        assert!((policy.backoff_factor - 3.0).abs() < f64::EPSILON);
        assert_eq!(policy.max_delay, Duration::from_secs(10));
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn retry_policy_clamps_invalid_values() {
        let policy = RetryPolicy::new()
            .with_max_attempts(0)
            .with_backoff_factor(0.5);
        assert_eq!(policy.max_attempts, 1);
        assert!((policy.backoff_factor - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn retry_policy_delay_exponential() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_backoff_factor(2.0)
            .with_max_delay(Duration::from_secs(60));

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
    }

    #[test]
    fn retry_policy_delay_capped() {
        let policy = RetryPolicy::new()
            .with_initial_delay(Duration::from_millis(500))
            .with_backoff_factor(10.0)
            .with_max_delay(Duration::from_secs(2));

        // attempt 2 would be 500*100 = 50_000 ms → capped at 2000 ms
        assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(2));
    }

    #[tokio::test]
    async fn retry_succeeds_first_try() {
        let policy = RetryPolicy::new().with_max_attempts(3);
        let result: Result<i32, AsyncError> = retry(&policy, || async { Ok::<_, String>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retry_exhausts_attempts() {
        let policy = RetryPolicy::new()
            .with_max_attempts(2)
            .with_initial_delay(Duration::from_millis(1));
        let result: Result<i32, AsyncError> =
            retry(&policy, || async { Err::<i32, _>("fail") }).await;
        assert_eq!(
            result.unwrap_err(),
            AsyncError::RetriesExhausted { attempts: 2 }
        );
    }

    #[tokio::test]
    async fn with_cancellation_completes() {
        let cts = CancellationTokenSource::new();
        let token = cts.token();
        let result = with_cancellation(token, async { 99 }).await;
        assert_eq!(result.unwrap(), 99);
        drop(cts);
    }

    #[tokio::test]
    async fn with_cancellation_cancelled() {
        let cts = CancellationTokenSource::new();
        let token = cts.token();
        cts.cancel();
        let result = with_cancellation(token, async {
            sleep(Duration::from_secs(60)).await;
            1
        })
        .await;
        assert_eq!(result.unwrap_err(), AsyncError::Cancelled);
    }

    #[tokio::test]
    async fn async_queue_send_recv() {
        let queue = AsyncQueue::new(4);
        assert_eq!(queue.capacity(), 4);
        queue.send(1).await.unwrap();
        queue.send(2).await.unwrap();
        assert_eq!(queue.recv().await, Some(1));
        assert_eq!(queue.recv().await, Some(2));
    }

    #[test]
    fn debug_impls() {
        let throttle = Throttle::new(Duration::from_millis(50), || 0);
        let dbg = format!("{:?}", throttle);
        assert!(dbg.contains("Throttle"));

        let debounce = Debounce::new(Duration::from_millis(50));
        let dbg = format!("{:?}", debounce);
        assert!(dbg.contains("Debounce"));

        let barrier = Barrier::new();
        let dbg = format!("{:?}", barrier);
        assert!(dbg.contains("Barrier"));

        let queue = AsyncQueue::<i32>::new(8);
        let dbg = format!("{:?}", queue);
        assert!(dbg.contains("AsyncQueue"));
    }

    // ---- RunOnceScheduler / IntervalRunner / RateLimiter tests ----

    #[test]
    fn test_run_once_scheduler_creates() {
        let scheduler = RunOnceScheduler::new(Duration::from_millis(100));
        assert!(!scheduler.is_pending());
    }

    #[tokio::test]
    async fn test_run_once_scheduler_cancel() {
        let scheduler = RunOnceScheduler::new(Duration::from_secs(10));
        scheduler.schedule(|| {});
        scheduler.cancel();
        // After cancel the handle is taken, so is_pending returns false.
        assert!(!scheduler.is_pending());
    }

    #[tokio::test]
    async fn test_run_once_scheduler_is_pending_after_schedule() {
        let scheduler = RunOnceScheduler::new(Duration::from_secs(10));
        scheduler.schedule(|| {});
        assert!(scheduler.is_pending());
        scheduler.cancel();
    }

    #[tokio::test]
    async fn test_interval_runner_start_stop() {
        let runner = IntervalRunner::new(Duration::from_millis(50));
        runner.start(|| {});
        assert!(runner.is_running());
        runner.stop();
        assert!(!runner.is_running());
    }

    #[test]
    fn test_interval_runner_not_running_initially() {
        let runner = IntervalRunner::new(Duration::from_millis(100));
        assert!(!runner.is_running());
    }

    #[test]
    fn test_interval_runner_set_interval() {
        let runner = IntervalRunner::new(Duration::from_millis(100));
        runner.set_interval(Duration::from_millis(200));
        let interval = *runner.interval.lock().unwrap();
        assert_eq!(interval, Duration::from_millis(200));
    }

    #[test]
    fn test_rate_limiter_acquire() {
        let limiter = RateLimiter::new(3, Duration::from_millis(100));
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
    }

    #[test]
    fn test_rate_limiter_exhausted() {
        let limiter = RateLimiter::new(1, Duration::from_millis(100));
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(2, Duration::from_millis(100));
        limiter.try_acquire();
        limiter.try_acquire();
        assert!(!limiter.try_acquire());
        limiter.reset();
        assert!(limiter.try_acquire());
    }

    #[test]
    fn test_rate_limiter_available() {
        let limiter = RateLimiter::new(5, Duration::from_millis(100));
        assert_eq!(limiter.available(), 5);
        limiter.try_acquire();
        assert_eq!(limiter.available(), 4);
    }

    #[test]
    fn async_stats_new_defaults() {
        let stats = AsyncStatsTracker::new();
        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.average_duration_ms(), 0.0);
    }

    #[test]
    fn async_stats_record_success_v2() {
        let mut stats = AsyncStatsTracker::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total_operations(), 2);
        assert_eq!(stats.completed, 2);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.average_duration_ms(), 150.0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn async_stats_record_failure_v2() {
        let mut stats = AsyncStatsTracker::new();
        stats.record_success(100);
        stats.record_failure();
        assert_eq!(stats.total_operations(), 2);
        assert_eq!(stats.failed, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn async_stats_default() {
        let stats = AsyncStatsTracker::new();
        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.average_duration_ms(), 0.0);
    }

    #[test]
    fn async_validator_accepts_valid_name() {
        let v = AsyncValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn async_validator_rejects_empty() {
        let v = AsyncValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn async_validator_rejects_too_long() {
        let v = AsyncValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn async_validator_forbidden_prefix() {
        let v = AsyncValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn async_validator_allowed_chars() {
        let v = AsyncValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn async_validator_range() {
        let v = AsyncValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn async_sanitize_removes_control() {
        let result = AsyncValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn async_truncate_short_string() {
        assert_eq!(AsyncValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn async_truncate_long_string() {
        let result = AsyncValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn async_is_ascii_printable() {
        assert!(AsyncValidator::is_ascii_printable("Hello World 123"));
        assert!(!AsyncValidator::is_ascii_printable("Hello\x00World"));
    }

    // ---- TaskPriority tests ----

    #[test]
    fn task_priority_ordering() {
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Critical);
    }

    #[test]
    fn task_priority_default_is_normal() {
        assert_eq!(TaskPriority::default(), TaskPriority::Normal);
    }

    #[test]
    fn task_priority_is_elevated() {
        assert!(!TaskPriority::Low.is_elevated());
        assert!(!TaskPriority::Normal.is_elevated());
        assert!(TaskPriority::High.is_elevated());
        assert!(TaskPriority::Critical.is_elevated());
    }

    #[test]
    fn task_priority_label_and_display() {
        assert_eq!(TaskPriority::Low.label(), "low");
        assert_eq!(format!("{}", TaskPriority::Critical), "critical");
        assert_eq!(TaskPriority::all().len(), 4);
    }

    // ---- ConcurrencyLimiter tests ----

    #[tokio::test]
    async fn concurrency_limiter_runs_task() {
        let limiter = ConcurrencyLimiter::new(2);
        assert_eq!(limiter.max_concurrent(), 2);
        assert_eq!(limiter.available_permits(), 2);
        let result = limiter.run(async { 42 }).await;
        assert_eq!(result, 42);
        assert_eq!(limiter.available_permits(), 2);
    }

    #[tokio::test]
    async fn concurrency_limiter_clamps_zero() {
        let limiter = ConcurrencyLimiter::new(0);
        assert_eq!(limiter.max_concurrent(), 1);
    }

    // ---- AsyncBatcher tests ----

    #[test]
    fn batcher_collects_and_flushes_on_threshold() {
        let batcher = AsyncBatcher::new(3);
        assert_eq!(batcher.batch_size(), 3);

        assert!(batcher.add(1).is_none());
        assert!(batcher.add(2).is_none());
        assert_eq!(batcher.pending(), 2);

        let batch = batcher.add(3);
        assert!(batch.is_some());
        assert_eq!(batch.unwrap(), vec![1, 2, 3]);
        assert_eq!(batcher.pending(), 0);
        assert_eq!(batcher.total_flushed(), 3);
    }

    #[test]
    fn batcher_manual_flush_drains_partial() {
        let batcher = AsyncBatcher::new(10);
        batcher.add("a");
        batcher.add("b");
        let remaining = batcher.flush();
        assert_eq!(remaining, vec!["a", "b"]);
        assert_eq!(batcher.pending(), 0);
        assert_eq!(batcher.total_flushed(), 2);
    }

    #[test]
    fn batcher_debug_output() {
        let batcher = AsyncBatcher::<u8>::new(5);
        let dbg = format!("{:?}", batcher);
        assert!(dbg.contains("AsyncBatcher"));
        assert!(dbg.contains("batch_size"));
    }

    // ---- race tests ----

    #[tokio::test]
    async fn race_returns_first_result() {
        let result = race(async { 1 }, async { 2 }).await;
        // Either 1 or 2 is valid; both complete immediately.
        assert!(result == 1 || result == 2);
    }

    #[tokio::test]
    async fn race_fast_beats_slow() {
        let result = race(
            async { 42 },
            async {
                sleep(Duration::from_secs(60)).await;
                0
            },
        )
        .await;
        assert_eq!(result, 42);
    }

    // ---- CircuitBreaker tests ----

    #[test]
    fn circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new(3, 1);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_call_permitted());
        assert_eq!(cb.total_trips(), 0);
    }

    #[test]
    fn circuit_breaker_opens_after_threshold() {
        let mut cb = CircuitBreaker::new(2, 1);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_call_permitted());
        assert_eq!(cb.total_trips(), 1);
    }

    #[test]
    fn circuit_breaker_half_open_and_recovery() {
        let mut cb = CircuitBreaker::new(1, 2);
        cb.record_failure(); // opens
        assert_eq!(cb.state(), CircuitState::Open);
        cb.attempt_reset();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.is_call_permitted());
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen); // need 2 successes
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn circuit_breaker_half_open_failure_reopens() {
        let mut cb = CircuitBreaker::new(1, 3);
        cb.record_failure();
        cb.attempt_reset();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.total_trips(), 2);
    }

    #[test]
    fn circuit_breaker_force_close() {
        let mut cb = CircuitBreaker::new(1, 1);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        cb.force_close();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_call_permitted());
    }

    #[test]
    fn circuit_state_display() {
        assert_eq!(CircuitState::Closed.to_string(), "closed");
        assert_eq!(CircuitState::Open.to_string(), "open");
        assert_eq!(CircuitState::HalfOpen.to_string(), "half-open");
    }

    // ---- TaskDependencyGraph tests ----

    #[test]
    fn dep_graph_ready_tasks() {
        let mut g = TaskDependencyGraph::new();
        g.add_task("a");
        g.add_task("b");
        g.add_dependency("c", "a");
        g.add_dependency("c", "b");

        let ready = g.ready_tasks();
        assert!(ready.contains(&"a".to_string()));
        assert!(ready.contains(&"b".to_string()));
        assert!(!ready.contains(&"c".to_string()));

        g.complete("a");
        g.complete("b");
        assert!(g.is_ready("c"));
        assert!(!g.all_complete());
        g.complete("c");
        assert!(g.all_complete());
    }

    #[test]
    fn dep_graph_would_cycle() {
        let mut g = TaskDependencyGraph::new();
        g.add_dependency("b", "a");
        g.add_dependency("c", "b");
        // Adding a -> c would create a cycle a -> ... -> c -> a
        assert!(g.would_cycle("a", "c"));
        assert!(!g.would_cycle("d", "c"));
    }

    #[test]
    fn dep_graph_counts() {
        let mut g = TaskDependencyGraph::new();
        g.add_task("x");
        g.add_task("y");
        assert_eq!(g.task_count(), 2);
        assert_eq!(g.completed_count(), 0);
        g.complete("x");
        assert_eq!(g.completed_count(), 1);
    }

    // ---- TaskProgress tests ----

    #[test]
    fn task_progress_percentage() {
        let mut p = TaskProgress::new(200);
        assert!((p.percentage() - 0.0).abs() < f64::EPSILON);
        assert!(!p.is_complete());
        p.advance(100);
        assert!((p.percentage() - 50.0).abs() < f64::EPSILON);
        assert_eq!(p.remaining(), 100);
        p.set(200);
        assert!(p.is_complete());
        assert!((p.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn task_progress_display() {
        let mut p = TaskProgress::new(10);
        p.advance(3);
        p.set_message("loading");
        let s = format!("{p}");
        assert!(s.contains("3/10"));
        assert!(s.contains("loading"));
    }

    #[test]
    fn task_progress_clamps() {
        let mut p = TaskProgress::new(5);
        p.advance(100);
        assert_eq!(p.current(), 5);
        p.set(0);
        p.advance(3);
        p.advance(u64::MAX);
        assert_eq!(p.current(), 5);
    }

    // ---- AdaptiveTimeout tests ----

    #[test]
    fn adaptive_timeout_no_samples() {
        let at = AdaptiveTimeout::new(10, 2.0, Duration::from_millis(50), Duration::from_secs(5));
        assert_eq!(at.timeout(), Duration::from_millis(50));
        assert_eq!(at.sample_count(), 0);
        assert_eq!(at.p95(), None);
    }

    #[test]
    fn adaptive_timeout_computes_correctly() {
        let mut at =
            AdaptiveTimeout::new(10, 2.0, Duration::from_millis(10), Duration::from_secs(10));
        at.record(100);
        at.record(200);
        // mean = 150, * 2.0 = 300 ms
        assert_eq!(at.timeout(), Duration::from_millis(300));
        assert_eq!(at.sample_count(), 2);
    }

    #[test]
    fn adaptive_timeout_respects_ceiling() {
        let mut at =
            AdaptiveTimeout::new(5, 10.0, Duration::from_millis(10), Duration::from_millis(500));
        at.record(1000);
        // mean=1000, *10 = 10000 ms, capped to 500 ms
        assert_eq!(at.timeout(), Duration::from_millis(500));
    }

    #[test]
    fn adaptive_timeout_p95() {
        let mut at =
            AdaptiveTimeout::new(100, 2.0, Duration::from_millis(10), Duration::from_secs(60));
        for i in 1..=100 {
            at.record(i);
        }
        let p95 = at.p95().unwrap();
        assert_eq!(p95, Duration::from_millis(95));
    }

    // ---- TaskResultCache tests ----

    #[test]
    fn cache_insert_and_get() {
        let mut c = TaskResultCache::new(3);
        assert!(c.is_empty());
        c.insert("a", 1);
        c.insert("b", 2);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.get("z"), None);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn cache_evicts_oldest() {
        let mut c = TaskResultCache::new(2);
        c.insert("a", 1);
        c.insert("b", 2);
        c.insert("c", 3); // evicts "a"
        assert_eq!(c.get("a"), None);
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.get("c"), Some(&3));
    }

    #[test]
    fn cache_update_existing() {
        let mut c = TaskResultCache::new(3);
        c.insert("a", 1);
        c.insert("a", 99);
        assert_eq!(c.get("a"), Some(&99));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn cache_remove_and_clear() {
        let mut c = TaskResultCache::new(5);
        c.insert("x", 10);
        c.insert("y", 20);
        assert_eq!(c.remove("x"), Some(10));
        assert_eq!(c.len(), 1);
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn cache_keys() {
        let mut c = TaskResultCache::new(5);
        c.insert("a", 1);
        c.insert("b", 2);
        let keys = c.keys();
        assert!(keys.contains(&"a"));
        assert!(keys.contains(&"b"));
        assert_eq!(c.capacity(), 5);
    }

    #[test] fn asyncTaskPool_new() { let s = AsyncTaskPool::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn asyncTaskPool_add() { let mut s = AsyncTaskPool::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn asyncTaskPool_remove() { let mut s = AsyncTaskPool::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn asyncTaskPool_config() { let mut s = AsyncTaskPool::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn asyncTaskPool_nav() { let mut s = AsyncTaskPool::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn asyncTaskPool_filter() { let mut s = AsyncTaskPool::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn asyncTaskPool_display() { assert!(format!("{}", AsyncTaskPool::new()).contains("AsyncTaskPool")); }
    #[test] fn asyncPriorityScheduler_new() { let s = AsyncPriorityScheduler::new(); assert!(s.is_empty()); }
    #[test] fn asyncPriorityScheduler_add() { let mut s = AsyncPriorityScheduler::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn asyncPriorityScheduler_active() { let mut s = AsyncPriorityScheduler::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn asyncPriorityScheduler_error() { let mut s = AsyncPriorityScheduler::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn asyncPriorityScheduler_rm_group() { let mut s = AsyncPriorityScheduler::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn asyncPriorityScheduler_display() { assert!(format!("{}", AsyncPriorityScheduler::new()).contains("AsyncPriorityScheduler")); }


    #[test] fn asyncTaskPool_snap_capture() {
        let s = AsyncTaskPool::new();
        let snap = AsyncTaskPoolSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn asyncTaskPool_snap_stale() {
        let s = AsyncTaskPool::new();
        let snap = AsyncTaskPoolSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn asyncTaskPool_snap_diff() {
        let s = AsyncTaskPool::new();
        let s1v = AsyncTaskPoolSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn asyncTaskPool_snap_display() {
        let s = AsyncTaskPool::new();
        let snap = AsyncTaskPoolSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn asyncPriorityScheduler_stats_record() {
        let mut st = AsyncPrioritySchedulerStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn asyncPriorityScheduler_stats_hit_ratio() {
        let mut st = AsyncPrioritySchedulerStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn asyncPriorityScheduler_stats_merge() {
        let mut a = AsyncPrioritySchedulerStats::new();
        a.total_adds = 5;
        let mut b = AsyncPrioritySchedulerStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn asyncPriorityScheduler_stats_display() {
        let st = AsyncPrioritySchedulerStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn asyncTaskPool_config_default() {
        let c = AsyncTaskPoolConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn asyncTaskPool_config_builder() {
        let c = AsyncTaskPoolConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn asyncTaskPool_config_labels() {
        let mut c = AsyncTaskPoolConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn asyncTaskPool_config_cleanup_threshold() {
        let c = AsyncTaskPoolConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn asyncTaskPool_config_display() {
        assert!(format!("{}", AsyncTaskPoolConfig::new()).contains("Config"));
    }
    #[test] fn asyncPriorityScheduler_stats_peaks() {
        let mut st = AsyncPrioritySchedulerStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- BatchProgress -------------------------------------------------------

    #[test]
    fn batch_progress_starts_at_zero() {
        let bp = BatchProgress::new(10);
        assert_eq!(bp.percentage(), 0.0);
        assert!(!bp.is_done());
        assert_eq!(bp.remaining(), 10);
    }

    #[test]
    fn batch_progress_marks() {
        let mut bp = BatchProgress::new(4);
        bp.mark_completed();
        bp.mark_completed();
        bp.mark_failed();
        bp.mark_skipped();
        assert!(bp.is_done());
        assert_eq!(bp.percentage(), 100.0);
    }

    #[test]
    fn batch_progress_success_rate() {
        let mut bp = BatchProgress::new(10);
        bp.mark_completed();
        bp.mark_completed();
        bp.mark_failed();
        assert!((bp.success_rate() - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn batch_progress_display() {
        let mut bp = BatchProgress::new(5);
        bp.mark_completed();
        let s = format!("{bp}");
        assert!(s.contains("1/5"));
    }

    #[test]
    fn batch_progress_zero_total() {
        let bp = BatchProgress::new(0);
        assert!(bp.is_done());
        assert_eq!(bp.percentage(), 100.0);
    }

    // -- SlidingWindowLimiter ------------------------------------------------

    #[test]
    fn sliding_window_allows_within_limit() {
        let mut lim = SlidingWindowLimiter::new(1000, 3);
        assert!(lim.try_acquire(100));
        assert!(lim.try_acquire(200));
        assert!(lim.try_acquire(300));
        assert!(!lim.try_acquire(400)); // over limit
    }

    #[test]
    fn sliding_window_expires_old_entries() {
        let mut lim = SlidingWindowLimiter::new(1000, 2);
        assert!(lim.try_acquire(100));
        assert!(lim.try_acquire(200));
        assert!(!lim.try_acquire(500));
        // after window expires
        assert!(lim.try_acquire(1200));
    }

    #[test]
    fn sliding_window_remaining() {
        let mut lim = SlidingWindowLimiter::new(1000, 5);
        lim.try_acquire(100);
        lim.try_acquire(200);
        assert_eq!(lim.remaining_at(300), 3);
    }

    #[test]
    fn sliding_window_next_available() {
        let mut lim = SlidingWindowLimiter::new(1000, 1);
        lim.try_acquire(100);
        assert_eq!(lim.next_available_in(100), 1000);
        assert_eq!(lim.next_available_in(600), 500);
    }

    // -- RetryPolicy extra helpers -------------------------------------------

    #[test]
    fn retry_should_retry() {
        let rp = RetryPolicy::new().with_max_attempts(3);
        assert!(rp.should_retry(0));
        assert!(rp.should_retry(1));
        assert!(!rp.should_retry(2));
    }

    #[test]
    fn retry_total_max_wait() {
        let rp = RetryPolicy::new()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_millis(100))
            .with_backoff_factor(2.0)
            .with_max_delay(Duration::from_secs(10));
        let total = rp.total_max_wait();
        // attempt 0: 100ms, attempt 1: 200ms => total 300ms
        assert!(total >= Duration::from_millis(300));
    }

    #[test]
    fn retry_single_attempt_no_wait() {
        let rp = RetryPolicy::new().with_max_attempts(1);
        assert!(!rp.should_retry(0));
        assert_eq!(rp.total_max_wait(), Duration::ZERO);
    }


    #[test]
    fn coalescing_timer_no_trigger_no_fire() {
        let ct = CoalescingTimer::new(Duration::from_millis(50));
        assert!(!ct.should_fire());
        assert_eq!(ct.fire_count(), 0);
    }

    #[test]
    fn coalescing_timer_trigger_then_fire() {
        let ct = CoalescingTimer::new(Duration::from_millis(0));
        ct.trigger();
        assert!(ct.should_fire());
        ct.fire();
        assert_eq!(ct.fire_count(), 1);
    }

    #[test]
    fn coalescing_timer_reset() {
        let ct = CoalescingTimer::new(Duration::from_millis(0));
        ct.trigger();
        ct.fire();
        ct.reset();
        assert_eq!(ct.fire_count(), 0);
        assert!(!ct.should_fire());
    }

    #[test]
    fn async_stats_initial_state() {
        let stats = AsyncStatsTracker::new();
        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.average_duration_ms(), 0.0);
    }

    #[test]
    fn async_stats_record_success_v3() {
        let mut stats = AsyncStatsTracker::new();
        stats.record_success(50);
        stats.record_failure();
        assert_eq!(stats.success_rate(), 0.5);
        assert_eq!(stats.total_operations(), 2);
    }

    #[test]
    fn priority_queue_ordering() {
        let mut q = PriorityTaskQueue::new();
        q.enqueue(3, "low".into());
        q.enqueue(1, "high".into());
        q.enqueue(2, "med".into());
        assert_eq!(q.dequeue().unwrap().1, "high");
        assert_eq!(q.dequeue().unwrap().1, "med");
    }

    #[test]
    fn priority_queue_empty() {
        let mut q = PriorityTaskQueue::new();
        assert!(q.is_empty());
        assert!(q.dequeue().is_none());
        assert!(q.peek().is_none());
    }

    #[test]
    fn priority_queue_peek() {
        let mut q = PriorityTaskQueue::new();
        q.enqueue(5, "task".into());
        assert_eq!(q.peek().unwrap().0, 5);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn priority_queue_clear() {
        let mut q = PriorityTaskQueue::new();
        q.enqueue(1, "a".into());
        q.enqueue(2, "b".into());
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn priority_queue_drain_by_priority() {
        let mut q = PriorityTaskQueue::new();
        q.enqueue(1, "a".into());
        q.enqueue(5, "b".into());
        q.enqueue(3, "c".into());
        let drained = q.drain_by_priority(3);
        assert_eq!(drained.len(), 2);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn async_stats_all_failures() {
        let mut stats = AsyncStatsTracker::new();
        stats.record_failure();
        stats.record_failure();
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.average_duration_ms(), 0.0);
    }


    // -- async extended domain tests ----------------------------------------

    #[test]
    fn y_async_enum_index() {
        assert_eq!(YAsyncAsyncTaskState::Pending.index(), 0);
        assert_eq!(YAsyncAsyncTaskState::Running.index(), 1);
        assert_eq!(YAsyncAsyncTaskState::Completed.index(), 2);
        assert_eq!(YAsyncAsyncTaskState::Cancelled.index(), 3);
    }

    #[test]
    fn y_async_enum_label() {
        assert_eq!(YAsyncAsyncTaskState::Pending.label(), "Pending");
        assert_eq!(YAsyncAsyncTaskState::Running.label(), "Running");
        assert_eq!(YAsyncAsyncTaskState::Completed.label(), "Completed");
        assert_eq!(YAsyncAsyncTaskState::Cancelled.label(), "Cancelled");
    }

    #[test]
    fn y_async_enum_all() {
        let all = YAsyncAsyncTaskState::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_async_enum_is_default() {
        assert!(YAsyncAsyncTaskState::Pending.is_default());
        assert!(!YAsyncAsyncTaskState::Cancelled.is_default());
    }

    #[test]
    fn y_async_enum_display() {
        assert_eq!(format!("{}", YAsyncAsyncTaskState::Pending), "Pending");
    }

    #[test]
    fn y_async_struct_new() {
        let s = YAsyncAsyncBarrier::new();
        let _ = s.summary();
    }

    #[test]
    fn y_async_fingerprint_deterministic() {
        let h1 = y_async_fingerprint("hello");
        let h2 = y_async_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_async_fingerprint("a"), y_async_fingerprint("b"));
    }

    #[test]
    fn y_async_truncate_short() {
        assert_eq!(y_async_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_async_truncate_long() {
        let r = y_async_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_async_normalize_key_basic() {
        assert_eq!(y_async_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_async_split_path_basic() {
        let parts = y_async_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_async_count_occurrences_basic() {
        assert_eq!(y_async_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_async_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_async_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_async_in_range_basic() {
        assert!(y_async_in_range(5, 1, 10));
        assert!(y_async_in_range(1, 1, 10));
        assert!(y_async_in_range(10, 1, 10));
        assert!(!y_async_in_range(0, 1, 10));
        assert!(!y_async_in_range(11, 1, 10));
    }

    #[test]
    fn y_async_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_async_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_async_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_async_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- async Z-extended tests -----------------------------------------------

    #[test]
    fn z_async_priority_weight() {
        assert_eq!(ZAsyncPriority::Idle.weight(), 0);
        assert_eq!(ZAsyncPriority::Normal.weight(), 2);
        assert_eq!(ZAsyncPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_async_priority_label() {
        assert_eq!(ZAsyncPriority::Low.label(), "low");
        assert_eq!(ZAsyncPriority::High.label(), "high");
    }

    #[test]
    fn z_async_priority_is_elevated() {
        assert!(!ZAsyncPriority::Normal.is_elevated());
        assert!(ZAsyncPriority::High.is_elevated());
        assert!(ZAsyncPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_async_priority_display() {
        assert_eq!(format!("{}", ZAsyncPriority::Idle), "idle");
    }

    #[test]
    fn z_async_priority_all_asc() {
        let all = ZAsyncPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZAsyncPriority::Idle);
        assert_eq!(all[4], ZAsyncPriority::Realtime);
    }

    #[test]
    fn z_async_struct_new() {
        let s = ZAsyncAsyncSemaphore::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_async_struct_toggled_clone() {
        let s = ZAsyncAsyncSemaphore::new();
        let t = s.toggled_clone();
        let _ = t.max_permits;
    }

    #[test]
    fn z_async_rolling_hash_deterministic() {
        let h1 = z_async_rolling_hash(b"test");
        let h2 = z_async_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_async_rolling_hash(b"a"), z_async_rolling_hash(b"b"));
    }

    #[test]
    fn z_async_pad_to_basic() {
        assert_eq!(z_async_pad_to("hi", 5), "hi   ");
        assert_eq!(z_async_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_async_is_identifier_basic() {
        assert!(z_async_is_identifier("foo_bar"));
        assert!(z_async_is_identifier("abc123"));
        assert!(!z_async_is_identifier(""));
        assert!(!z_async_is_identifier("has space"));
    }

    #[test]
    fn z_async_levenshtein_basic() {
        assert_eq!(z_async_levenshtein("", ""), 0);
        assert_eq!(z_async_levenshtein("abc", "abc"), 0);
        assert_eq!(z_async_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_async_unique_words_basic() {
        let w = z_async_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_async_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_async_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_async_common_prefix_basic() {
        assert_eq!(z_async_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_async_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_async_struct_clear() {
        let mut s = ZAsyncAsyncSemaphore::new();
        s.waiters.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_async_rolling_hash_empty() {
        let h = z_async_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_102_push_and_len() {
        let mut rb = super::XbRingBuffer102::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_102_overwrite() {
        let mut rb = super::XbRingBuffer102::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_102_get_out_of_bounds() {
        let rb = super::XbRingBuffer102::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_102_drain_all() {
        let mut rb = super::XbRingBuffer102::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_102_peek_front_back() {
        let mut rb = super::XbRingBuffer102::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_102_clear() {
        let mut rb = super::XbRingBuffer102::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_102_capacity() {
        let rb = super::XbRingBuffer102::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_102_basic() {
        let h = super::xb_fnv1a_102(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_102(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_102_different_inputs() {
        let h1 = super::xb_fnv1a_102(b"abc");
        let h2 = super::xb_fnv1a_102(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_102_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_102(&data);
        let dec = super::xb_rle_decode_102(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_102_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_102(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_102(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_102_values() {
        assert!((super::xb_clamp_102(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_102(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_102(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_102_values() {
        assert!((super::xb_lerp_102(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_102(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_102(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_102_wrap_around_twice() {
        let mut rb = super::XbRingBuffer102::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 5 ----

    #[test]
    fn xc_5_pool_new_empty() {
        let pool: super::Xc5Pool<i32> = super::Xc5Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_5_pool_release_acquire() {
        let mut pool = super::Xc5Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_5_pool_acquire_empty() {
        let mut pool: super::Xc5Pool<i32> = super::Xc5Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_5_pool_full() {
        let mut pool = super::Xc5Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_5_pool_drain() {
        let mut pool = super::Xc5Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_5_pool_stats() {
        let mut pool = super::Xc5Pool::new(8);
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
    fn xc_5_pool_clear() {
        let mut pool = super::Xc5Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_5_pool_shrink() {
        let mut pool = super::Xc5Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_5_pool_default() {
        let pool: super::Xc5Pool<String> = super::Xc5Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_5_pool_extend() {
        let mut pool = super::Xc5Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_5_pool_retain() {
        let mut pool = super::Xc5Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_5_scheduler_round_robin() {
        let mut sched = super::Xc5Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_5_scheduler_empty() {
        let mut sched = super::Xc5Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_5_scheduler_reset() {
        let mut sched = super::Xc5Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_5_scheduler_add_remove() {
        let mut sched = super::Xc5Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_5_scheduler_targets() {
        let sched = super::Xc5Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_5_hash_empty() {
        assert_eq!(super::xc_5_hash(b""), 5381);
    }

    #[test]
    fn xc_5_hash_data() {
        let h = super::xc_5_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_5_hash(b"hello"), h);
    }

    #[test]
    fn xc_5_reverse_str() {
        assert_eq!(super::xc_5_reverse("abc"), "cba");
        assert_eq!(super::xc_5_reverse(""), "");
    }


    #[test]
    fn xe_115_pipeline_empty() {
        let p = super::Xe115Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_115_pipeline_parse_stage() {
        let p = super::Xe115Pipeline::new()
            .add_parse(super::xe_115_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_115_pipeline_transform_double() {
        let p = super::Xe115Pipeline::new()
            .add_transform(super::xe_115_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_115_pipeline_validate_reverse() {
        let p = super::Xe115Pipeline::new()
            .add_validate(super::xe_115_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_115_pipeline_emit_filter() {
        let p = super::Xe115Pipeline::new()
            .add_emit(super::xe_115_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_115_pipeline_multi_stage() {
        let p = super::Xe115Pipeline::new()
            .add_parse(super::xe_115_pipeline_identity)
            .add_transform(super::xe_115_pipeline_double)
            .add_validate(super::xe_115_pipeline_reverse)
            .add_emit(super::xe_115_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_115_pipeline_error_propagation() {
        let p = super::Xe115Pipeline::new()
            .add_parse(super::xe_115_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe115Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_115_pipeline_compose() {
        let p1 = super::Xe115Pipeline::new()
            .add_parse(super::xe_115_pipeline_identity);
        let p2 = super::Xe115Pipeline::new()
            .add_transform(super::xe_115_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_115_pipeline_error_display() {
        let e = super::Xe115PipelineError {
            stage: super::Xe115Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_115_cache_put_get() {
        let mut c = super::Xe115Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_115_cache_miss() {
        let mut c: super::Xe115Cache<&str, i32> = super::Xe115Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_115_cache_ttl_expiry() {
        let mut c = super::Xe115Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_115_cache_evict() {
        let mut c = super::Xe115Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_115_cache_capacity() {
        let mut c = super::Xe115Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_115_cache_stats() {
        let mut c = super::Xe115Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_115_cache_clear() {
        let mut c = super::Xe115Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_113 graph tests ------------------------------------------------

    #[test]
    fn xg_113_graph_empty() {
        let g = super::Xg113Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_113_graph_add_node() {
        let mut g = super::Xg113Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_113_graph_add_edge() {
        let mut g = super::Xg113Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_113_graph_neighbors() {
        let mut g = super::Xg113Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_113_graph_has_path() {
        let mut g = super::Xg113Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_113_graph_self_path() {
        let g = super::Xg113Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_113_graph_topo_sort() {
        let mut g = super::Xg113Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_113_graph_cycle_detect_false() {
        let mut g = super::Xg113Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_113_graph_cycle_detect_true() {
        let mut g = super::Xg113Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_113 heap tests -------------------------------------------------

    #[test]
    fn xg_113_heap_empty() {
        let h: super::Xg113Heap<i32> = super::Xg113Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_113_heap_push_pop() {
        let mut h = super::Xg113Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_113_heap_peek() {
        let mut h = super::Xg113Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_113_heap_drain_sorted() {
        let mut h = super::Xg113Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_113_heap_merge() {
        let mut a = super::Xg113Heap::new();
        let mut b = super::Xg113Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_113_heap_default() {
        let h: super::Xg113Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_113_graph_default() {
        let g: super::Xg113Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh4_skip_insert_contains() {
        let mut sl = super::Xh4SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh4_skip_remove() {
        let mut sl = super::Xh4SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh4_skip_len() {
        let mut sl = super::Xh4SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh4_skip_range_query() {
        let mut sl = super::Xh4SkipList::xh_new(4);
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
    fn xh4_skip_floor_ceiling() {
        let mut sl = super::Xh4SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh4_skip_rank() {
        let mut sl = super::Xh4SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh4_skip_empty() {
        let sl = super::Xh4SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh4_skip_duplicates() {
        let mut sl = super::Xh4SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh4_bitset_set_test() {
        let mut bs = super::Xh4BitSet::xh_new(256);
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
    fn xh4_bitset_clear_count() {
        let mut bs = super::Xh4BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh4_bitset_and_or_xor() {
        let mut a = super::Xh4BitSet::xh_new(128);
        let mut b = super::Xh4BitSet::xh_new(128);
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
    fn xh4_bitset_iter_ones() {
        let mut bs = super::Xh4BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh4_bitset_first_last() {
        let mut bs = super::Xh4BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh4_bitset_empty() {
        let bs = super::Xh4BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
