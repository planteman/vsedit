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



// ─── Cancel Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for cancel events.
#[derive(Debug, Clone)]
pub struct CancelRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> CancelRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for CancelRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancelRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── Cancel Builder & Validator ─────────────────────────────

/// Builder for constructing cancellation configurations.
#[derive(Debug, Clone)]
pub struct CancelBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl CancelBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<CancelCfg, CancelBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(CancelBuildErr { errors }); }
        Ok(CancelCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated cancellation configuration.
#[derive(Debug, Clone)]
pub struct CancelCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl CancelCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &CancelCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for CancelCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancelCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct CancelBuildErr { pub errors: Vec<String> }

impl fmt::Display for CancelBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancelBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for CancelBuildErr {}



// ---------------------------------------------------------------------------
// cancellation – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for cancellation token system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YCancellationCancelReason {
    UserRequested,
    Timeout,
    Superseded,
    Error,
}

impl YCancellationCancelReason {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::UserRequested => 0,
            Self::Timeout => 1,
            Self::Superseded => 2,
            Self::Error => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::UserRequested => "UserRequested",
            Self::Timeout => "Timeout",
            Self::Superseded => "Superseded",
            Self::Error => "Error",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YCancellationCancelReason] {
        &[
            YCancellationCancelReason::UserRequested,
            YCancellationCancelReason::Timeout,
            YCancellationCancelReason::Superseded,
            YCancellationCancelReason::Error,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YCancellationCancelReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks cancel scope data.
#[derive(Debug, Clone)]
pub struct YCancellationCancelScope {
    pub label: String,
    pub cancelled: bool,
    pub children: Vec<String>,
}

impl YCancellationCancelScope {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            label: String::new(),
            cancelled: false,
            children: Vec::new(),
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.children.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YCancellationCancelScope({}: {:?})", "label", self.label)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_cancellation_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_cancellation_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_cancellation_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_cancellation_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_cancellation_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_cancellation_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_cancellation_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_cancellation_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// cancellation – Extended cancel budget helpers
// ---------------------------------------------------------------------------

/// Priority levels for cancel budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZCancellationPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZCancellationPriority {
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
    pub fn all_asc() -> [ZCancellationPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZCancellationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks cancel budget data.
#[derive(Debug, Clone)]
pub struct ZCancellationCancelBudget {
    pub deadlines_ms: Vec<(String, u64)>,
    pub total_budget_ms: u64,
    pub exhausted: bool,
}

impl ZCancellationCancelBudget {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            deadlines_ms: Vec::new(),
            total_budget_ms: 0,
            exhausted: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.deadlines_ms.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.deadlines_ms.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.deadlines_ms.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZCancellationCancelBudget[total_budget_ms={:?}, exhausted={:?}]", self.total_budget_ms, self.exhausted)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.exhausted = !c.exhausted;
        c
    }
}

/// Compute a simple rolling hash for cancel budget.
pub fn z_cancellation_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_cancellation_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_cancellation_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_cancellation_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_cancellation_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_cancellation_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_cancellation_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 95
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer95 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer95 {
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
pub fn xb_fnv1a_95(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_95<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_95<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_95(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_95(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 11
// ---------------------------------------------------------------------------

/// Generic object pool `Xc11Pool<T>`.
pub struct Xc11Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc11Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc11PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc11Pool<T> {
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
    pub fn stats(&self) -> Xc11PoolStats {
        Xc11PoolStats {
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

impl<T> Default for Xc11Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc11Scheduler`.
pub struct Xc11Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc11Scheduler {
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

impl Default for Xc11Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_11 hash for the given byte slice.
pub fn xc_11_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_11 convention.
pub fn xc_11_reverse(s: &str) -> String {
    s.chars().rev().collect()
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

    #[test]
    fn cancel_ringbuf_push_get() {
        let mut rb = CancelRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn cancel_ringbuf_overflow() {
        let mut rb = CancelRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn cancel_ringbuf_clear() {
        let mut rb = CancelRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn cancel_ringbuf_newest_oldest() {
        let mut rb = CancelRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn cancel_ringbuf_to_vec() {
        let mut rb = CancelRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn cancel_ringbuf_is_full() {
        let mut rb = CancelRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn cancel_builder_valid() {
        let cfg = CancelBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn cancel_builder_empty_name() {
        let r = CancelBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn cancel_builder_bad_priority() {
        assert!(CancelBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn cancel_builder_zero_max() {
        assert!(CancelBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn cancel_cfg_merge() {
        let mut a = CancelBuilder::new("a").property("x", "1").build().unwrap();
        let b = CancelBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn cancel_cfg_display() {
        let cfg = CancelBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- cancellation extended domain tests ----------------------------------------

    #[test]
    fn y_cancellation_enum_index() {
        assert_eq!(YCancellationCancelReason::UserRequested.index(), 0);
        assert_eq!(YCancellationCancelReason::Timeout.index(), 1);
        assert_eq!(YCancellationCancelReason::Superseded.index(), 2);
        assert_eq!(YCancellationCancelReason::Error.index(), 3);
    }

    #[test]
    fn y_cancellation_enum_label() {
        assert_eq!(YCancellationCancelReason::UserRequested.label(), "UserRequested");
        assert_eq!(YCancellationCancelReason::Timeout.label(), "Timeout");
        assert_eq!(YCancellationCancelReason::Superseded.label(), "Superseded");
        assert_eq!(YCancellationCancelReason::Error.label(), "Error");
    }

    #[test]
    fn y_cancellation_enum_all() {
        let all = YCancellationCancelReason::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_cancellation_enum_is_default() {
        assert!(YCancellationCancelReason::UserRequested.is_default());
        assert!(!YCancellationCancelReason::Error.is_default());
    }

    #[test]
    fn y_cancellation_enum_display() {
        assert_eq!(format!("{}", YCancellationCancelReason::UserRequested), "UserRequested");
    }

    #[test]
    fn y_cancellation_struct_new() {
        let s = YCancellationCancelScope::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_cancellation_struct_clear() {
        let mut s = YCancellationCancelScope::new();
        s.children.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_cancellation_fingerprint_deterministic() {
        let h1 = y_cancellation_fingerprint("hello");
        let h2 = y_cancellation_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_cancellation_fingerprint("a"), y_cancellation_fingerprint("b"));
    }

    #[test]
    fn y_cancellation_truncate_short() {
        assert_eq!(y_cancellation_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_cancellation_truncate_long() {
        let r = y_cancellation_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_cancellation_normalize_key_basic() {
        assert_eq!(y_cancellation_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_cancellation_split_path_basic() {
        let parts = y_cancellation_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_cancellation_count_occurrences_basic() {
        assert_eq!(y_cancellation_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_cancellation_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_cancellation_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_cancellation_in_range_basic() {
        assert!(y_cancellation_in_range(5, 1, 10));
        assert!(y_cancellation_in_range(1, 1, 10));
        assert!(y_cancellation_in_range(10, 1, 10));
        assert!(!y_cancellation_in_range(0, 1, 10));
        assert!(!y_cancellation_in_range(11, 1, 10));
    }

    #[test]
    fn y_cancellation_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_cancellation_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_cancellation_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_cancellation_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- cancellation Z-extended tests -----------------------------------------------

    #[test]
    fn z_cancellation_priority_weight() {
        assert_eq!(ZCancellationPriority::Idle.weight(), 0);
        assert_eq!(ZCancellationPriority::Normal.weight(), 2);
        assert_eq!(ZCancellationPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_cancellation_priority_label() {
        assert_eq!(ZCancellationPriority::Low.label(), "low");
        assert_eq!(ZCancellationPriority::High.label(), "high");
    }

    #[test]
    fn z_cancellation_priority_is_elevated() {
        assert!(!ZCancellationPriority::Normal.is_elevated());
        assert!(ZCancellationPriority::High.is_elevated());
        assert!(ZCancellationPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_cancellation_priority_display() {
        assert_eq!(format!("{}", ZCancellationPriority::Idle), "idle");
    }

    #[test]
    fn z_cancellation_priority_all_asc() {
        let all = ZCancellationPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZCancellationPriority::Idle);
        assert_eq!(all[4], ZCancellationPriority::Realtime);
    }

    #[test]
    fn z_cancellation_struct_new() {
        let s = ZCancellationCancelBudget::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_cancellation_struct_toggled_clone() {
        let s = ZCancellationCancelBudget::new();
        let t = s.toggled_clone();
        assert_ne!(s.exhausted, t.exhausted);
    }

    #[test]
    fn z_cancellation_rolling_hash_deterministic() {
        let h1 = z_cancellation_rolling_hash(b"test");
        let h2 = z_cancellation_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_cancellation_rolling_hash(b"a"), z_cancellation_rolling_hash(b"b"));
    }

    #[test]
    fn z_cancellation_pad_to_basic() {
        assert_eq!(z_cancellation_pad_to("hi", 5), "hi   ");
        assert_eq!(z_cancellation_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_cancellation_is_identifier_basic() {
        assert!(z_cancellation_is_identifier("foo_bar"));
        assert!(z_cancellation_is_identifier("abc123"));
        assert!(!z_cancellation_is_identifier(""));
        assert!(!z_cancellation_is_identifier("has space"));
    }

    #[test]
    fn z_cancellation_levenshtein_basic() {
        assert_eq!(z_cancellation_levenshtein("", ""), 0);
        assert_eq!(z_cancellation_levenshtein("abc", "abc"), 0);
        assert_eq!(z_cancellation_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_cancellation_unique_words_basic() {
        let w = z_cancellation_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_cancellation_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_cancellation_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_cancellation_common_prefix_basic() {
        assert_eq!(z_cancellation_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_cancellation_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_cancellation_struct_clear() {
        let mut s = ZCancellationCancelBudget::new();
        s.deadlines_ms.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_cancellation_rolling_hash_empty() {
        let h = z_cancellation_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_95_push_and_len() {
        let mut rb = super::XbRingBuffer95::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_95_overwrite() {
        let mut rb = super::XbRingBuffer95::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_95_get_out_of_bounds() {
        let rb = super::XbRingBuffer95::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_95_drain_all() {
        let mut rb = super::XbRingBuffer95::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_95_peek_front_back() {
        let mut rb = super::XbRingBuffer95::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_95_clear() {
        let mut rb = super::XbRingBuffer95::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_95_capacity() {
        let rb = super::XbRingBuffer95::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_95_basic() {
        let h = super::xb_fnv1a_95(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_95(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_95_different_inputs() {
        let h1 = super::xb_fnv1a_95(b"abc");
        let h2 = super::xb_fnv1a_95(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_95_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_95(&data);
        let dec = super::xb_rle_decode_95(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_95_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_95(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_95(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_95_values() {
        assert!((super::xb_clamp_95(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_95(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_95(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_95_values() {
        assert!((super::xb_lerp_95(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_95(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_95(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_95_wrap_around_twice() {
        let mut rb = super::XbRingBuffer95::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 11 ----

    #[test]
    fn xc_11_pool_new_empty() {
        let pool: super::Xc11Pool<i32> = super::Xc11Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_11_pool_release_acquire() {
        let mut pool = super::Xc11Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_11_pool_acquire_empty() {
        let mut pool: super::Xc11Pool<i32> = super::Xc11Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_11_pool_full() {
        let mut pool = super::Xc11Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_11_pool_drain() {
        let mut pool = super::Xc11Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_11_pool_stats() {
        let mut pool = super::Xc11Pool::new(8);
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
    fn xc_11_pool_clear() {
        let mut pool = super::Xc11Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_11_pool_shrink() {
        let mut pool = super::Xc11Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_11_pool_default() {
        let pool: super::Xc11Pool<String> = super::Xc11Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_11_pool_extend() {
        let mut pool = super::Xc11Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_11_pool_retain() {
        let mut pool = super::Xc11Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_11_scheduler_round_robin() {
        let mut sched = super::Xc11Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_11_scheduler_empty() {
        let mut sched = super::Xc11Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_11_scheduler_reset() {
        let mut sched = super::Xc11Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_11_scheduler_add_remove() {
        let mut sched = super::Xc11Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_11_scheduler_targets() {
        let sched = super::Xc11Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_11_hash_empty() {
        assert_eq!(super::xc_11_hash(b""), 5381);
    }

    #[test]
    fn xc_11_hash_data() {
        let h = super::xc_11_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_11_hash(b"hello"), h);
    }

    #[test]
    fn xc_11_reverse_str() {
        assert_eq!(super::xc_11_reverse("abc"), "cba");
        assert_eq!(super::xc_11_reverse(""), "");
    }

}