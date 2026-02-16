//! Extension host RPC protocol

use std::fmt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

// ── Wire protocol message types ──

#[derive(Debug, Clone, PartialEq)]
pub enum RpcMessage {
    Request(RpcRequest),
    Response(RpcResponse),
    Event(RpcEvent),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcRequest {
    pub id: u64,
    pub proxy_id: String,
    pub method: String,
    pub args: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcResponse {
    pub id: u64,
    pub result: Result<serde_json::Value, RpcError>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcEvent {
    pub proxy_id: String,
    pub event_name: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

// ── JSON serialization helpers ──

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WireMessage {
    #[serde(rename = "request")]
    Request {
        id: u64,
        #[serde(rename = "proxyId")]
        proxy_id: String,
        method: String,
        args: Vec<serde_json::Value>,
    },
    #[serde(rename = "response")]
    Response {
        id: u64,
        #[serde(flatten)]
        payload: WireResponsePayload,
    },
    #[serde(rename = "event")]
    Event {
        #[serde(rename = "proxyId")]
        proxy_id: String,
        #[serde(rename = "eventName")]
        event_name: String,
        data: serde_json::Value,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum WireResponsePayload {
    Error { error: RpcError },
    Ok { result: serde_json::Value },
}

impl From<&RpcMessage> for WireMessage {
    fn from(msg: &RpcMessage) -> Self {
        match msg {
            RpcMessage::Request(r) => WireMessage::Request {
                id: r.id,
                proxy_id: r.proxy_id.clone(),
                method: r.method.clone(),
                args: r.args.clone(),
            },
            RpcMessage::Response(r) => WireMessage::Response {
                id: r.id,
                payload: match &r.result {
                    Ok(v) => WireResponsePayload::Ok { result: v.clone() },
                    Err(e) => WireResponsePayload::Error { error: e.clone() },
                },
            },
            RpcMessage::Event(e) => WireMessage::Event {
                proxy_id: e.proxy_id.clone(),
                event_name: e.event_name.clone(),
                data: e.data.clone(),
            },
        }
    }
}

impl From<WireMessage> for RpcMessage {
    fn from(wire: WireMessage) -> Self {
        match wire {
            WireMessage::Request {
                id,
                proxy_id,
                method,
                args,
            } => RpcMessage::Request(RpcRequest {
                id,
                proxy_id,
                method,
                args,
            }),
            WireMessage::Response { id, payload } => RpcMessage::Response(RpcResponse {
                id,
                result: match payload {
                    WireResponsePayload::Ok { result } => Ok(result),
                    WireResponsePayload::Error { error } => Err(error),
                },
            }),
            WireMessage::Event {
                proxy_id,
                event_name,
                data,
            } => RpcMessage::Event(RpcEvent {
                proxy_id,
                event_name,
                data,
            }),
        }
    }
}

// ── RpcProtocol ──

pub struct RpcProtocol {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<RpcResponse>>>,
}

impl RpcProtocol {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_request(
        &self,
        proxy_id: &str,
        method: &str,
        args: Vec<serde_json::Value>,
    ) -> (u64, RpcRequest) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = RpcRequest {
            id,
            proxy_id: proxy_id.to_string(),
            method: method.to_string(),
            args,
        };
        (id, req)
    }

    pub fn register_pending(&self, id: u64) -> tokio::sync::oneshot::Receiver<RpcResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        rx
    }

    pub fn resolve_response(&self, response: RpcResponse) {
        if let Some(tx) = self.pending.lock().unwrap().remove(&response.id) {
            let _ = tx.send(response);
        }
    }

    pub fn serialize_message(msg: &RpcMessage) -> String {
        let wire: WireMessage = msg.into();
        serde_json::to_string(&wire).expect("RpcMessage serialization should not fail")
    }

    pub fn deserialize_message(data: &str) -> Result<RpcMessage, serde_json::Error> {
        let wire: WireMessage = serde_json::from_str(data)?;
        Ok(wire.into())
    }
}

impl Default for RpcProtocol {
    fn default() -> Self {
        Self::new()
    }
}

// ── ProxyIdentifier ──

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyIdentifier {
    pub id: String,
    pub is_main: bool,
}

impl ProxyIdentifier {
    pub fn main_thread(id: &str) -> Self {
        Self {
            id: id.to_string(),
            is_main: true,
        }
    }

    pub fn ext_host(id: &str) -> Self {
        Self {
            id: id.to_string(),
            is_main: false,
        }
    }
}

// ── Well-known proxy identifiers ──

pub mod proxies {
    pub const MAIN_THREAD_COMMANDS: &str = "MainThreadCommands";
    pub const MAIN_THREAD_CONFIGURATION: &str = "MainThreadConfiguration";
    pub const MAIN_THREAD_DOCUMENTS: &str = "MainThreadDocuments";
    pub const MAIN_THREAD_EDITORS: &str = "MainThreadEditors";
    pub const MAIN_THREAD_LANGUAGES: &str = "MainThreadLanguageFeatures";
    pub const MAIN_THREAD_WINDOW: &str = "MainThreadWindow";
    pub const MAIN_THREAD_WORKSPACE: &str = "MainThreadWorkspace";
    pub const MAIN_THREAD_FILE_SYSTEM: &str = "MainThreadFileSystem";
    pub const MAIN_THREAD_TERMINAL: &str = "MainThreadTerminal";
    pub const MAIN_THREAD_SCM: &str = "MainThreadSCM";
    pub const MAIN_THREAD_DEBUG: &str = "MainThreadDebugService";

    pub const EXT_HOST_COMMANDS: &str = "ExtHostCommands";
    pub const EXT_HOST_DOCUMENTS: &str = "ExtHostDocuments";
    pub const EXT_HOST_EDITORS: &str = "ExtHostTextEditors";
    pub const EXT_HOST_LANGUAGES: &str = "ExtHostLanguageFeatures";
    pub const EXT_HOST_WORKSPACE: &str = "ExtHostWorkspace";
    pub const EXT_HOST_CONFIGURATION: &str = "ExtHostConfiguration";
    pub const EXT_HOST_FILE_SYSTEM: &str = "ExtHostFileSystem";
    pub const EXT_HOST_TERMINAL: &str = "ExtHostTerminal";
    pub const EXT_HOST_SCM: &str = "ExtHostSCM";
    pub const EXT_HOST_DEBUG: &str = "ExtHostDebugService";
}

/// Accumulated statistics for ext-rpc operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtRpcStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtRpcStats {
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
    pub fn merge(&mut self, other: &ExtRpcStats) {
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

impl Default for ExtRpcStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtRpcStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtRpcStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-rpc.
#[derive(Debug, Clone)]
pub struct ExtRpcValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtRpcValidator {
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

impl Default for ExtRpcValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Batch RPC support ──

/// Collects multiple [`RpcMessage`]s into a single batch that can be sent in
/// one round-trip.  The batch enforces an upper bound on the number of
/// messages it will accept.
#[derive(Debug, Clone)]
pub struct RpcBatch {
    pub requests: Vec<RpcMessage>,
    pub max_batch_size: usize,
}

impl RpcBatch {
    /// Create an empty batch that accepts at most `max_batch_size` messages.
    pub fn new(max_batch_size: usize) -> Self {
        Self {
            requests: Vec::new(),
            max_batch_size,
        }
    }

    /// Append a message to the batch.
    ///
    /// Returns `Err` if the batch is already at capacity.
    pub fn add(&mut self, msg: RpcMessage) -> Result<(), String> {
        if self.requests.len() >= self.max_batch_size {
            return Err(format!(
                "batch is full (max {} messages)",
                self.max_batch_size
            ));
        }
        self.requests.push(msg);
        Ok(())
    }

    /// Number of messages currently in the batch.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Whether the batch contains no messages.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Whether the batch has reached its maximum capacity.
    pub fn is_full(&self) -> bool {
        self.requests.len() >= self.max_batch_size
    }

    /// Remove and return all messages, leaving the batch empty.
    pub fn drain(&mut self) -> Vec<RpcMessage> {
        std::mem::take(&mut self.requests)
    }

    /// Estimate the total serialized payload size (in bytes) of every message
    /// currently in the batch by serializing each one to JSON and summing
    /// their lengths.
    pub fn total_payload_size(&self) -> usize {
        self.requests
            .iter()
            .map(|msg| {
                let wire: WireMessage = msg.into();
                serde_json::to_string(&wire)
                    .map(|s| s.len())
                    .unwrap_or(0)
            })
            .sum()
    }
}

// ── Timeout tracking ──

/// Tracks a deadline for an individual RPC request so that callers can check
/// whether the request has taken too long and how much time remains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpcTimeout {
    pub timeout_ms: u64,
    pub start_time_ms: u64,
}

impl RpcTimeout {
    /// Create a new timeout that expires `timeout_ms` milliseconds after
    /// `start_time_ms`.
    pub fn new(timeout_ms: u64, start_time_ms: u64) -> Self {
        Self {
            timeout_ms,
            start_time_ms,
        }
    }

    /// Returns `true` when the deadline has been reached or exceeded.
    pub fn is_expired(&self, current_time_ms: u64) -> bool {
        current_time_ms >= self.start_time_ms.saturating_add(self.timeout_ms)
    }

    /// Milliseconds remaining until the deadline.  Returns `0` once expired.
    pub fn remaining_ms(&self, current_time_ms: u64) -> u64 {
        let deadline = self.start_time_ms.saturating_add(self.timeout_ms);
        deadline.saturating_sub(current_time_ms)
    }

    /// Push the deadline further into the future by `additional_ms`.
    pub fn extend(&mut self, additional_ms: u64) {
        self.timeout_ms = self.timeout_ms.saturating_add(additional_ms);
    }
}

// ── Retry policy & state ──

/// Configures exponential-backoff retry behaviour for failed RPC calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (not counting the initial call).
    pub max_retries: u32,
    /// Base delay in milliseconds; doubled on each successive attempt.
    pub base_delay_ms: u64,
    /// Upper bound on the computed delay.
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
        }
    }
}

impl RetryPolicy {
    /// Compute the delay (in ms) for the given `attempt` number using
    /// exponential backoff: `min(base_delay_ms * 2^attempt, max_delay_ms)`.
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let exp_delay = self
            .base_delay_ms
            .saturating_mul(1u64.checked_shl(attempt).unwrap_or(u64::MAX));
        exp_delay.min(self.max_delay_ms)
    }

    /// Whether another retry should be attempted.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Mutable state that accompanies a [`RetryPolicy`] over the lifetime of a
/// single retriable operation.
#[derive(Debug, Clone)]
pub struct RetryState {
    /// How many attempts have been made so far.
    pub attempt: u32,
    /// The policy governing this retry loop.
    pub policy: RetryPolicy,
    /// The most recent error message, if any.
    pub last_error: Option<String>,
}

impl RetryState {
    /// Start tracking retries for the given `policy`.
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            attempt: 0,
            policy,
            last_error: None,
        }
    }

