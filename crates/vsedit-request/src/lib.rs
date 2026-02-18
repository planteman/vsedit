//! Cancellable async request service.

use std::collections::HashMap;
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
    pub url: Option<String>,
    pub headers: Vec<(String, String)>,
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
    url: Option<String>,
    headers: Vec<(String, String)>,
}

impl RequestBuilder {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            created_at: None,
            url: None,
            headers: Vec::new(),
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
            url: self.url,
            headers: self.headers,
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
            url: None,
            headers: Vec::new(),
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

impl RequestService {
    /// Return all requests whose method starts with the given prefix.
    pub fn find_by_method_prefix(&self, prefix: &str) -> Vec<&Request> {
        self.requests.iter().filter(|r| r.method.starts_with(prefix)).collect()
    }

    /// Return the IDs of all requests in a given state.
    pub fn ids_in_state(&self, state: &RequestState) -> Vec<RequestId> {
        self.requests.iter()
            .filter(|r| std::mem::discriminant(&r.state) == std::mem::discriminant(state))
            .map(|r| r.id)
            .collect()
    }

    /// Fail all pending requests with the given reason.
    pub fn fail_all_pending(&mut self, reason: &str) {
        for req in &mut self.requests {
            if matches!(req.state, RequestState::Pending) {
                req.state = RequestState::Failed(reason.to_string());
            }
        }
    }

    /// Return the number of unique methods across all requests.
    pub fn unique_method_count(&self) -> usize {
        let set: std::collections::HashSet<&str> = self.requests.iter().map(|r| r.method.as_str()).collect();
        set.len()
    }

    /// Return requests sorted by created_at descending.
    pub fn recent_requests(&self, limit: usize) -> Vec<&Request> {
        let mut sorted: Vec<&Request> = self.requests.iter().collect();
        sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sorted.truncate(limit);
        sorted
    }
}

impl PriorityRequestQueue {
    /// Return the number of requests with a given priority.
    pub fn count_by_priority(&self, priority: RequestPriority) -> usize {
        self.requests.iter().filter(|r| r.priority == priority).count()
    }

    /// Return true if the queue has any requests in the given priority.
    pub fn has_priority(&self, priority: RequestPriority) -> bool {
        self.requests.iter().any(|r| r.priority == priority)
    }
}

impl RequestBatch {
    /// Returns the list of method names in the batch.
    pub fn methods(&self) -> &[String] {
        &self.methods
    }

    /// Returns true if the batch contains a method with the given name.
    pub fn contains_method(&self, method: &str) -> bool {
        self.methods.iter().any(|m| m == method)
    }
}

// ---------------------------------------------------------------------------
// RequestState transition validation
// ---------------------------------------------------------------------------

impl RequestState {
    /// Returns the set of state discriminants reachable from the current state.
    pub fn valid_transitions(&self) -> Vec<RequestState> {
        match self {
            RequestState::Pending => vec![
                RequestState::InProgress,
                RequestState::Cancelled,
            ],
            RequestState::InProgress => vec![
                RequestState::Completed,
                RequestState::Cancelled,
                RequestState::Failed(String::new()),
            ],
            // Terminal states have no valid transitions.
            RequestState::Completed
            | RequestState::Cancelled
            | RequestState::Failed(_) => vec![],
        }
    }

    /// Returns `true` if transitioning from `self` to `target` is valid.
    pub fn can_transition_to(&self, target: &RequestState) -> bool {
        self.valid_transitions()
            .iter()
            .any(|s| std::mem::discriminant(s) == std::mem::discriminant(target))
    }
}

impl RequestService {
    /// Attempt a validated state transition. Returns an error when the
    /// transition is not permitted by the state machine.
    pub fn try_transition(
        &mut self,
        id: RequestId,
        target: RequestState,
    ) -> Result<(), RequestError> {
        let req = self
            .requests
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(RequestError::RequestNotFound(id))?;

        if !req.state.can_transition_to(&target) {
            return Err(RequestError::InvalidTransition {
                id,
                from: req.state.to_string(),
                to: target.to_string(),
            });
        }
        req.state = target;
        Ok(())
    }

    /// Drain and return all requests in a terminal state, leaving only
    /// active (Pending / InProgress) requests in the service.
    pub fn drain_terminal(&mut self) -> Vec<Request> {
        let (terminal, active): (Vec<_>, Vec<_>) = self
            .requests
            .drain(..)
            .partition(|r| r.state.is_terminal());
        self.requests = active;
        terminal
    }

    /// Return an iterator over all tracked requests.
    pub fn iter(&self) -> impl Iterator<Item = &Request> {
        self.requests.iter()
    }
}

// ---------------------------------------------------------------------------
// RequestBuilder: headers and url support
// ---------------------------------------------------------------------------

impl RequestBuilder {
    /// Attach a URL to the request.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Add a header key-value pair to the request.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }
}

impl Request {
    /// Returns `true` when the request is in a terminal state.
    pub fn is_done(&self) -> bool {
        self.state.is_terminal()
    }

    /// Returns the elapsed time since the request was created, given the
    /// current timestamp.
    pub fn age(&self, now: u64) -> u64 {
        now.saturating_sub(self.created_at)
    }
}

// ---------------------------------------------------------------------------
// RequestId helpers
// ---------------------------------------------------------------------------

impl RequestId {
    /// The inner numeric identifier.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// PriorityRequestQueue: cancel, fail, and drain helpers
// ---------------------------------------------------------------------------

impl PriorityRequestQueue {
    /// Cancel a request by ID. Returns `true` if the request was found and
    /// was in a non-terminal state.
    pub fn cancel(&mut self, id: RequestId) -> bool {
        if let Some(req) = self.requests.iter_mut().find(|r| r.id == id) {
            if !req.state.is_terminal() {
                req.state = RequestState::Cancelled;
                return true;
            }
        }
        false
    }

    /// Drain all completed/cancelled/failed requests, returning them.
    pub fn drain_completed(&mut self) -> Vec<PrioritizedRequest> {
        let (terminal, active): (Vec<_>, Vec<_>) = self
            .requests
            .drain(..)
            .partition(|r| r.state.is_terminal());
        self.requests = active;
        terminal
    }

