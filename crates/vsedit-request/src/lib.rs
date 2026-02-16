//! Cancellable async request service.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RequestError {
    RequestNotFound(RequestId),
    AlreadyCompleted(RequestId),
    InvalidTransition { id: RequestId, from: String, to: String },
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestError::RequestNotFound(id) => write!(f, "request {} not found", id),
            RequestError::AlreadyCompleted(id) => write!(f, "request {} already completed", id),
            RequestError::InvalidTransition { id, from, to } => {
                write!(f, "invalid transition for {}: {} -> {}", id, from, to)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(pub u64);

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
    Failed(String),
}

impl fmt::Display for RequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestState::Pending => write!(f, "Pending"),
            RequestState::InProgress => write!(f, "InProgress"),
            RequestState::Completed => write!(f, "Completed"),
            RequestState::Cancelled => write!(f, "Cancelled"),
            RequestState::Failed(reason) => write!(f, "Failed({})", reason),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub id: RequestId,
    pub method: String,
    pub state: RequestState,
    pub created_at: u64,
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Request(id={}, method={}, state={})", self.id.0, self.method, self.state)
    }
}

pub struct RequestBuilder {
    method: String,
    created_at: Option<u64>,
}

impl RequestBuilder {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            created_at: None,
        }
    }

    pub fn created_at(mut self, ts: u64) -> Self {
        self.created_at = Some(ts);
        self
    }

    pub fn build(self, service: &mut RequestService) -> RequestId {
        let id = RequestId(service.next_id);
        service.next_id += 1;
        service.requests.push(Request {
            id,
            method: self.method,
            state: RequestState::Pending,
            created_at: self.created_at.unwrap_or(0),
        });
        id
    }
}

pub struct RequestService {
    requests: Vec<Request>,
    next_id: u64,
}

impl RequestService {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_request(&mut self, method: impl Into<String>) -> RequestId {
        let id = RequestId(self.next_id);
        self.next_id += 1;
        self.requests.push(Request {
            id,
            method: method.into(),
            state: RequestState::Pending,
            created_at: 0,
        });
        id
    }

    fn set_state(&mut self, id: RequestId, state: RequestState) {
        if let Some(req) = self.requests.iter_mut().find(|r| r.id == id) {
            req.state = state;
        }
    }

    pub fn start(&mut self, id: RequestId) {
        self.set_state(id, RequestState::InProgress);
    }

    pub fn complete(&mut self, id: RequestId) {
        self.set_state(id, RequestState::Completed);
    }

    pub fn cancel(&mut self, id: RequestId) {
        self.set_state(id, RequestState::Cancelled);
    }

    pub fn fail(&mut self, id: RequestId, reason: impl Into<String>) {
        self.set_state(id, RequestState::Failed(reason.into()));
    }

    pub fn get_state(&self, id: RequestId) -> Option<&RequestState> {
        self.requests.iter().find(|r| r.id == id).map(|r| &r.state)
    }

    pub fn pending_count(&self) -> usize {
        self.requests
            .iter()
            .filter(|r| r.state == RequestState::Pending)
            .count()
    }

    pub fn cancel_all(&mut self) {
        for req in &mut self.requests {
            if matches!(req.state, RequestState::Pending | RequestState::InProgress) {
                req.state = RequestState::Cancelled;
            }
        }
    }

    pub fn get_request(&self, id: RequestId) -> Option<&Request> {
        self.requests.iter().find(|r| r.id == id)
    }

    pub fn try_cancel(&mut self, id: RequestId) -> Result<(), RequestError> {
        let req = self.requests.iter_mut().find(|r| r.id == id)
            .ok_or(RequestError::RequestNotFound(id))?;
        if req.state == RequestState::Completed {
            return Err(RequestError::AlreadyCompleted(id));
        }
        req.state = RequestState::Cancelled;
        Ok(())
    }

    pub fn in_progress_count(&self) -> usize {
        self.requests.iter().filter(|r| r.state == RequestState::InProgress).count()
    }

    pub fn completed_count(&self) -> usize {
        self.requests.iter().filter(|r| r.state == RequestState::Completed).count()
    }

    pub fn list_by_state(&self, state: &RequestState) -> Vec<&Request> {
        self.requests.iter().filter(|r| r.state == *state).collect()
    }

    pub fn remove_completed(&mut self) {
        self.requests.retain(|r| {
            !matches!(r.state, RequestState::Completed | RequestState::Cancelled | RequestState::Failed(_))
        });
    }

    pub fn total_count(&self) -> usize {
        self.requests.len()
    }
}

impl Default for RequestService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Request priority and queue
// ---------------------------------------------------------------------------

/// Priority levels for requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl fmt::Display for RequestPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestPriority::Low => write!(f, "Low"),
            RequestPriority::Normal => write!(f, "Normal"),
            RequestPriority::High => write!(f, "High"),
            RequestPriority::Critical => write!(f, "Critical"),
        }
    }
}

/// A request with an associated priority.
#[derive(Debug, Clone)]
pub struct PrioritizedRequest {
    pub id: RequestId,
    pub method: String,
    pub priority: RequestPriority,
    pub state: RequestState,
}

impl fmt::Display for PrioritizedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrioritizedRequest(id={}, priority={}, state={})", self.id, self.priority, self.state)
    }
}

/// A request queue that processes requests by priority.
pub struct PriorityRequestQueue {
    requests: Vec<PrioritizedRequest>,
    next_id: u64,
}

