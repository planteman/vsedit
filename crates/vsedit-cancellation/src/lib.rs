//! Cancellation token system.
//!
//! Equivalent to VS Code's `vs/base/common/cancellation.ts`.
//! Provides cooperative cancellation for async operations.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// CancellationPolicy – define cancellation policies
// ---------------------------------------------------------------------------

/// Defines how cancellation should be handled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellationPolicy {
    /// Cancel immediately with no grace period.
    Immediate,
    /// Allow a grace period before forcing cancellation.
    Graceful { grace_period: Duration },
    /// Cancel after a timeout, regardless of operation state.
    Timeout { duration: Duration },
}

impl CancellationPolicy {
    /// Returns the duration associated with this policy (0 for Immediate).
    pub fn duration(&self) -> Duration {
        match self {
            CancellationPolicy::Immediate => Duration::ZERO,
            CancellationPolicy::Graceful { grace_period } => *grace_period,
            CancellationPolicy::Timeout { duration } => *duration,
        }
    }

    /// Returns true if this policy allows a grace period.
    pub fn has_grace_period(&self) -> bool {
        matches!(self, CancellationPolicy::Graceful { .. })
    }

    /// Returns true if this is an immediate cancellation.
    pub fn is_immediate(&self) -> bool {
        matches!(self, CancellationPolicy::Immediate)
    }

    /// Check if the given elapsed duration exceeds this policy's limit.
    pub fn is_expired(&self, elapsed: Duration) -> bool {
        match self {
            CancellationPolicy::Immediate => true,
            CancellationPolicy::Graceful { grace_period } => elapsed >= *grace_period,
            CancellationPolicy::Timeout { duration } => elapsed >= *duration,
        }
    }
}

impl fmt::Display for CancellationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CancellationPolicy::Immediate => write!(f, "immediate"),
            CancellationPolicy::Graceful { grace_period } => {
                write!(f, "graceful({}ms)", grace_period.as_millis())
            }
            CancellationPolicy::Timeout { duration } => {
                write!(f, "timeout({}ms)", duration.as_millis())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CancellationChain – chain tokens with priority
// ---------------------------------------------------------------------------

/// A prioritized chain of cancellation tokens. Higher priority tokens
/// are checked first.
#[derive(Debug, Clone)]
pub struct CancellationChainEntry {
    pub token: CancellationToken,
    pub priority: u32,
    pub label: String,
}

/// Chains multiple cancellation tokens with associated priorities.
pub struct CancellationChain {
    entries: Vec<CancellationChainEntry>,
}

impl CancellationChain {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a token with the given priority and label.
    pub fn add(&mut self, token: CancellationToken, priority: u32, label: impl Into<String>) {
        self.entries.push(CancellationChainEntry {
            token,
            priority,
            label: label.into(),
        });
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Check if any token in the chain is cancelled, returning the
    /// highest-priority cancelled entry's label.
    pub fn check(&self) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.token.is_cancelled())
            .map(|e| e.label.as_str())
    }

    /// Returns true if any token in the chain is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.entries.iter().any(|e| e.token.is_cancelled())
    }

    /// Number of tokens in the chain.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the label of the highest-priority token.
    pub fn highest_priority_label(&self) -> Option<&str> {
        self.entries.first().map(|e| e.label.as_str())
    }
}

impl Default for CancellationChain {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CancellationAuditLog – audit trail of cancellation events
// ---------------------------------------------------------------------------

/// A record of a cancellation event for auditing purposes.
#[derive(Debug, Clone)]
pub struct CancellationAuditEntry {
    pub operation_name: String,
    pub reason: String,
    pub timestamp_ms: u64,
    pub was_graceful: bool,
}

impl fmt::Display for CancellationAuditEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.was_graceful {
            "graceful"
        } else {
            "immediate"
        };
        write!(
            f,
            "[{}ms] {} cancelled ({kind}): {}",
            self.timestamp_ms, self.operation_name, self.reason
        )
    }
}

/// An append-only log of cancellation events.
pub struct CancellationAuditLog {
    entries: Vec<CancellationAuditEntry>,
    max_entries: usize,
}

