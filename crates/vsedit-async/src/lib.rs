//! Async utilities, throttle, debounce.
//!
//! Equivalent to VS Code's `vs/base/common/async.ts`.

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
// Tests
// ---------------------------------------------------------------------------

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
}
