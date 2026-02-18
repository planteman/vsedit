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


// ─── NetBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for network log.
#[derive(Debug, Clone)]
pub struct NetBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> NetBufRingBuffer<T> {
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

impl<T: Clone + fmt::Display> fmt::Display for NetBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NetBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── NetC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for network responses.
#[derive(Debug)]
pub struct NetCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> NetCLruCache<V> {
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

impl<V: Clone + fmt::Display> fmt::Display for NetCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NetCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}


// ---------------------------------------------------------------------------
// network – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XNetworkValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XNetworkValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(m) | Self::Warning(m) => Some(m),
        }
    }
}

/// A key-value pair with optional metadata tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XNetworkTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XNetworkTaggedEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

/// Validate that a string is non-empty and within a max length.
pub fn x_network_validate_string(value: &str, max_len: usize) -> XNetworkValidationResult {
    if value.is_empty() {
        return XNetworkValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XNetworkValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XNetworkValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_network_validate_range(value: i64, min: i64, max: i64) -> XNetworkValidationResult {
    if value < min || value > max {
        XNetworkValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XNetworkValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_network_filter_by_tag<'a>(
    entries: &'a [XNetworkTaggedEntry],
    tag: &str,
) -> Vec<&'a XNetworkTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_network_group_by_tag(
    entries: &[XNetworkTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XNetworkTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_network_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_network_dedup_entries(entries: Vec<XNetworkTaggedEntry>) -> Vec<XNetworkTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
}



// ---------------------------------------------------------------------------
// network – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for network request layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YNetworkNetworkProxyType {
    None,
    Http,
    Socks5,
    System,
}

impl YNetworkNetworkProxyType {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Http => 1,
            Self::Socks5 => 2,
            Self::System => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Http => "Http",
            Self::Socks5 => "Socks5",
            Self::System => "System",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YNetworkNetworkProxyType] {
        &[
            YNetworkNetworkProxyType::None,
            YNetworkNetworkProxyType::Http,
            YNetworkNetworkProxyType::Socks5,
            YNetworkNetworkProxyType::System,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YNetworkNetworkProxyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks retry config data.
#[derive(Debug, Clone)]
pub struct YNetworkNetworkRetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl YNetworkNetworkRetryConfig {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            max_retries: 0,
            base_delay_ms: 0,
            max_delay_ms: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YNetworkNetworkRetryConfig({}: {:?})", "max_retries", self.max_retries)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_network_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_network_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_network_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_network_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_network_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_network_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_network_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_network_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// network – Extended network bandwidth helpers
// ---------------------------------------------------------------------------

/// Priority levels for network bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZNetworkPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZNetworkPriority {
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
    pub fn all_asc() -> [ZNetworkPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZNetworkPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks network bandwidth data.
#[derive(Debug, Clone)]
pub struct ZNetworkNetworkBandwidthSample {
    pub samples_bps: Vec<u64>,
    pub window_sec: u32,
    pub peak_bps: u64,
}

impl ZNetworkNetworkBandwidthSample {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            samples_bps: Vec::new(),
            window_sec: 0,
            peak_bps: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.samples_bps.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.samples_bps.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.samples_bps.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZNetworkNetworkBandwidthSample[window_sec={:?}, peak_bps={:?}]", self.window_sec, self.peak_bps)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for network bandwidth.
pub fn z_network_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_network_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_network_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_network_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_network_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_network_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_network_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 63
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer63 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer63 {
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
pub fn xb_fnv1a_63(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_63<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_63<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_63(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_63(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 127
// ---------------------------------------------------------------------------

/// Generic object pool `Xc127Pool<T>`.
pub struct Xc127Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc127Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc127PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc127Pool<T> {
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
    pub fn stats(&self) -> Xc127PoolStats {
        Xc127PoolStats {
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

impl<T> Default for Xc127Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc127Scheduler`.
pub struct Xc127Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc127Scheduler {
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

impl Default for Xc127Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_127 hash for the given byte slice.
pub fn xc_127_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_127 convention.
pub fn xc_127_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe76 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe76Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe76PipelineError {
    pub stage: Xe76Stage,
    pub message: String,
}

impl std::fmt::Display for Xe76PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe76Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe76Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError>>>,
    stage_names: Vec<Xe76Stage>,
}

impl Xe76Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe76Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe76Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe76Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe76Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> {
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

    pub fn compose(mut self, other: Xe76Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe76CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe76CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe76Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe76CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe76CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe76Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe76CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_76_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe76CacheEntry {
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

    fn xe_76_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe76CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_76_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> {
    Ok(data)
}

pub fn xe_76_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_76_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_76_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_76_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe76PipelineError> {
    Err(Xe76PipelineError {
        stage: Xe76Stage::Parse,
        message: "intentional failure".to_string(),
    })
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
    fn set_proxy_works() {
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

    #[test]
    fn netbuf_ringbuf_push_get() {
        let mut rb = NetBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn netbuf_ringbuf_overflow() {
        let mut rb = NetBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn netbuf_ringbuf_clear() {
        let mut rb = NetBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn netbuf_ringbuf_newest_oldest() {
        let mut rb = NetBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn netbuf_ringbuf_to_vec() {
        let mut rb = NetBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn netbuf_ringbuf_is_full() {
        let mut rb = NetBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn netc_lru_insert_get() {
        let mut c = NetCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn netc_lru_eviction() {
        let mut c = NetCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn netc_lru_hit_ratio() {
        let mut c = NetCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn netc_lru_clear() {
        let mut c = NetCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn netc_lru_remove() {
        let mut c = NetCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn netc_lru_peek() {
        let mut c = NetCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    // -- network additional tests -------------------------------------------

    #[test]
    fn x_network_validation_ok() {
        let r = x_network_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_network_validation_empty() {
        let r = x_network_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_network_validation_too_long() {
        let r = x_network_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_network_validate_range_ok() {
        assert!(x_network_validate_range(5, 1, 10).is_ok());
        assert!(x_network_validate_range(1, 1, 10).is_ok());
        assert!(x_network_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_network_validate_range_out() {
        assert!(!x_network_validate_range(0, 1, 10).is_ok());
        assert!(!x_network_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_network_tagged_entry_basic() {
        let e = XNetworkTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_network_tagged_entry_with_tag() {
        let e = XNetworkTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_network_filter_by_tag_basic() {
        let entries = vec![
            XNetworkTaggedEntry::new("a", "1").with_tag("x"),
            XNetworkTaggedEntry::new("b", "2").with_tag("y"),
            XNetworkTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_network_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_network_group_by_tag_basic() {
        let entries = vec![
            XNetworkTaggedEntry::new("a", "1").with_tag("x"),
            XNetworkTaggedEntry::new("b", "2"),
            XNetworkTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_network_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_network_djb2_hash_deterministic() {
        let h1 = x_network_djb2_hash("hello");
        let h2 = x_network_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_network_djb2_hash("hello"), x_network_djb2_hash("world"));
    }

    #[test]
    fn x_network_dedup_entries_basic() {
        let entries = vec![
            XNetworkTaggedEntry::new("a", "1"),
            XNetworkTaggedEntry::new("a", "2"),
            XNetworkTaggedEntry::new("b", "3"),
        ];
        let deduped = x_network_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_network_validation_result_warning() {
        let w = XNetworkValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_network_filter_by_tag_empty() {
        let entries: Vec<XNetworkTaggedEntry> = vec![];
        assert!(x_network_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_network_tagged_entry_no_tag_match() {
        let e = XNetworkTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }


    // -- network extended domain tests ----------------------------------------

    #[test]
    fn y_network_enum_index() {
        assert_eq!(YNetworkNetworkProxyType::None.index(), 0);
        assert_eq!(YNetworkNetworkProxyType::Http.index(), 1);
        assert_eq!(YNetworkNetworkProxyType::Socks5.index(), 2);
        assert_eq!(YNetworkNetworkProxyType::System.index(), 3);
    }

    #[test]
    fn y_network_enum_label() {
        assert_eq!(YNetworkNetworkProxyType::None.label(), "None");
        assert_eq!(YNetworkNetworkProxyType::Http.label(), "Http");
        assert_eq!(YNetworkNetworkProxyType::Socks5.label(), "Socks5");
        assert_eq!(YNetworkNetworkProxyType::System.label(), "System");
    }

    #[test]
    fn y_network_enum_all() {
        let all = YNetworkNetworkProxyType::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_network_enum_is_default() {
        assert!(YNetworkNetworkProxyType::None.is_default());
        assert!(!YNetworkNetworkProxyType::System.is_default());
    }

    #[test]
    fn y_network_enum_display() {
        assert_eq!(format!("{}", YNetworkNetworkProxyType::None), "None");
    }

    #[test]
    fn y_network_struct_new() {
        let s = YNetworkNetworkRetryConfig::new();
        let _ = s.summary();
    }

    #[test]
    fn y_network_fingerprint_deterministic() {
        let h1 = y_network_fingerprint("hello");
        let h2 = y_network_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_network_fingerprint("a"), y_network_fingerprint("b"));
    }

    #[test]
    fn y_network_truncate_short() {
        assert_eq!(y_network_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_network_truncate_long() {
        let r = y_network_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_network_normalize_key_basic() {
        assert_eq!(y_network_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_network_split_path_basic() {
        let parts = y_network_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_network_count_occurrences_basic() {
        assert_eq!(y_network_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_network_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_network_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_network_in_range_basic() {
        assert!(y_network_in_range(5, 1, 10));
        assert!(y_network_in_range(1, 1, 10));
        assert!(y_network_in_range(10, 1, 10));
        assert!(!y_network_in_range(0, 1, 10));
        assert!(!y_network_in_range(11, 1, 10));
    }

    #[test]
    fn y_network_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_network_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_network_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_network_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- network Z-extended tests -----------------------------------------------

    #[test]
    fn z_network_priority_weight() {
        assert_eq!(ZNetworkPriority::Idle.weight(), 0);
        assert_eq!(ZNetworkPriority::Normal.weight(), 2);
        assert_eq!(ZNetworkPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_network_priority_label() {
        assert_eq!(ZNetworkPriority::Low.label(), "low");
        assert_eq!(ZNetworkPriority::High.label(), "high");
    }

    #[test]
    fn z_network_priority_is_elevated() {
        assert!(!ZNetworkPriority::Normal.is_elevated());
        assert!(ZNetworkPriority::High.is_elevated());
        assert!(ZNetworkPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_network_priority_display() {
        assert_eq!(format!("{}", ZNetworkPriority::Idle), "idle");
    }

    #[test]
    fn z_network_priority_all_asc() {
        let all = ZNetworkPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZNetworkPriority::Idle);
        assert_eq!(all[4], ZNetworkPriority::Realtime);
    }

    #[test]
    fn z_network_struct_new() {
        let s = ZNetworkNetworkBandwidthSample::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_network_struct_toggled_clone() {
        let s = ZNetworkNetworkBandwidthSample::new();
        let t = s.toggled_clone();
        let _ = t.peak_bps;
    }

    #[test]
    fn z_network_rolling_hash_deterministic() {
        let h1 = z_network_rolling_hash(b"test");
        let h2 = z_network_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_network_rolling_hash(b"a"), z_network_rolling_hash(b"b"));
    }

    #[test]
    fn z_network_pad_to_basic() {
        assert_eq!(z_network_pad_to("hi", 5), "hi   ");
        assert_eq!(z_network_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_network_is_identifier_basic() {
        assert!(z_network_is_identifier("foo_bar"));
        assert!(z_network_is_identifier("abc123"));
        assert!(!z_network_is_identifier(""));
        assert!(!z_network_is_identifier("has space"));
    }

    #[test]
    fn z_network_levenshtein_basic() {
        assert_eq!(z_network_levenshtein("", ""), 0);
        assert_eq!(z_network_levenshtein("abc", "abc"), 0);
        assert_eq!(z_network_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_network_unique_words_basic() {
        let w = z_network_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_network_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_network_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_network_common_prefix_basic() {
        assert_eq!(z_network_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_network_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_network_struct_clear() {
        let mut s = ZNetworkNetworkBandwidthSample::new();
        s.samples_bps.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_network_rolling_hash_empty() {
        let h = z_network_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_63_push_and_len() {
        let mut rb = super::XbRingBuffer63::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_63_overwrite() {
        let mut rb = super::XbRingBuffer63::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_63_get_out_of_bounds() {
        let rb = super::XbRingBuffer63::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_63_drain_all() {
        let mut rb = super::XbRingBuffer63::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_63_peek_front_back() {
        let mut rb = super::XbRingBuffer63::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_63_clear() {
        let mut rb = super::XbRingBuffer63::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_63_capacity() {
        let rb = super::XbRingBuffer63::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_63_basic() {
        let h = super::xb_fnv1a_63(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_63(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_63_different_inputs() {
        let h1 = super::xb_fnv1a_63(b"abc");
        let h2 = super::xb_fnv1a_63(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_63_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_63(&data);
        let dec = super::xb_rle_decode_63(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_63_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_63(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_63(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_63_values() {
        assert!((super::xb_clamp_63(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_63(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_63(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_63_values() {
        assert!((super::xb_lerp_63(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_63(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_63(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_63_wrap_around_twice() {
        let mut rb = super::XbRingBuffer63::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 127 ----

    #[test]
    fn xc_127_pool_new_empty() {
        let pool: super::Xc127Pool<i32> = super::Xc127Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_127_pool_release_acquire() {
        let mut pool = super::Xc127Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_127_pool_acquire_empty() {
        let mut pool: super::Xc127Pool<i32> = super::Xc127Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_127_pool_full() {
        let mut pool = super::Xc127Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_127_pool_drain() {
        let mut pool = super::Xc127Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_127_pool_stats() {
        let mut pool = super::Xc127Pool::new(8);
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
    fn xc_127_pool_clear() {
        let mut pool = super::Xc127Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_127_pool_shrink() {
        let mut pool = super::Xc127Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_127_pool_default() {
        let pool: super::Xc127Pool<String> = super::Xc127Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_127_pool_extend() {
        let mut pool = super::Xc127Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_127_pool_retain() {
        let mut pool = super::Xc127Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_127_scheduler_round_robin() {
        let mut sched = super::Xc127Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_127_scheduler_empty() {
        let mut sched = super::Xc127Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_127_scheduler_reset() {
        let mut sched = super::Xc127Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_127_scheduler_add_remove() {
        let mut sched = super::Xc127Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_127_scheduler_targets() {
        let sched = super::Xc127Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_127_hash_empty() {
        assert_eq!(super::xc_127_hash(b""), 5381);
    }

    #[test]
    fn xc_127_hash_data() {
        let h = super::xc_127_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_127_hash(b"hello"), h);
    }

    #[test]
    fn xc_127_reverse_str() {
        assert_eq!(super::xc_127_reverse("abc"), "cba");
        assert_eq!(super::xc_127_reverse(""), "");
    }


    #[test]
    fn xe_76_pipeline_empty() {
        let p = super::Xe76Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_76_pipeline_parse_stage() {
        let p = super::Xe76Pipeline::new()
            .add_parse(super::xe_76_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_76_pipeline_transform_double() {
        let p = super::Xe76Pipeline::new()
            .add_transform(super::xe_76_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_76_pipeline_validate_reverse() {
        let p = super::Xe76Pipeline::new()
            .add_validate(super::xe_76_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_76_pipeline_emit_filter() {
        let p = super::Xe76Pipeline::new()
            .add_emit(super::xe_76_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_76_pipeline_multi_stage() {
        let p = super::Xe76Pipeline::new()
            .add_parse(super::xe_76_pipeline_identity)
            .add_transform(super::xe_76_pipeline_double)
            .add_validate(super::xe_76_pipeline_reverse)
            .add_emit(super::xe_76_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_76_pipeline_error_propagation() {
        let p = super::Xe76Pipeline::new()
            .add_parse(super::xe_76_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe76Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_76_pipeline_compose() {
        let p1 = super::Xe76Pipeline::new()
            .add_parse(super::xe_76_pipeline_identity);
        let p2 = super::Xe76Pipeline::new()
            .add_transform(super::xe_76_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_76_pipeline_error_display() {
        let e = super::Xe76PipelineError {
            stage: super::Xe76Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_76_cache_put_get() {
        let mut c = super::Xe76Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_76_cache_miss() {
        let mut c: super::Xe76Cache<&str, i32> = super::Xe76Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_76_cache_ttl_expiry() {
        let mut c = super::Xe76Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_76_cache_evict() {
        let mut c = super::Xe76Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_76_cache_capacity() {
        let mut c = super::Xe76Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_76_cache_stats() {
        let mut c = super::Xe76Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_76_cache_clear() {
        let mut c = super::Xe76Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}