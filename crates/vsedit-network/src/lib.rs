//! Network utilities.

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use std::fmt;
// ---------------------------------------------------------------------------
// HTTP method
// ---------------------------------------------------------------------------

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

// ---------------------------------------------------------------------------
// HTTP request / response
// ---------------------------------------------------------------------------

/// An outgoing HTTP request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

/// An incoming HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Returns `true` when the status code is in the 2xx range.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Returns `true` when the status code is in the 3xx range.
    pub fn is_redirect(&self) -> bool {
        (300..400).contains(&self.status)
    }

    /// Interprets the body as a UTF-8 string.
    pub fn body_as_string(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.clone())
    }
}

// ---------------------------------------------------------------------------
// Proxy configuration
// ---------------------------------------------------------------------------

/// Proxy settings for outbound requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

// ---------------------------------------------------------------------------
// Network service
// ---------------------------------------------------------------------------

/// High-level network service.
#[derive(Debug, Clone)]
pub struct NetworkService {
    proxy: Option<ProxyConfig>,
}

impl NetworkService {
    pub fn new() -> Self {
        Self { proxy: None }
    }

    /// Configures a proxy for subsequent requests.
    pub fn set_proxy(&mut self, config: ProxyConfig) {
        self.proxy = Some(config);
    }

    /// Creates a new [`HttpRequest`] with the given method and URL.
    pub fn create_request(&self, method: HttpMethod, url: impl Into<String>) -> HttpRequest {
        HttpRequest {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// Stub: returns `true` indicating network connectivity.
    pub fn is_online(&self) -> bool {
        true
    }
}

impl Default for NetworkService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HttpRequest helpers
// ---------------------------------------------------------------------------

impl HttpRequest {
    /// Add a header to this request.
    pub fn add_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.push((name.into(), value.into()));
    }

    /// Set the request body.
    pub fn set_body(&mut self, body: Vec<u8>) {
        self.body = Some(body);
    }

    /// Get the value of a header (case-insensitive lookup).
    pub fn get_header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }
}

// ---------------------------------------------------------------------------
// HttpResponse helpers
// ---------------------------------------------------------------------------

impl HttpResponse {
    /// Get the value of a header (case-insensitive lookup).
    pub fn get_header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }

    /// Return the `Content-Length` header value parsed as `u64`, if present.
    pub fn content_length(&self) -> Option<u64> {
        self.get_header("content-length")
            .and_then(|v| v.parse::<u64>().ok())
    }

    /// Returns `true` when the status code is in the 4xx range.
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status)
    }

    /// Returns `true` when the status code is in the 5xx range.
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status)
    }
}

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// A simple token-bucket rate limiter (non-threaded).
#[derive(Debug, Clone)]
pub struct RateLimiter {
    max_tokens: u32,
    available: u32,
}

impl RateLimiter {
    pub fn new(max_tokens: u32) -> Self {
        Self {
            max_tokens,
            available: max_tokens,
        }
    }

    /// Try to acquire a token. Returns `true` if successful.
    pub fn try_acquire(&mut self) -> bool {
        if self.available > 0 {
            self.available -= 1;
            true
        } else {
            false
        }
    }

    /// Return the number of remaining tokens.
    pub fn remaining(&self) -> u32 {
        self.available
    }

    /// Reset the limiter to its full capacity.
    pub fn reset(&mut self) {
        self.available = self.max_tokens;
    }
}

// ---------------------------------------------------------------------------
// Retry policy
// ---------------------------------------------------------------------------

/// Retry policy with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

impl RetryPolicy {
    pub fn new(max_retries: u32, base_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
        }
    }

    /// Returns `true` if the attempt should be retried.
    pub fn should_retry(&self, attempt: u32, status: u16) -> bool {
        if attempt >= self.max_retries {
            return false;
        }
        // Retry on server errors and 429 Too Many Requests
        (500..600).contains(&status) || status == 429
    }

    /// Compute the delay in ms for `attempt` using exponential backoff.
    pub fn delay_ms(&self, attempt: u32) -> u64 {
        self.base_delay_ms * 2u64.pow(attempt)
    }
}

// ---------------------------------------------------------------------------
// NetworkService extras
// ---------------------------------------------------------------------------

impl NetworkService {
    /// Remove any configured proxy.
    pub fn clear_proxy(&mut self) {
        self.proxy = None;
    }

    /// Return a reference to the current proxy configuration, if set.
    pub fn get_proxy(&self) -> Option<&ProxyConfig> {
        self.proxy.as_ref()
    }
}

// ---------------------------------------------------------------------------
// URL parsing
// ---------------------------------------------------------------------------

/// Parsed components of a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlParts {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
}

/// Parse a URL string into its component parts.
///
/// This is a minimal parser that handles `scheme://host[:port][/path]`.
pub fn parse_url(url: &str) -> Option<UrlParts> {
    let (scheme, rest) = url.split_once("://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().ok()),
        None => (authority.to_string(), None),
    };
    Some(UrlParts {
        scheme: scheme.to_string(),
        host,
        port,
        path: path.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for network operations.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl NetworkStats {
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
    pub fn merge(&mut self, other: &NetworkStats) {
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

impl Default for NetworkStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NetworkStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NetworkStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for network.
#[derive(Debug, Clone)]
pub struct NetworkValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl NetworkValidator {
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

impl Default for NetworkValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ProxyConfig extensions
// ---------------------------------------------------------------------------

impl ProxyConfig {
    /// Format as `http://host:port`.
    pub fn proxy_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// Returns `true` if the proxy requires authentication.
    pub fn requires_auth(&self) -> bool {
        self.username.is_some()
    }

    /// Set authentication credentials (builder pattern).
    pub fn with_auth(mut self, user: &str, pass: &str) -> Self {
        self.username = Some(user.to_string());
        self.password = Some(pass.to_string());
        self
    }
}

// ---------------------------------------------------------------------------
// NetworkStatus
// ---------------------------------------------------------------------------

/// Network connectivity status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkStatus {
    Online,
    Offline,
    Limited,
}

impl NetworkStatus {
    /// Returns `true` if network is available (Online or Limited).
    pub fn is_available(&self) -> bool {
        matches!(self, NetworkStatus::Online | NetworkStatus::Limited)
    }
}

impl fmt::Display for NetworkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkStatus::Online => f.write_str("Online"),
            NetworkStatus::Offline => f.write_str("Offline"),
            NetworkStatus::Limited => f.write_str("Limited"),
        }
    }
}

// ---------------------------------------------------------------------------
// DownloadProgress
// ---------------------------------------------------------------------------

/// Tracks download progress.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    total_bytes: u64,
    received_bytes: u64,
}

impl DownloadProgress {
    /// Create a new progress tracker with the given total size.
    pub fn new(total_bytes: u64) -> Self {
        Self {
            total_bytes,
            received_bytes: 0,
        }
    }

    /// Add received bytes.
    pub fn update(&mut self, bytes_received: u64) {
        self.received_bytes += bytes_received;
        if self.received_bytes > self.total_bytes {
            self.received_bytes = self.total_bytes;
        }
    }

    /// Percentage complete (0.0 to 100.0).
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.received_bytes as f64 / self.total_bytes as f64) * 100.0
    }

    /// Returns `true` if all bytes have been received.
    pub fn is_complete(&self) -> bool {
        self.received_bytes >= self.total_bytes
    }

    /// Bytes remaining to download.
    pub fn bytes_remaining(&self) -> u64 {
        self.total_bytes.saturating_sub(self.received_bytes)
    }

    /// Bytes received so far.
    pub fn received(&self) -> u64 {
        self.received_bytes
    }
}