impl PriorityRequestQueue {
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            next_id: 1,
        }
    }

    /// Enqueue a request with the given priority. Returns its ID.
    pub fn enqueue(&mut self, method: impl Into<String>, priority: RequestPriority) -> RequestId {
        let id = RequestId(self.next_id);
        self.next_id += 1;
        self.requests.push(PrioritizedRequest {
            id,
            method: method.into(),
            priority,
            state: RequestState::Pending,
        });
        id
    }

    /// Dequeue the highest-priority pending request (FIFO within same priority).
    pub fn dequeue(&mut self) -> Option<RequestId> {
        let mut best_idx: Option<usize> = None;
        let mut best_prio = None;
        for (i, req) in self.requests.iter().enumerate() {
            if req.state != RequestState::Pending {
                continue;
            }
            if best_prio.is_none() || req.priority > best_prio.unwrap() {
                best_prio = Some(req.priority);
                best_idx = Some(i);
            }
        }
        if let Some(idx) = best_idx {
            self.requests[idx].state = RequestState::InProgress;
            Some(self.requests[idx].id)
        } else {
            None
        }
    }

    /// Number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.requests.iter().filter(|r| r.state == RequestState::Pending).count()
    }

    /// Total requests tracked.
    pub fn total_count(&self) -> usize {
        self.requests.len()
    }

    /// Mark a request as completed.
    pub fn complete(&mut self, id: RequestId) {
        if let Some(req) = self.requests.iter_mut().find(|r| r.id == id) {
            req.state = RequestState::Completed;
        }
    }

    /// Get a request by ID.
    pub fn get(&self, id: RequestId) -> Option<&PrioritizedRequest> {
        self.requests.iter().find(|r| r.id == id)
    }
}

impl Default for PriorityRequestQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Request statistics
// ---------------------------------------------------------------------------

/// Aggregated statistics about requests processed by a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestStats {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub cancelled: usize,
    pub failed: usize,
}

impl RequestService {
    /// Compute aggregate statistics across all tracked requests.
    pub fn stats(&self) -> RequestStats {
        let mut s = RequestStats {
            total: self.requests.len(),
            pending: 0,
            in_progress: 0,
            completed: 0,
            cancelled: 0,
            failed: 0,
        };
        for req in &self.requests {
            match &req.state {
                RequestState::Pending => s.pending += 1,
                RequestState::InProgress => s.in_progress += 1,
                RequestState::Completed => s.completed += 1,
                RequestState::Cancelled => s.cancelled += 1,
                RequestState::Failed(_) => s.failed += 1,
            }
        }
        s
    }

    /// Find requests whose method contains the given substring.
    pub fn find_by_method(&self, substring: &str) -> Vec<&Request> {
        self.requests.iter().filter(|r| r.method.contains(substring)).collect()
    }

    /// Return the oldest pending request, if any.
    pub fn oldest_pending(&self) -> Option<&Request> {
        self.requests.iter().find(|r| r.state == RequestState::Pending)
    }

    /// Fail all currently in-progress requests with the given reason.
    pub fn fail_all_in_progress(&mut self, reason: &str) {
        for req in &mut self.requests {
            if req.state == RequestState::InProgress {
                req.state = RequestState::Failed(reason.to_string());
            }
        }
    }
}

impl fmt::Display for RequestStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "total={} pending={} in_progress={} completed={} cancelled={} failed={}",
            self.total, self.pending, self.in_progress, self.completed, self.cancelled, self.failed
        )
    }
}

// ---------------------------------------------------------------------------
// Retry logic, timeout tracking, request batching, and more statistics
// ---------------------------------------------------------------------------

/// Configuration for request retry behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

impl RetryConfig {
    pub fn new(max_retries: u32, base_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
        }
    }

    /// Compute the delay for the nth retry (exponential backoff).
    pub fn delay_for_retry(&self, attempt: u32) -> u64 {
        self.base_delay_ms.saturating_mul(1u64 << attempt.min(16))
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
        }
    }
}

/// Tracks retry state for a single request.
#[derive(Debug, Clone)]
pub struct RetryTracker {
    pub request_id: RequestId,
    pub attempts: u32,
    pub config: RetryConfig,
}

impl RetryTracker {
    pub fn new(request_id: RequestId, config: RetryConfig) -> Self {
        Self {
            request_id,
            attempts: 0,
            config,
        }
    }

    /// Record an attempt and return true if more retries are allowed.
    pub fn record_attempt(&mut self) -> bool {
        self.attempts += 1;
        self.attempts <= self.config.max_retries
    }

    /// Check if retries are exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.attempts > self.config.max_retries
    }

    /// Compute the delay before the next retry.
    pub fn next_delay(&self) -> u64 {
        self.config.delay_for_retry(self.attempts)
    }
}

/// Tracks request timeout state.
#[derive(Debug, Clone)]
pub struct TimeoutTracker {
    pub request_id: RequestId,
    pub timeout_ms: u64,
    pub started_at_ms: u64,
}

impl TimeoutTracker {
    pub fn new(request_id: RequestId, timeout_ms: u64, started_at_ms: u64) -> Self {
        Self {
            request_id,
            timeout_ms,
            started_at_ms,
        }
    }

    /// Check if the request has timed out given the current time.
    pub fn is_timed_out(&self, current_time_ms: u64) -> bool {
        current_time_ms.saturating_sub(self.started_at_ms) >= self.timeout_ms
    }

    /// Return how many milliseconds remain before timeout.
    pub fn remaining_ms(&self, current_time_ms: u64) -> u64 {
        let elapsed = current_time_ms.saturating_sub(self.started_at_ms);
        self.timeout_ms.saturating_sub(elapsed)
    }
}

