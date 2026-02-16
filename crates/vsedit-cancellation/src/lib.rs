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

impl fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancellationToken")
            .field("is_cancelled", &self.is_cancelled())
            .finish()
    }
}

/// A named scope of cancellable work, supporting hierarchical cancellation.
#[derive(Debug)]
pub struct CancellationScope {
    name: String,
    source: CancellationTokenSource,
    children: Vec<CancellationScope>,
}

impl CancellationScope {
    /// Create a new cancellation scope with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: CancellationTokenSource::new(),
            children: Vec::new(),
        }
    }

    /// Get a cancellation token for this scope.
    pub fn token(&self) -> CancellationToken {
        self.source.token()
    }

    /// Cancel this scope.
    pub fn cancel(&self) {
        self.source.cancel();
    }

    /// Check if this scope has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.source.is_cancelled()
    }

    /// Add a child scope with the given name.
    pub fn add_child(&mut self, name: impl Into<String>) -> &mut CancellationScope {
        self.children.push(CancellationScope::new(name));
        self.children.last_mut().unwrap()
    }

    /// Return the number of direct child scopes.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Return the name of this scope.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Cancel this scope and all children recursively.
    pub fn cancel_all(&self) {
        self.source.cancel();
        for child in &self.children {
            child.cancel_all();
        }
    }
}

impl fmt::Display for CancellationScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CancellationScope(name={}, cancelled={}, children={})",
            self.name,
            self.is_cancelled(),
            self.children.len()
        )
    }
}

/// A guard that auto-checks cancellation for an operation.
#[derive(Debug, Clone)]
pub struct OperationGuard {
    token: CancellationToken,
    operation_name: String,
}

impl OperationGuard {
    /// Create a new operation guard with the given token and name.
    pub fn new(token: CancellationToken, name: impl Into<String>) -> Self {
        Self {
            token,
            operation_name: name.into(),
        }
    }

    /// Check if the operation is still active (not cancelled).
    /// Returns `Err(CancellationError::AlreadyCancelled)` if the token is cancelled.
    pub fn check(&self) -> Result<(), CancellationError> {
        if self.token.is_cancelled() {
            Err(CancellationError::AlreadyCancelled)
        } else {
            Ok(())
        }
    }

    /// Return the name of the operation.
    pub fn operation_name(&self) -> &str {
        &self.operation_name
    }

    /// Return `true` if the operation is still active (not cancelled).
    pub fn is_active(&self) -> bool {
        !self.token.is_cancelled()
    }
}

impl fmt::Display for OperationGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OperationGuard(op={}, active={})",
            self.operation_name,
            self.is_active()
        )
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

    #[test]
    fn test_scope_new() {
        let scope = CancellationScope::new("my-scope");
        assert_eq!(scope.name(), "my-scope");
        assert!(!scope.is_cancelled());
        assert_eq!(scope.child_count(), 0);
    }

    #[test]
    fn test_scope_cancel() {
        let scope = CancellationScope::new("work");
        let token = scope.token();
        assert!(!token.is_cancelled());
        scope.cancel();
        assert!(token.is_cancelled());
        assert!(scope.is_cancelled());
    }

    #[test]
    fn test_scope_add_child() {
        let mut scope = CancellationScope::new("parent");
        let child = scope.add_child("child-1");
        assert_eq!(child.name(), "child-1");
        assert!(!child.is_cancelled());
        assert_eq!(scope.child_count(), 1);
    }

    #[test]
    fn test_scope_cancel_all_recursive() {
        let mut scope = CancellationScope::new("root");
        scope.add_child("child-a");
        scope.add_child("child-b");
        // Add a grandchild
        scope.children[0].add_child("grandchild");

        let root_token = scope.token();
        let child_a_token = scope.children[0].token();
        let child_b_token = scope.children[1].token();
        let grandchild_token = scope.children[0].children[0].token();

        assert!(!root_token.is_cancelled());
        assert!(!child_a_token.is_cancelled());
        assert!(!child_b_token.is_cancelled());
        assert!(!grandchild_token.is_cancelled());

        scope.cancel_all();

        assert!(root_token.is_cancelled());
        assert!(child_a_token.is_cancelled());
        assert!(child_b_token.is_cancelled());
        assert!(grandchild_token.is_cancelled());
    }

    #[test]
    fn test_scope_child_count() {
        let mut scope = CancellationScope::new("root");
        assert_eq!(scope.child_count(), 0);
        scope.add_child("a");
        assert_eq!(scope.child_count(), 1);
        scope.add_child("b");
        assert_eq!(scope.child_count(), 2);
        scope.add_child("c");
        assert_eq!(scope.child_count(), 3);
    }

    #[test]
    fn test_scope_name() {
        let scope = CancellationScope::new("test-scope-name");
        assert_eq!(scope.name(), "test-scope-name");
    }

    #[test]
    fn test_scope_display() {
        let mut scope = CancellationScope::new("display-test");
        scope.add_child("child");
        let display = format!("{}", scope);
        assert!(display.contains("display-test"));
        assert!(display.contains("cancelled=false"));
        assert!(display.contains("children=1"));
    }

    #[test]
    fn test_guard_new() {
        let token = CancellationToken::none();
        let guard = OperationGuard::new(token, "my-op");
        assert_eq!(guard.operation_name(), "my-op");
        assert!(guard.is_active());
    }

    #[test]
    fn test_guard_check_active() {
        let token = CancellationToken::none();
        let guard = OperationGuard::new(token, "active-op");
        assert!(guard.check().is_ok());
    }

    #[test]
    fn test_guard_check_cancelled() {
        let token = CancellationToken::cancelled_token();
        let guard = OperationGuard::new(token, "cancelled-op");
        let result = guard.check();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CancellationError::AlreadyCancelled);
    }

    #[test]
    fn test_guard_operation_name() {
        let token = CancellationToken::none();
        let guard = OperationGuard::new(token, "some-operation");
        assert_eq!(guard.operation_name(), "some-operation");
    }

    #[test]
    fn test_guard_display() {
        let token = CancellationToken::none();
        let guard = OperationGuard::new(token, "display-op");
        let display = format!("{}", guard);
        assert!(display.contains("display-op"));
        assert!(display.contains("active=true"));

        let token2 = CancellationToken::cancelled_token();
        let guard2 = OperationGuard::new(token2, "done-op");
        let display2 = format!("{}", guard2);
        assert!(display2.contains("done-op"));
        assert!(display2.contains("active=false"));
    }

    #[test]
    fn test_guard_clone() {
        let source = CancellationTokenSource::new();
        let guard = OperationGuard::new(source.token(), "clone-op");
        let guard_clone = guard.clone();
        assert_eq!(guard_clone.operation_name(), "clone-op");
        assert!(guard_clone.is_active());

        source.cancel();
        assert!(!guard.is_active());
        assert!(!guard_clone.is_active());
    }
}