// ---------------------------------------------------------------------------
// NetworkService extensions
// ---------------------------------------------------------------------------

impl NetworkService {
    /// Return the current network status (stub: always Online).
    pub fn status(&self) -> NetworkStatus {
        NetworkStatus::Online
    }

    /// Set a proxy configuration (builder pattern).
    pub fn with_proxy(mut self, config: ProxyConfig) -> Self {
        self.proxy = Some(config);
        self
    }
}

// ---------------------------------------------------------------------------
// ConnectionPool
// ---------------------------------------------------------------------------

/// Simple connection pool tracker.
#[derive(Debug, Clone)]
pub struct ConnectionPool {
    max_connections: usize,
    active: usize,
}

impl ConnectionPool {
    /// Create a new pool with the given maximum connections.
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            active: 0,
        }
    }

    /// Try to acquire a connection. Returns `true` if successful.
    pub fn acquire(&mut self) -> bool {
        if self.active < self.max_connections {
            self.active += 1;
            true
        } else {
            false
        }
    }

    /// Release a connection back to the pool.
    pub fn release(&mut self) {
        if self.active > 0 {
            self.active -= 1;
        }
    }

    /// Number of available connections.
    pub fn available(&self) -> usize {
        self.max_connections - self.active
    }

    /// Number of connections currently in use.
    pub fn in_use(&self) -> usize {
        self.active
    }
}

// ---------------------------------------------------------------------------
// Network request log
// ---------------------------------------------------------------------------

/// A single recorded network request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestLogEntry {
    pub url: String,
    pub method: String,
    pub status_code: u16,
    pub duration_ms: u64,
    pub timestamp: u64,
}

/// Logs network requests for debugging.
#[derive(Debug, Clone)]
pub struct NetworkRequestLog {
    pub entries: Vec<RequestLogEntry>,
    next_timestamp: u64,
}

impl NetworkRequestLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_timestamp: 0,
        }
    }

    pub fn log_request(&mut self, url: &str, method: &str, status_code: u16, duration_ms: u64) {
        let ts = self.next_timestamp;
        self.next_timestamp += 1;
        self.entries.push(RequestLogEntry {
            url: url.to_string(),
            method: method.to_string(),
            status_code,
            duration_ms,
            timestamp: ts,
        });
    }

    pub fn entries_for_url(&self, url: &str) -> Vec<&RequestLogEntry> {
        self.entries.iter().filter(|e| e.url == url).collect()
    }

    pub fn entries_by_status(&self, status: u16) -> Vec<&RequestLogEntry> {
        self.entries.iter().filter(|e| e.status_code == status).collect()
    }

    pub fn average_duration_ms(&self) -> u64 {
        if self.entries.is_empty() {
            return 0;
        }
        let total: u64 = self.entries.iter().map(|e| e.duration_ms).sum();
        total / self.entries.len() as u64
    }

    pub fn total_requests(&self) -> usize {
        self.entries.len()
    }

    pub fn failed_requests(&self) -> usize {
        self.entries.iter().filter(|e| e.status_code >= 400).count()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Network cache
// ---------------------------------------------------------------------------

/// A single cache entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub data: Vec<u8>,
    pub etag: Option<String>,
    pub inserted_at: u64,
    pub ttl_ms: u64,
}

/// Simple in-memory network cache.
#[derive(Debug, Clone)]
pub struct NetworkCache {
    pub entries: HashMap<String, CacheEntry>,
    next_timestamp: u64,
}