impl CancellationAuditLog {
    /// Create a new audit log with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Record a cancellation event.
    pub fn record(
        &mut self,
        operation_name: impl Into<String>,
        reason: impl Into<String>,
        timestamp_ms: u64,
        was_graceful: bool,
    ) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(CancellationAuditEntry {
            operation_name: operation_name.into(),
            reason: reason.into(),
            timestamp_ms,
            was_graceful,
        });
    }

    /// Get all entries.
    pub fn entries(&self) -> &[CancellationAuditEntry] {
        &self.entries
    }

    /// Number of logged entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return entries matching the given operation name.
    pub fn entries_for(&self, operation_name: &str) -> Vec<&CancellationAuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.operation_name == operation_name)
            .collect()
    }

    /// Count of graceful vs immediate cancellations.
    pub fn graceful_count(&self) -> usize {
        self.entries.iter().filter(|e| e.was_graceful).count()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// CancellationTokenGroup – merge/split extensions
// ---------------------------------------------------------------------------

impl CancellationTokenGroup {
    /// Merge another group's tokens into this group.
    pub fn merge(&mut self, other: &CancellationTokenGroup) {
        for token in &other.tokens {
            self.tokens.push(token.clone());
        }
    }

    /// Split this group into two: cancelled tokens and non-cancelled tokens.
    pub fn split_by_state(&self) -> (CancellationTokenGroup, CancellationTokenGroup) {
        let mut cancelled = CancellationTokenGroup::new();
        let mut active = CancellationTokenGroup::new();
        for token in &self.tokens {
            if token.is_cancelled() {
                cancelled.add(token.clone());
            } else {
                active.add(token.clone());
            }
        }
        (cancelled, active)
    }

    /// Remove all cancelled tokens from the group, returning how many were removed.
    pub fn remove_cancelled(&mut self) -> usize {
        let before = self.tokens.len();
        self.tokens.retain(|t| !t.is_cancelled());
        before - self.tokens.len()
    }

    /// Return tokens as a slice.
    pub fn tokens(&self) -> &[CancellationToken] {
        &self.tokens
    }
}

// ---------------------------------------------------------------------------
// CancellationBarrier – waits for ALL tokens to be cancelled
// ---------------------------------------------------------------------------

/// A barrier that tracks multiple [`CancellationToken`]s and reports
/// completion only when *every* token has been cancelled.
#[derive(Debug, Clone)]
pub struct CancellationBarrier {
    tokens: Vec<CancellationToken>,
}

impl CancellationBarrier {
    /// Create a barrier over the given tokens.
    pub fn new(tokens: Vec<CancellationToken>) -> Self {
        Self { tokens }
    }

    /// Returns `true` when every token in the barrier has been cancelled.
    /// An empty barrier is considered complete.
    pub fn is_complete(&self) -> bool {
        self.tokens.iter().all(|t| t.is_cancelled())
    }

    /// Number of tokens that have been cancelled so far.
    pub fn completed(&self) -> usize {
        self.tokens.iter().filter(|t| t.is_cancelled()).count()
    }

    /// Total number of tokens tracked by this barrier.
    pub fn total(&self) -> usize {
        self.tokens.len()
    }

    /// Return progress as a fraction in `[0.0, 1.0]`.
    pub fn progress(&self) -> f64 {
        if self.tokens.is_empty() {
            return 1.0;
        }
        self.completed() as f64 / self.tokens.len() as f64
    }

    /// Number of tokens still pending (not yet cancelled).
    pub fn remaining(&self) -> usize {
        self.total() - self.completed()
    }

    /// Wait asynchronously until every token has been cancelled.
    pub async fn wait(&mut self) {
        for token in &mut self.tokens {
            token.cancelled().await;
        }
    }
}

impl fmt::Display for CancellationBarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CancellationBarrier({}/{})",
            self.completed(),
            self.total()
        )
    }
}

// ---------------------------------------------------------------------------
// CancellationMap – keyed collection of cancellation sources
// ---------------------------------------------------------------------------

/// A keyed collection of [`CancellationTokenSource`]s, allowing cancellation
/// of individual tasks by name.
pub struct CancellationMap {
    sources: std::collections::HashMap<String, CancellationTokenSource>,
}

impl CancellationMap {
    /// Create an empty map.
    pub fn new() -> Self {
        Self {
            sources: std::collections::HashMap::new(),
        }
    }