/// A batch of requests that can be submitted together.
#[derive(Debug, Clone)]
pub struct RequestBatch {
    pub methods: Vec<String>,
}

impl RequestBatch {
    pub fn new() -> Self {
        Self {
            methods: Vec::new(),
        }
    }

    pub fn add(&mut self, method: impl Into<String>) {
        self.methods.push(method.into());
    }

    pub fn len(&self) -> usize {
        self.methods.len()
    }

    pub fn is_empty(&self) -> bool {
        self.methods.is_empty()
    }

    /// Submit all methods in the batch to a `RequestService`, returning their IDs.
    pub fn submit(&self, service: &mut RequestService) -> Vec<RequestId> {
        self.methods.iter().map(|m| service.create_request(m.clone())).collect()
    }
}

impl Default for RequestBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestService {
    /// Returns the pending request with the largest `created_at` timestamp.
    pub fn newest_pending(&self) -> Option<&Request> {
        self.requests
            .iter()
            .filter(|r| matches!(r.state, RequestState::Pending))
            .max_by_key(|r| r.created_at)
    }
}

// ---------------------------------------------------------------------------
// Request retry orchestrator
// ---------------------------------------------------------------------------

/// Outcome of a single retry attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryOutcome {
    /// The request succeeded.
    Success,
    /// The request failed but can be retried.
    Retriable(String),
    /// The request failed with a non-retriable error.
    Fatal(String),
}

impl fmt::Display for RetryOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetryOutcome::Success => write!(f, "Success"),
            RetryOutcome::Retriable(msg) => write!(f, "Retriable: {msg}"),
            RetryOutcome::Fatal(msg) => write!(f, "Fatal: {msg}"),
        }
    }
}

/// Full retry orchestrator that tracks attempts for a request.
#[derive(Debug, Clone)]
pub struct RequestRetry {
    pub request_id: RequestId,
    pub config: RetryConfig,
    pub attempts: Vec<RetryAttempt>,
    pub max_total_delay_ms: u64,
}

/// Record of a single retry attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryAttempt {
    pub attempt_number: u32,
    pub outcome: RetryOutcome,
    pub delay_ms: u64,
}

impl RequestRetry {
    pub fn new(request_id: RequestId, config: RetryConfig) -> Self {
        Self {
            request_id,
            config,
            attempts: Vec::new(),
            max_total_delay_ms: u64::MAX,
        }
    }

    /// Set a maximum total delay across all retries.
    pub fn with_max_total_delay(mut self, max_ms: u64) -> Self {
        self.max_total_delay_ms = max_ms;
        self
    }

    /// Record an attempt outcome. Returns true if a retry should be attempted.
    pub fn record(&mut self, outcome: RetryOutcome) -> bool {
        let attempt_number = self.attempts.len() as u32;
        let delay = self.config.delay_for_retry(attempt_number);
        self.attempts.push(RetryAttempt {
            attempt_number,
            outcome: outcome.clone(),
            delay_ms: delay,
        });

        match outcome {
            RetryOutcome::Success | RetryOutcome::Fatal(_) => false,
            RetryOutcome::Retriable(_) => {
                if self.attempts.len() as u32 > self.config.max_retries {
                    return false;
                }
                if self.total_delay_ms() > self.max_total_delay_ms {
                    return false;
                }
                true
            }
        }
    }

    /// Total delay accumulated across all attempts.
    pub fn total_delay_ms(&self) -> u64 {
        self.attempts.iter().map(|a| a.delay_ms).sum()
    }

    /// Number of attempts made so far.
    pub fn attempt_count(&self) -> usize {
        self.attempts.len()
    }

    /// Whether the last attempt was successful.
    pub fn succeeded(&self) -> bool {
        self.attempts.last().map_or(false, |a| a.outcome == RetryOutcome::Success)
    }

    /// Whether retries are exhausted (either max retries reached or fatal error).
    pub fn is_exhausted(&self) -> bool {
        if self.succeeded() {
            return false;
        }
        if self.attempts.len() as u32 > self.config.max_retries {
            return true;
        }
        self.attempts.last().map_or(false, |a| matches!(a.outcome, RetryOutcome::Fatal(_)))
    }

    /// Get the delay to use before the next retry attempt.
    pub fn next_delay_ms(&self) -> u64 {
        self.config.delay_for_retry(self.attempts.len() as u32)
    }

    /// Get all failure reasons from attempts.
    pub fn failure_reasons(&self) -> Vec<&str> {
        self.attempts
            .iter()
            .filter_map(|a| match &a.outcome {
                RetryOutcome::Retriable(msg) | RetryOutcome::Fatal(msg) => Some(msg.as_str()),
                RetryOutcome::Success => None,
            })
            .collect()
    }
}

/// Simulate retry_with_backoff: run a closure up to max_retries times.
///
/// Returns the list of attempts and whether the final outcome was success.
pub fn retry_with_backoff<F>(request_id: RequestId, config: RetryConfig, mut attempt_fn: F) -> RequestRetry
where
    F: FnMut(u32) -> RetryOutcome,
{
    let mut retry = RequestRetry::new(request_id, config);
    loop {
        let attempt_num = retry.attempt_count() as u32;
        let outcome = attempt_fn(attempt_num);
        let should_retry = retry.record(outcome);
        if !should_retry {
            break;
        }
    }
    retry
}

// ---------------------------------------------------------------------------
// Additional RequestState helpers
// ---------------------------------------------------------------------------

