//! Cancellation token system.
//!
//! Equivalent to VS Code's `vs/base/common/cancellation.ts`.
//! Provides cooperative cancellation for async operations.

use tokio::sync::watch;

/// A token that can be checked for cancellation.
#[derive(Clone)]
pub struct CancellationToken {
    receiver: watch::Receiver<bool>,
}

impl CancellationToken {
    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    /// Wait until cancellation is requested.
    pub async fn cancelled(&mut self) {
        while !*self.receiver.borrow_and_update() {
            if self.receiver.changed().await.is_err() {
                return;
            }
        }
    }

    /// A token that is never cancelled.
    pub fn none() -> Self {
        let (_, receiver) = watch::channel(false);
        Self { receiver }
    }

    /// A token that is already cancelled.
    pub fn cancelled_token() -> Self {
        let (sender, receiver) = watch::channel(false);
        let _ = sender.send(true);
        Self { receiver }
    }
}

/// Source that creates cancellation tokens and can trigger cancellation.
pub struct CancellationTokenSource {
    sender: watch::Sender<bool>,
    token: CancellationToken,
}

impl CancellationTokenSource {
    /// Create a new cancellation token source.
    pub fn new() -> Self {
        let (sender, receiver) = watch::channel(false);
        Self {
            sender,
            token: CancellationToken { receiver },
        }
    }

    /// Get a cancellation token from this source.
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        let _ = self.sender.send(true);
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl Default for CancellationTokenSource {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CancellationTokenSource {
    fn drop(&mut self) {
        // Cancel on drop to unblock any waiters
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellation() {
        let source = CancellationTokenSource::new();
        let token = source.token();
        assert!(!token.is_cancelled());
        source.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_none_token() {
        let token = CancellationToken::none();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_already_cancelled() {
        let token = CancellationToken::cancelled_token();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_drop_cancels() {
        let source = CancellationTokenSource::new();
        let token = source.token();
        drop(source);
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_await_cancellation() {
        let source = CancellationTokenSource::new();
        let mut token = source.token();

        let handle = tokio::spawn(async move {
            token.cancelled().await;
            true
        });

        tokio::task::yield_now().await;
        source.cancel();
        assert!(handle.await.unwrap());
    }
}