impl NetworkCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_timestamp: 0,
        }
    }

    pub fn insert(&mut self, url: &str, data: Vec<u8>, ttl_ms: u64) {
        let ts = self.next_timestamp;
        self.next_timestamp += 1;
        self.entries.insert(url.to_string(), CacheEntry {
            data,
            etag: None,
            inserted_at: ts,
            ttl_ms,
        });
    }

    pub fn insert_with_etag(&mut self, url: &str, data: Vec<u8>, etag: String, ttl_ms: u64) {
        let ts = self.next_timestamp;
        self.next_timestamp += 1;
        self.entries.insert(url.to_string(), CacheEntry {
            data,
            etag: Some(etag),
            inserted_at: ts,
            ttl_ms,
        });
    }

    pub fn get(&self, url: &str) -> Option<&[u8]> {
        self.entries.get(url).map(|e| e.data.as_slice())
    }

    pub fn get_etag(&self, url: &str) -> Option<&str> {
        self.entries.get(url).and_then(|e| e.etag.as_deref())
    }

    pub fn remove(&mut self, url: &str) -> bool {
        self.entries.remove(url).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// URL normalization helpers
// ---------------------------------------------------------------------------

/// Normalize a URL by lowercasing scheme and host, removing default ports
/// (80 for http, 443 for https), and stripping trailing slashes from the path.
pub fn normalize_network_url(url: &str) -> String {
    let parts = match parse_url(url) {
        Some(p) => p,
        None => return url.to_string(),
    };
    let scheme = parts.scheme.to_lowercase();
    let host = parts.host.to_lowercase();
    let skip_port = match (scheme.as_str(), parts.port) {
        ("http", Some(80)) | ("https", Some(443)) => true,
        _ => false,
    };
    let authority = if skip_port || parts.port.is_none() {
        host.clone()
    } else {
        format!("{}:{}", host, parts.port.unwrap())
    };
    let path = if parts.path.len() > 1 {
        parts.path.trim_end_matches('/').to_string()
    } else {
        parts.path.clone()
    };
    format!("{scheme}://{authority}{path}")
}

// ---------------------------------------------------------------------------
// Hostname validation
// ---------------------------------------------------------------------------

/// Validate a hostname (RFC 952 / RFC 1123 style).
///
/// Returns `Ok(())` if the hostname is syntactically valid.
pub fn validate_hostname(host: &str) -> Result<(), String> {
    if host.is_empty() {
        return Err("hostname is empty".into());
    }
    if host.len() > 253 {
        return Err("hostname exceeds 253 characters".into());
    }
    for label in host.split('.') {
        if label.is_empty() {
            return Err("hostname contains empty label".into());
        }
        if label.len() > 63 {
            return Err("hostname label exceeds 63 characters".into());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!("label '{}' starts or ends with hyphen", label));
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("label '{}' contains invalid characters", label));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Port range validation
// ---------------------------------------------------------------------------

/// Validate that a port number is within the valid range (1–65535).
pub fn validate_port(port: u16) -> Result<(), String> {
    if port == 0 {
        Err("port 0 is not valid".into())
    } else {
        Ok(())
    }
}

/// Validate that a port number falls within a custom inclusive range.
pub fn validate_port_range(port: u16, min: u16, max: u16) -> Result<(), String> {
    if port < min || port > max {
        Err(format!("port {} is outside range [{}, {}]", port, min, max))
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Network address formatting
// ---------------------------------------------------------------------------

/// Format a host and optional port into a network address string.
pub fn format_address(host: &str, port: Option<u16>) -> String {
    match port {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Connection state tracking
// ---------------------------------------------------------------------------

/// Tracks the state of a network connection with transition history.
#[derive(Debug, Clone)]
pub struct ConnectionTracker {
    current: NetworkStatus,
    transitions: Vec<(NetworkStatus, NetworkStatus)>,
}

impl ConnectionTracker {
    /// Create a new tracker starting in the given state.
    pub fn new(initial: NetworkStatus) -> Self {
        Self {
            current: initial,
            transitions: Vec::new(),
        }
    }

    /// Transition to a new state, recording the change.
    pub fn transition(&mut self, new_state: NetworkStatus) {
        if new_state != self.current {
            self.transitions.push((self.current, new_state));
            self.current = new_state;
        }
    }

    /// Current state.
    pub fn current(&self) -> NetworkStatus {
        self.current
    }

    /// Number of state transitions recorded.
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Get the full transition log.
    pub fn transitions(&self) -> &[(NetworkStatus, NetworkStatus)] {
        &self.transitions
    }
}

// ---------------------------------------------------------------------------
// HTTP Header Builder
// ---------------------------------------------------------------------------

/// Fluent builder for constructing common HTTP headers.
#[derive(Debug, Clone, Default)]
pub struct HeaderBuilder {
    headers: Vec<(String, String)>,
}

impl HeaderBuilder {
    /// Create a new empty header builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an arbitrary header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Set the `Content-Type` header.
    pub fn content_type(self, mime: impl Into<String>) -> Self {
        self.header("Content-Type", mime)
    }

    /// Set `Content-Type: application/json`.
    pub fn json(self) -> Self {
        self.content_type("application/json")
    }

    /// Set the `Authorization` header with a Bearer token.
    pub fn bearer_token(self, token: impl Into<String>) -> Self {
        self.header("Authorization", format!("Bearer {}", token.into()))
    }

    /// Set the `Accept` header.
    pub fn accept(self, mime: impl Into<String>) -> Self {
        self.header("Accept", mime)
    }

    /// Set the `User-Agent` header.
    pub fn user_agent(self, ua: impl Into<String>) -> Self {
        self.header("User-Agent", ua)
    }

    /// Apply common defaults for a JSON API request (Accept + Content-Type).
    pub fn json_api_defaults(self) -> Self {
        self.json().accept("application/json")
    }

    /// Consume the builder and return the header list.
    pub fn build(self) -> Vec<(String, String)> {
        self.headers
    }

    /// Return how many headers have been added.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Return `true` if no headers have been added.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Look up a header value by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }
}

// ---------------------------------------------------------------------------
// Request Priority Queue
// ---------------------------------------------------------------------------

/// Priority level for queued network requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestPriority {
    /// Background tasks (telemetry, prefetch).
    Low = 0,
    /// Normal user-initiated requests.
    Normal = 1,
    /// Interactive requests (autocomplete, hover info).
    High = 2,
    /// Requests that block the UI.
    Critical = 3,
}

impl RequestPriority {
    /// Return the numeric priority value (higher = more urgent).
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// A network request wrapped with a priority and insertion order.
#[derive(Debug, Clone)]
pub struct PrioritizedRequest {
    pub request: HttpRequest,
    pub priority: RequestPriority,
    sequence: u64,
}

impl PartialEq for PrioritizedRequest {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.sequence == other.sequence
    }
}
impl Eq for PrioritizedRequest {}

impl PartialOrd for PrioritizedRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first; within same priority, lower sequence (FIFO).
        (self.priority.value(), other.sequence).cmp(&(other.priority.value(), self.sequence))
    }
}

/// A priority queue for outgoing network requests.
///
/// Higher-priority requests are dequeued first. Within the same priority,
/// requests are served in FIFO order.
#[derive(Debug)]
pub struct RequestQueue {
    heap: BinaryHeap<PrioritizedRequest>,
    next_seq: u64,
}

impl RequestQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            next_seq: 0,
        }
    }

    /// Enqueue a request with the given priority.
    pub fn push(&mut self, request: HttpRequest, priority: RequestPriority) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.heap.push(PrioritizedRequest {
            request,
            priority,
            sequence: seq,
        });
    }

    /// Dequeue the highest-priority request.
    pub fn pop(&mut self) -> Option<PrioritizedRequest> {
        self.heap.pop()
    }

    /// Peek at the highest-priority request without removing it.
    pub fn peek(&self) -> Option<&PrioritizedRequest> {
        self.heap.peek()
    }

    /// Return the number of queued requests.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Return `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Drain all requests in priority order.
    pub fn drain(&mut self) -> Vec<PrioritizedRequest> {
        let mut out = Vec::with_capacity(self.heap.len());
        while let Some(req) = self.heap.pop() {
            out.push(req);
        }
        out
    }
}

impl Default for RequestQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// URL query string utilities
// ---------------------------------------------------------------------------

/// Parse a query string (without leading `?`) into key-value pairs.
///
/// Duplicate keys produce multiple entries in the returned vector.
pub fn parse_query_string(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| {
            if let Some((k, v)) = pair.split_once('=') {
                (k.to_string(), v.to_string())
            } else {
                (pair.to_string(), String::new())
            }
        })
        .collect()
}