impl RequestState {
    /// Returns `true` when the state is terminal (Completed, Cancelled, or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, RequestState::Completed | RequestState::Cancelled | RequestState::Failed(_))
    }

    /// Human-readable label for the state.
    pub fn label(&self) -> &'static str {
        match self {
            RequestState::Pending => "Pending",
            RequestState::InProgress => "In Progress",
            RequestState::Completed => "Completed",
            RequestState::Cancelled => "Cancelled",
            RequestState::Failed(_) => "Failed",
        }
    }
}

// ---------------------------------------------------------------------------
// Additional RequestService helpers
// ---------------------------------------------------------------------------

impl RequestService {
    /// Returns `true` when the service has no tracked requests.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Count of requests currently in a `Failed` state.
    pub fn failed_count(&self) -> usize {
        self.requests
            .iter()
            .filter(|r| matches!(r.state, RequestState::Failed(_)))
            .count()
    }

    /// Return the method string of a request by its ID.
    pub fn get_method(&self, id: RequestId) -> Option<&str> {
        self.requests
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.method.as_str())
    }

    /// Cancel all pending/in-progress requests whose `created_at` timestamp is
    /// older than `max_age` (i.e. `created_at < max_age`). Returns the number
    /// of requests cancelled.
    pub fn cancel_timed_out(&mut self, max_age: u64) -> usize {
        let mut count = 0;
        for req in &mut self.requests {
            if req.created_at < max_age
                && matches!(req.state, RequestState::Pending | RequestState::InProgress)
            {
                req.state = RequestState::Cancelled;
                count += 1;
            }
        }
        count
    }
}

// ---------------------------------------------------------------------------
// RequestRetryPolicy
// ---------------------------------------------------------------------------

/// A configurable retry policy with exponential backoff and jitter.
#[derive(Debug, Clone)]
pub struct RequestRetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
    pub jitter_percent: f64,
}

impl RequestRetryPolicy {
    /// Create a new retry policy.
    pub fn new(max_retries: u32, base_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms: 30_000,
            backoff_factor: 2.0,
            jitter_percent: 0.0,
        }
    }

    /// Set the maximum delay cap.
    pub fn with_max_delay(mut self, ms: u64) -> Self {
        self.max_delay_ms = ms;
        self
    }

    /// Set the backoff multiplication factor.
    pub fn with_backoff_factor(mut self, factor: f64) -> Self {
        self.backoff_factor = factor;
        self
    }

    /// Set jitter as a percentage (0.0–1.0) of the computed delay.
    pub fn with_jitter(mut self, percent: f64) -> Self {
        self.jitter_percent = percent.clamp(0.0, 1.0);
        self
    }

    /// Compute the delay for a given attempt number (0-based).
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let base = self.base_delay_ms as f64 * self.backoff_factor.powi(attempt as i32);
        let capped = base.min(self.max_delay_ms as f64);
        capped as u64
    }

    /// Whether the given attempt number is within the allowed retries.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    /// Total maximum delay if all retries are used (without jitter).
    pub fn total_max_delay(&self) -> u64 {
        (0..self.max_retries).map(|a| self.delay_for_attempt(a)).sum()
    }
}

impl Default for RequestRetryPolicy {
    fn default() -> Self {
        Self::new(3, 100)
    }
}

// ---------------------------------------------------------------------------
// RequestMetrics
// ---------------------------------------------------------------------------

/// Tracks request metrics: latency, success rate, counts.
#[derive(Debug, Clone)]
pub struct RequestMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_latency_ms: u64,
    min_latency_ms: Option<u64>,
    max_latency_ms: Option<u64>,
}

impl RequestMetrics {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_latency_ms: 0,
            min_latency_ms: None,
            max_latency_ms: None,
        }
    }

    /// Record a successful request with its latency.
    pub fn record_success(&mut self, latency_ms: u64) {
        self.total_requests += 1;
        self.successful_requests += 1;
        self.record_latency(latency_ms);
    }

    /// Record a failed request with its latency.
    pub fn record_failure(&mut self, latency_ms: u64) {
        self.total_requests += 1;
        self.failed_requests += 1;
        self.record_latency(latency_ms);
    }

    fn record_latency(&mut self, latency_ms: u64) {
        self.total_latency_ms += latency_ms;
        self.min_latency_ms = Some(self.min_latency_ms.map_or(latency_ms, |m| m.min(latency_ms)));
        self.max_latency_ms = Some(self.max_latency_ms.map_or(latency_ms, |m| m.max(latency_ms)));
    }

    /// Average latency in milliseconds.
    pub fn average_latency_ms(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.total_latency_ms as f64 / self.total_requests as f64
    }

    /// Success rate as a percentage (0.0–100.0).
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.successful_requests as f64 / self.total_requests as f64) * 100.0
    }

    /// Failure rate as a percentage (0.0–100.0).
    pub fn failure_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.failed_requests as f64 / self.total_requests as f64) * 100.0
    }

    pub fn total(&self) -> u64 {
        self.total_requests
    }

    pub fn min_latency(&self) -> Option<u64> {
        self.min_latency_ms
    }

    pub fn max_latency(&self) -> Option<u64> {
        self.max_latency_ms
    }

    /// Reset all metrics.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for RequestMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Metrics: {} total, {:.1}% success, {:.1}ms avg latency",
            self.total_requests,
            self.success_rate(),
            self.average_latency_ms(),
        )
    }
}

// ---------------------------------------------------------------------------
// RequestDeduplicator
// ---------------------------------------------------------------------------

/// Deduplicates requests by method, coalescing identical in-flight requests.
pub struct RequestDeduplicator {
    in_flight: std::collections::HashMap<String, RequestId>,
}

impl RequestDeduplicator {
    pub fn new() -> Self {
        Self {
            in_flight: std::collections::HashMap::new(),
        }
    }

