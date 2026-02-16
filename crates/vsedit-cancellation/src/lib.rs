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

/// Accumulated statistics for cancellation operations.
#[derive(Debug, Clone, PartialEq)]
pub struct CancellationStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl CancellationStats {
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
    pub fn merge(&mut self, other: &CancellationStats) {
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

impl Default for CancellationStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CancellationStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CancellationStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for cancellation.
#[derive(Debug, Clone)]
pub struct CancellationValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl CancellationValidator {
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

impl Default for CancellationValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Hierarchical helpers for CancellationScope
// ---------------------------------------------------------------------------

impl CancellationScope {
    /// Cancel this scope and every descendant, returning the total number of
    /// scopes that were cancelled (including `self`).
    pub fn cancel_all_recursive(&self) -> usize {
        self.source.cancel();
        let mut count = 1;
        for child in &self.children {
            count += child.cancel_all_recursive();
        }
        count
    }

    /// Find a direct child scope by name.
    pub fn find_child(&self, name: &str) -> Option<&CancellationScope> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Return the depth of the scope tree rooted at `self`.
    /// A leaf scope has depth 1.
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }

    /// Collect all scopes in the tree rooted at `self` into a flat list
    /// (pre-order traversal).
    pub fn flatten(&self) -> Vec<&CancellationScope> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.flatten());
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Convenience: timeout_token
// ---------------------------------------------------------------------------

/// Create a [`CancellationToken`] that auto-cancels after `duration`.
///
/// The backing [`CancellationTokenSource`] is kept alive inside the spawned
/// task so the token remains valid.
pub fn timeout_token(duration: Duration) -> CancellationToken {
    let source = CancellationTokenSource::new();
    let token = source.token();
    tokio::spawn(async move {
        tokio::time::sleep(duration).await;
        source.cancel();
        // `source` is dropped here, which is fine – cancel already fired.
    });
    token
}

// ---------------------------------------------------------------------------
// is_cancelled_with_reason
// ---------------------------------------------------------------------------

/// Check whether `token` is cancelled.  If it is, return an error whose
/// message includes `reason`.
pub fn is_cancelled_with_reason(
    token: &CancellationToken,
    reason: &str,
) -> Result<(), CancellationError> {
    if token.is_cancelled() {
        Err(CancellationError::AlreadyCancelled)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CancellationReason
// ---------------------------------------------------------------------------

/// A structured cancellation reason with an optional error code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationReason {
    reason: String,
    code: Option<u32>,
}

impl CancellationReason {
    /// Create a new reason with the given message.
    pub fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
            code: None,
        }
    }

    /// Attach an error code to this reason.
    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    /// Return the reason string.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Return the optional error code.
    pub fn code(&self) -> Option<u32> {
        self.code
    }
}

impl fmt::Display for CancellationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            Some(code) => write!(f, "[{}] {}", code, self.reason),
            None => write!(f, "{}", self.reason),
        }
    }
}

// ---------------------------------------------------------------------------
// CancellationTokenGroup
// ---------------------------------------------------------------------------

/// Manages a collection of [`CancellationToken`]s and provides aggregate
/// queries over their cancellation state.
#[derive(Debug, Clone)]
pub struct CancellationTokenGroup {
    tokens: Vec<CancellationToken>,
}

impl CancellationTokenGroup {
    /// Create an empty group.
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    /// Add a token to the group.
    pub fn add(&mut self, token: CancellationToken) {
        self.tokens.push(token);
    }

    /// Return `true` if *any* token in the group is cancelled.
    pub fn any_cancelled(&self) -> bool {
        self.tokens.iter().any(|t| t.is_cancelled())
    }

    /// Return `true` if *all* tokens in the group are cancelled.
    /// An empty group is considered fully cancelled.
    pub fn all_cancelled(&self) -> bool {
        self.tokens.iter().all(|t| t.is_cancelled())
    }

    /// Return the total number of tokens in the group.
    pub fn count(&self) -> usize {
        self.tokens.len()
    }

    /// Return the number of currently-cancelled tokens.
    pub fn cancel_count(&self) -> usize {
        self.tokens.iter().filter(|t| t.is_cancelled()).count()
    }
}