    /// Peek at the highest-priority pending request without changing state.
    pub fn peek(&self) -> Option<&PrioritizedRequest> {
        self.requests
            .iter()
            .filter(|r| r.state == RequestState::Pending)
            .max_by_key(|r| r.priority)
    }
}


// ---------------------------------------------------------------------------
// RetryBackoffStrategy — retry with jitter
// ---------------------------------------------------------------------------

/// Retry configuration with exponential backoff.
pub struct RetryBackoffStrategy {
    max_retries: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
}

impl RetryBackoffStrategy {
    pub fn new(max_retries: u32) -> Self {
        Self { max_retries, base_delay_ms: 100, max_delay_ms: 10_000 }
    }

    pub fn with_base_delay(mut self, ms: u64) -> Self { self.base_delay_ms = ms; self }
    pub fn with_max_delay(mut self, ms: u64) -> Self { self.max_delay_ms = ms; self }

    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let delay = self.base_delay_ms.saturating_mul(1u64 << attempt.min(20));
        delay.min(self.max_delay_ms)
    }

    pub fn should_retry(&self, attempt: u32) -> bool { attempt < self.max_retries }
    pub fn max_retries(&self) -> u32 { self.max_retries }
}

impl fmt::Display for RetryBackoffStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RetryBackoffStrategy(max={}, base={}ms)", self.max_retries, self.base_delay_ms)
    }
}

// ---------------------------------------------------------------------------
// RequestCache — cache with TTL
// ---------------------------------------------------------------------------

/// A cache entry with TTL.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub value: String,
    pub created_at_ms: u64,
    pub ttl_ms: u64,
}

impl CacheEntry {
    pub fn is_expired(&self, now_ms: u64) -> bool { now_ms > self.created_at_ms + self.ttl_ms }
}

/// Request cache with TTL support.
pub struct RequestCache {
    entries: HashMap<String, CacheEntry>,
    default_ttl_ms: u64,
}

impl RequestCache {
    pub fn new(default_ttl_ms: u64) -> Self {
        Self { entries: HashMap::new(), default_ttl_ms }
    }

    pub fn get(&self, key: &str, now_ms: u64) -> Option<&str> {
        self.entries.get(key).and_then(|e| if e.is_expired(now_ms) { None } else { Some(e.value.as_str()) })
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>, now_ms: u64) {
        let ttl = self.default_ttl_ms;
        self.entries.insert(key.into(), CacheEntry { value: value.into(), created_at_ms: now_ms, ttl_ms: ttl });
    }

    pub fn insert_with_ttl(&mut self, key: impl Into<String>, value: impl Into<String>, now_ms: u64, ttl_ms: u64) {
        self.entries.insert(key.into(), CacheEntry { value: value.into(), created_at_ms: now_ms, ttl_ms });
    }

    pub fn evict_expired(&mut self, now_ms: u64) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, e| !e.is_expired(now_ms));
        before - self.entries.len()
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn clear(&mut self) { self.entries.clear(); }
    pub fn contains_key(&self, key: &str) -> bool { self.entries.contains_key(key) }
}

impl fmt::Display for RequestCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RequestCache({} entries, ttl={}ms)", self.entries.len(), self.default_ttl_ms)
    }
}

// ---------------------------------------------------------------------------
// RequestBatch — parallel requests
// ---------------------------------------------------------------------------

/// A batch of requests to be executed together.
pub struct RequestBatchExecutor {
    requests: Vec<String>,
    results: Vec<Option<RequestState>>,
}

impl RequestBatchExecutor {
    pub fn new() -> Self { Self { requests: Vec::new(), results: Vec::new() } }

    pub fn add(&mut self, method: impl Into<String>) {
        self.requests.push(method.into());
        self.results.push(None);
    }

    pub fn len(&self) -> usize { self.requests.len() }
    pub fn is_empty(&self) -> bool { self.requests.is_empty() }

    pub fn set_result(&mut self, index: usize, state: RequestState) {
        if index < self.results.len() { self.results[index] = Some(state); }
    }

    pub fn is_complete(&self) -> bool { self.results.iter().all(|r| r.is_some()) }

    pub fn completed_count(&self) -> usize { self.results.iter().filter(|r| r.is_some()).count() }

    pub fn methods(&self) -> &[String] { &self.requests }
}

impl Default for RequestBatchExecutor {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for RequestBatchExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RequestBatchExecutorExecutor({}/{})", self.completed_count(), self.len())
    }
}

// ---------------------------------------------------------------------------
// RequestLatencyTracker — timing metrics
// ---------------------------------------------------------------------------

/// Tracks timing metrics for requests.
pub struct RequestLatencyTracker {
    timings: Vec<u64>,
}

impl RequestLatencyTracker {
    pub fn new() -> Self { Self { timings: Vec::new() } }

    pub fn record(&mut self, duration_ms: u64) { self.timings.push(duration_ms); }

    pub fn average_ms(&self) -> Option<u64> {
        if self.timings.is_empty() { None } else { Some(self.timings.iter().sum::<u64>() / self.timings.len() as u64) }
    }

    pub fn min_ms(&self) -> Option<u64> { self.timings.iter().copied().min() }
    pub fn max_ms(&self) -> Option<u64> { self.timings.iter().copied().max() }

    pub fn percentile(&self, p: f64) -> Option<u64> {
        if self.timings.is_empty() { return None; }
        let mut sorted = self.timings.clone();
        sorted.sort();
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    pub fn count(&self) -> usize { self.timings.len() }
    pub fn reset(&mut self) { self.timings.clear(); }
}

impl Default for RequestLatencyTracker {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for RequestLatencyTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RequestLatencyTracker({} samples)", self.timings.len())
    }
}

// ---------------------------------------------------------------------------
// RequestCircuitBreaker
// ---------------------------------------------------------------------------

/// State of a circuit breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,
    Open { until: u64 },
    HalfOpen,
}

impl std::fmt::Display for CircuitBreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerState::Closed => write!(f, "closed"),
            CircuitBreakerState::Open { until } => write!(f, "open (until {until})"),
            CircuitBreakerState::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Circuit breaker pattern for HTTP requests.