    /// Try to register a method as in-flight. Returns `Some(existing_id)` if
    /// the method is already in flight, `None` if it was newly registered.
    pub fn try_register(&mut self, method: &str, id: RequestId) -> Option<RequestId> {
        if let Some(&existing) = self.in_flight.get(method) {
            Some(existing)
        } else {
            self.in_flight.insert(method.to_string(), id);
            None
        }
    }

    /// Mark a method as completed, removing it from in-flight tracking.
    pub fn complete(&mut self, method: &str) -> Option<RequestId> {
        self.in_flight.remove(method)
    }

    /// Check if a method is currently in flight.
    pub fn is_in_flight(&self, method: &str) -> bool {
        self.in_flight.contains_key(method)
    }

    /// Number of in-flight methods.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Get the request ID for an in-flight method.
    pub fn get_in_flight(&self, method: &str) -> Option<RequestId> {
        self.in_flight.get(method).copied()
    }

    /// Clear all in-flight tracking.
    pub fn clear(&mut self) {
        self.in_flight.clear();
    }
}

impl Default for RequestDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_lifecycle() {
        let mut svc = RequestService::new();
        let id = svc.create_request("GET /api");
        assert_eq!(svc.get_state(id), Some(&RequestState::Pending));
        svc.start(id);
        assert_eq!(svc.get_state(id), Some(&RequestState::InProgress));
        svc.complete(id);
        assert_eq!(svc.get_state(id), Some(&RequestState::Completed));
    }

    #[test]
    fn cancel_and_fail() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("POST /data");
        let id2 = svc.create_request("PUT /data");
        svc.cancel(id1);
        svc.fail(id2, "timeout");
        assert_eq!(svc.get_state(id1), Some(&RequestState::Cancelled));
        assert_eq!(
            svc.get_state(id2),
            Some(&RequestState::Failed("timeout".into()))
        );
    }

    #[test]
    fn pending_count_and_cancel_all() {
        let mut svc = RequestService::new();
        svc.create_request("a");
        svc.create_request("b");
        let id3 = svc.create_request("c");
        svc.start(id3);
        assert_eq!(svc.pending_count(), 2);
        svc.cancel_all();
        assert_eq!(svc.pending_count(), 0);
    }

    #[test]
    fn get_request_returns_full_request() {
        let mut svc = RequestService::new();
        let id = svc.create_request("GET /users");
        let req = svc.get_request(id).unwrap();
        assert_eq!(req.method, "GET /users");
        assert_eq!(req.state, RequestState::Pending);
        assert!(svc.get_request(RequestId(999)).is_none());
    }

    #[test]
    fn try_cancel_already_completed() {
        let mut svc = RequestService::new();
        let id = svc.create_request("POST /submit");
        svc.start(id);
        svc.complete(id);
        let err = svc.try_cancel(id).unwrap_err();
        assert_eq!(err, RequestError::AlreadyCompleted(id));
    }

    #[test]
    fn try_cancel_not_found() {
        let mut svc = RequestService::new();
        let err = svc.try_cancel(RequestId(42)).unwrap_err();
        assert_eq!(err, RequestError::RequestNotFound(RequestId(42)));
    }

    #[test]
    fn try_cancel_success() {
        let mut svc = RequestService::new();
        let id = svc.create_request("DELETE /item");
        svc.start(id);
        assert!(svc.try_cancel(id).is_ok());
        assert_eq!(svc.get_state(id), Some(&RequestState::Cancelled));
    }

    #[test]
    fn in_progress_and_completed_counts() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        let id3 = svc.create_request("c");
        svc.start(id1);
        svc.start(id2);
        svc.start(id3);
        svc.complete(id3);
        assert_eq!(svc.in_progress_count(), 2);
        assert_eq!(svc.completed_count(), 1);
    }

    #[test]
    fn list_by_state_filters_correctly() {
        let mut svc = RequestService::new();
        svc.create_request("a");
        let id2 = svc.create_request("b");
        svc.create_request("c");
        svc.start(id2);
        let pending = svc.list_by_state(&RequestState::Pending);
        assert_eq!(pending.len(), 2);
        let in_progress = svc.list_by_state(&RequestState::InProgress);
        assert_eq!(in_progress.len(), 1);
        assert_eq!(in_progress[0].method, "b");
    }

    #[test]
    fn remove_completed_cleans_terminal_states() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        let id3 = svc.create_request("c");
        let id4 = svc.create_request("d");
        svc.start(id1);
        svc.complete(id1);
        svc.cancel(id2);
        svc.fail(id3, "err");
        // id4 stays pending
        assert_eq!(svc.total_count(), 4);
        svc.remove_completed();
        assert_eq!(svc.total_count(), 1);
        assert_eq!(svc.get_request(id4).unwrap().method, "d");
    }

    #[test]
    fn total_count_tracks_all() {
        let mut svc = RequestService::new();
        assert_eq!(svc.total_count(), 0);
        svc.create_request("x");
        svc.create_request("y");
        assert_eq!(svc.total_count(), 2);
    }

    #[test]
    fn display_request_state() {
        assert_eq!(format!("{}", RequestState::Pending), "Pending");
        assert_eq!(format!("{}", RequestState::InProgress), "InProgress");
        assert_eq!(format!("{}", RequestState::Completed), "Completed");
        assert_eq!(format!("{}", RequestState::Cancelled), "Cancelled");
        assert_eq!(format!("{}", RequestState::Failed("oops".into())), "Failed(oops)");
    }

    #[test]
    fn display_request_and_id() {
        let req = Request {
            id: RequestId(7),
            method: "GET /health".into(),
            state: RequestState::Pending,
            created_at: 0,
        };
        assert_eq!(format!("{}", req), "Request(id=7, method=GET /health, state=Pending)");
        assert_eq!(format!("{}", RequestId(42)), "req-42");
    }

    #[test]
    fn builder_with_defaults() {
        let mut svc = RequestService::new();
        let id = RequestBuilder::new("PATCH /item").build(&mut svc);
        let req = svc.get_request(id).unwrap();
        assert_eq!(req.method, "PATCH /item");
        assert_eq!(req.created_at, 0);
    }

    #[test]
    fn builder_with_created_at() {
        let mut svc = RequestService::new();
        let id = RequestBuilder::new("GET /ts")
            .created_at(1700000000)
            .build(&mut svc);
        let req = svc.get_request(id).unwrap();
        assert_eq!(req.created_at, 1700000000);
    }

    #[test]
    fn error_display() {
        let e1 = RequestError::RequestNotFound(RequestId(5));
        assert_eq!(format!("{}", e1), "request req-5 not found");
        let e2 = RequestError::AlreadyCompleted(RequestId(3));
        assert_eq!(format!("{}", e2), "request req-3 already completed");
        let e3 = RequestError::InvalidTransition {
            id: RequestId(1),
            from: "Completed".into(),
            to: "Pending".into(),
        };
        assert_eq!(format!("{}", e3), "invalid transition for req-1: Completed -> Pending");
    }

    #[test]
    fn priority_ordering() {
        assert!(RequestPriority::Critical > RequestPriority::High);
        assert!(RequestPriority::High > RequestPriority::Normal);
        assert!(RequestPriority::Normal > RequestPriority::Low);
    }

    #[test]
    fn priority_display() {
        assert_eq!(format!("{}", RequestPriority::Low), "Low");
        assert_eq!(format!("{}", RequestPriority::Critical), "Critical");
    }

    #[test]
    fn priority_queue_dequeue_order() {
        let mut q = PriorityRequestQueue::new();
        let id_low = q.enqueue("low_req", RequestPriority::Low);
        let id_high = q.enqueue("high_req", RequestPriority::High);
        let _id_normal = q.enqueue("normal_req", RequestPriority::Normal);

        // Should dequeue highest priority first
        let dequeued = q.dequeue().unwrap();
        assert_eq!(dequeued, id_high);
        assert_eq!(q.pending_count(), 2);

        // Mark it complete, dequeue next
        q.complete(dequeued);
        let next = q.dequeue().unwrap();
        assert_ne!(next, id_low); // normal > low
    }

    #[test]
    fn priority_queue_empty_dequeue() {
        let mut q = PriorityRequestQueue::new();
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn priority_queue_get() {
        let mut q = PriorityRequestQueue::new();
        let id = q.enqueue("test", RequestPriority::Normal);
        let req = q.get(id).unwrap();
        assert_eq!(req.method, "test");
        assert_eq!(req.priority, RequestPriority::Normal);
    }

    #[test]
    fn priority_queue_complete() {
        let mut q = PriorityRequestQueue::new();
        let id = q.enqueue("x", RequestPriority::High);
        q.dequeue();
        q.complete(id);
        let req = q.get(id).unwrap();
        assert_eq!(req.state, RequestState::Completed);
    }

    #[test]
    fn prioritized_request_display() {
        let pr = PrioritizedRequest {
            id: RequestId(1),
            method: "GET".into(),
            priority: RequestPriority::High,
            state: RequestState::Pending,
        };
        let s = format!("{}", pr);
        assert!(s.contains("High"));
        assert!(s.contains("Pending"));
    }

    #[test]
    fn request_stats_computation() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        let id3 = svc.create_request("c");
        svc.start(id1);
        svc.complete(id1);
        svc.cancel(id2);
        svc.fail(id3, "err");
        svc.create_request("d"); // pending

        let stats = svc.stats();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.cancelled, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.pending, 1);
    }

    #[test]
    fn request_stats_display() {
        let stats = RequestStats {
            total: 5, pending: 1, in_progress: 2, completed: 1, cancelled: 0, failed: 1,
        };
        let s = format!("{}", stats);
        assert!(s.contains("total=5"));
        assert!(s.contains("failed=1"));
    }

    #[test]
    fn find_by_method_substring() {
        let mut svc = RequestService::new();
        svc.create_request("GET /users");
        svc.create_request("POST /users");
        svc.create_request("GET /items");
        let results = svc.find_by_method("/users");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn oldest_pending_returns_first() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("first");
        svc.create_request("second");
        let oldest = svc.oldest_pending().unwrap();
        assert_eq!(oldest.id, id1);
    }

    #[test]
    fn fail_all_in_progress() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        svc.create_request("c"); // stays pending
        svc.start(id1);
        svc.start(id2);
        svc.fail_all_in_progress("shutdown");
        assert_eq!(svc.get_state(id1), Some(&RequestState::Failed("shutdown".into())));
        assert_eq!(svc.get_state(id2), Some(&RequestState::Failed("shutdown".into())));
        assert_eq!(svc.pending_count(), 1);
    }

    #[test]
    fn retry_config_delay_exponential() {
        let cfg = RetryConfig::new(3, 100);
        assert_eq!(cfg.delay_for_retry(0), 100);
        assert_eq!(cfg.delay_for_retry(1), 200);
        assert_eq!(cfg.delay_for_retry(2), 400);
        assert_eq!(cfg.delay_for_retry(3), 800);
    }

    #[test]
    fn retry_tracker_exhaustion() {
        let cfg = RetryConfig::new(2, 50);
        let mut rt = RetryTracker::new(RequestId(1), cfg);
        assert!(!rt.is_exhausted());
        assert!(rt.record_attempt()); // attempt 1 <= 2
        assert!(rt.record_attempt()); // attempt 2 <= 2
        assert!(!rt.record_attempt()); // attempt 3 > 2
        assert!(rt.is_exhausted());
    }

    #[test]
    fn timeout_tracker_check() {
        let tt = TimeoutTracker::new(RequestId(1), 1000, 500);
        assert!(!tt.is_timed_out(1000));
        assert!(tt.is_timed_out(1500));
        assert!(tt.is_timed_out(2000));
        assert_eq!(tt.remaining_ms(1000), 500);
        assert_eq!(tt.remaining_ms(1500), 0);
    }

    #[test]
    fn request_batch_submit() {
        let mut batch = RequestBatch::new();
        assert!(batch.is_empty());
        batch.add("GET /a");
        batch.add("POST /b");
        assert_eq!(batch.len(), 2);
        let mut svc = RequestService::new();
        let ids = batch.submit(&mut svc);
        assert_eq!(ids.len(), 2);
        assert_eq!(svc.total_count(), 2);
    }

    #[test]
    fn retry_config_default() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.base_delay_ms, 100);
    }

    #[test]
    fn newest_pending_returns_largest_created_at() {
        let mut svc = RequestService::new();
        let _a = RequestBuilder::new("GET /a").created_at(50).build(&mut svc);
        let _b = RequestBuilder::new("GET /b").created_at(10).build(&mut svc);
        let _c = RequestBuilder::new("GET /c").created_at(30).build(&mut svc);
        let newest = svc.newest_pending().unwrap();
        assert_eq!(newest.created_at, 50);
    }

    #[test]
    fn newest_pending_none_when_empty() {
        let svc = RequestService::new();
        assert!(svc.newest_pending().is_none());
    }

    #[test]
    fn retry_outcome_display() {
        assert_eq!(format!("{}", RetryOutcome::Success), "Success");
        assert!(format!("{}", RetryOutcome::Retriable("timeout".into())).contains("timeout"));
        assert!(format!("{}", RetryOutcome::Fatal("crash".into())).contains("crash"));
    }

    #[test]
    fn request_retry_succeeds_first_try() {
        let mut r = RequestRetry::new(RequestId(1), RetryConfig::new(3, 100));
        let should_retry = r.record(RetryOutcome::Success);
        assert!(!should_retry);
        assert!(r.succeeded());
        assert!(!r.is_exhausted());
        assert_eq!(r.attempt_count(), 1);
    }

    #[test]
    fn request_retry_retries_then_succeeds() {
        let mut r = RequestRetry::new(RequestId(1), RetryConfig::new(3, 100));
        assert!(r.record(RetryOutcome::Retriable("err".into())));
        assert!(r.record(RetryOutcome::Retriable("err".into())));
        assert!(!r.record(RetryOutcome::Success));
        assert!(r.succeeded());
        assert_eq!(r.attempt_count(), 3);
    }

    #[test]
    fn request_retry_exhausted() {
        let mut r = RequestRetry::new(RequestId(1), RetryConfig::new(2, 100));
        r.record(RetryOutcome::Retriable("err".into()));
        r.record(RetryOutcome::Retriable("err".into()));
        let should_retry = r.record(RetryOutcome::Retriable("err".into()));
        assert!(!should_retry);
        assert!(r.is_exhausted());
        assert!(!r.succeeded());
    }

    #[test]
    fn request_retry_fatal_stops_immediately() {
        let mut r = RequestRetry::new(RequestId(1), RetryConfig::new(5, 100));
        let should_retry = r.record(RetryOutcome::Fatal("crash".into()));
        assert!(!should_retry);
        assert!(r.is_exhausted());
        assert_eq!(r.attempt_count(), 1);
    }

    #[test]
    fn request_retry_total_delay() {
        let mut r = RequestRetry::new(RequestId(1), RetryConfig::new(3, 100));
        r.record(RetryOutcome::Retriable("err".into()));
        r.record(RetryOutcome::Retriable("err".into()));
        r.record(RetryOutcome::Success);
        assert!(r.total_delay_ms() > 0);
    }

    #[test]
    fn request_retry_max_total_delay() {
        let mut r = RequestRetry::new(RequestId(1), RetryConfig::new(10, 100))
            .with_max_total_delay(150);
        r.record(RetryOutcome::Retriable("err".into())); // delay 100
        let should_retry = r.record(RetryOutcome::Retriable("err".into())); // delay 200, total > 150
        assert!(!should_retry);
    }

    #[test]
    fn request_retry_failure_reasons() {
        let mut r = RequestRetry::new(RequestId(1), RetryConfig::new(3, 100));
        r.record(RetryOutcome::Retriable("timeout".into()));
        r.record(RetryOutcome::Retriable("connection reset".into()));
        r.record(RetryOutcome::Success);
        let reasons = r.failure_reasons();
        assert_eq!(reasons.len(), 2);
        assert_eq!(reasons[0], "timeout");
        assert_eq!(reasons[1], "connection reset");
    }

    #[test]
    fn retry_with_backoff_succeeds_after_retries() {
        let result = retry_with_backoff(RequestId(1), RetryConfig::new(3, 100), |attempt| {
            if attempt < 2 {
                RetryOutcome::Retriable("not ready".into())
            } else {
                RetryOutcome::Success
            }
        });
        assert!(result.succeeded());
        assert_eq!(result.attempt_count(), 3);
    }

    #[test]
    fn retry_with_backoff_exhausts_retries() {
        let result = retry_with_backoff(RequestId(1), RetryConfig::new(2, 50), |_| {
            RetryOutcome::Retriable("always fails".into())
        });
        assert!(!result.succeeded());
        assert!(result.is_exhausted());
    }

    #[test]
    fn retry_with_backoff_fatal_error() {
        let result = retry_with_backoff(RequestId(1), RetryConfig::new(5, 100), |_| {
            RetryOutcome::Fatal("unrecoverable".into())
        });
        assert!(!result.succeeded());
        assert_eq!(result.attempt_count(), 1);
    }

    // -----------------------------------------------------------------------
    // Tests for newly added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn is_empty_on_new_service() {
        let svc = RequestService::new();
        assert!(svc.is_empty());
        let mut svc2 = RequestService::new();
        svc2.create_request("GET /ping");
        assert!(!svc2.is_empty());
    }

    #[test]
    fn failed_count_tracks_failures() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        svc.create_request("c");
        svc.fail(id1, "timeout");
        svc.fail(id2, "connection refused");
        assert_eq!(svc.failed_count(), 2);
    }

    #[test]
    fn get_method_returns_method_string() {
        let mut svc = RequestService::new();
        let id = svc.create_request("POST /upload");
        assert_eq!(svc.get_method(id), Some("POST /upload"));
        assert_eq!(svc.get_method(RequestId(999)), None);
    }

    #[test]
    fn request_state_is_terminal() {
        assert!(!RequestState::Pending.is_terminal());
        assert!(!RequestState::InProgress.is_terminal());
        assert!(RequestState::Completed.is_terminal());
        assert!(RequestState::Cancelled.is_terminal());
        assert!(RequestState::Failed("err".into()).is_terminal());
    }

    #[test]
    fn request_state_label() {
        assert_eq!(RequestState::Pending.label(), "Pending");
        assert_eq!(RequestState::InProgress.label(), "In Progress");
        assert_eq!(RequestState::Completed.label(), "Completed");
        assert_eq!(RequestState::Cancelled.label(), "Cancelled");
        assert_eq!(RequestState::Failed("x".into()).label(), "Failed");
    }

    #[test]
    fn cancel_timed_out_cancels_old_requests() {
        let mut svc = RequestService::new();
        let _old = RequestBuilder::new("GET /old").created_at(10).build(&mut svc);
        let _mid = RequestBuilder::new("GET /mid").created_at(50).build(&mut svc);
        let _new = RequestBuilder::new("GET /new").created_at(100).build(&mut svc);
        let cancelled = svc.cancel_timed_out(60);
        assert_eq!(cancelled, 2);
        assert_eq!(svc.pending_count(), 1);
        assert_eq!(svc.get_method(_new), Some("GET /new"));
    }

    #[test]
    fn cancel_timed_out_skips_terminal_requests() {
        let mut svc = RequestService::new();
        let id = RequestBuilder::new("GET /done").created_at(5).build(&mut svc);
        svc.start(id);
        svc.complete(id);
        let cancelled = svc.cancel_timed_out(100);
        assert_eq!(cancelled, 0);
        assert_eq!(svc.get_state(id), Some(&RequestState::Completed));
    }

    // ── RetryPolicy / Metrics / Deduplicator tests ──

    #[test]
    fn retry_policy_exponential_backoff() {
        let policy = RequestRetryPolicy::new(5, 100).with_backoff_factor(2.0);
        assert_eq!(policy.delay_for_attempt(0), 100);
        assert_eq!(policy.delay_for_attempt(1), 200);
        assert_eq!(policy.delay_for_attempt(2), 400);
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(5));
    }

    #[test]
    fn retry_policy_max_delay_cap() {
        let policy = RequestRetryPolicy::new(10, 1000).with_max_delay(5000);
        assert_eq!(policy.delay_for_attempt(0), 1000);
        assert_eq!(policy.delay_for_attempt(10), 5000); // capped
    }

    #[test]
    fn request_metrics_tracking() {
        let mut metrics = RequestMetrics::new();
        metrics.record_success(50);
        metrics.record_success(100);
        metrics.record_failure(200);
        assert_eq!(metrics.total(), 3);
        assert!((metrics.success_rate() - 66.66).abs() < 1.0);
        assert!((metrics.average_latency_ms() - 116.666).abs() < 1.0);
        assert_eq!(metrics.min_latency(), Some(50));
        assert_eq!(metrics.max_latency(), Some(200));
    }

    #[test]
    fn request_metrics_reset() {
        let mut metrics = RequestMetrics::new();
        metrics.record_success(100);
        metrics.reset();
        assert_eq!(metrics.total(), 0);
        assert_eq!(metrics.min_latency(), None);
    }

    #[test]
    fn deduplicator_coalesces_requests() {
        let mut dedup = RequestDeduplicator::new();
        let id1 = RequestId(1);
        let id2 = RequestId(2);
        assert!(dedup.try_register("GET /users", id1).is_none());
        assert_eq!(dedup.try_register("GET /users", id2), Some(id1));
        assert!(dedup.is_in_flight("GET /users"));
        assert_eq!(dedup.in_flight_count(), 1);
        dedup.complete("GET /users");
        assert!(!dedup.is_in_flight("GET /users"));
    }

    #[test]
    fn retry_policy_total_max_delay() {
        let policy = RequestRetryPolicy::new(3, 100).with_backoff_factor(2.0);
        // 100 + 200 + 400 = 700
        assert_eq!(policy.total_max_delay(), 700);
    }
}