/// Build a query string from key-value pairs (without leading `?`).
pub fn build_query_string(params: &[(impl AsRef<str>, impl AsRef<str>)]) -> String {
    params
        .iter()
        .map(|(k, v)| {
            let k = k.as_ref();
            let v = v.as_ref();
            if v.is_empty() {
                k.to_string()
            } else {
                format!("{k}={v}")
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Extract the query string portion from a full URL, if present.
pub fn extract_query(url: &str) -> Option<&str> {
    let without_fragment = url.split('#').next().unwrap_or(url);
    without_fragment.split_once('?').map(|(_, q)| q)
}

/// Return the URL without its query string and fragment.
pub fn strip_query(url: &str) -> &str {
    let without_fragment = url.split('#').next().unwrap_or(url);
    without_fragment.split('?').next().unwrap_or(without_fragment)
}

// ---------------------------------------------------------------------------
// Proxy bypass list
// ---------------------------------------------------------------------------

/// Extended proxy configuration with a bypass (no-proxy) list.
#[derive(Debug, Clone)]
pub struct ProxyManager {
    config: Option<ProxyConfig>,
    bypass_hosts: Vec<String>,
}

impl ProxyManager {
    /// Create a new proxy manager with no proxy configured.
    pub fn new() -> Self {
        Self {
            config: None,
            bypass_hosts: Vec::new(),
        }
    }

    /// Set the proxy configuration.
    pub fn set_proxy(&mut self, config: ProxyConfig) {
        self.config = Some(config);
    }

    /// Clear the proxy configuration.
    pub fn clear_proxy(&mut self) {
        self.config = None;
    }

    /// Add a host pattern to the bypass list (e.g. `"localhost"`, `".internal.corp"`).
    pub fn add_bypass(&mut self, host: impl Into<String>) {
        self.bypass_hosts.push(host.into());
    }

    /// Return `true` if the given host should bypass the proxy.
    pub fn should_bypass(&self, host: &str) -> bool {
        let lower = host.to_ascii_lowercase();
        self.bypass_hosts.iter().any(|pattern| {
            let pat = pattern.to_ascii_lowercase();
            if pat.starts_with('.') {
                lower.ends_with(&pat) || lower == pat[1..]
            } else {
                lower == pat
            }
        })
    }

    /// Return the proxy URL to use for the given host, or `None` if bypassed
    /// or no proxy is configured.
    pub fn proxy_for(&self, host: &str) -> Option<String> {
        if self.config.is_none() || self.should_bypass(host) {
            return None;
        }
        self.config.as_ref().map(|c| c.proxy_url())
    }

    /// Return a reference to the current proxy config.
    pub fn config(&self) -> Option<&ProxyConfig> {
        self.config.as_ref()
    }

    /// Return the bypass list.
    pub fn bypass_list(&self) -> &[String] {
        &self.bypass_hosts
    }
}

impl Default for ProxyManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Request timeout configuration
// ---------------------------------------------------------------------------

/// Configurable timeout policy for network requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutConfig {
    /// Connection timeout in milliseconds.
    pub connect_ms: u64,
    /// Read/write timeout in milliseconds.
    pub read_ms: u64,
    /// Total request timeout in milliseconds (0 = no limit).
    pub total_ms: u64,
}

impl TimeoutConfig {
    /// Create a timeout config with the given connect and read timeouts.
    pub fn new(connect_ms: u64, read_ms: u64) -> Self {
        Self {
            connect_ms,
            read_ms,
            total_ms: 0,
        }
    }

    /// Set a total request timeout.
    pub fn with_total(mut self, total_ms: u64) -> Self {
        self.total_ms = total_ms;
        self
    }

    /// Return a fast timeout preset (1s connect, 5s read).
    pub fn fast() -> Self {
        Self::new(1_000, 5_000)
    }

    /// Return a default timeout preset (5s connect, 30s read).
    pub fn standard() -> Self {
        Self::new(5_000, 30_000)
    }

    /// Return a patient timeout preset (10s connect, 120s read).
    pub fn patient() -> Self {
        Self::new(10_000, 120_000)
    }

    /// Return `true` if a total timeout is configured.
    pub fn has_total_timeout(&self) -> bool {
        self.total_ms > 0
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self::standard()
    }
}

// ---------------------------------------------------------------------------
// Proxy selector – selects a proxy URL based on URL pattern rules
// ---------------------------------------------------------------------------

/// A rule mapping a URL pattern to a proxy URL.
#[derive(Debug, Clone)]
struct ProxyRule {
    pattern: String,
    proxy_url: String,
}

/// Selects a proxy based on URL pattern matching and bypass rules.
#[derive(Debug, Clone)]
pub struct NetworkProxySelector {
    rules: Vec<ProxyRule>,
    bypass_patterns: Vec<String>,
}

impl NetworkProxySelector {
    /// Create an empty proxy selector.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            bypass_patterns: Vec::new(),
        }
    }

    /// Add a rule: URLs containing `pattern` will use `proxy_url`.
    pub fn add_rule(&mut self, pattern: &str, proxy_url: &str) {
        self.rules.push(ProxyRule {
            pattern: pattern.to_string(),
            proxy_url: proxy_url.to_string(),
        });
    }

    /// Select the first matching proxy for `url`, or `None`.
    pub fn select_proxy(&self, url: &str) -> Option<&str> {
        if self.should_bypass(url) {
            return None;
        }
        self.rules
            .iter()
            .find(|r| url.contains(&r.pattern))
            .map(|r| r.proxy_url.as_str())
    }

    /// Register a bypass pattern – URLs containing `pattern` skip the proxy.
    pub fn add_bypass(&mut self, pattern: &str) {
        self.bypass_patterns.push(pattern.to_string());
    }

    /// Return `true` if `url` matches any bypass pattern.
    pub fn should_bypass(&self, url: &str) -> bool {
        self.bypass_patterns.iter().any(|p| url.contains(p))
    }

    /// Number of configured rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for NetworkProxySelector {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NetworkProxySelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NetworkProxySelector(rules={}, bypasses={})",
            self.rules.len(),
            self.bypass_patterns.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Retry strategy with circuit breaker
// ---------------------------------------------------------------------------

/// Configurable retry strategy with built-in circuit breaker.
#[derive(Debug, Clone)]
pub struct NetworkRetryStrategy {
    max_retries: u32,
    base_delay_ms: u64,
    circuit_open: bool,
}

impl NetworkRetryStrategy {
    /// Create a new retry strategy.
    pub fn new(max_retries: u32, base_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            circuit_open: false,
        }
    }

    /// Decide whether to retry the given attempt based on status code.
    /// Returns `false` when the circuit is open, the attempt count is
    /// exhausted, or the status code is not retryable.
    pub fn should_retry(&mut self, attempt: u32, status_code: u16) -> bool {
        if self.circuit_open {
            return false;
        }
        if attempt >= self.max_retries {
            return false;
        }
        // Retry on server errors (5xx) and 429 (Too Many Requests).
        (500..600).contains(&status_code) || status_code == 429
    }

    /// Compute the delay for `attempt` using exponential backoff.
    pub fn next_delay_ms(&self, attempt: u32) -> u64 {
        self.base_delay_ms.saturating_mul(2u64.saturating_pow(attempt))
    }

    /// Open the circuit breaker – all subsequent retries are refused.
    pub fn trip_circuit(&mut self) {
        self.circuit_open = true;
    }

    /// Return `true` when the circuit breaker is open.
    pub fn is_circuit_open(&self) -> bool {
        self.circuit_open
    }

    /// Reset the strategy (closes the circuit breaker).
    pub fn reset(&mut self) {
        self.circuit_open = false;
    }
}

impl fmt::Display for NetworkRetryStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NetworkRetryStrategy(max={}, base_ms={}, circuit={})",
            self.max_retries,
            self.base_delay_ms,
            if self.circuit_open { "open" } else { "closed" }
        )
    }
}

// ---------------------------------------------------------------------------
// ETag-aware response cache
// ---------------------------------------------------------------------------

/// A cached response with its ETag and body text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtagCacheEntry {
    pub etag: String,
    pub body: String,
}

/// Cache that stores responses keyed by URL with ETag support.
#[derive(Debug, Clone)]
pub struct EtagResponseCache {
    entries: HashMap<String, EtagCacheEntry>,
}