/// Tracks failures and opens the circuit when failures exceed a threshold.
pub struct RequestCircuitBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    success_count: u32,
    failure_threshold: u32,
    success_threshold: u32,
    reset_timeout: u64,
    total_requests: u64,
    total_failures: u64,
}

impl RequestCircuitBreaker {
    pub fn new(failure_threshold: u32, success_threshold: u32, reset_timeout: u64) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold,
            success_threshold,
            reset_timeout,
            total_requests: 0,
            total_failures: 0,
        }
    }

    pub fn state(&self) -> &CircuitBreakerState {
        &self.state
    }

    pub fn is_closed(&self) -> bool {
        self.state == CircuitBreakerState::Closed
    }

    /// Whether a request should be allowed through.
    pub fn allow_request(&mut self, now: u64) -> bool {
        self.total_requests += 1;
        match &self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open { until } => {
                if now >= *until {
                    self.state = CircuitBreakerState::HalfOpen;
                    self.success_count = 0;
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    /// Record a successful request.
    pub fn record_success(&mut self) {
        match &self.state {
            CircuitBreakerState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    self.state = CircuitBreakerState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            }
            CircuitBreakerState::Closed => {
                self.failure_count = 0;
            }
            _ => {}
        }
    }

    /// Record a failed request.
    pub fn record_failure(&mut self, now: u64) {
        self.total_failures += 1;
        match &self.state {
            CircuitBreakerState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open {
                        until: now + self.reset_timeout,
                    };
                }
            }
            CircuitBreakerState::HalfOpen => {
                self.state = CircuitBreakerState::Open {
                    until: now + self.reset_timeout,
                };
                self.success_count = 0;
            }
            _ => {}
        }
    }

    /// Manually reset to closed.
    pub fn reset(&mut self) {
        self.state = CircuitBreakerState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    pub fn total_requests(&self) -> u64 {
        self.total_requests
    }

    pub fn total_failures(&self) -> u64 {
        self.total_failures
    }

    /// Failure rate as a fraction (0.0 - 1.0).
    pub fn failure_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_failures as f64 / self.total_requests as f64
        }
    }
}

impl std::fmt::Display for RequestCircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CircuitBreaker[{}] failures={}/{} total={}", self.state, self.failure_count, self.failure_threshold, self.total_requests)
    }
}

// ---------------------------------------------------------------------------
// RequestDeduplicatorService
// ---------------------------------------------------------------------------

/// Tracks in-flight requests by key to avoid duplicate concurrent requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InFlightEntry {
    pub key: String,
    pub started_at: u64,
    pub request_count: u32,
}

impl InFlightEntry {
    pub fn new(key: impl Into<String>, started_at: u64) -> Self {
        Self { key: key.into(), started_at, request_count: 1 }
    }

    pub fn elapsed(&self, now: u64) -> u64 {
        now.saturating_sub(self.started_at)
    }
}

impl std::fmt::Display for InFlightEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InFlight('{}', count={}, started={})", self.key, self.request_count, self.started_at)
    }
}

/// Deduplicates in-flight requests by key. When a request with a matching key
/// is already in-flight, the duplicate is coalesced.
pub struct RequestDeduplicatorService {
    in_flight: std::collections::HashMap<String, InFlightEntry>,
    dedup_count: u64,
    total_requests: u64,
}

impl RequestDeduplicatorService {
    pub fn new() -> Self {
        Self {
            in_flight: std::collections::HashMap::new(),
            dedup_count: 0,
            total_requests: 0,
        }
    }

    /// Try to start a request. Returns true if this is a new request,
    /// false if it was deduplicated (already in-flight).
    pub fn try_start(&mut self, key: impl Into<String>, now: u64) -> bool {
        let k = key.into();
        self.total_requests += 1;
        if let Some(entry) = self.in_flight.get_mut(&k) {
            entry.request_count += 1;
            self.dedup_count += 1;
            false
        } else {
            self.in_flight.insert(k.clone(), InFlightEntry::new(k, now));
            true
        }
    }

    /// Complete a request, removing it from in-flight tracking.
    pub fn complete(&mut self, key: &str) -> Option<InFlightEntry> {
        self.in_flight.remove(key)
    }

    pub fn is_in_flight(&self, key: &str) -> bool {
        self.in_flight.contains_key(key)
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn dedup_count(&self) -> u64 {
        self.dedup_count
    }

    pub fn total_requests(&self) -> u64 {
        self.total_requests
    }

    /// Deduplication ratio (0.0 - 1.0).
    pub fn dedup_ratio(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.dedup_count as f64 / self.total_requests as f64
        }
    }

    /// Evict in-flight entries that have been running too long.
    pub fn evict_stale(&mut self, now: u64, max_age: u64) -> usize {
        let before = self.in_flight.len();
        self.in_flight.retain(|_, e| e.elapsed(now) <= max_age);
        before - self.in_flight.len()
    }

    /// All currently in-flight keys.
    pub fn in_flight_keys(&self) -> Vec<String> {
        self.in_flight.keys().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.in_flight.clear();
    }
}

impl std::fmt::Display for RequestDeduplicatorService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RequestDeduplicatorService(in_flight={}, deduped={}, total={})",
            self.in_flight.len(), self.dedup_count, self.total_requests)
    }
}



// ─── Req LRU Cache ───────────────────────────────────────

/// A simple LRU cache for HTTP responses.
#[derive(Debug)]
pub struct ReqLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> ReqLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for ReqLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReqLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── Req Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for request history.
#[derive(Debug, Clone)]
pub struct ReqRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> ReqRingBuffer<T> {
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

impl<T: Clone + fmt::Display> fmt::Display for ReqRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReqRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}



// ---------------------------------------------------------------------------
// request – Extended request rate limiter helpers
// ---------------------------------------------------------------------------

