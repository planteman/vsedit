//! Async utilities, throttle, debounce.
//!
//! Equivalent to VS Code's `vs/base/common/async.ts`.

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
        let stats = AsyncStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn async_stats_record_success() {
        let mut stats = AsyncStats::new();
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
    fn async_stats_record_failure() {
        let mut stats = AsyncStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn async_stats_reset() {
        let mut stats = AsyncStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn async_stats_merge() {
        let mut a = AsyncStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = AsyncStats::new();
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
    fn async_stats_display() {
        let mut stats = AsyncStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn async_stats_default() {
        let stats = AsyncStats::default();
        assert_eq!(stats.total(), 0);
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
}