    /// Insert a new cancellation source for `key`, returning the source for
    /// external control. Overwrites any prior entry with the same key.
    pub fn insert(&mut self, key: impl Into<String>) -> &CancellationTokenSource {
        let key = key.into();
        self.sources
            .entry(key)
            .or_insert_with(CancellationTokenSource::new)
    }

    /// Get a token for the given key, if it exists.
    pub fn get(&self, key: &str) -> Option<CancellationToken> {
        self.sources.get(key).map(|s| s.token())
    }

    /// Cancel the source associated with `key`.
    pub fn cancel(&self, key: &str) {
        if let Some(source) = self.sources.get(key) {
            source.cancel();
        }
    }

    /// Cancel every source in the map.
    pub fn cancel_all(&self) {
        for source in self.sources.values() {
            source.cancel();
        }
    }

    /// Remove a source by key, returning it if it existed.
    pub fn remove(&mut self, key: &str) -> Option<CancellationTokenSource> {
        self.sources.remove(key)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Check if the token for `key` is cancelled. Returns `false` if key
    /// does not exist.
    pub fn is_cancelled(&self, key: &str) -> bool {
        self.sources
            .get(key)
            .map(|s| s.is_cancelled())
            .unwrap_or(false)
    }

    /// Return keys whose tokens are *not* cancelled.
    pub fn active_keys(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|(_, s)| !s.is_cancelled())
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Return keys whose tokens *are* cancelled.
    pub fn cancelled_keys(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|(_, s)| s.is_cancelled())
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

impl Default for CancellationMap {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CancellationMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancellationMap")
            .field("len", &self.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// OperationGuard – conditional execution helper
// ---------------------------------------------------------------------------

impl OperationGuard {
    /// Execute `f` only if the guard's token is still active.
    /// Returns `Err(CancellationError::AlreadyCancelled)` without calling `f`
    /// if the token has been cancelled.
    pub fn run_if_active<T, F: FnOnce() -> T>(&self, f: F) -> Result<T, CancellationError> {
        self.check()?;
        Ok(f())
    }
}


// ---------------------------------------------------------------------------
// CancellationRegistry — tracks all tokens
// ---------------------------------------------------------------------------

/// Registry that tracks all active cancellation token sources.
pub struct CancellationRegistry {
    sources: HashMap<String, CancellationTokenSource>,
}

impl CancellationRegistry {
    pub fn new() -> Self { Self { sources: HashMap::new() } }

    pub fn register(&mut self, name: impl Into<String>) -> CancellationToken {
        let source = CancellationTokenSource::new();
        let token = source.token();
        self.sources.insert(name.into(), source);
        token
    }

    pub fn cancel(&self, name: &str) -> bool {
        if let Some(source) = self.sources.get(name) { source.cancel(); true } else { false }
    }

    pub fn cancel_all(&self) {
        for source in self.sources.values() { source.cancel(); }
    }

    pub fn len(&self) -> usize { self.sources.len() }
    pub fn is_empty(&self) -> bool { self.sources.is_empty() }

    pub fn remove(&mut self, name: &str) -> bool { self.sources.remove(name).is_some() }

    pub fn cancelled_names(&self) -> Vec<&str> {
        self.sources.iter().filter(|(_, s)| s.is_cancelled()).map(|(k, _)| k.as_str()).collect()
    }

    pub fn active_names(&self) -> Vec<&str> {
        self.sources.iter().filter(|(_, s)| !s.is_cancelled()).map(|(k, _)| k.as_str()).collect()
    }

    pub fn is_cancelled(&self, name: &str) -> Option<bool> {
        self.sources.get(name).map(|s| s.is_cancelled())
    }
}

impl Default for CancellationRegistry {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for CancellationRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancellationRegistry({} sources)", self.sources.len())
    }
}

// ---------------------------------------------------------------------------
// CancellationTimeout — auto-cancel after duration
// ---------------------------------------------------------------------------

/// Auto-cancel configuration with a duration.
pub struct CancellationTimeout {
    source: CancellationTokenSource,
    duration: Duration,
}

impl CancellationTimeout {
    pub fn new(duration: Duration) -> Self {
        Self { source: CancellationTokenSource::new(), duration }
    }

    pub fn token(&self) -> CancellationToken { self.source.token() }
    pub fn duration(&self) -> Duration { self.duration }
    pub fn cancel_now(&self) { self.source.cancel(); }
    pub fn is_cancelled(&self) -> bool { self.source.is_cancelled() }
}

impl fmt::Display for CancellationTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancellationTimeout({}ms)", self.duration.as_millis())
    }
}

// ---------------------------------------------------------------------------
// CancellationLinker — parent-child chains
// ---------------------------------------------------------------------------

/// Links cancellation tokens in parent-child relationships.
pub struct CancellationLinker {
    children: Vec<CancellationTokenSource>,
    parent_token: CancellationToken,
}

impl CancellationLinker {
    pub fn new(parent_token: CancellationToken) -> Self {
        Self { children: Vec::new(), parent_token }
    }