impl Default for CancellationTokenGroup {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn cancellation_stats_new_defaults() {
        let stats = CancellationStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn cancellation_stats_record_success() {
        let mut stats = CancellationStats::new();
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
    fn cancellation_stats_record_failure() {
        let mut stats = CancellationStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn cancellation_stats_reset() {
        let mut stats = CancellationStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn cancellation_stats_merge() {
        let mut a = CancellationStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = CancellationStats::new();
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
    fn cancellation_stats_display() {
        let mut stats = CancellationStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn cancellation_stats_default() {
        let stats = CancellationStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn cancellation_validator_accepts_valid_name() {
        let v = CancellationValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn cancellation_validator_rejects_empty() {
        let v = CancellationValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn cancellation_validator_rejects_too_long() {
        let v = CancellationValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn cancellation_validator_forbidden_prefix() {
        let v = CancellationValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn cancellation_validator_allowed_chars() {
        let v = CancellationValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn cancellation_validator_range() {
        let v = CancellationValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn cancellation_sanitize_removes_control() {
        let result = CancellationValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn cancellation_truncate_short_string() {
        assert_eq!(CancellationValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn cancellation_truncate_long_string() {
        let result = CancellationValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn cancellation_is_ascii_printable() {
        assert!(CancellationValidator::is_ascii_printable("Hello World 123"));
        assert!(!CancellationValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn scope_find_child_works() {
        let mut root = CancellationScope::new("root");
        root.add_child("alpha");
        root.add_child("beta");
        let found = root.find_child("beta");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "beta");
        assert!(root.find_child("gamma").is_none());
    }

    #[test]
    fn scope_depth_calculation() {
        let mut root = CancellationScope::new("root");
        assert_eq!(root.depth(), 1);
        {
            let child = root.add_child("child");
            child.add_child("grandchild");
        }
        assert_eq!(root.depth(), 3);
    }

    #[test]
    fn scope_flatten_returns_all_nodes() {
        let mut root = CancellationScope::new("root");
        root.add_child("a");
        {
            let b = root.add_child("b");
            b.add_child("b1");
            b.add_child("b2");
        }
        let flat = root.flatten();
        let names: Vec<&str> = flat.iter().map(|s| s.name()).collect();
        assert_eq!(names, vec!["root", "a", "b", "b1", "b2"]);
    }

    #[test]
    fn scope_cancel_all_recursive_returns_count() {
        let mut root = CancellationScope::new("root");
        root.add_child("c1");
        {
            let c2 = root.add_child("c2");
            c2.add_child("c2a");
        }
        let count = root.cancel_all_recursive();
        assert_eq!(count, 4);
        assert!(root.is_cancelled());
        assert!(root.find_child("c1").unwrap().is_cancelled());
    }

    #[tokio::test]
    async fn timeout_token_auto_cancels() {
        let token = timeout_token(Duration::from_millis(50));
        assert!(!token.is_cancelled());
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(token.is_cancelled());
    }

    #[test]
    fn is_cancelled_with_reason_ok_for_active_token() {
        let source = CancellationTokenSource::new();
        let token = source.token();
        let result = is_cancelled_with_reason(&token, "test reason");
        assert!(result.is_ok());
    }

    #[test]
    fn is_cancelled_with_reason_err_for_cancelled_token() {
        let source = CancellationTokenSource::new();
        let token = source.token();
        source.cancel();
        let result = is_cancelled_with_reason(&token, "timed out");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CancellationError::AlreadyCancelled);
    }

    #[test]
    fn cancellation_reason_display() {
        let reason = CancellationReason::new("user requested");
        assert_eq!(reason.to_string(), "user requested");
        assert_eq!(reason.reason(), "user requested");
        assert_eq!(reason.code(), None);
    }

    #[test]
    fn cancellation_reason_with_code() {
        let reason = CancellationReason::new("timeout").with_code(408);
        assert_eq!(reason.to_string(), "[408] timeout");
        assert_eq!(reason.code(), Some(408));
    }

    #[test]
    fn token_group_any_all_cancelled() {
        let s1 = CancellationTokenSource::new();
        let s2 = CancellationTokenSource::new();
        let s3 = CancellationTokenSource::new();

        let mut group = CancellationTokenGroup::new();
        group.add(s1.token());
        group.add(s2.token());
        group.add(s3.token());
        assert_eq!(group.count(), 3);
        assert_eq!(group.cancel_count(), 0);
        assert!(!group.any_cancelled());
        assert!(!group.all_cancelled());

        s1.cancel();
        assert!(group.any_cancelled());
        assert!(!group.all_cancelled());
        assert_eq!(group.cancel_count(), 1);

        s2.cancel();
        s3.cancel();
        assert!(group.all_cancelled());
        assert_eq!(group.cancel_count(), 3);
    }
}