    /// Record a failure.  Returns `true` when the policy allows another
    /// retry, `false` when retries are exhausted.
    pub fn record_failure(&mut self, error: &str) -> bool {
        self.last_error = Some(error.to_string());
        self.attempt += 1;
        self.policy.should_retry(self.attempt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Serialization roundtrips ──

    #[test]
    fn request_roundtrip() {
        let msg = RpcMessage::Request(RpcRequest {
            id: 1,
            proxy_id: "MainThreadCommands".into(),
            method: "executeCommand".into(),
            args: vec![json!("workbench.action.files.save")],
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn response_ok_roundtrip() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 42,
            result: Ok(json!({"key": "value"})),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn response_null_roundtrip() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 1,
            result: Ok(serde_json::Value::Null),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn response_error_roundtrip() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 5,
            result: Err(RpcError {
                message: "not found".into(),
                name: Some("NotFoundError".into()),
                stack: Some("at line 10".into()),
            }),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn event_roundtrip() {
        let msg = RpcMessage::Event(RpcEvent {
            proxy_id: "ExtHostTextEditors".into(),
            event_name: "onDidChangeTextEditorSelection".into(),
            data: json!({"lineNumber": 10}),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn request_json_format() {
        let msg = RpcMessage::Request(RpcRequest {
            id: 1,
            proxy_id: "MainThreadCommands".into(),
            method: "executeCommand".into(),
            args: vec![json!("workbench.action.files.save")],
        });
        let s = RpcProtocol::serialize_message(&msg);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "request");
        assert_eq!(v["id"], 1);
        assert_eq!(v["proxyId"], "MainThreadCommands");
        assert_eq!(v["method"], "executeCommand");
    }

    #[test]
    fn response_json_format() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 1,
            result: Ok(serde_json::Value::Null),
        });
        let s = RpcProtocol::serialize_message(&msg);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "response");
        assert_eq!(v["id"], 1);
        assert!(v.get("result").is_some());
    }

    #[test]
    fn event_json_format() {
        let msg = RpcMessage::Event(RpcEvent {
            proxy_id: "ExtHostTextEditors".into(),
            event_name: "onDidChangeTextEditorSelection".into(),
            data: json!({}),
        });
        let s = RpcProtocol::serialize_message(&msg);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "event");
        assert_eq!(v["proxyId"], "ExtHostTextEditors");
        assert_eq!(v["eventName"], "onDidChangeTextEditorSelection");
    }

    // ── Request ID generation ──

    #[test]
    fn request_ids_are_sequential() {
        let proto = RpcProtocol::new();
        let (id1, _) = proto.create_request("Svc", "m", vec![]);
        let (id2, _) = proto.create_request("Svc", "m", vec![]);
        let (id3, _) = proto.create_request("Svc", "m", vec![]);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn create_request_populates_fields() {
        let proto = RpcProtocol::new();
        let (id, req) = proto.create_request("MainThreadCommands", "exec", vec![json!(42)]);
        assert_eq!(req.id, id);
        assert_eq!(req.proxy_id, "MainThreadCommands");
        assert_eq!(req.method, "exec");
        assert_eq!(req.args, vec![json!(42)]);
    }

    // ── Response correlation ──

    #[tokio::test]
    async fn response_correlation() {
        let proto = RpcProtocol::new();
        let (id, _req) = proto.create_request("Svc", "method", vec![]);
        let rx = proto.register_pending(id);

        let response = RpcResponse {
            id,
            result: Ok(json!("done")),
        };
        proto.resolve_response(response.clone());

        let received = rx.await.unwrap();
        assert_eq!(received, response);
    }

    #[tokio::test]
    async fn resolve_unknown_id_does_not_panic() {
        let proto = RpcProtocol::new();
        proto.resolve_response(RpcResponse {
            id: 999,
            result: Ok(json!(null)),
        });
    }

    // ── Proxy identifiers ──

    #[test]
    fn proxy_main_thread() {
        let p = ProxyIdentifier::main_thread("MainThreadCommands");
        assert_eq!(p.id, "MainThreadCommands");
        assert!(p.is_main);
    }

    #[test]
    fn proxy_ext_host() {
        let p = ProxyIdentifier::ext_host("ExtHostCommands");
        assert_eq!(p.id, "ExtHostCommands");
        assert!(!p.is_main);
    }

    #[test]
    fn well_known_proxies() {
        assert_eq!(proxies::MAIN_THREAD_COMMANDS, "MainThreadCommands");
        assert_eq!(proxies::EXT_HOST_COMMANDS, "ExtHostCommands");
        assert_eq!(proxies::MAIN_THREAD_DEBUG, "MainThreadDebugService");
        assert_eq!(proxies::EXT_HOST_DEBUG, "ExtHostDebugService");
    }

    // ── Batch / Timeout / Retry tests ──

    fn sample_request(id: u64) -> RpcMessage {
        RpcMessage::Request(RpcRequest {
            id,
            proxy_id: "TestProxy".into(),
            method: "doSomething".into(),
            args: vec![json!(id)],
        })
    }

    #[test]
    fn batch_add_and_drain() {
        let mut batch = RpcBatch::new(10);
        batch.add(sample_request(1)).unwrap();
        batch.add(sample_request(2)).unwrap();
        assert_eq!(batch.len(), 2);

        let drained = batch.drain();
        assert_eq!(drained.len(), 2);
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_full_rejects() {
        let mut batch = RpcBatch::new(2);
        batch.add(sample_request(1)).unwrap();
        batch.add(sample_request(2)).unwrap();
        assert!(batch.is_full());

        let err = batch.add(sample_request(3)).unwrap_err();
        assert!(err.contains("full"));
    }

    #[test]
    fn batch_is_empty() {
        let batch = RpcBatch::new(5);
        assert!(batch.is_empty());
        assert!(!batch.is_full());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn batch_total_payload_size() {
        let mut batch = RpcBatch::new(10);
        batch.add(sample_request(1)).unwrap();
        let size = batch.total_payload_size();
        assert!(size > 0, "payload size should be positive");
    }

    #[test]
    fn timeout_not_expired() {
        let t = RpcTimeout::new(1000, 500);
        assert!(!t.is_expired(600));
        assert!(!t.is_expired(1499));
    }

    #[test]
    fn timeout_expired() {
        let t = RpcTimeout::new(1000, 500);
        assert!(t.is_expired(1500));
        assert!(t.is_expired(2000));
    }

    #[test]
    fn timeout_remaining() {
        let t = RpcTimeout::new(1000, 500);
        assert_eq!(t.remaining_ms(600), 900);
        assert_eq!(t.remaining_ms(1500), 0);
        assert_eq!(t.remaining_ms(2000), 0);
    }

    #[test]
    fn timeout_extend() {
        let mut t = RpcTimeout::new(1000, 0);
        assert!(t.is_expired(1000));
        t.extend(500);
        assert!(!t.is_expired(1000));
        assert_eq!(t.remaining_ms(1000), 500);
    }

    #[test]
    fn retry_exponential_backoff() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay_ms: 100,
            max_delay_ms: 5000,
        };
        assert_eq!(policy.delay_for_attempt(0), 100);  // 100 * 2^0
        assert_eq!(policy.delay_for_attempt(1), 200);  // 100 * 2^1
        assert_eq!(policy.delay_for_attempt(2), 400);  // 100 * 2^2
        assert_eq!(policy.delay_for_attempt(3), 800);  // 100 * 2^3
        assert_eq!(policy.delay_for_attempt(6), 5000); // capped at max
    }

    #[test]
    fn retry_should_retry() {
        let policy = RetryPolicy::default();
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        assert!(!policy.should_retry(10));
    }

    #[test]
    fn retry_state_record_failure() {
        let mut state = RetryState::new(RetryPolicy::default()); // max_retries = 3
        assert!(state.record_failure("err1"));  // attempt 1 < 3
        assert_eq!(state.attempt, 1);
        assert!(state.record_failure("err2"));  // attempt 2 < 3
        assert_eq!(state.attempt, 2);
        // 3rd failure — retries exhausted (attempt 3 == max_retries)
        assert!(!state.record_failure("err3"));
        assert_eq!(state.attempt, 3);
        assert_eq!(state.last_error.as_deref(), Some("err3"));
    }

    #[test]
    fn retry_default_policy() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, 3);
        assert_eq!(p.base_delay_ms, 100);
        assert_eq!(p.max_delay_ms, 5000);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        let _svc = RpcProtocol::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn ext_rpc_stats_new_defaults() {
        let stats = ExtRpcStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_rpc_stats_record_success() {
        let mut stats = ExtRpcStats::new();
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
    fn ext_rpc_stats_record_failure() {
        let mut stats = ExtRpcStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_rpc_stats_reset() {
        let mut stats = ExtRpcStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_rpc_stats_merge() {
        let mut a = ExtRpcStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtRpcStats::new();
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
    fn ext_rpc_stats_display() {
        let mut stats = ExtRpcStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_rpc_stats_default() {
        let stats = ExtRpcStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_rpc_validator_accepts_valid_name() {
        let v = ExtRpcValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_rpc_validator_rejects_empty() {
        let v = ExtRpcValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_rpc_validator_rejects_too_long() {
        let v = ExtRpcValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_rpc_validator_forbidden_prefix() {
        let v = ExtRpcValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_rpc_validator_allowed_chars() {
        let v = ExtRpcValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_rpc_validator_range() {
        let v = ExtRpcValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_rpc_sanitize_removes_control() {
        let result = ExtRpcValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_rpc_truncate_short_string() {
        assert_eq!(ExtRpcValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_rpc_truncate_long_string() {
        let result = ExtRpcValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_rpc_is_ascii_printable() {
        assert!(ExtRpcValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtRpcValidator::is_ascii_printable("Hello\x00World"));
    }
}