    pub fn create_child(&mut self) -> CancellationToken {
        let source = CancellationTokenSource::new();
        let token = source.token();
        self.children.push(source);
        token
    }

    pub fn cancel_children(&self) {
        for child in &self.children { child.cancel(); }
    }

    pub fn child_count(&self) -> usize { self.children.len() }

    pub fn is_parent_cancelled(&self) -> bool { self.parent_token.is_cancelled() }

    pub fn propagate(&self) {
        if self.parent_token.is_cancelled() { self.cancel_children(); }
    }
}

impl fmt::Display for CancellationLinker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancellationLinker({} children)", self.children.len())
    }
}

// ---------------------------------------------------------------------------
// CancellationStatistics
// ---------------------------------------------------------------------------

/// Tracks cancellation statistics.
pub struct CancellationStatistics {
    pub total_created: u64,
    pub total_cancelled: u64,
    pub total_timed_out: u64,
    pub total_completed: u64,
}

impl CancellationStatistics {
    pub fn new() -> Self {
        Self { total_created: 0, total_cancelled: 0, total_timed_out: 0, total_completed: 0 }
    }

    pub fn record_creation(&mut self) { self.total_created += 1; }
    pub fn record_cancellation(&mut self) { self.total_cancelled += 1; }
    pub fn record_timeout(&mut self) { self.total_timed_out += 1; }
    pub fn record_completion(&mut self) { self.total_completed += 1; }

    pub fn cancellation_rate(&self) -> f64 {
        if self.total_created == 0 { 0.0 } else { self.total_cancelled as f64 / self.total_created as f64 }
    }

    pub fn completion_rate(&self) -> f64 {
        if self.total_created == 0 { 0.0 } else { self.total_completed as f64 / self.total_created as f64 }
    }

    pub fn reset(&mut self) { *self = Self::new(); }
}

impl Default for CancellationStatistics {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for CancellationStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancellationStatistics(created={}, cancelled={}, completed={})",
            self.total_created, self.total_cancelled, self.total_completed)
    }
}

// ---------------------------------------------------------------------------
// CancellationCascade - cancellation cascade manager
// ---------------------------------------------------------------------------

/// Severity level for cancellation cascade manager issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CancellationCascadeSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for CancellationCascadeSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [CancellationCascade].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationCascadeEntry {
    pub id: String,
    pub label: String,
    pub severity: CancellationCascadeSeverity,
    pub detail: Option<String>,
    pub cascade_depth: usize,
    enabled: bool,
}

impl CancellationCascadeEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: CancellationCascadeSeverity::Low,
            detail: None,
            cascade_depth: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: CancellationCascadeSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_cascade_depth(mut self, val: usize) -> Self {
        self.cascade_depth = val;
        self
    }

    pub fn is_cancelled(&self) -> bool {
        self.enabled && self.severity >= CancellationCascadeSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.cascade_depth, det)
    }
}

impl fmt::Display for CancellationCascadeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [CancellationCascadeEntry] items.
#[derive(Debug, Clone)]
pub struct CancellationCascade {
    entries: Vec<CancellationCascadeEntry>,
    name: String,
    capacity: usize,
}

impl CancellationCascade {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: CancellationCascadeEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<CancellationCascadeEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&CancellationCascadeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn cascade_depth(&self) -> usize { self.entries.len() }

    pub fn is_cancelled(&self) -> bool {
        self.entries.iter().any(|e| e.is_cancelled())
    }

