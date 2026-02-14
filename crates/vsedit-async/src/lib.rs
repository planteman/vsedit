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
}