/// Priority levels for request rate limiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZRequestPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZRequestPriority {
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
    pub fn all_asc() -> [ZRequestPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZRequestPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks request rate limiter data.
#[derive(Debug, Clone)]
pub struct ZRequestRequestRateLimiter {
    pub window_hits: Vec<u64>,
    pub max_per_window: u32,
    pub window_ms: u64,
}

impl ZRequestRequestRateLimiter {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            window_hits: Vec::new(),
            max_per_window: 0,
            window_ms: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.window_hits.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.window_hits.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.window_hits.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZRequestRequestRateLimiter[max_per_window={:?}, window_ms={:?}]", self.max_per_window, self.window_ms)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for request rate limiter.
pub fn z_request_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_request_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_request_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_request_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_request_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_request_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_request_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 46
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer46 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer46 {
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
pub fn xb_fnv1a_46(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_46<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_46<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_46(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_46(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 151
// ---------------------------------------------------------------------------

/// Generic object pool `Xc151Pool<T>`.
pub struct Xc151Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc151Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc151PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc151Pool<T> {
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
    pub fn stats(&self) -> Xc151PoolStats {
        Xc151PoolStats {
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

impl<T> Default for Xc151Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc151Scheduler`.
pub struct Xc151Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc151Scheduler {
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

impl Default for Xc151Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_151 hash for the given byte slice.
pub fn xc_151_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_151 convention.
pub fn xc_151_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe59 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe59Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe59PipelineError {
    pub stage: Xe59Stage,
    pub message: String,
}

impl std::fmt::Display for Xe59PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe59Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe59Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError>>>,
    stage_names: Vec<Xe59Stage>,
}

impl Xe59Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe59Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe59Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe59Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe59Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe59Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe59CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe59CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe59Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe59CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe59CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe59Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe59CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_59_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe59CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_59_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe59CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_59_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> {
    Ok(data)
}

pub fn xe_59_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_59_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_59_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_59_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe59PipelineError> {
    Err(Xe59PipelineError {
        stage: Xe59Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_57: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg57Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg57Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg57Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_57: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg57Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg57Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg57Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg57Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 150).
pub struct Xh150SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh150SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 192 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 150).
pub struct Xh150BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh150BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 150).
pub struct Xi150Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi150Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi150Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi150Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 150).
pub struct Xi150IntervalTree {
    xi_intervals: Vec<Xi150Interval>,
}

impl Xi150IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi150Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi150Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi150Interval) -> Vec<&Xi150Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi150Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi150Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi150Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi150Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi150Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi150Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
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
            url: None,
            headers: Vec::new(),
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
        let mut batch = RequestBatchExecutor::new();
        assert!(batch.is_empty());
        batch.add("GET /a");
        batch.add("POST /b");
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_complete());
        batch.set_result(0, RequestState::Completed);
        batch.set_result(1, RequestState::Completed);
        assert!(batch.is_complete());
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

    #[test]
    fn find_by_method_prefix_filters() {
        let mut svc = RequestService::new();
        svc.create_request("textDocument/completion");
        svc.create_request("textDocument/hover");
        svc.create_request("workspace/symbol");
        let td = svc.find_by_method_prefix("textDocument/");
        assert_eq!(td.len(), 2);
        let ws = svc.find_by_method_prefix("workspace/");
        assert_eq!(ws.len(), 1);
    }

    #[test]
    fn ids_in_state_returns_correct_ids() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        svc.start(id1);
        let pending_ids = svc.ids_in_state(&RequestState::Pending);
        assert_eq!(pending_ids, vec![id2]);
    }

    #[test]
    fn fail_all_pending_fails_only_pending() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        svc.start(id1);
        svc.fail_all_pending("timeout");
        assert!(matches!(svc.get_state(id2), Some(RequestState::Failed(_))));
        assert!(matches!(svc.get_state(id1), Some(RequestState::InProgress)));
    }

    #[test]
    fn unique_method_count_deduplicates() {
        let mut svc = RequestService::new();
        svc.create_request("a");
        svc.create_request("a");
        svc.create_request("b");
        assert_eq!(svc.unique_method_count(), 2);
    }

    #[test]
    fn recent_requests_returns_sorted() {
        let mut svc = RequestService::new();
        RequestBuilder::new("old").created_at(10).build(&mut svc);
        RequestBuilder::new("new").created_at(50).build(&mut svc);
        RequestBuilder::new("mid").created_at(30).build(&mut svc);
        let recent = svc.recent_requests(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].method, "new");
        assert_eq!(recent[1].method, "mid");
    }

    #[test]
    fn priority_queue_count_by_priority() {
        let mut q = PriorityRequestQueue::new();
        q.enqueue("a", RequestPriority::High);
        q.enqueue("b", RequestPriority::Low);
        q.enqueue("c", RequestPriority::High);
        assert_eq!(q.count_by_priority(RequestPriority::High), 2);
        assert_eq!(q.count_by_priority(RequestPriority::Low), 1);
        assert!(q.has_priority(RequestPriority::High));
    }

    #[test]
    fn batch_contains_method_check() {
        let mut batch = RequestBatchExecutor::new();
        batch.add("textDocument/completion");
        batch.add("workspace/symbol");
        assert!(batch.methods().contains(&"textDocument/completion".to_string()));
        assert!(!batch.methods().contains(&"textDocument/hover".to_string()));
        assert_eq!(batch.methods().len(), 2);
    }

    // -----------------------------------------------------------------------
    // Tests for state transitions, drain, builder extensions, and queue ops
    // -----------------------------------------------------------------------

    #[test]
    fn valid_transitions_from_pending() {
        let pending = RequestState::Pending;
        assert!(pending.can_transition_to(&RequestState::InProgress));
        assert!(pending.can_transition_to(&RequestState::Cancelled));
        assert!(!pending.can_transition_to(&RequestState::Completed));
        assert!(!pending.can_transition_to(&RequestState::Failed("x".into())));
    }

    #[test]
    fn valid_transitions_from_in_progress() {
        let ip = RequestState::InProgress;
        assert!(ip.can_transition_to(&RequestState::Completed));
        assert!(ip.can_transition_to(&RequestState::Cancelled));
        assert!(ip.can_transition_to(&RequestState::Failed("err".into())));
        assert!(!ip.can_transition_to(&RequestState::Pending));
    }

    #[test]
    fn terminal_states_have_no_transitions() {
        for state in &[
            RequestState::Completed,
            RequestState::Cancelled,
            RequestState::Failed("err".into()),
        ] {
            assert!(state.valid_transitions().is_empty());
            assert!(!state.can_transition_to(&RequestState::Pending));
        }
    }

    #[test]
    fn try_transition_success() {
        let mut svc = RequestService::new();
        let id = svc.create_request("GET /api");
        assert!(svc.try_transition(id, RequestState::InProgress).is_ok());
        assert_eq!(svc.get_state(id), Some(&RequestState::InProgress));
        assert!(svc.try_transition(id, RequestState::Completed).is_ok());
        assert_eq!(svc.get_state(id), Some(&RequestState::Completed));
    }

    #[test]
    fn try_transition_invalid() {
        let mut svc = RequestService::new();
        let id = svc.create_request("GET /api");
        // Pending -> Completed is not allowed
        let err = svc.try_transition(id, RequestState::Completed).unwrap_err();
        assert!(matches!(err, RequestError::InvalidTransition { .. }));
    }

    #[test]
    fn try_transition_not_found() {
        let mut svc = RequestService::new();
        let err = svc.try_transition(RequestId(999), RequestState::InProgress).unwrap_err();
        assert!(matches!(err, RequestError::RequestNotFound(_)));
    }

    #[test]
    fn drain_terminal_removes_done_requests() {
        let mut svc = RequestService::new();
        let id1 = svc.create_request("a");
        let id2 = svc.create_request("b");
        let _id3 = svc.create_request("c");
        svc.start(id1);
        svc.complete(id1);
        svc.cancel(id2);
        let drained = svc.drain_terminal();
        assert_eq!(drained.len(), 2);
        assert_eq!(svc.total_count(), 1);
    }

    #[test]
    fn request_builder_with_url_and_headers() {
        let mut svc = RequestService::new();
        let id = RequestBuilder::new("POST")
            .url("https://example.com/api")
            .header("Content-Type", "application/json")
            .header("Authorization", "Bearer tok")
            .created_at(42)
            .build(&mut svc);
        let req = svc.get_request(id).unwrap();
        assert_eq!(req.url.as_deref(), Some("https://example.com/api"));
        assert_eq!(req.headers.len(), 2);
        assert_eq!(req.headers[0], ("Content-Type".into(), "application/json".into()));
    }

    #[test]
    fn request_is_done_and_age() {
        let mut svc = RequestService::new();
        let id = RequestBuilder::new("GET").created_at(100).build(&mut svc);
        let req = svc.get_request(id).unwrap();
        assert!(!req.is_done());
        assert_eq!(req.age(150), 50);

        svc.start(id);
        svc.complete(id);
        let req = svc.get_request(id).unwrap();
        assert!(req.is_done());
    }

    #[test]
    fn request_id_as_u64() {
        let id = RequestId(42);
        assert_eq!(id.as_u64(), 42);
    }

    #[test]
    fn service_iter_yields_all_requests() {
        let mut svc = RequestService::new();
        svc.create_request("a");
        svc.create_request("b");
        svc.create_request("c");
        let methods: Vec<&str> = svc.iter().map(|r| r.method.as_str()).collect();
        assert_eq!(methods, vec!["a", "b", "c"]);
    }

    #[test]
    fn priority_queue_cancel() {
        let mut q = PriorityRequestQueue::new();
        let id = q.enqueue("x", RequestPriority::Normal);
        assert!(q.cancel(id));
        assert_eq!(q.get(id).unwrap().state, RequestState::Cancelled);
        // cancelling again should return false (already terminal)
        assert!(!q.cancel(id));
    }

    #[test]
    fn priority_queue_drain_completed() {
        let mut q = PriorityRequestQueue::new();
        let id1 = q.enqueue("a", RequestPriority::Low);
        let _id2 = q.enqueue("b", RequestPriority::High);
        q.cancel(id1);
        let drained = q.drain_completed();
        assert_eq!(drained.len(), 1);
        assert_eq!(q.total_count(), 1);
    }

    #[test]
    fn priority_queue_peek() {
        let mut q = PriorityRequestQueue::new();
        q.enqueue("low", RequestPriority::Low);
        q.enqueue("high", RequestPriority::High);
        let peeked = q.peek().unwrap();
        assert_eq!(peeked.method, "high");
        // peek should not change state
        assert_eq!(q.pending_count(), 2);
    }


    #[test]
    fn retry_middleware_delay() {
        let retry = RetryBackoffStrategy::new(3);
        assert_eq!(retry.delay_for_attempt(0), 100);
        assert_eq!(retry.delay_for_attempt(1), 200);
    }

    #[test]
    fn retry_middleware_should_retry() {
        let retry = RetryBackoffStrategy::new(2);
        assert!(retry.should_retry(0));
        assert!(!retry.should_retry(2));
    }

    #[test]
    fn retry_middleware_max_delay() {
        let retry = RetryBackoffStrategy::new(5).with_max_delay(500);
        assert!(retry.delay_for_attempt(10) <= 500);
    }

    #[test]
    fn cache_basic() {
        let mut cache = RequestCache::new(1000);
        cache.insert("k", "v", 100);
        assert_eq!(cache.get("k", 200), Some("v"));
        assert_eq!(cache.get("k", 1200), None);
    }

    #[test]
    fn cache_evict() {
        let mut cache = RequestCache::new(100);
        cache.insert("a", "1", 0);
        cache.insert("b", "2", 50);
        assert_eq!(cache.evict_expired(150), 1);
    }

    #[test]
    fn cache_custom_ttl() {
        let mut cache = RequestCache::new(1000);
        cache.insert_with_ttl("k", "v", 0, 50);
        assert_eq!(cache.get("k", 40), Some("v"));
        assert_eq!(cache.get("k", 60), None);
    }

    #[test]
    fn batch_basic() {
        let mut batch = RequestBatchExecutor::new();
        batch.add("GET /a");
        batch.add("GET /b");
        assert!(!batch.is_complete());
        batch.set_result(0, RequestState::Completed);
        batch.set_result(1, RequestState::Completed);
        assert!(batch.is_complete());
    }

    #[test]
    fn batch_completed_count() {
        let mut batch = RequestBatchExecutor::new();
        batch.add("a");
        batch.add("b");
        batch.set_result(0, RequestState::Completed);
        assert_eq!(batch.completed_count(), 1);
    }

    #[test]
    fn timing_metrics_basic() {
        let mut m = RequestLatencyTracker::new();
        m.record(100);
        m.record(200);
        m.record(300);
        assert_eq!(m.average_ms(), Some(200));
        assert_eq!(m.min_ms(), Some(100));
        assert_eq!(m.max_ms(), Some(300));
    }

    #[test]
    fn timing_metrics_percentile() {
        let mut m = RequestLatencyTracker::new();
        for i in 1..=100 { m.record(i); }
        assert_eq!(m.percentile(50.0), Some(51));
    }

    #[test]
    fn timing_metrics_empty() {
        let m = RequestLatencyTracker::new();
        assert_eq!(m.average_ms(), None);
    }

    #[test]
    fn retry_display() {
        let r = RetryBackoffStrategy::new(3);
        assert!(format!("{r}").contains("max=3"));
    }

    #[test]
    fn cache_display() {
        let c = RequestCache::new(1000);
        assert!(format!("{c}").contains("0 entries"));
    }


    #[test]
    fn circuit_breaker_starts_closed() {
        let cb = RequestCircuitBreaker::new(3, 2, 100);
        assert!(cb.is_closed());
    }

    #[test]
    fn circuit_breaker_opens_on_failures() {
        let mut cb = RequestCircuitBreaker::new(3, 2, 100);
        cb.allow_request(0);
        cb.record_failure(10);
        cb.allow_request(10);
        cb.record_failure(20);
        cb.allow_request(20);
        cb.record_failure(30);
        assert!(!cb.is_closed());
        assert_eq!(cb.failure_count(), 3);
    }

    #[test]
    fn circuit_breaker_rejects_when_open() {
        let mut cb = RequestCircuitBreaker::new(1, 1, 100);
        cb.allow_request(0);
        cb.record_failure(10);
        assert!(!cb.allow_request(20)); // still open
    }

    #[test]
    fn circuit_breaker_half_open_after_timeout() {
        let mut cb = RequestCircuitBreaker::new(1, 1, 100);
        cb.allow_request(0);
        cb.record_failure(10);
        assert!(cb.allow_request(200)); // timeout passed
        assert_eq!(*cb.state(), CircuitBreakerState::HalfOpen);
    }

    #[test]
    fn circuit_breaker_closes_after_success_in_half_open() {
        let mut cb = RequestCircuitBreaker::new(1, 1, 100);
        cb.allow_request(0);
        cb.record_failure(10);
        cb.allow_request(200);
        cb.record_success();
        assert!(cb.is_closed());
    }

    #[test]
    fn circuit_breaker_reopens_on_failure_in_half_open() {
        let mut cb = RequestCircuitBreaker::new(1, 2, 100);
        cb.allow_request(0);
        cb.record_failure(10);
        cb.allow_request(200);
        cb.record_failure(200);
        assert!(matches!(cb.state(), CircuitBreakerState::Open { .. }));
    }

    #[test]
    fn circuit_breaker_reset() {
        let mut cb = RequestCircuitBreaker::new(1, 1, 100);
        cb.allow_request(0);
        cb.record_failure(10);
        cb.reset();
        assert!(cb.is_closed());
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn circuit_breaker_failure_rate() {
        let mut cb = RequestCircuitBreaker::new(5, 1, 100);
        cb.allow_request(0); cb.record_failure(0);
        cb.allow_request(1); cb.record_success();
        let rate = cb.failure_rate();
        assert!((rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn circuit_breaker_display() {
        let cb = RequestCircuitBreaker::new(3, 2, 100);
        let s = format!("{cb}");
        assert!(s.contains("closed"));
        assert!(s.contains("0/3"));
    }

    #[test]
    fn circuit_breaker_state_display() {
        assert_eq!(format!("{}", CircuitBreakerState::Closed), "closed");
        assert!(format!("{}", CircuitBreakerState::Open { until: 42 }).contains("42"));
        assert_eq!(format!("{}", CircuitBreakerState::HalfOpen), "half-open");
    }

    #[test]
    fn dedup_service_new_request() {
        let mut svc = RequestDeduplicatorService::new();
        assert!(svc.try_start("key1", 100));
        assert!(svc.is_in_flight("key1"));
        assert_eq!(svc.in_flight_count(), 1);
    }

    #[test]
    fn dedup_service_duplicate_request() {
        let mut svc = RequestDeduplicatorService::new();
        assert!(svc.try_start("key1", 100));
        assert!(!svc.try_start("key1", 200)); // deduped
        assert_eq!(svc.dedup_count(), 1);
    }

    #[test]
    fn dedup_service_complete() {
        let mut svc = RequestDeduplicatorService::new();
        svc.try_start("key1", 100);
        let entry = svc.complete("key1").unwrap();
        assert_eq!(entry.key, "key1");
        assert!(!svc.is_in_flight("key1"));
    }

    #[test]
    fn dedup_service_dedup_ratio() {
        let mut svc = RequestDeduplicatorService::new();
        svc.try_start("a", 1); // new
        svc.try_start("a", 2); // deduped
        svc.try_start("a", 3); // deduped
        let ratio = svc.dedup_ratio();
        assert!((ratio - 2.0/3.0).abs() < 0.01);
    }

    #[test]
    fn dedup_service_evict_stale() {
        let mut svc = RequestDeduplicatorService::new();
        svc.try_start("old", 10);
        svc.try_start("new", 500);
        let evicted = svc.evict_stale(500, 100);
        assert_eq!(evicted, 1);
        assert!(!svc.is_in_flight("old"));
    }

    #[test]
    fn dedup_service_in_flight_keys() {
        let mut svc = RequestDeduplicatorService::new();
        svc.try_start("a", 1);
        svc.try_start("b", 2);
        let keys = svc.in_flight_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn dedup_service_clear_and_display() {
        let mut svc = RequestDeduplicatorService::new();
        svc.try_start("a", 1);
        assert!(format!("{svc}").contains("in_flight=1"));
        svc.clear();
        assert_eq!(svc.in_flight_count(), 0);
    }

    #[test]
    fn in_flight_entry_display_and_elapsed() {
        let e = InFlightEntry::new("k", 100);
        assert_eq!(e.elapsed(200), 100);
        assert!(format!("{e}").contains("k"));
    }

    #[test]
    fn dedup_service_complete_unknown_key() {
        let mut svc = RequestDeduplicatorService::new();
        assert!(svc.complete("nope").is_none());
    }


    #[test]
    fn req_lru_insert_get() {
        let mut c = ReqLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn req_lru_eviction() {
        let mut c = ReqLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn req_lru_hit_ratio() {
        let mut c = ReqLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn req_lru_clear() {
        let mut c = ReqLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn req_lru_remove() {
        let mut c = ReqLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn req_lru_peek() {
        let mut c = ReqLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn req_ringbuf_push_get() {
        let mut rb = ReqRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn req_ringbuf_overflow() {
        let mut rb = ReqRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn req_ringbuf_clear() {
        let mut rb = ReqRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn req_ringbuf_newest_oldest() {
        let mut rb = ReqRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn req_ringbuf_to_vec() {
        let mut rb = ReqRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn req_ringbuf_is_full() {
        let mut rb = ReqRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }


    // -- request Z-extended tests -----------------------------------------------

    #[test]
    fn z_request_priority_weight() {
        assert_eq!(ZRequestPriority::Idle.weight(), 0);
        assert_eq!(ZRequestPriority::Normal.weight(), 2);
        assert_eq!(ZRequestPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_request_priority_label() {
        assert_eq!(ZRequestPriority::Low.label(), "low");
        assert_eq!(ZRequestPriority::High.label(), "high");
    }

    #[test]
    fn z_request_priority_is_elevated() {
        assert!(!ZRequestPriority::Normal.is_elevated());
        assert!(ZRequestPriority::High.is_elevated());
        assert!(ZRequestPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_request_priority_display() {
        assert_eq!(format!("{}", ZRequestPriority::Idle), "idle");
    }

    #[test]
    fn z_request_priority_all_asc() {
        let all = ZRequestPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZRequestPriority::Idle);
        assert_eq!(all[4], ZRequestPriority::Realtime);
    }

    #[test]
    fn z_request_struct_new() {
        let s = ZRequestRequestRateLimiter::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_request_struct_toggled_clone() {
        let s = ZRequestRequestRateLimiter::new();
        let t = s.toggled_clone();
        let _ = t.window_ms;
    }

    #[test]
    fn z_request_rolling_hash_deterministic() {
        let h1 = z_request_rolling_hash(b"test");
        let h2 = z_request_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_request_rolling_hash(b"a"), z_request_rolling_hash(b"b"));
    }

    #[test]
    fn z_request_pad_to_basic() {
        assert_eq!(z_request_pad_to("hi", 5), "hi   ");
        assert_eq!(z_request_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_request_is_identifier_basic() {
        assert!(z_request_is_identifier("foo_bar"));
        assert!(z_request_is_identifier("abc123"));
        assert!(!z_request_is_identifier(""));
        assert!(!z_request_is_identifier("has space"));
    }

    #[test]
    fn z_request_levenshtein_basic() {
        assert_eq!(z_request_levenshtein("", ""), 0);
        assert_eq!(z_request_levenshtein("abc", "abc"), 0);
        assert_eq!(z_request_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_request_unique_words_basic() {
        let w = z_request_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_request_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_request_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_request_common_prefix_basic() {
        assert_eq!(z_request_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_request_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_request_struct_clear() {
        let mut s = ZRequestRequestRateLimiter::new();
        s.window_hits.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_request_rolling_hash_empty() {
        let h = z_request_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_46_push_and_len() {
        let mut rb = super::XbRingBuffer46::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_46_overwrite() {
        let mut rb = super::XbRingBuffer46::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_46_get_out_of_bounds() {
        let rb = super::XbRingBuffer46::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_46_drain_all() {
        let mut rb = super::XbRingBuffer46::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_46_peek_front_back() {
        let mut rb = super::XbRingBuffer46::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_46_clear() {
        let mut rb = super::XbRingBuffer46::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_46_capacity() {
        let rb = super::XbRingBuffer46::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_46_basic() {
        let h = super::xb_fnv1a_46(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_46(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_46_different_inputs() {
        let h1 = super::xb_fnv1a_46(b"abc");
        let h2 = super::xb_fnv1a_46(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_46_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_46(&data);
        let dec = super::xb_rle_decode_46(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_46_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_46(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_46(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_46_values() {
        assert!((super::xb_clamp_46(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_46(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_46(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_46_values() {
        assert!((super::xb_lerp_46(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_46(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_46(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_46_wrap_around_twice() {
        let mut rb = super::XbRingBuffer46::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 151 ----

    #[test]
    fn xc_151_pool_new_empty() {
        let pool: super::Xc151Pool<i32> = super::Xc151Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_151_pool_release_acquire() {
        let mut pool = super::Xc151Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_151_pool_acquire_empty() {
        let mut pool: super::Xc151Pool<i32> = super::Xc151Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_151_pool_full() {
        let mut pool = super::Xc151Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_151_pool_drain() {
        let mut pool = super::Xc151Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_151_pool_stats() {
        let mut pool = super::Xc151Pool::new(8);
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
    fn xc_151_pool_clear() {
        let mut pool = super::Xc151Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_151_pool_shrink() {
        let mut pool = super::Xc151Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_151_pool_default() {
        let pool: super::Xc151Pool<String> = super::Xc151Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_151_pool_extend() {
        let mut pool = super::Xc151Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_151_pool_retain() {
        let mut pool = super::Xc151Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_151_scheduler_round_robin() {
        let mut sched = super::Xc151Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_151_scheduler_empty() {
        let mut sched = super::Xc151Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_151_scheduler_reset() {
        let mut sched = super::Xc151Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_151_scheduler_add_remove() {
        let mut sched = super::Xc151Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_151_scheduler_targets() {
        let sched = super::Xc151Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_151_hash_empty() {
        assert_eq!(super::xc_151_hash(b""), 5381);
    }

    #[test]
    fn xc_151_hash_data() {
        let h = super::xc_151_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_151_hash(b"hello"), h);
    }

    #[test]
    fn xc_151_reverse_str() {
        assert_eq!(super::xc_151_reverse("abc"), "cba");
        assert_eq!(super::xc_151_reverse(""), "");
    }


    #[test]
    fn xe_59_pipeline_empty() {
        let p = super::Xe59Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_59_pipeline_parse_stage() {
        let p = super::Xe59Pipeline::new()
            .add_parse(super::xe_59_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_59_pipeline_transform_double() {
        let p = super::Xe59Pipeline::new()
            .add_transform(super::xe_59_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_59_pipeline_validate_reverse() {
        let p = super::Xe59Pipeline::new()
            .add_validate(super::xe_59_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_59_pipeline_emit_filter() {
        let p = super::Xe59Pipeline::new()
            .add_emit(super::xe_59_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_59_pipeline_multi_stage() {
        let p = super::Xe59Pipeline::new()
            .add_parse(super::xe_59_pipeline_identity)
            .add_transform(super::xe_59_pipeline_double)
            .add_validate(super::xe_59_pipeline_reverse)
            .add_emit(super::xe_59_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_59_pipeline_error_propagation() {
        let p = super::Xe59Pipeline::new()
            .add_parse(super::xe_59_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe59Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_59_pipeline_compose() {
        let p1 = super::Xe59Pipeline::new()
            .add_parse(super::xe_59_pipeline_identity);
        let p2 = super::Xe59Pipeline::new()
            .add_transform(super::xe_59_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_59_pipeline_error_display() {
        let e = super::Xe59PipelineError {
            stage: super::Xe59Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_59_cache_put_get() {
        let mut c = super::Xe59Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_59_cache_miss() {
        let mut c: super::Xe59Cache<&str, i32> = super::Xe59Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_59_cache_ttl_expiry() {
        let mut c = super::Xe59Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_59_cache_evict() {
        let mut c = super::Xe59Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_59_cache_capacity() {
        let mut c = super::Xe59Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_59_cache_stats() {
        let mut c = super::Xe59Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_59_cache_clear() {
        let mut c = super::Xe59Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_57 graph tests ------------------------------------------------

    #[test]
    fn xg_57_graph_empty() {
        let g = super::Xg57Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_57_graph_add_node() {
        let mut g = super::Xg57Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_57_graph_add_edge() {
        let mut g = super::Xg57Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_57_graph_neighbors() {
        let mut g = super::Xg57Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_57_graph_has_path() {
        let mut g = super::Xg57Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_57_graph_self_path() {
        let g = super::Xg57Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_57_graph_topo_sort() {
        let mut g = super::Xg57Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_57_graph_cycle_detect_false() {
        let mut g = super::Xg57Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_57_graph_cycle_detect_true() {
        let mut g = super::Xg57Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_57 heap tests -------------------------------------------------

    #[test]
    fn xg_57_heap_empty() {
        let h: super::Xg57Heap<i32> = super::Xg57Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_57_heap_push_pop() {
        let mut h = super::Xg57Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_57_heap_peek() {
        let mut h = super::Xg57Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_57_heap_drain_sorted() {
        let mut h = super::Xg57Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_57_heap_merge() {
        let mut a = super::Xg57Heap::new();
        let mut b = super::Xg57Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_57_heap_default() {
        let h: super::Xg57Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_57_graph_default() {
        let g: super::Xg57Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh150_skip_insert_contains() {
        let mut sl = super::Xh150SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh150_skip_remove() {
        let mut sl = super::Xh150SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh150_skip_len() {
        let mut sl = super::Xh150SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh150_skip_range_query() {
        let mut sl = super::Xh150SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh150_skip_floor_ceiling() {
        let mut sl = super::Xh150SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh150_skip_rank() {
        let mut sl = super::Xh150SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh150_skip_empty() {
        let sl = super::Xh150SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh150_skip_duplicates() {
        let mut sl = super::Xh150SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh150_bitset_set_test() {
        let mut bs = super::Xh150BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh150_bitset_clear_count() {
        let mut bs = super::Xh150BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh150_bitset_and_or_xor() {
        let mut a = super::Xh150BitSet::xh_new(128);
        let mut b = super::Xh150BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh150_bitset_iter_ones() {
        let mut bs = super::Xh150BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh150_bitset_first_last() {
        let mut bs = super::Xh150BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh150_bitset_empty() {
        let bs = super::Xh150BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi150_deque_push_pop_back() {
        let mut dq = super::Xi150Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi150_deque_push_pop_front() {
        let mut dq = super::Xi150Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi150_deque_mixed_ops() {
        let mut dq = super::Xi150Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi150_deque_get_and_split() {
        let mut dq = super::Xi150Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi150_deque_rotate_left() {
        let mut dq = super::Xi150Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi150_deque_rotate_right() {
        let mut dq = super::Xi150Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi150_deque_grow() {
        let mut dq = super::Xi150Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi150_deque_empty() {
        let dq = super::Xi150Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi150_interval_tree_insert_query() {
        let mut tree = super::Xi150IntervalTree::xi_new();
        tree.xi_insert(super::Xi150Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi150Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi150Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi150_interval_tree_overlap() {
        let mut tree = super::Xi150IntervalTree::xi_new();
        tree.xi_insert(super::Xi150Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi150Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi150Interval::xi_new(12, 20));
        let q = super::Xi150Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi150_interval_tree_remove() {
        let mut tree = super::Xi150IntervalTree::xi_new();
        tree.xi_insert(super::Xi150Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi150Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi150_interval_tree_gaps() {
        let mut tree = super::Xi150IntervalTree::xi_new();
        tree.xi_insert(super::Xi150Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi150Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi150Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi150Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi150Interval::xi_new(8, 10));
    }

    #[test]
    fn xi150_interval_tree_merge() {
        let mut tree = super::Xi150IntervalTree::xi_new();
        tree.xi_insert(super::Xi150Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi150Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi150Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi150Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi150Interval::xi_new(10, 15));
    }

    #[test]
    fn xi150_interval_tree_all() {
        let mut tree = super::Xi150IntervalTree::xi_new();
        tree.xi_insert(super::Xi150Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi150Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi150_interval_tree_empty() {
        let tree = super::Xi150IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi150_interval_tree_contains_point() {
        let iv = super::Xi150Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}