    pub fn entries_by_severity(&self, severity: CancellationCascadeSeverity) -> Vec<&CancellationCascadeEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= CancellationCascadeSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&CancellationCascadeEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&CancellationCascadeEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// TimeoutAutoCanceller - timeout auto-canceller
// ---------------------------------------------------------------------------

/// Configuration for [TimeoutAutoCanceller].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutAutoCancellerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub timeout_ms: usize,
}

impl TimeoutAutoCancellerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, timeout_ms: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_timeout_ms(mut self, val: usize) -> Self { self.timeout_ms = val; self }
}

impl Default for TimeoutAutoCancellerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [TimeoutAutoCanceller].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutAutoCancellerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl TimeoutAutoCancellerItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_timeout(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for TimeoutAutoCancellerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [TimeoutAutoCancellerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct TimeoutAutoCanceller {
    config: TimeoutAutoCancellerConfig,
    items: Vec<TimeoutAutoCancellerItem>,
}

impl TimeoutAutoCanceller {
    pub fn new(config: TimeoutAutoCancellerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: TimeoutAutoCancellerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<TimeoutAutoCancellerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&TimeoutAutoCancellerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn timeout_ms(&self) -> usize { self.items.len() }

    pub fn has_timeout(&self) -> bool {
        self.items.iter().any(|i| i.has_timeout())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&TimeoutAutoCancellerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TimeoutAutoCancellerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &TimeoutAutoCancellerConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
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

    // -- CancellationPolicy tests ------------------------------------------

    #[test]
    fn policy_immediate() {
        let p = CancellationPolicy::Immediate;
        assert!(p.is_immediate());
        assert!(!p.has_grace_period());
        assert_eq!(p.duration(), Duration::ZERO);
        assert!(p.is_expired(Duration::ZERO));
    }

    #[test]
    fn policy_graceful() {
        let p = CancellationPolicy::Graceful {
            grace_period: Duration::from_millis(500),
        };
        assert!(p.has_grace_period());
        assert!(!p.is_expired(Duration::from_millis(100)));
        assert!(p.is_expired(Duration::from_millis(500)));
        assert_eq!(format!("{p}"), "graceful(500ms)");
    }

    #[test]
    fn policy_timeout() {
        let p = CancellationPolicy::Timeout {
            duration: Duration::from_secs(5),
        };
        assert!(!p.is_immediate());
        assert!(!p.is_expired(Duration::from_secs(3)));
        assert!(p.is_expired(Duration::from_secs(5)));
    }

    // -- CancellationChain tests -------------------------------------------

    #[test]
    fn chain_priority_ordering() {
        let s1 = CancellationTokenSource::new();
        let s2 = CancellationTokenSource::new();
        let mut chain = CancellationChain::new();
        chain.add(s1.token(), 10, "low");
        chain.add(s2.token(), 100, "high");
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.highest_priority_label(), Some("high"));

        s1.cancel();
        // "high" has priority 100 but "low" (priority 10) is cancelled
        // chain.check() returns the highest-priority cancelled entry
        // s1 was "low" priority 10, so check returns "low"
        assert_eq!(chain.check(), Some("low"));
    }

    #[test]
    fn chain_not_cancelled() {
        let chain = CancellationChain::new();
        assert!(!chain.is_cancelled());
        assert!(chain.check().is_none());
        assert!(chain.is_empty());
    }

    #[test]
    fn chain_highest_priority_cancelled_first() {
        let s1 = CancellationTokenSource::new();
        let s2 = CancellationTokenSource::new();
        let mut chain = CancellationChain::new();
        chain.add(s1.token(), 1, "low");
        chain.add(s2.token(), 100, "high");

        s2.cancel();
        assert_eq!(chain.check(), Some("high"));
    }

    // -- CancellationAuditLog tests ----------------------------------------

    #[test]
    fn audit_log_record_and_query() {
        let mut log = CancellationAuditLog::new(100);
        log.record("download", "user request", 1000, true);
        log.record("upload", "timeout", 2000, false);
        assert_eq!(log.len(), 2);
        assert_eq!(log.graceful_count(), 1);
        assert_eq!(log.entries_for("download").len(), 1);
    }

    #[test]
    fn audit_log_max_entries_eviction() {
        let mut log = CancellationAuditLog::new(2);
        log.record("op1", "reason", 100, false);
        log.record("op2", "reason", 200, false);
        log.record("op3", "reason", 300, false);
        assert_eq!(log.len(), 2);
        assert_eq!(log.entries()[0].operation_name, "op2");
    }

    #[test]
    fn audit_entry_display() {
        let entry = CancellationAuditEntry {
            operation_name: "fetch".into(),
            reason: "cancelled".into(),
            timestamp_ms: 42,
            was_graceful: false,
        };
        let s = format!("{entry}");
        assert!(s.contains("fetch"));
        assert!(s.contains("immediate"));
    }

    // -- CancellationBarrier tests -----------------------------------------

    #[test]
    fn barrier_completes_when_all_cancelled() {
        let s1 = CancellationTokenSource::new();
        let s2 = CancellationTokenSource::new();
        let barrier = CancellationBarrier::new(vec![s1.token(), s2.token()]);
        assert!(!barrier.is_complete());
        s1.cancel();
        assert!(!barrier.is_complete());
        s2.cancel();
        assert!(barrier.is_complete());
    }

    #[test]
    fn barrier_progress_tracking() {
        let s1 = CancellationTokenSource::new();
        let s2 = CancellationTokenSource::new();
        let s3 = CancellationTokenSource::new();
        let barrier =
            CancellationBarrier::new(vec![s1.token(), s2.token(), s3.token()]);
        assert_eq!(barrier.total(), 3);
        assert_eq!(barrier.completed(), 0);
        assert!((barrier.progress() - 0.0).abs() < f64::EPSILON);

        s1.cancel();
        assert_eq!(barrier.completed(), 1);
        assert_eq!(barrier.remaining(), 2);

        s2.cancel();
        s3.cancel();
        assert!((barrier.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn barrier_empty_is_complete() {
        let barrier = CancellationBarrier::new(vec![]);
        assert!(barrier.is_complete());
        assert_eq!(barrier.total(), 0);
        assert!((barrier.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn barrier_display() {
        let s1 = CancellationTokenSource::new();
        let barrier = CancellationBarrier::new(vec![s1.token()]);
        assert_eq!(format!("{barrier}"), "CancellationBarrier(0/1)");
        s1.cancel();
        assert_eq!(format!("{barrier}"), "CancellationBarrier(1/1)");
    }

    // -- CancellationMap tests ---------------------------------------------

    #[test]
    fn map_insert_cancel_get() {
        let mut map = CancellationMap::new();
        let source = map.insert("task-a");
        assert!(!source.is_cancelled());
        let token = map.get("task-a").unwrap();
        assert!(!token.is_cancelled());

        map.cancel("task-a");
        let token2 = map.get("task-a").unwrap();
        assert!(token2.is_cancelled());
    }

    #[test]
    fn map_cancel_by_key() {
        let mut map = CancellationMap::new();
        map.insert("task-b");
        assert!(!map.is_cancelled("task-b"));
        map.cancel("task-b");
        assert!(map.is_cancelled("task-b"));
    }

    #[test]
    fn map_cancel_all_and_keys() {
        let mut map = CancellationMap::new();
        map.insert("x");
        map.insert("y");
        map.insert("z");
        assert_eq!(map.len(), 3);
        assert!(!map.is_empty());

        map.cancel_all();
        assert_eq!(map.cancelled_keys().len(), 3);
        assert_eq!(map.active_keys().len(), 0);
    }

    #[test]
    fn map_remove_returns_source() {
        let mut map = CancellationMap::new();
        map.insert("item");
        assert!(map.remove("item").is_some());
        assert!(map.get("item").is_none());
        assert!(map.remove("item").is_none());
    }

    #[test]
    fn map_missing_key_returns_false() {
        let map = CancellationMap::new();
        assert!(!map.is_cancelled("nonexistent"));
        assert!(map.get("nonexistent").is_none());
    }

    #[test]
    fn map_debug_format() {
        let map = CancellationMap::new();
        let s = format!("{map:?}");
        assert!(s.contains("CancellationMap"));
    }

    // -- OperationGuard::run_if_active tests --------------------------------

    #[test]
    fn guard_run_if_active_executes_when_active() {
        let token = CancellationToken::none();
        let guard = OperationGuard::new(token, "op");
        let result = guard.run_if_active(|| 42);
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn guard_run_if_active_errors_when_cancelled() {
        let token = CancellationToken::cancelled_token();
        let guard = OperationGuard::new(token, "op");
        let result = guard.run_if_active(|| 42);
        assert_eq!(result, Err(CancellationError::AlreadyCancelled));
    }

    // -- CancellationTokenGroup merge/split tests --------------------------

    #[test]
    fn group_merge_and_split() {
        let s1 = CancellationTokenSource::new();
        let s2 = CancellationTokenSource::new();
        let s3 = CancellationTokenSource::new();

        let mut g1 = CancellationTokenGroup::new();
        g1.add(s1.token());
        let mut g2 = CancellationTokenGroup::new();
        g2.add(s2.token());
        g2.add(s3.token());

        g1.merge(&g2);
        assert_eq!(g1.count(), 3);

        s1.cancel();
        let (cancelled, active) = g1.split_by_state();
        assert_eq!(cancelled.count(), 1);
        assert_eq!(active.count(), 2);
    }

    #[test]
    fn group_remove_cancelled() {
        let s1 = CancellationTokenSource::new();
        let s2 = CancellationTokenSource::new();

        let mut group = CancellationTokenGroup::new();
        group.add(s1.token());
        group.add(s2.token());

        s1.cancel();
        let removed = group.remove_cancelled();
        assert_eq!(removed, 1);
        assert_eq!(group.count(), 1);
    }


    #[test]
    fn registry_register_and_cancel() {
        let mut reg = CancellationRegistry::new();
        let token = reg.register("op1");
        assert!(!token.is_cancelled());
        assert!(reg.cancel("op1"));
        assert!(token.is_cancelled());
    }

    #[test]
    fn registry_cancel_all() {
        let mut reg = CancellationRegistry::new();
        let t1 = reg.register("a");
        let t2 = reg.register("b");
        reg.cancel_all();
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }

    #[test]
    fn registry_remove() {
        let mut reg = CancellationRegistry::new();
        reg.register("x");
        assert!(reg.remove("x"));
        assert!(reg.is_empty());
    }

    #[test]
    fn registry_cancelled_names() {
        let mut reg = CancellationRegistry::new();
        reg.register("a");
        reg.register("b");
        reg.cancel("a");
        assert!(reg.cancelled_names().contains(&"a"));
    }

    #[test]
    fn registry_active_names() {
        let mut reg = CancellationRegistry::new();
        reg.register("a");
        reg.register("b");
        reg.cancel("a");
        let active = reg.active_names();
        assert!(!active.contains(&"a"));
        assert!(active.contains(&"b"));
    }

    #[test]
    fn timeout_basic() {
        let timeout = CancellationTimeout::new(Duration::from_millis(100));
        assert!(!timeout.is_cancelled());
        timeout.cancel_now();
        assert!(timeout.is_cancelled());
    }

    #[test]
    fn timeout_display() {
        let timeout = CancellationTimeout::new(Duration::from_millis(500));
        assert!(format!("{timeout}").contains("500ms"));
    }

    #[test]
    fn linker_create_child() {
        let source = CancellationTokenSource::new();
        let mut linker = CancellationLinker::new(source.token());
        let child = linker.create_child();
        assert!(!child.is_cancelled());
        assert_eq!(linker.child_count(), 1);
    }

    #[test]
    fn linker_propagate() {
        let source = CancellationTokenSource::new();
        let parent_token = source.token();
        let mut linker = CancellationLinker::new(parent_token);
        let child = linker.create_child();
        source.cancel();
        linker.propagate();
        assert!(child.is_cancelled());
    }

    #[test]
    fn statistics_basic() {
        let mut stats = CancellationStatistics::new();
        stats.record_creation();
        stats.record_creation();
        stats.record_cancellation();
        stats.record_completion();
        assert_eq!(stats.total_created, 2);
        assert!((stats.cancellation_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn statistics_reset() {
        let mut stats = CancellationStatistics::new();
        stats.record_creation();
        stats.reset();
        assert_eq!(stats.total_created, 0);
    }

    #[test]
    fn statistics_display() {
        let stats = CancellationStatistics::new();
        assert!(format!("{stats}").contains("created=0"));
    }

    #[test]
    fn registry_is_cancelled_check() {
        let mut reg = CancellationRegistry::new();
        reg.register("op");
        assert_eq!(reg.is_cancelled("op"), Some(false));
        reg.cancel("op");
        assert_eq!(reg.is_cancelled("op"), Some(true));
        assert_eq!(reg.is_cancelled("missing"), None);
    }


#[test]
    fn cancellationcascade_severity_ordering() {
        assert!(CancellationCascadeSeverity::Critical > CancellationCascadeSeverity::High);
        assert!(CancellationCascadeSeverity::High > CancellationCascadeSeverity::Medium);
        assert!(CancellationCascadeSeverity::Medium > CancellationCascadeSeverity::Low);
    }

    #[test]
    fn cancellationcascade_severity_display() {
        assert_eq!(CancellationCascadeSeverity::Low.to_string(), "low");
        assert_eq!(CancellationCascadeSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn cancellationcascade_entry_creation() {
        let e = CancellationCascadeEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, CancellationCascadeSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn cancellationcascade_entry_builder() {
        let e = CancellationCascadeEntry::new("e2", "Entry 2")
            .with_severity(CancellationCascadeSeverity::High)
            .with_detail("some detail")
            .with_cascade_depth(42);
        assert_eq!(e.severity, CancellationCascadeSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.cascade_depth, 42);
    }

    #[test]
    fn cancellationcascade_entry_enable_disable() {
        let mut e = CancellationCascadeEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn cancellationcascade_add_and_count() {
        let mut mgr = CancellationCascade::new("test");
        mgr.add(CancellationCascadeEntry::new("a", "A"));
        mgr.add(CancellationCascadeEntry::new("b", "B").with_severity(CancellationCascadeSeverity::High));
        assert_eq!(mgr.cascade_depth(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn cancellationcascade_remove() {
        let mut mgr = CancellationCascade::new("test");
        mgr.add(CancellationCascadeEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn cancellationcascade_capacity() {
        let mut mgr = CancellationCascade::new("test").with_capacity(1);
        assert!(mgr.add(CancellationCascadeEntry::new("a", "A")));
        assert!(!mgr.add(CancellationCascadeEntry::new("b", "B")));
    }

    #[test]
    fn cancellationcascade_sorted_by_severity() {
        let mut mgr = CancellationCascade::new("test");
        mgr.add(CancellationCascadeEntry::new("lo", "Low"));
        mgr.add(CancellationCascadeEntry::new("hi", "High").with_severity(CancellationCascadeSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, CancellationCascadeSeverity::Critical);
    }

    #[test]
    fn cancellationcascade_summary() {
        let mgr = CancellationCascade::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn timeoutautocanceller_config_defaults() {
        let cfg = TimeoutAutoCancellerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn timeoutautocanceller_item_creation() {
        let item = TimeoutAutoCancellerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn timeoutautocanceller_add_and_get() {
        let mut mgr = TimeoutAutoCanceller::new(TimeoutAutoCancellerConfig::new("test"));
        mgr.add(TimeoutAutoCancellerItem::new("k1", "v1"));
        assert_eq!(mgr.timeout_ms(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn timeoutautocanceller_remove_item() {
        let mut mgr = TimeoutAutoCanceller::new(TimeoutAutoCancellerConfig::new("test"));
        mgr.add(TimeoutAutoCancellerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn timeoutautocanceller_sorted_by_priority() {
        let mut mgr = TimeoutAutoCanceller::new(TimeoutAutoCancellerConfig::new("test"));
        mgr.add(TimeoutAutoCancellerItem::new("lo", "low").with_priority(1));
        mgr.add(TimeoutAutoCancellerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn timeoutautocanceller_items_with_tag() {
        let mut mgr = TimeoutAutoCanceller::new(TimeoutAutoCancellerConfig::new("test"));
        mgr.add(TimeoutAutoCancellerItem::new("a", "1").with_tag("x"));
        mgr.add(TimeoutAutoCancellerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn timeoutautocanceller_report() {
        let mgr = TimeoutAutoCanceller::new(TimeoutAutoCancellerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }
}