impl EtagResponseCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Insert or replace a cached response.
    pub fn put(&mut self, url: &str, etag: &str, body: &str) {
        self.entries.insert(
            url.to_string(),
            EtagCacheEntry {
                etag: etag.to_string(),
                body: body.to_string(),
            },
        );
    }

    /// Retrieve the cached entry for `url`.
    pub fn get(&self, url: &str) -> Option<&EtagCacheEntry> {
        self.entries.get(url)
    }

    /// Retrieve just the ETag for `url`.
    pub fn get_etag(&self, url: &str) -> Option<&str> {
        self.entries.get(url).map(|e| e.etag.as_str())
    }

    /// Remove the entry for `url`.
    pub fn invalidate(&mut self, url: &str) {
        self.entries.remove(url);
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` when the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for EtagResponseCache {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EtagResponseCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EtagResponseCache(entries={})", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// Throughput monitor
// ---------------------------------------------------------------------------

/// Tracks bytes transferred across network operations.
#[derive(Debug, Clone)]
pub struct NetworkThroughputMonitor {
    transfers: Vec<u64>,
}

impl NetworkThroughputMonitor {
    /// Create a new, empty monitor.
    pub fn new() -> Self {
        Self {
            transfers: Vec::new(),
        }
    }

    /// Record a single transfer of `bytes` bytes.
    pub fn record_transfer(&mut self, bytes: u64) {
        self.transfers.push(bytes);
    }

    /// Total bytes transferred across all recorded operations.
    pub fn total_bytes(&self) -> u64 {
        self.transfers.iter().sum()
    }

    /// Number of recorded transfers.
    pub fn transfer_count(&self) -> usize {
        self.transfers.len()
    }

    /// Average bytes per transfer (returns 0.0 when empty).
    pub fn average_bytes(&self) -> f64 {
        if self.transfers.is_empty() {
            0.0
        } else {
            self.total_bytes() as f64 / self.transfers.len() as f64
        }
    }

    /// Discard all recorded data.
    pub fn reset(&mut self) {
        self.transfers.clear();
    }
}

impl Default for NetworkThroughputMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NetworkThroughputMonitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NetworkThroughputMonitor(transfers={}, total_bytes={})",
            self.transfer_count(),
            self.total_bytes()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_defaults() {
        let svc = NetworkService::new();
        let req = svc.create_request(HttpMethod::Get, "https://example.com");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.url, "https://example.com");
        assert!(req.headers.is_empty());
        assert!(req.body.is_none());
    }

    #[test]
    fn response_status_helpers() {
        let ok = HttpResponse { status: 200, headers: vec![], body: vec![] };
        assert!(ok.is_success());
        assert!(!ok.is_redirect());

        let redirect = HttpResponse { status: 301, headers: vec![], body: vec![] };
        assert!(!redirect.is_success());
        assert!(redirect.is_redirect());

        let err = HttpResponse { status: 404, headers: vec![], body: vec![] };
        assert!(!err.is_success());
        assert!(!err.is_redirect());
    }

    #[test]
    fn response_body_as_string() {
        let resp = HttpResponse {
            status: 200,
            headers: vec![],
            body: b"hello world".to_vec(),
        };
        assert_eq!(resp.body_as_string().unwrap(), "hello world");
    }

    #[test]
    fn set_proxy() {
        let mut svc = NetworkService::new();
        assert!(svc.proxy.is_none());
        svc.set_proxy(ProxyConfig {
            host: "proxy.local".into(),
            port: 8080,
            username: None,
            password: None,
        });
        assert_eq!(svc.proxy.as_ref().unwrap().port, 8080);
    }

    #[test]
    fn is_online_stub() {
        let svc = NetworkService::new();
        assert!(svc.is_online());
    }

    #[test]
    fn add_header_and_get_header() {
        let svc = NetworkService::new();
        let mut req = svc.create_request(HttpMethod::Post, "https://example.com");
        req.add_header("Content-Type", "application/json");
        assert_eq!(req.get_header("content-type"), Some("application/json"));
        assert_eq!(req.get_header("Content-Type"), Some("application/json"));
        assert!(req.get_header("X-Missing").is_none());
    }

    #[test]
    fn set_body_on_request() {
        let svc = NetworkService::new();
        let mut req = svc.create_request(HttpMethod::Post, "https://example.com");
        assert!(req.body.is_none());
        req.set_body(b"hello".to_vec());
        assert_eq!(req.body.as_deref(), Some(b"hello".as_slice()));
    }

    #[test]
    fn response_get_header_case_insensitive() {
        let resp = HttpResponse {
            status: 200,
            headers: vec![("Content-Type".into(), "text/html".into())],
            body: vec![],
        };
        assert_eq!(resp.get_header("content-type"), Some("text/html"));
    }

    #[test]
    fn response_content_length() {
        let resp = HttpResponse {
            status: 200,
            headers: vec![("Content-Length".into(), "42".into())],
            body: vec![],
        };
        assert_eq!(resp.content_length(), Some(42));

        let no_cl = HttpResponse { status: 200, headers: vec![], body: vec![] };
        assert_eq!(no_cl.content_length(), None);
    }

    #[test]
    fn response_error_helpers() {
        let client = HttpResponse { status: 404, headers: vec![], body: vec![] };
        assert!(client.is_client_error());
        assert!(!client.is_server_error());

        let server = HttpResponse { status: 500, headers: vec![], body: vec![] };
        assert!(!server.is_client_error());
        assert!(server.is_server_error());
    }

    #[test]
    fn rate_limiter_basic() {
        let mut rl = RateLimiter::new(2);
        assert_eq!(rl.remaining(), 2);
        assert!(rl.try_acquire());
        assert_eq!(rl.remaining(), 1);
        assert!(rl.try_acquire());
        assert!(!rl.try_acquire());
        rl.reset();
        assert_eq!(rl.remaining(), 2);
    }

    #[test]
    fn retry_policy_should_retry() {
        let policy = RetryPolicy::new(3, 100);
        assert!(policy.should_retry(0, 500));
        assert!(policy.should_retry(2, 429));
        assert!(!policy.should_retry(3, 500)); // exhausted
        assert!(!policy.should_retry(0, 200)); // not retryable
    }

    #[test]
    fn retry_policy_delay_exponential() {
        let policy = RetryPolicy::new(5, 100);
        assert_eq!(policy.delay_ms(0), 100);
        assert_eq!(policy.delay_ms(1), 200);
        assert_eq!(policy.delay_ms(2), 400);
        assert_eq!(policy.delay_ms(3), 800);
    }

    #[test]
    fn clear_and_get_proxy() {
        let mut svc = NetworkService::new();
        assert!(svc.get_proxy().is_none());
        svc.set_proxy(ProxyConfig {
            host: "proxy.local".into(),
            port: 3128,
            username: None,
            password: None,
        });
        assert!(svc.get_proxy().is_some());
        svc.clear_proxy();
        assert!(svc.get_proxy().is_none());
    }

    #[test]
    fn parse_url_basic() {
        let parts = parse_url("https://example.com:8080/api/v1").unwrap();
        assert_eq!(parts.scheme, "https");
        assert_eq!(parts.host, "example.com");
        assert_eq!(parts.port, Some(8080));
        assert_eq!(parts.path, "/api/v1");

        let simple = parse_url("http://localhost").unwrap();
        assert_eq!(simple.host, "localhost");
        assert_eq!(simple.port, None);
        assert_eq!(simple.path, "/");

        assert!(parse_url("not-a-url").is_none());
    }

    #[test]
    fn eq_httpmethod_same() {
        assert_eq!(HttpMethod::Get, HttpMethod::Get);
    }

    #[test]
    fn ne_httpmethod_diff() {
        assert_ne!(HttpMethod::Get, HttpMethod::Post);
    }

    #[test]
    fn proxy_config_url_and_auth() {
        let proxy = ProxyConfig {
            host: "proxy.example.com".into(),
            port: 8080,
            username: None,
            password: None,
        };
        assert_eq!(proxy.proxy_url(), "http://proxy.example.com:8080");
        assert!(!proxy.requires_auth());
        let authed = proxy.with_auth("user", "pass");
        assert!(authed.requires_auth());
        assert_eq!(authed.username.as_deref(), Some("user"));
        assert_eq!(authed.password.as_deref(), Some("pass"));
    }

    #[test]
    fn network_status_display_and_available() {
        assert_eq!(format!("{}", NetworkStatus::Online), "Online");
        assert_eq!(format!("{}", NetworkStatus::Offline), "Offline");
        assert_eq!(format!("{}", NetworkStatus::Limited), "Limited");
        assert!(NetworkStatus::Online.is_available());
        assert!(!NetworkStatus::Offline.is_available());
        assert!(NetworkStatus::Limited.is_available());
    }

    #[test]
    fn download_progress_basic() {
        let mut progress = DownloadProgress::new(1000);
        assert_eq!(progress.received(), 0);
        assert_eq!(progress.bytes_remaining(), 1000);
        assert!(!progress.is_complete());
        assert!((progress.percentage() - 0.0).abs() < f64::EPSILON);

        progress.update(500);
        assert_eq!(progress.received(), 500);
        assert_eq!(progress.bytes_remaining(), 500);
        assert!((progress.percentage() - 50.0).abs() < f64::EPSILON);
        assert!(!progress.is_complete());
    }

    #[test]
    fn download_progress_complete() {
        let mut progress = DownloadProgress::new(100);
        progress.update(100);
        assert!(progress.is_complete());
        assert!((progress.percentage() - 100.0).abs() < f64::EPSILON);
        assert_eq!(progress.bytes_remaining(), 0);
    }

    #[test]
    fn download_progress_overflow_clamped() {
        let mut progress = DownloadProgress::new(100);
        progress.update(200);
        assert_eq!(progress.received(), 100);
        assert!(progress.is_complete());
    }

    #[test]
    fn download_progress_zero_total() {
        let progress = DownloadProgress::new(0);
        assert!(progress.is_complete());
        assert!((progress.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn network_service_status_stub() {
        let svc = NetworkService::new();
        assert_eq!(svc.status(), NetworkStatus::Online);
    }

    #[test]
    fn network_service_with_proxy() {
        let proxy = ProxyConfig {
            host: "p.local".into(),
            port: 3128,
            username: None,
            password: None,
        };
        let svc = NetworkService::new().with_proxy(proxy);
        assert!(svc.get_proxy().is_some());
        assert_eq!(svc.get_proxy().unwrap().port, 3128);
    }

    #[test]
    fn connection_pool_basic() {
        let mut pool = ConnectionPool::new(2);
        assert_eq!(pool.available(), 2);
        assert_eq!(pool.in_use(), 0);
        assert!(pool.acquire());
        assert_eq!(pool.available(), 1);
        assert_eq!(pool.in_use(), 1);
        assert!(pool.acquire());
        assert!(!pool.acquire()); // exhausted
        assert_eq!(pool.available(), 0);
        pool.release();
        assert_eq!(pool.available(), 1);
        assert!(pool.acquire());
    }

    #[test]
    fn connection_pool_release_at_zero() {
        let mut pool = ConnectionPool::new(1);
        pool.release(); // should not underflow
        assert_eq!(pool.in_use(), 0);
        assert_eq!(pool.available(), 1);
    }

    #[test]
    fn network_stats_new_defaults() {
        let stats = NetworkStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn network_stats_record_success() {
        let mut stats = NetworkStats::new();
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
    fn network_stats_record_failure() {
        let mut stats = NetworkStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn network_stats_reset() {
        let mut stats = NetworkStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn network_stats_merge() {
        let mut a = NetworkStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = NetworkStats::new();
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
    fn network_stats_display() {
        let mut stats = NetworkStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn network_stats_default() {
        let stats = NetworkStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn network_validator_accepts_valid_name() {
        let v = NetworkValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn network_validator_rejects_empty() {
        let v = NetworkValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn network_validator_rejects_too_long() {
        let v = NetworkValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn network_validator_forbidden_prefix() {
        let v = NetworkValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn network_validator_allowed_chars() {
        let v = NetworkValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn network_validator_range() {
        let v = NetworkValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn network_sanitize_removes_control() {
        let result = NetworkValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn network_truncate_short_string() {
        assert_eq!(NetworkValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn network_truncate_long_string() {
        let result = NetworkValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn network_is_ascii_printable() {
        assert!(NetworkValidator::is_ascii_printable("Hello World 123"));
        assert!(!NetworkValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn test_request_log_basic() {
        let mut log = NetworkRequestLog::new();
        assert_eq!(log.total_requests(), 0);
        log.log_request("https://example.com", "GET", 200, 100);
        assert_eq!(log.total_requests(), 1);
        assert_eq!(log.entries[0].url, "https://example.com");
        assert_eq!(log.entries[0].method, "GET");
        assert_eq!(log.entries[0].status_code, 200);
        assert_eq!(log.entries[0].duration_ms, 100);
        assert_eq!(log.entries[0].timestamp, 0);
    }

    #[test]
    fn test_request_log_filter_by_url() {
        let mut log = NetworkRequestLog::new();
        log.log_request("https://a.com", "GET", 200, 50);
        log.log_request("https://b.com", "POST", 201, 60);
        log.log_request("https://a.com", "PUT", 200, 70);
        let filtered = log.entries_for_url("https://a.com");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].method, "GET");
        assert_eq!(filtered[1].method, "PUT");
    }

    #[test]
    fn test_request_log_filter_by_status() {
        let mut log = NetworkRequestLog::new();
        log.log_request("https://a.com", "GET", 200, 50);
        log.log_request("https://b.com", "GET", 404, 60);
        log.log_request("https://c.com", "GET", 200, 70);
        let ok = log.entries_by_status(200);
        assert_eq!(ok.len(), 2);
        let not_found = log.entries_by_status(404);
        assert_eq!(not_found.len(), 1);
        assert_eq!(log.failed_requests(), 1);
    }

    #[test]
    fn test_request_log_average_duration() {
        let mut log = NetworkRequestLog::new();
        assert_eq!(log.average_duration_ms(), 0);
        log.log_request("https://a.com", "GET", 200, 100);
        log.log_request("https://b.com", "GET", 200, 200);
        log.log_request("https://c.com", "GET", 200, 300);
        assert_eq!(log.average_duration_ms(), 200);
        log.clear();
        assert_eq!(log.total_requests(), 0);
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = NetworkCache::new();
        assert_eq!(cache.len(), 0);
        cache.insert("https://example.com", b"hello".to_vec(), 5000);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("https://example.com"), Some(b"hello".as_slice()));
        assert!(cache.get("https://other.com").is_none());
    }

    #[test]
    fn test_cache_with_etag() {
        let mut cache = NetworkCache::new();
        cache.insert_with_etag("https://example.com", b"data".to_vec(), "abc123".to_string(), 3000);
        assert_eq!(cache.get_etag("https://example.com"), Some("abc123"));
        assert!(cache.get_etag("https://missing.com").is_none());
        // Entry without etag
        cache.insert("https://no-etag.com", b"x".to_vec(), 1000);
        assert!(cache.get_etag("https://no-etag.com").is_none());
    }

    #[test]
    fn test_cache_remove_and_clear() {
        let mut cache = NetworkCache::new();
        cache.insert("https://a.com", b"a".to_vec(), 1000);
        cache.insert("https://b.com", b"b".to_vec(), 1000);
        assert_eq!(cache.len(), 2);
        assert!(cache.remove("https://a.com"));
        assert!(!cache.remove("https://a.com"));
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn normalize_network_url_strips_default_port() {
        assert_eq!(
            normalize_network_url("HTTP://EXAMPLE.COM:80/path/"),
            "http://example.com/path"
        );
        assert_eq!(
            normalize_network_url("https://Example.com:443/"),
            "https://example.com/"
        );
    }

    #[test]
    fn normalize_network_url_keeps_non_default_port() {
        assert_eq!(
            normalize_network_url("http://example.com:8080/api"),
            "http://example.com:8080/api"
        );
    }

    #[test]
    fn validate_hostname_valid() {
        assert!(validate_hostname("example.com").is_ok());
        assert!(validate_hostname("sub.domain.example.com").is_ok());
        assert!(validate_hostname("my-host").is_ok());
    }

    #[test]
    fn validate_hostname_invalid() {
        assert!(validate_hostname("").is_err());
        assert!(validate_hostname("-bad.com").is_err());
        assert!(validate_hostname("bad-.com").is_err());
        assert!(validate_hostname("inv@lid.com").is_err());
    }

    #[test]
    fn validate_port_and_range() {
        assert!(validate_port(80).is_ok());
        assert!(validate_port(0).is_err());
        assert!(validate_port_range(8080, 1024, 65535).is_ok());
        assert!(validate_port_range(80, 1024, 65535).is_err());
    }

    #[test]
    fn format_address_with_and_without_port() {
        assert_eq!(format_address("example.com", Some(443)), "example.com:443");
        assert_eq!(format_address("localhost", None), "localhost");
    }

    #[test]
    fn connection_tracker_records_transitions() {
        let mut tracker = ConnectionTracker::new(NetworkStatus::Online);
        assert_eq!(tracker.current(), NetworkStatus::Online);
        tracker.transition(NetworkStatus::Offline);
        tracker.transition(NetworkStatus::Offline); // no-op
        tracker.transition(NetworkStatus::Limited);
        assert_eq!(tracker.current(), NetworkStatus::Limited);
        assert_eq!(tracker.transition_count(), 2);
        assert_eq!(tracker.transitions()[0], (NetworkStatus::Online, NetworkStatus::Offline));
    }

    // -----------------------------------------------------------------------
    // New tests: HeaderBuilder, RequestQueue, query string, ProxyManager,
    // TimeoutConfig
    // -----------------------------------------------------------------------

    #[test]
    fn header_builder_json_api_defaults() {
        let headers = HeaderBuilder::new()
            .json_api_defaults()
            .user_agent("vsedit/1.0")
            .bearer_token("tok_abc")
            .build();
        assert_eq!(headers.len(), 4);

        let builder = HeaderBuilder::new().json_api_defaults().user_agent("vsedit/1.0");
        assert_eq!(builder.get("content-type"), Some("application/json"));
        assert_eq!(builder.get("Accept"), Some("application/json"));
        assert_eq!(builder.get("user-agent"), Some("vsedit/1.0"));
        assert!(builder.get("X-Missing").is_none());
    }

    #[test]
    fn header_builder_empty() {
        let builder = HeaderBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        let headers = builder.build();
        assert!(headers.is_empty());
    }

    #[test]
    fn request_queue_priority_ordering() {
        let mut queue = RequestQueue::new();
        let make_req = |url: &str| HttpRequest {
            method: HttpMethod::Get,
            url: url.to_string(),
            headers: vec![],
            body: None,
        };

        queue.push(make_req("low"), RequestPriority::Low);
        queue.push(make_req("normal"), RequestPriority::Normal);
        queue.push(make_req("critical"), RequestPriority::Critical);
        queue.push(make_req("high"), RequestPriority::High);

        assert_eq!(queue.len(), 4);
        assert!(!queue.is_empty());

        let first = queue.pop().unwrap();
        assert_eq!(first.request.url, "critical");
        assert_eq!(first.priority, RequestPriority::Critical);

        let second = queue.pop().unwrap();
        assert_eq!(second.request.url, "high");

        let third = queue.pop().unwrap();
        assert_eq!(third.request.url, "normal");

        let fourth = queue.pop().unwrap();
        assert_eq!(fourth.request.url, "low");

        assert!(queue.pop().is_none());
        assert!(queue.is_empty());
    }

    #[test]
    fn request_queue_fifo_within_same_priority() {
        let mut queue = RequestQueue::new();
        let make_req = |url: &str| HttpRequest {
            method: HttpMethod::Get,
            url: url.to_string(),
            headers: vec![],
            body: None,
        };

        queue.push(make_req("first"), RequestPriority::Normal);
        queue.push(make_req("second"), RequestPriority::Normal);
        queue.push(make_req("third"), RequestPriority::Normal);

        assert_eq!(queue.pop().unwrap().request.url, "first");
        assert_eq!(queue.pop().unwrap().request.url, "second");
        assert_eq!(queue.pop().unwrap().request.url, "third");
    }

    #[test]
    fn request_queue_drain() {
        let mut queue = RequestQueue::new();
        let make_req = |url: &str| HttpRequest {
            method: HttpMethod::Get,
            url: url.to_string(),
            headers: vec![],
            body: None,
        };
        queue.push(make_req("a"), RequestPriority::Low);
        queue.push(make_req("b"), RequestPriority::High);
        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].request.url, "b");
        assert_eq!(drained[1].request.url, "a");
        assert!(queue.is_empty());
    }

    #[test]
    fn parse_and_build_query_string() {
        let pairs = parse_query_string("foo=bar&baz=42&flag");
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("foo".to_string(), "bar".to_string()));
        assert_eq!(pairs[1], ("baz".to_string(), "42".to_string()));
        assert_eq!(pairs[2], ("flag".to_string(), String::new()));

        let rebuilt = build_query_string(&[("foo", "bar"), ("baz", "42"), ("flag", "")]);
        assert_eq!(rebuilt, "foo=bar&baz=42&flag");

        assert!(parse_query_string("").is_empty());
    }

    #[test]
    fn extract_and_strip_query() {
        assert_eq!(
            extract_query("https://example.com/path?foo=1&bar=2#frag"),
            Some("foo=1&bar=2")
        );
        assert_eq!(extract_query("https://example.com/path"), None);
        assert_eq!(
            strip_query("https://example.com/path?foo=1&bar=2#frag"),
            "https://example.com/path"
        );
        assert_eq!(
            strip_query("https://example.com/path"),
            "https://example.com/path"
        );
    }

    #[test]
    fn proxy_manager_bypass_and_selection() {
        let mut mgr = ProxyManager::new();
        assert!(mgr.config().is_none());
        assert!(mgr.proxy_for("example.com").is_none());

        mgr.set_proxy(ProxyConfig {
            host: "proxy.corp".into(),
            port: 3128,
            username: None,
            password: None,
        });
        mgr.add_bypass("localhost");
        mgr.add_bypass(".internal.corp");

        // Bypassed hosts
        assert!(mgr.should_bypass("localhost"));
        assert!(mgr.should_bypass("LOCALHOST"));
        assert!(mgr.should_bypass("app.internal.corp"));
        assert!(mgr.should_bypass("internal.corp"));
        assert!(mgr.proxy_for("localhost").is_none());

        // Non-bypassed host
        assert!(!mgr.should_bypass("example.com"));
        assert_eq!(
            mgr.proxy_for("example.com"),
            Some("http://proxy.corp:3128".to_string())
        );

        assert_eq!(mgr.bypass_list().len(), 2);

        mgr.clear_proxy();
        assert!(mgr.proxy_for("example.com").is_none());
    }

    #[test]
    fn timeout_config_presets() {
        let fast = TimeoutConfig::fast();
        assert_eq!(fast.connect_ms, 1_000);
        assert_eq!(fast.read_ms, 5_000);
        assert!(!fast.has_total_timeout());

        let standard = TimeoutConfig::default();
        assert_eq!(standard.connect_ms, 5_000);
        assert_eq!(standard.read_ms, 30_000);

        let patient = TimeoutConfig::patient().with_total(300_000);
        assert_eq!(patient.connect_ms, 10_000);
        assert_eq!(patient.read_ms, 120_000);
        assert!(patient.has_total_timeout());
        assert_eq!(patient.total_ms, 300_000);
    }

    #[test]
    fn request_priority_values() {
        assert!(RequestPriority::Critical.value() > RequestPriority::High.value());
        assert!(RequestPriority::High.value() > RequestPriority::Normal.value());
        assert!(RequestPriority::Normal.value() > RequestPriority::Low.value());
    }

    // --- NetworkProxySelector tests ---

    #[test]
    fn proxy_selector_matches_rule() {
        let mut sel = NetworkProxySelector::new();
        sel.add_rule("github.com", "http://proxy:8080");
        assert_eq!(sel.select_proxy("https://github.com/repo"), Some("http://proxy:8080"));
        assert!(sel.select_proxy("https://example.com").is_none());
    }

    #[test]
    fn proxy_selector_bypass() {
        let mut sel = NetworkProxySelector::new();
        sel.add_rule("example.com", "http://proxy:3128");
        sel.add_bypass("localhost");
        sel.add_bypass("127.0.0.1");
        assert!(sel.should_bypass("http://localhost:8080/path"));
        assert!(!sel.should_bypass("https://example.com"));
        // Bypass takes precedence even when a rule matches.
        sel.add_bypass("example.com");
        assert!(sel.select_proxy("https://example.com").is_none());
    }

    #[test]
    fn proxy_selector_display() {
        let mut sel = NetworkProxySelector::new();
        sel.add_rule("a", "b");
        sel.add_bypass("c");
        let s = format!("{sel}");
        assert!(s.contains("rules=1"));
        assert!(s.contains("bypasses=1"));
    }

    // --- NetworkRetryStrategy tests ---

    #[test]
    fn retry_strategy_should_retry() {
        let mut strat = NetworkRetryStrategy::new(3, 100);
        assert!(strat.should_retry(0, 500));
        assert!(strat.should_retry(0, 429));
        assert!(!strat.should_retry(0, 200));
        assert!(!strat.should_retry(3, 500));
    }

    #[test]
    fn retry_strategy_exponential_backoff() {
        let strat = NetworkRetryStrategy::new(5, 100);
        assert_eq!(strat.next_delay_ms(0), 100);
        assert_eq!(strat.next_delay_ms(1), 200);
        assert_eq!(strat.next_delay_ms(2), 400);
        assert_eq!(strat.next_delay_ms(3), 800);
    }

    #[test]
    fn retry_strategy_circuit_breaker() {
        let mut strat = NetworkRetryStrategy::new(3, 50);
        assert!(!strat.is_circuit_open());
        strat.trip_circuit();
        assert!(strat.is_circuit_open());
        assert!(!strat.should_retry(0, 500));
        strat.reset();
        assert!(!strat.is_circuit_open());
        assert!(strat.should_retry(0, 503));
    }

    #[test]
    fn retry_strategy_display() {
        let strat = NetworkRetryStrategy::new(3, 100);
        let s = format!("{strat}");
        assert!(s.contains("max=3"));
        assert!(s.contains("closed"));
    }

    // --- EtagResponseCache tests ---

    #[test]
    fn etag_cache_put_get() {
        let mut cache = EtagResponseCache::new();
        cache.put("https://api.example.com/v1", "\"abc123\"", "{\"ok\":true}");
        assert_eq!(cache.len(), 1);
        let entry = cache.get("https://api.example.com/v1").unwrap();
        assert_eq!(entry.etag, "\"abc123\"");
        assert_eq!(entry.body, "{\"ok\":true}");
        assert_eq!(cache.get_etag("https://api.example.com/v1"), Some("\"abc123\""));
    }

    #[test]
    fn etag_cache_invalidate_and_clear() {
        let mut cache = EtagResponseCache::new();
        cache.put("https://a.com", "e1", "b1");
        cache.put("https://b.com", "e2", "b2");
        assert_eq!(cache.len(), 2);
        cache.invalidate("https://a.com");
        assert_eq!(cache.len(), 1);
        assert!(cache.get("https://a.com").is_none());
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn etag_cache_display() {
        let cache = EtagResponseCache::new();
        let s = format!("{cache}");
        assert!(s.contains("entries=0"));
    }

    // --- NetworkThroughputMonitor tests ---

    #[test]
    fn throughput_monitor_stats() {
        let mut mon = NetworkThroughputMonitor::new();
        assert_eq!(mon.total_bytes(), 0);
        assert_eq!(mon.transfer_count(), 0);
        assert_eq!(mon.average_bytes(), 0.0);
        mon.record_transfer(1000);
        mon.record_transfer(3000);
        assert_eq!(mon.total_bytes(), 4000);
        assert_eq!(mon.transfer_count(), 2);
        assert!((mon.average_bytes() - 2000.0).abs() < f64::EPSILON);
        mon.reset();
        assert_eq!(mon.transfer_count(), 0);
    }

    #[test]
    fn throughput_monitor_display() {
        let mut mon = NetworkThroughputMonitor::new();
        mon.record_transfer(512);
        let s = format!("{mon}");
        assert!(s.contains("transfers=1"));
        assert!(s.contains("total_bytes=512"));
    }

    #[test]
    fn request_priority_ordering() {
        assert!(RequestPriority::Critical.value() > RequestPriority::High.value());
        assert!(RequestPriority::High.value() > RequestPriority::Normal.value());
        assert!(RequestPriority::Normal.value() > RequestPriority::Low.value());
    }
}
