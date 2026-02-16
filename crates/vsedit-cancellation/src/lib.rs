//! Cancellation token system.
//!
//! Equivalent to VS Code's `vs/base/common/cancellation.ts`.
//! Provides cooperative cancellation for async operations.

use std::fmt;
use std::time::Duration;

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

    /// Create a token that auto-cancels after the given duration.
    pub fn with_timeout(duration: Duration) -> (CancellationTokenSource, Self) {
        let source = CancellationTokenSource::new();
        let token = source.token();
        let sender_clone = source.sender.clone();
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let _ = sender_clone.send(true);
        });
        (source, token)
    }
}

impl fmt::Display for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CancellationToken(cancelled={})",
            self.is_cancelled()
        )
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

    /// Reset the source so that its tokens are no longer cancelled.
    /// Existing tokens will observe the reset state.
    pub fn reset(&self) {
        let _ = self.sender.send(false);
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

impl fmt::Debug for CancellationTokenSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancellationTokenSource")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Errors that can occur when working with cancellation tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellationError {
    /// The token was already cancelled before the operation began.
    AlreadyCancelled,
    /// The cancellation source was dropped, implicitly cancelling the token.
    SourceDropped,
}

impl fmt::Display for CancellationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CancellationError::AlreadyCancelled => {
                write!(f, "operation cancelled: token was already cancelled")
            }
            CancellationError::SourceDropped => {
                write!(f, "operation cancelled: cancellation source was dropped")
            }
        }
    }
}

impl std::error::Error for CancellationError {}

/// A cancellation token source that is cancelled when ANY of its parent tokens
/// are cancelled. Useful for composing multiple cancellation scopes.
pub struct LinkedCancellationTokenSource {
    source: CancellationTokenSource,
    _tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl LinkedCancellationTokenSource {
    /// Create a linked source that cancels when any of the provided parent
    /// tokens are cancelled.
    pub fn new(parents: &[CancellationToken]) -> Self {
        let source = CancellationTokenSource::new();
        let mut tasks = Vec::with_capacity(parents.len());

        for parent in parents {
            let mut parent_clone = parent.clone();
            let child_sender = source.sender.clone();
            let handle = tokio::spawn(async move {
                parent_clone.cancelled().await;
                let _ = child_sender.send(true);
            });
            tasks.push(handle);
        }

        Self {
            source,
            _tasks: tasks,
        }
    }

    /// Get a cancellation token from this linked source.
    pub fn token(&self) -> CancellationToken {
        self.source.token()
    }

    /// Check if cancellation has been requested (either directly or via a parent).
    pub fn is_cancelled(&self) -> bool {
        self.source.is_cancelled()
    }

    /// Directly cancel this linked source regardless of parent state.
    pub fn cancel(&self) {
        self.source.cancel();
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

    #[test]
    fn test_cancellation_error_display() {
        let err = CancellationError::AlreadyCancelled;
        assert_eq!(
            err.to_string(),
            "operation cancelled: token was already cancelled"
        );

        let err = CancellationError::SourceDropped;
        assert_eq!(
            err.to_string(),
            "operation cancelled: cancellation source was dropped"
        );
    }

    #[test]
    fn test_cancellation_error_eq() {
        assert_eq!(CancellationError::AlreadyCancelled, CancellationError::AlreadyCancelled);
        assert_ne!(CancellationError::AlreadyCancelled, CancellationError::SourceDropped);
    }

    #[test]
    fn test_cancellation_error_is_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(CancellationError::AlreadyCancelled);
        assert!(err.to_string().contains("already cancelled"));
    }

    #[test]
    fn test_token_display_not_cancelled() {
        let token = CancellationToken::none();
        assert_eq!(format!("{}", token), "CancellationToken(cancelled=false)");
    }

    #[test]
    fn test_token_display_cancelled() {
        let token = CancellationToken::cancelled_token();
        assert_eq!(format!("{}", token), "CancellationToken(cancelled=true)");
    }

    #[test]
    fn test_source_debug() {
        let source = CancellationTokenSource::new();
        let debug_str = format!("{:?}", source);
        assert!(debug_str.contains("CancellationTokenSource"));
        assert!(debug_str.contains("is_cancelled"));
    }

    #[test]
    fn test_source_reset() {
        let source = CancellationTokenSource::new();
        let token = source.token();
        assert!(!token.is_cancelled());

        source.cancel();
        assert!(token.is_cancelled());

        source.reset();
        assert!(!token.is_cancelled());
        assert!(!source.is_cancelled());
    }

    #[test]
    fn test_source_reset_and_recancel() {
        let source = CancellationTokenSource::new();
        let token = source.token();

        source.cancel();
        assert!(token.is_cancelled());

        source.reset();
        assert!(!token.is_cancelled());

        source.cancel();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_linked_cancellation_first_parent() {
        let parent1 = CancellationTokenSource::new();
        let parent2 = CancellationTokenSource::new();
        let linked =
            LinkedCancellationTokenSource::new(&[parent1.token(), parent2.token()]);
        let token = linked.token();

        assert!(!token.is_cancelled());
        parent1.cancel();
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_linked_cancellation_second_parent() {
        let parent1 = CancellationTokenSource::new();
        let parent2 = CancellationTokenSource::new();
        let linked =
            LinkedCancellationTokenSource::new(&[parent1.token(), parent2.token()]);
        let token = linked.token();

        assert!(!token.is_cancelled());
        parent2.cancel();
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_linked_direct_cancel() {
        let parent = CancellationTokenSource::new();
        let linked = LinkedCancellationTokenSource::new(&[parent.token()]);
        let token = linked.token();

        assert!(!linked.is_cancelled());
        linked.cancel();
        assert!(token.is_cancelled());
        assert!(linked.is_cancelled());
    }

    #[tokio::test]
    async fn test_with_timeout_cancels() {
        let (_source, mut token) =
            CancellationToken::with_timeout(Duration::from_millis(50));
        assert!(!token.is_cancelled());
        token.cancelled().await;
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_multiple_tokens_from_source() {
        let source = CancellationTokenSource::new();
        let t1 = source.token();
        let t2 = source.token();
        let t3 = source.token();

        assert!(!t1.is_cancelled());
        assert!(!t2.is_cancelled());
        assert!(!t3.is_cancelled());

        source.cancel();

        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
        assert!(t3.is_cancelled());
    }

    #[test]
    fn test_source_default() {
        let source = CancellationTokenSource::default();
        assert!(!source.is_cancelled());
    }
}
