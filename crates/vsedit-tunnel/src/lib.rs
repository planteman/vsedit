//! Dev tunnel management.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during tunnel operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TunnelError {
    /// The referenced tunnel does not exist.
    NotFound(String),
    /// A tunnel with the given name already exists.
    DuplicateName(String),
    /// The requested port is already forwarded on this tunnel.
    DuplicatePort { tunnel_id: String, port: u16 },
    /// A port number of zero is not valid.
    InvalidPort,
    /// The tunnel name is empty or contains invalid characters.
    InvalidName(String),
    /// The tunnel is not in the expected state for this operation.
    InvalidState { expected: TunnelStatus, actual: TunnelStatus },
    /// Connection negotiation failed.
    ConnectionFailed(String),
}

impl fmt::Display for TunnelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelError::NotFound(id) => write!(f, "tunnel not found: {id}"),
            TunnelError::DuplicateName(n) => write!(f, "duplicate tunnel name: {n}"),
            TunnelError::DuplicatePort { tunnel_id, port } => {
                write!(f, "port {port} already forwarded on tunnel {tunnel_id}")
            }
            TunnelError::InvalidPort => write!(f, "port number must be non-zero"),
            TunnelError::InvalidName(reason) => write!(f, "invalid tunnel name: {reason}"),
            TunnelError::InvalidState { expected, actual } => {
                write!(f, "expected state {expected}, got {actual}")
            }
            TunnelError::ConnectionFailed(reason) => write!(f, "connection failed: {reason}"),
        }
    }
}

impl std::error::Error for TunnelError {}

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl fmt::Display for TunnelStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelStatus::Disconnected => write!(f, "Disconnected"),
            TunnelStatus::Connecting => write!(f, "Connecting"),
            TunnelStatus::Connected => write!(f, "Connected"),
            TunnelStatus::Error(msg) => write!(f, "Error: {msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelAccess {
    Private,
    Organization,
    Public,
}

impl fmt::Display for TunnelAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelAccess::Private => write!(f, "Private"),
            TunnelAccess::Organization => write!(f, "Organization"),
            TunnelAccess::Public => write!(f, "Public"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TunnelPort {
    pub port: u16,
    pub protocol: String,
    pub label: Option<String>,
}

impl fmt::Display for TunnelPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.label {
            Some(lbl) => write!(f, "{}:{} ({})", self.protocol, self.port, lbl),
            None => write!(f, "{}:{}", self.protocol, self.port),
        }
    }
}

impl TunnelPort {
    /// Validate that the port configuration is usable.
    pub fn validate(&self) -> Result<(), TunnelError> {
        if self.port == 0 {
            return Err(TunnelError::InvalidPort);
        }
        Ok(())
    }

    /// Returns `true` if this port uses a secure protocol.
    pub fn is_secure(&self) -> bool {
        matches!(self.protocol.as_str(), "https" | "ssh" | "tls")
    }
}

// ---------------------------------------------------------------------------
// TunnelPort builder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`TunnelPort`] with validation.
#[derive(Debug, Clone)]
pub struct TunnelPortBuilder {
    port: u16,
    protocol: String,
    label: Option<String>,
}

impl TunnelPortBuilder {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            protocol: "https".to_string(),
            label: None,
        }
    }

    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = protocol.into();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Build the `TunnelPort`, returning an error if validation fails.
    pub fn build(self) -> Result<TunnelPort, TunnelError> {
        let tp = TunnelPort {
            port: self.port,
            protocol: self.protocol,
            label: self.label,
        };
        tp.validate()?;
        Ok(tp)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TunnelInfo {
    pub id: String,
    pub name: String,
    pub status: TunnelStatus,
    pub access: TunnelAccess,
    pub uri: Option<String>,
    pub ports: Vec<TunnelPort>,
}

impl TunnelInfo {
    /// Returns `true` when the tunnel is actively connected.
    pub fn is_connected(&self) -> bool {
        self.status == TunnelStatus::Connected
    }

    /// Returns `true` when the tunnel is in an error state.
    pub fn is_error(&self) -> bool {
        matches!(self.status, TunnelStatus::Error(_))
    }

    /// Count of forwarded ports.
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    /// Returns all secure ports forwarded on this tunnel.
    pub fn secure_ports(&self) -> Vec<&TunnelPort> {
        self.ports.iter().filter(|p| p.is_secure()).collect()
    }

    /// Validate the tunnel name (non-empty, ASCII alphanumeric / hyphens only).
    pub fn validate_name(name: &str) -> Result<(), TunnelError> {
        if name.is_empty() {
            return Err(TunnelError::InvalidName("name must not be empty".into()));
        }
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(TunnelError::InvalidName(
                "name must contain only ASCII alphanumerics, hyphens, or underscores".into(),
            ));
        }
        Ok(())
    }
}

impl fmt::Display for TunnelInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.status)
    }
}

pub struct TunnelService {
    tunnels: Vec<TunnelInfo>,
    next_id: u64,
}

impl TunnelService {
    pub fn new() -> Self {
        Self {
            tunnels: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_tunnel(&mut self, name: impl Into<String>, access: TunnelAccess) -> String {
        let id = format!("tunnel-{}", self.next_id);
        self.next_id += 1;
        self.tunnels.push(TunnelInfo {
            id: id.clone(),
            name: name.into(),
            status: TunnelStatus::Disconnected,
            access,
            uri: None,
            ports: Vec::new(),
        });
        id
    }

    pub fn connect(&mut self, id: &str) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.status = TunnelStatus::Connected;
            true
        } else {
            false
        }
    }

    pub fn disconnect(&mut self, id: &str) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.status = TunnelStatus::Disconnected;
        }
    }

    pub fn get_tunnel(&self, id: &str) -> Option<&TunnelInfo> {
        self.tunnels.iter().find(|t| t.id == id)
    }

    pub fn remove_tunnel(&mut self, id: &str) -> bool {
        let len = self.tunnels.len();
        self.tunnels.retain(|t| t.id != id);
        self.tunnels.len() < len
    }

    pub fn active_count(&self) -> usize {
        self.tunnels
            .iter()
            .filter(|t| t.status == TunnelStatus::Connected)
            .count()
    }

    pub fn add_port(&mut self, tunnel_id: &str, port: TunnelPort) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == tunnel_id) {
            if t.ports.iter().any(|p| p.port == port.port) {
                return false;
            }
            t.ports.push(port);
            true
        } else {
            false
        }
    }

    pub fn remove_port(&mut self, tunnel_id: &str, port_num: u16) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == tunnel_id) {
            let len = t.ports.len();
            t.ports.retain(|p| p.port != port_num);
            t.ports.len() < len
        } else {
            false
        }
    }

    pub fn get_all_tunnels(&self) -> &[TunnelInfo] {
        &self.tunnels
    }

    pub fn tunnel_count(&self) -> usize {
        self.tunnels.len()
    }

    pub fn set_error(&mut self, id: &str, message: String) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.status = TunnelStatus::Error(message);
        }
    }

    pub fn set_uri(&mut self, id: &str, uri: String) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.uri = Some(uri);
        }
    }

    pub fn find_by_name(&self, name: &str) -> Option<&TunnelInfo> {
        self.tunnels.iter().find(|t| t.name == name)
    }

    pub fn disconnect_all(&mut self) {
        for t in &mut self.tunnels {
            if t.status == TunnelStatus::Connected || t.status == TunnelStatus::Connecting {
                t.status = TunnelStatus::Disconnected;
            }
        }
    }

    /// Create a tunnel with name validation, rejecting duplicates.
    pub fn create_validated(
        &mut self,
        name: impl Into<String>,
        access: TunnelAccess,
    ) -> Result<String, TunnelError> {
        let name = name.into();
        TunnelInfo::validate_name(&name)?;
        if self.tunnels.iter().any(|t| t.name == name) {
            return Err(TunnelError::DuplicateName(name));
        }
        Ok(self.create_tunnel(name, access))
    }

    /// Add a port with full validation, returning a typed error.
    pub fn add_port_validated(
        &mut self,
        tunnel_id: &str,
        port: TunnelPort,
    ) -> Result<(), TunnelError> {
        port.validate()?;
        let tunnel = self
            .tunnels
            .iter_mut()
            .find(|t| t.id == tunnel_id)
            .ok_or_else(|| TunnelError::NotFound(tunnel_id.to_string()))?;
        if tunnel.ports.iter().any(|p| p.port == port.port) {
            return Err(TunnelError::DuplicatePort {
                tunnel_id: tunnel_id.to_string(),
                port: port.port,
            });
        }
        tunnel.ports.push(port);
        Ok(())
    }

    /// Connect a tunnel only if it is currently disconnected.
    pub fn connect_checked(&mut self, id: &str) -> Result<(), TunnelError> {
        let tunnel = self
            .tunnels
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TunnelError::NotFound(id.to_string()))?;
        if tunnel.status != TunnelStatus::Disconnected {
            return Err(TunnelError::InvalidState {
                expected: TunnelStatus::Disconnected,
                actual: tunnel.status.clone(),
            });
        }
        tunnel.status = TunnelStatus::Connected;
        Ok(())
    }

    /// Return tunnels filtered by access level.
    pub fn tunnels_by_access(&self, access: &TunnelAccess) -> Vec<&TunnelInfo> {
        self.tunnels.iter().filter(|t| &t.access == access).collect()
    }

    /// Summary string listing every tunnel and its status.
    pub fn summary(&self) -> String {
        if self.tunnels.is_empty() {
            return "No tunnels configured.".to_string();
        }
        self.tunnels
            .iter()
            .map(|t| format!("  {} ({}) [{}] - {} ({} ports)", t.id, t.name, t.access, t.status, t.port_count()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Debug for TunnelService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TunnelService")
            .field("tunnel_count", &self.tunnels.len())
            .field("next_id", &self.next_id)
            .finish()
    }
}

impl fmt::Display for TunnelService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TunnelService({} tunnels, {} active)",
            self.tunnel_count(),
            self.active_count()
        )
    }
}

impl Default for TunnelService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for tunnel operations.
#[derive(Debug, Clone, PartialEq)]
pub struct TunnelStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl TunnelStats {
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
    pub fn merge(&mut self, other: &TunnelStats) {
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

impl Default for TunnelStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TunnelStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TunnelStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for tunnel.
#[derive(Debug, Clone)]
pub struct TunnelValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl TunnelValidator {
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

impl Default for TunnelValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

/// Status of a health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Monitors tunnel connection health over a sliding window of latency samples.
pub struct TunnelHealthCheck {
    samples: Vec<u64>,
    max_samples: usize,
    degraded_threshold_ms: u64,
    unhealthy_threshold_ms: u64,
    consecutive_failures: u32,
    failure_limit: u32,
}

impl TunnelHealthCheck {
    pub fn new(max_samples: usize, degraded_ms: u64, unhealthy_ms: u64) -> Self {
        Self {
            samples: Vec::new(),
            max_samples,
            degraded_threshold_ms: degraded_ms,
            unhealthy_threshold_ms: unhealthy_ms,
            consecutive_failures: 0,
            failure_limit: 3,
        }
    }

    pub fn with_failure_limit(mut self, limit: u32) -> Self {
        self.failure_limit = limit;
        self
    }

    pub fn record_latency(&mut self, latency_ms: u64) {
        self.consecutive_failures = 0;
        if self.samples.len() >= self.max_samples {
            self.samples.remove(0);
        }
        self.samples.push(latency_ms);
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
    }

    pub fn average_latency(&self) -> Option<u64> {
        if self.samples.is_empty() {
            return None;
        }
        Some(self.samples.iter().sum::<u64>() / self.samples.len() as u64)
    }

    pub fn status(&self) -> HealthStatus {
        if self.consecutive_failures >= self.failure_limit {
            return HealthStatus::Unhealthy;
        }
        match self.average_latency() {
            None => HealthStatus::Healthy,
            Some(avg) if avg >= self.unhealthy_threshold_ms => HealthStatus::Unhealthy,
            Some(avg) if avg >= self.degraded_threshold_ms => HealthStatus::Degraded,
            _ => HealthStatus::Healthy,
        }
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn reset(&mut self) {
        self.samples.clear();
        self.consecutive_failures = 0;
    }
}

// ---------------------------------------------------------------------------
// Tunnel iteration & Display helpers
// ---------------------------------------------------------------------------

impl TunnelService {
    /// Iterate over all tunnels.
    pub fn iter(&self) -> std::slice::Iter<'_, TunnelInfo> {
        self.tunnels.iter()
    }

    /// Returns all tunnel names.
    pub fn tunnel_names(&self) -> Vec<&str> {
        self.tunnels.iter().map(|t| t.name.as_str()).collect()
    }

    /// Returns tunnels that are in error state.
    pub fn errored_tunnels(&self) -> Vec<&TunnelInfo> {
        self.tunnels.iter().filter(|t| t.is_error()).collect()
    }

    /// Returns the total number of ports across all tunnels.
    pub fn total_port_count(&self) -> usize {
        self.tunnels.iter().map(|t| t.port_count()).sum()
    }

    /// Returns true if any tunnel has an error.
    pub fn has_errors(&self) -> bool {
        self.tunnels.iter().any(|t| t.is_error())
    }
}

impl TunnelAccess {
    /// Returns true if this access level allows public access.
    pub fn is_public(&self) -> bool {
        matches!(self, TunnelAccess::Public)
    }

    /// Returns a short label for display.
    pub fn short_label(&self) -> &'static str {
        match self {
            TunnelAccess::Private => "priv",
            TunnelAccess::Public => "pub",
            TunnelAccess::Organization => "org",
        }
    }
}

impl TunnelStatus {
    /// Returns true if the tunnel is in a connectable state.
    pub fn is_connectable(&self) -> bool {
        matches!(self, TunnelStatus::Connected)
    }

    /// Returns true if the tunnel is in a terminal state (disconnected or error).
    pub fn is_terminal(&self) -> bool {
        matches!(self, TunnelStatus::Disconnected | TunnelStatus::Error(_))
    }
}

impl TunnelInfo {
    /// Returns a short status line for display.
    pub fn status_line(&self) -> String {
        format!(
            "{}: {} ({}, {} ports)",
            self.name,
            self.status,
            self.access,
            self.port_count(),
        )
    }

    /// Find a port by number.
    pub fn find_port(&self, port_num: u16) -> Option<&TunnelPort> {
        self.ports.iter().find(|p| p.port == port_num)
    }

    /// Returns true if any port uses HTTPS.
    pub fn has_secure_port(&self) -> bool {
        self.ports.iter().any(|p| p.is_secure())
    }
}

impl TunnelHealthCheck {
    /// Returns the minimum recorded latency.
    pub fn min_latency(&self) -> Option<u64> {
        self.samples.iter().copied().min()
    }

    /// Returns the maximum recorded latency.
    pub fn max_latency(&self) -> Option<u64> {
        self.samples.iter().copied().max()
    }
}

// ---------------------------------------------------------------------------
// Tunnel address resolution
// ---------------------------------------------------------------------------

/// Represents a resolved tunnel address with host and port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelAddress {
    pub host: String,
    pub port: u16,
    pub scheme: String,
}

impl TunnelAddress {
    /// Parse a URI-like string into a TunnelAddress.
    /// Accepted formats: `scheme://host:port`, `host:port`, `host`.
    pub fn parse(uri: &str) -> Option<Self> {
        let (scheme, rest) = if let Some(idx) = uri.find("://") {
            (uri[..idx].to_string(), &uri[idx + 3..])
        } else {
            ("https".to_string(), uri)
        };
        let (host, port) = if let Some(idx) = rest.rfind(':') {
            let port_str = &rest[idx + 1..];
            match port_str.parse::<u16>() {
                Ok(p) if p > 0 => (rest[..idx].to_string(), p),
                _ => return None,
            }
        } else {
            let default_port = match scheme.as_str() {
                "http" => 80,
                "https" => 443,
                "ssh" => 22,
                _ => return None,
            };
            (rest.to_string(), default_port)
        };
        if host.is_empty() {
            return None;
        }
        Some(TunnelAddress { host, port, scheme })
    }

    /// Reconstruct the address as a URI string.
    pub fn to_uri(&self) -> String {
        format!("{}://{}:{}", self.scheme, self.host, self.port)
    }

    /// Returns true if using a secure scheme (https, ssh, tls).
    pub fn is_secure(&self) -> bool {
        matches!(self.scheme.as_str(), "https" | "ssh" | "tls")
    }
}

impl fmt::Display for TunnelAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uri())
    }
}

// ---------------------------------------------------------------------------
// Tunnel connection log
// ---------------------------------------------------------------------------

/// Records tunnel connection events for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelConnectionEvent {
    pub tunnel_id: String,
    pub event_type: TunnelEventType,
    pub timestamp: u64,
    pub message: Option<String>,
}

/// Type of tunnel connection event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelEventType {
    Connected,
    Disconnected,
    Reconnecting,
    Error,
}

impl fmt::Display for TunnelEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connected => write!(f, "connected"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::Reconnecting => write!(f, "reconnecting"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Collects connection events for all tunnels.
#[derive(Debug, Clone, Default)]
pub struct TunnelConnectionLog {
    events: Vec<TunnelConnectionEvent>,
}

impl TunnelConnectionLog {
    /// Create a new empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a connection event.
    pub fn record(
        &mut self,
        tunnel_id: &str,
        event_type: TunnelEventType,
        timestamp: u64,
        message: Option<&str>,
    ) {
        self.events.push(TunnelConnectionEvent {
            tunnel_id: tunnel_id.to_string(),
            event_type,
            timestamp,
            message: message.map(|s| s.to_string()),
        });
    }

    /// Get all events for a specific tunnel, ordered by timestamp.
    pub fn events_for_tunnel(&self, tunnel_id: &str) -> Vec<&TunnelConnectionEvent> {
        self.events
            .iter()
            .filter(|e| e.tunnel_id == tunnel_id)
            .collect()
    }

    /// Get the last event for a tunnel.
    pub fn last_event(&self, tunnel_id: &str) -> Option<&TunnelConnectionEvent> {
        self.events.iter().rev().find(|e| e.tunnel_id == tunnel_id)
    }

    /// Count total events.
    pub fn total_events(&self) -> usize {
        self.events.len()
    }

    /// Count error events.
    pub fn error_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.event_type == TunnelEventType::Error)
            .count()
    }
}

// ---------------------------------------------------------------------------
// Tunnel bandwidth statistics
// ---------------------------------------------------------------------------

/// Accumulates bytes transferred per tunnel for bandwidth reporting.
#[derive(Debug, Clone, Default)]
pub struct TunnelBandwidthStats {
    bytes_in: std::collections::HashMap<String, u64>,
    bytes_out: std::collections::HashMap<String, u64>,
}

impl TunnelBandwidthStats {
    /// Create a new stats accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record inbound bytes for a tunnel.
    pub fn record_inbound(&mut self, tunnel_id: &str, bytes: u64) {
        *self.bytes_in.entry(tunnel_id.to_string()).or_insert(0) += bytes;
    }

    /// Record outbound bytes for a tunnel.
    pub fn record_outbound(&mut self, tunnel_id: &str, bytes: u64) {
        *self.bytes_out.entry(tunnel_id.to_string()).or_insert(0) += bytes;
    }

    /// Get total inbound bytes for a tunnel.
    pub fn inbound(&self, tunnel_id: &str) -> u64 {
        self.bytes_in.get(tunnel_id).copied().unwrap_or(0)
    }

    /// Get total outbound bytes for a tunnel.
    pub fn outbound(&self, tunnel_id: &str) -> u64 {
        self.bytes_out.get(tunnel_id).copied().unwrap_or(0)
    }

    /// Total bytes (in + out) across all tunnels.
    pub fn total_bytes(&self) -> u64 {
        let total_in: u64 = self.bytes_in.values().sum();
        let total_out: u64 = self.bytes_out.values().sum();
        total_in + total_out
    }
}

// ---------------------------------------------------------------------------
// TunnelPortMapping — maps local ports to remote ports
// ---------------------------------------------------------------------------

/// Maps a local port to a remote port with an optional label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelPortMapping {
    pub local_port: u16,
    pub remote_port: u16,
    pub label: Option<String>,
    pub protocol: PortProtocol,
}

/// Protocol used by a port mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortProtocol {
    Http,
    Https,
    Tcp,
}

impl fmt::Display for PortProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortProtocol::Http => write!(f, "http"),
            PortProtocol::Https => write!(f, "https"),
            PortProtocol::Tcp => write!(f, "tcp"),
        }
    }
}

impl TunnelPortMapping {
    pub fn new(local_port: u16, remote_port: u16, protocol: PortProtocol) -> Self {
        Self {
            local_port,
            remote_port,
            label: None,
            protocol,
        }
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Generate the full URL for this mapping.
    pub fn url(&self, host: &str) -> String {
        format!("{}://{}:{}", self.protocol, host, self.remote_port)
    }

    /// Check if this is an identity mapping (local == remote).
    pub fn is_identity(&self) -> bool {
        self.local_port == self.remote_port
    }
}

impl fmt::Display for TunnelPortMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.label {
            Some(lbl) => write!(f, "{} ({}) {} → {}", lbl, self.protocol, self.local_port, self.remote_port),
            None => write!(f, "{} {} → {}", self.protocol, self.local_port, self.remote_port),
        }
    }
}

// ---------------------------------------------------------------------------
// TunnelPortRouter — route requests to the correct port mapping
// ---------------------------------------------------------------------------

/// Manages a collection of port mappings for a tunnel.
#[derive(Debug, Clone, Default)]
pub struct TunnelPortRouter {
    mappings: Vec<TunnelPortMapping>,
}

impl TunnelPortRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a port mapping.
    pub fn add_mapping(&mut self, mapping: TunnelPortMapping) {
        self.mappings.push(mapping);
    }

    /// Look up the remote port for a given local port.
    pub fn resolve_local(&self, local_port: u16) -> Option<u16> {
        self.mappings
            .iter()
            .find(|m| m.local_port == local_port)
            .map(|m| m.remote_port)
    }

    /// Look up the local port for a given remote port.
    pub fn resolve_remote(&self, remote_port: u16) -> Option<u16> {
        self.mappings
            .iter()
            .find(|m| m.remote_port == remote_port)
            .map(|m| m.local_port)
    }

    /// Return all mappings using a specific protocol.
    pub fn by_protocol(&self, protocol: PortProtocol) -> Vec<&TunnelPortMapping> {
        self.mappings.iter().filter(|m| m.protocol == protocol).collect()
    }

    /// Check if a local port is already mapped.
    pub fn has_local_port(&self, port: u16) -> bool {
        self.mappings.iter().any(|m| m.local_port == port)
    }

    /// Remove mapping by local port. Returns true if found.
    pub fn remove_by_local_port(&mut self, port: u16) -> bool {
        let before = self.mappings.len();
        self.mappings.retain(|m| m.local_port != port);
        self.mappings.len() < before
    }

    /// Total number of mappings.
    pub fn count(&self) -> usize {
        self.mappings.len()
    }
}

// ---------------------------------------------------------------------------
// Port range validation
// ---------------------------------------------------------------------------

/// Well-known port range boundaries.
pub const WELL_KNOWN_PORT_MAX: u16 = 1023;
pub const REGISTERED_PORT_MIN: u16 = 1024;
pub const REGISTERED_PORT_MAX: u16 = 49151;
pub const DYNAMIC_PORT_MIN: u16 = 49152;

/// Classifies a port number into its IANA range category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortCategory {
    /// Ports 0 (invalid) — rejected elsewhere.
    Invalid,
    /// Ports 1–1023: well-known / system ports.
    WellKnown,
    /// Ports 1024–49151: registered / user ports.
    Registered,
    /// Ports 49152–65535: dynamic / ephemeral ports.
    Dynamic,
}

impl fmt::Display for PortCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => write!(f, "invalid"),
            Self::WellKnown => write!(f, "well-known"),
            Self::Registered => write!(f, "registered"),
            Self::Dynamic => write!(f, "dynamic"),
        }
    }
}

/// Classify a port number into its IANA category.
pub fn classify_port(port: u16) -> PortCategory {
    match port {
        0 => PortCategory::Invalid,
        1..=WELL_KNOWN_PORT_MAX => PortCategory::WellKnown,
        REGISTERED_PORT_MIN..=REGISTERED_PORT_MAX => PortCategory::Registered,
        _ => PortCategory::Dynamic,
    }
}

/// Returns `true` if the port is in the privileged / well-known range (1–1023).
pub fn is_privileged_port(port: u16) -> bool {
    port >= 1 && port <= WELL_KNOWN_PORT_MAX
}

/// Returns `true` if `port` is a commonly used development port.
pub fn is_common_dev_port(port: u16) -> bool {
    matches!(
        port,
        3000 | 3001 | 4200 | 5000 | 5173 | 5174 | 8000 | 8080 | 8443 | 8888 | 9000
    )
}

// ---------------------------------------------------------------------------
// Protocol detection from port number
// ---------------------------------------------------------------------------

/// Detect the most likely protocol for a given port number.
pub fn detect_protocol(port: u16) -> &'static str {
    match port {
        22 => "ssh",
        80 | 8080 | 8000 | 3000 | 5000 => "http",
        443 | 8443 => "https",
        3306 => "mysql",
        5432 => "postgres",
        6379 => "redis",
        27017 => "mongodb",
        _ => "tcp",
    }
}

// ---------------------------------------------------------------------------
// URL construction for forwarded ports
// ---------------------------------------------------------------------------

/// Build a forwarded-port URL from a tunnel's base URI and a port number.
///
/// If `base_uri` is `https://abc.devtunnels.ms` and `port` is 3000 the result
/// is `https://abc-3000.devtunnels.ms`.  When the base URI has no recognisable
/// host component we fall back to `{base_uri}:{port}`.
pub fn forwarded_port_url(base_uri: &str, port: u16) -> String {
    // Strip trailing slash
    let base = base_uri.trim_end_matches('/');

    // Try to split on "://" to get scheme + host
    if let Some(idx) = base.find("://") {
        let scheme = &base[..idx];
        let host = &base[idx + 3..];
        // Insert port as a sub-domain style label: host → host-PORT
        if let Some(dot_idx) = host.find('.') {
            let subdomain = &host[..dot_idx];
            let rest = &host[dot_idx..];
            return format!("{scheme}://{subdomain}-{port}{rest}");
        }
        return format!("{scheme}://{host}:{port}");
    }
    format!("{base}:{port}")
}

// ---------------------------------------------------------------------------
// Tunnel label formatting
// ---------------------------------------------------------------------------

/// Format a human-readable label for a tunnel port entry.
///
/// Examples:
/// - `format_port_label(8080, Some("Web UI"))` → `"Web UI (:8080)"`
/// - `format_port_label(3000, None)`            → `"Port 3000"`
pub fn format_port_label(port: u16, label: Option<&str>) -> String {
    match label {
        Some(lbl) => format!("{lbl} (:{port})"),
        None => format!("Port {port}"),
    }
}

/// Format a compact tunnel summary suitable for status-bar display.
pub fn format_tunnel_badge(name: &str, status: &TunnelStatus, port_count: usize) -> String {
    let icon = match status {
        TunnelStatus::Connected => "●",
        TunnelStatus::Connecting => "◌",
        TunnelStatus::Disconnected => "○",
        TunnelStatus::Error(_) => "✖",
    };
    if port_count == 0 {
        format!("{icon} {name}")
    } else {
        format!("{icon} {name} [{port_count} port{s}]", s = if port_count == 1 { "" } else { "s" })
    }
}

// ---------------------------------------------------------------------------
// Port conflict detection
// ---------------------------------------------------------------------------

/// Given a set of existing allocated ports, find the first available port
/// starting from `start` (inclusive).  Returns `None` if the entire range
/// up to `u16::MAX` is exhausted.
pub fn find_available_port(allocated: &[u16], start: u16) -> Option<u16> {
    let mut candidate = start;
    loop {
        if candidate == 0 {
            candidate = 1;
        }
        if !allocated.contains(&candidate) {
            return Some(candidate);
        }
        if candidate == u16::MAX {
            return None;
        }
        candidate += 1;
    }
}

/// Check all tunnels in a `TunnelService` for port conflicts (same port
/// forwarded on more than one tunnel).  Returns a vec of `(port, Vec<tunnel_id>)`.
pub fn detect_port_conflicts(service: &TunnelService) -> Vec<(u16, Vec<String>)> {
    let mut port_owners: std::collections::HashMap<u16, Vec<String>> =
        std::collections::HashMap::new();

    for tunnel in service.get_all_tunnels() {
        for p in &tunnel.ports {
            port_owners
                .entry(p.port)
                .or_default()
                .push(tunnel.id.clone());
        }
    }

    let mut conflicts: Vec<(u16, Vec<String>)> = port_owners
        .into_iter()
        .filter(|(_, owners)| owners.len() > 1)
        .collect();
    conflicts.sort_by_key(|(port, _)| *port);
    conflicts
}

// ---------------------------------------------------------------------------
// Connection pool
// ---------------------------------------------------------------------------

/// A simple bounded pool of reusable connection slots identified by ID.
#[derive(Debug)]
pub struct ConnectionPool {
    capacity: usize,
    active: Vec<String>,
}

impl ConnectionPool {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            active: Vec::new(),
        }
    }

    /// Try to acquire a slot.  Returns `true` if the connection was accepted.
    pub fn acquire(&mut self, id: impl Into<String>) -> bool {
        if self.active.len() >= self.capacity {
            return false;
        }
        let id = id.into();
        if self.active.contains(&id) {
            return false; // already acquired
        }
        self.active.push(id);
        true
    }

    /// Release a previously acquired slot.
    pub fn release(&mut self, id: &str) -> bool {
        let before = self.active.len();
        self.active.retain(|s| s != id);
        self.active.len() < before
    }

    /// Returns `true` if the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.active.len() >= self.capacity
    }

    /// Number of active connections.
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Number of remaining available slots.
    pub fn available(&self) -> usize {
        self.capacity.saturating_sub(self.active.len())
    }

    /// Check whether a specific ID is currently held.
    pub fn contains(&self, id: &str) -> bool {
        self.active.iter().any(|s| s == id)
    }

    /// Drain all active connections, returning them.
    pub fn drain_all(&mut self) -> Vec<String> {
        std::mem::take(&mut self.active)
    }
}

impl fmt::Display for ConnectionPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ConnectionPool({}/{} active)",
            self.active.len(),
            self.capacity
        )
    }
}


// ---------------------------------------------------------------------------
// Tunnel metrics collector
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TunnelMetrics {
    pub tunnel_id: String, pub bytes_sent: u64, pub bytes_received: u64,
    pub packets_sent: u64, pub packets_received: u64,
    pub latency_samples_ms: Vec<u64>, pub errors: u64,
}
impl TunnelMetrics {
    pub fn new(id: impl Into<String>) -> Self { Self { tunnel_id: id.into(), bytes_sent: 0, bytes_received: 0, packets_sent: 0, packets_received: 0, latency_samples_ms: Vec::new(), errors: 0 } }
    pub fn record_send(&mut self, b: u64) { self.bytes_sent += b; self.packets_sent += 1; }
    pub fn record_recv(&mut self, b: u64) { self.bytes_received += b; self.packets_received += 1; }
    pub fn record_latency(&mut self, ms: u64) { self.latency_samples_ms.push(ms); }
    pub fn record_error(&mut self) { self.errors += 1; }
    pub fn avg_latency(&self) -> f64 { if self.latency_samples_ms.is_empty() { 0.0 } else { self.latency_samples_ms.iter().sum::<u64>() as f64 / self.latency_samples_ms.len() as f64 } }
    pub fn max_latency(&self) -> u64 { self.latency_samples_ms.iter().copied().max().unwrap_or(0) }
    pub fn min_latency(&self) -> u64 { self.latency_samples_ms.iter().copied().min().unwrap_or(0) }
    pub fn total_bytes(&self) -> u64 { self.bytes_sent + self.bytes_received }
    pub fn total_packets(&self) -> u64 { self.packets_sent + self.packets_received }
    pub fn error_rate(&self) -> f64 { let t = self.total_packets(); if t == 0 { 0.0 } else { self.errors as f64 / t as f64 } }
    pub fn reset(&mut self) { self.bytes_sent = 0; self.bytes_received = 0; self.packets_sent = 0; self.packets_received = 0; self.latency_samples_ms.clear(); self.errors = 0; }
}
impl fmt::Display for TunnelMetrics { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Metrics({}: {}B sent, {}B recv)", self.tunnel_id, self.bytes_sent, self.bytes_received) } }
impl Default for TunnelMetrics { fn default() -> Self { Self::new("default") } }

pub struct TunnelMetricsCollector { metrics: HashMap<String, TunnelMetrics> }
impl TunnelMetricsCollector {
    pub fn new() -> Self { Self { metrics: HashMap::new() } }
    pub fn get_or_create(&mut self, id: &str) -> &mut TunnelMetrics { self.metrics.entry(id.to_string()).or_insert_with(|| TunnelMetrics::new(id)) }
    pub fn get(&self, id: &str) -> Option<&TunnelMetrics> { self.metrics.get(id) }
    pub fn remove(&mut self, id: &str) -> Option<TunnelMetrics> { self.metrics.remove(id) }
    pub fn total_bytes_all(&self) -> u64 { self.metrics.values().map(|m| m.total_bytes()).sum() }
    pub fn tunnel_count(&self) -> usize { self.metrics.len() }
    pub fn clear(&mut self) { self.metrics.clear(); }
}
impl Default for TunnelMetricsCollector { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Tunnel protocol negotiator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TunnelProtocol { WebSocket, Http2, Ssh, Raw }
impl fmt::Display for TunnelProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self { Self::WebSocket => write!(f, "ws"), Self::Http2 => write!(f, "h2"), Self::Ssh => write!(f, "ssh"), Self::Raw => write!(f, "raw") } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiationResult { pub selected: TunnelProtocol, pub client_prefs: Vec<TunnelProtocol>, pub server_supported: Vec<TunnelProtocol> }
impl fmt::Display for NegotiationResult { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Negotiated({})", self.selected) } }

pub struct TunnelProtocolNegotiator { client: Vec<TunnelProtocol> }
impl TunnelProtocolNegotiator {
    pub fn new(c: Vec<TunnelProtocol>) -> Self { Self { client: c } }
    pub fn negotiate(&self, server: &[TunnelProtocol]) -> Result<NegotiationResult, TunnelError> {
        for p in &self.client { if server.contains(p) { return Ok(NegotiationResult { selected: *p, client_prefs: self.client.clone(), server_supported: server.to_vec() }); } }
        Err(TunnelError::ConnectionFailed("no common protocol".into()))
    }
    pub fn supports(&self, p: TunnelProtocol) -> bool { self.client.contains(&p) }
    pub fn add(&mut self, p: TunnelProtocol) { if !self.client.contains(&p) { self.client.push(p); } }
    pub fn remove(&mut self, p: TunnelProtocol) { self.client.retain(|x| *x != p); }
    pub fn count(&self) -> usize { self.client.len() }
}
impl Default for TunnelProtocolNegotiator { fn default() -> Self { Self::new(vec![TunnelProtocol::WebSocket, TunnelProtocol::Http2]) } }


// ---------------------------------------------------------------------------
// TunnelMetricsConfig — configuration for TunnelMetrics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TunnelMetricsConfig {
    pub max_entries: usize,
    pub auto_refresh: bool,
    pub refresh_interval_ms: u64,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl TunnelMetricsConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_refresh(mut self, a: bool) -> Self { self.auto_refresh = a; self }
    pub fn with_refresh_interval(mut self, ms: u64) -> Self { self.refresh_interval_ms = ms; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn is_refresh_due(&self, elapsed_ms: u64) -> bool { self.auto_refresh && elapsed_ms >= self.refresh_interval_ms }
}

impl Default for TunnelMetricsConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_refresh: true, refresh_interval_ms: 5000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for TunnelMetricsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_refresh={}, interval={}ms)", self.max_entries, self.auto_refresh, self.refresh_interval_ms)
    }
}

// ---------------------------------------------------------------------------
// TunnelMetricsCollectorStats — statistics tracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TunnelMetricsCollectorStats {
    pub total_operations: u64,
    pub successful: u64,
    pub failed: u64,
    pub total_duration_ms: u64,
    pub peak_concurrent: usize,
    pub current_concurrent: usize,
}

impl TunnelMetricsCollectorStats {
    pub fn new() -> Self { Self::default() }
    pub fn record_success(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.successful += 1; self.total_duration_ms += duration_ms;
    }
    pub fn record_failure(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.failed += 1; self.total_duration_ms += duration_ms;
    }
    pub fn success_rate(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.successful as f64 / self.total_operations as f64 } }
    pub fn avg_duration_ms(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.total_duration_ms as f64 / self.total_operations as f64 } }
    pub fn update_concurrent(&mut self, current: usize) {
        self.current_concurrent = current;
        if current > self.peak_concurrent { self.peak_concurrent = current; }
    }
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn merge(&mut self, other: &Self) {
        self.total_operations += other.total_operations;
        self.successful += other.successful;
        self.failed += other.failed;
        self.total_duration_ms += other.total_duration_ms;
        if other.peak_concurrent > self.peak_concurrent { self.peak_concurrent = other.peak_concurrent; }
    }
}

impl fmt::Display for TunnelMetricsCollectorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(ops={}, success={:.1}%, avg={:.1}ms)", self.total_operations, self.success_rate() * 100.0, self.avg_duration_ms())
    }
}

// ---------------------------------------------------------------------------
// TunnelMetricsEventKind — event types for TunnelMetrics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelMetricsEventKind {
    Created,
    Updated,
    Deleted,
    Refreshed,
    Error,
}

impl fmt::Display for TunnelMetricsEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Updated => write!(f, "updated"),
            Self::Deleted => write!(f, "deleted"),
            Self::Refreshed => write!(f, "refreshed"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A recorded event in the TunnelMetrics lifecycle.
#[derive(Debug, Clone)]
pub struct TunnelMetricsEvent {
    pub kind: TunnelMetricsEventKind,
    pub timestamp: u64,
    pub detail: Option<String>,
}

impl TunnelMetricsEvent {
    pub fn new(kind: TunnelMetricsEventKind, timestamp: u64) -> Self {
        Self { kind, timestamp, detail: None }
    }
    pub fn with_detail(mut self, d: impl Into<String>) -> Self { self.detail = Some(d.into()); self }
    pub fn is_error(&self) -> bool { self.kind == TunnelMetricsEventKind::Error }
}

impl fmt::Display for TunnelMetricsEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Event({}, t={})", self.kind, self.timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_connect() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("my-tunnel", TunnelAccess::Private);
        assert_eq!(svc.active_count(), 0);
        assert!(svc.connect(&id));
        assert_eq!(svc.active_count(), 1);
    }

    #[test]
    fn disconnect_and_remove() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t1", TunnelAccess::Public);
        svc.connect(&id);
        svc.disconnect(&id);
        assert_eq!(svc.active_count(), 0);
        assert!(svc.remove_tunnel(&id));
        assert!(svc.get_tunnel(&id).is_none());
    }

    #[test]
    fn connect_invalid() {
        let mut svc = TunnelService::new();
        assert!(!svc.connect("nonexistent"));
        assert!(!svc.remove_tunnel("nonexistent"));
    }

    #[test]
    fn add_and_remove_port() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("port-test", TunnelAccess::Private);
        let port = TunnelPort { port: 8080, protocol: "https".into(), label: Some("web".into()) };
        assert!(svc.add_port(&id, port));
        assert_eq!(svc.get_tunnel(&id).unwrap().ports.len(), 1);
        // duplicate port number rejected
        let dup = TunnelPort { port: 8080, protocol: "http".into(), label: None };
        assert!(!svc.add_port(&id, dup));
        assert!(svc.remove_port(&id, 8080));
        assert!(svc.get_tunnel(&id).unwrap().ports.is_empty());
    }

    #[test]
    fn port_on_nonexistent_tunnel() {
        let mut svc = TunnelService::new();
        let port = TunnelPort { port: 3000, protocol: "http".into(), label: None };
        assert!(!svc.add_port("bad-id", port));
        assert!(!svc.remove_port("bad-id", 3000));
    }

    #[test]
    fn tunnel_count_and_get_all() {
        let mut svc = TunnelService::new();
        assert_eq!(svc.tunnel_count(), 0);
        svc.create_tunnel("a", TunnelAccess::Private);
        svc.create_tunnel("b", TunnelAccess::Public);
        assert_eq!(svc.tunnel_count(), 2);
        assert_eq!(svc.get_all_tunnels().len(), 2);
    }

    #[test]
    fn set_error_and_uri() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("err-test", TunnelAccess::Private);
        svc.set_error(&id, "connection refused".into());
        assert_eq!(svc.get_tunnel(&id).unwrap().status, TunnelStatus::Error("connection refused".into()));
        svc.set_uri(&id, "https://example.devtunnels.ms".into());
        assert_eq!(svc.get_tunnel(&id).unwrap().uri.as_deref(), Some("https://example.devtunnels.ms"));
    }

    #[test]
    fn find_by_name_and_disconnect_all() {
        let mut svc = TunnelService::new();
        let id1 = svc.create_tunnel("alpha", TunnelAccess::Private);
        let id2 = svc.create_tunnel("beta", TunnelAccess::Organization);
        svc.connect(&id1);
        svc.connect(&id2);
        assert_eq!(svc.active_count(), 2);
        assert!(svc.find_by_name("alpha").is_some());
        assert!(svc.find_by_name("missing").is_none());
        svc.disconnect_all();
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn display_impls() {
        assert_eq!(TunnelStatus::Connected.to_string(), "Connected");
        assert_eq!(TunnelStatus::Disconnected.to_string(), "Disconnected");
        assert_eq!(TunnelStatus::Connecting.to_string(), "Connecting");
        assert_eq!(TunnelStatus::Error("fail".into()).to_string(), "Error: fail");
        assert_eq!(TunnelAccess::Private.to_string(), "Private");
        assert_eq!(TunnelAccess::Organization.to_string(), "Organization");
        assert_eq!(TunnelAccess::Public.to_string(), "Public");

        let info = TunnelInfo {
            id: "t1".into(),
            name: "my-tunnel".into(),
            status: TunnelStatus::Connected,
            access: TunnelAccess::Public,
            uri: None,
            ports: Vec::new(),
        };
        assert_eq!(info.to_string(), "my-tunnel (Connected)");
    }

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn tunnel_error_display() {
        assert_eq!(
            TunnelError::NotFound("x".into()).to_string(),
            "tunnel not found: x"
        );
        assert_eq!(
            TunnelError::DuplicateName("dup".into()).to_string(),
            "duplicate tunnel name: dup"
        );
        assert_eq!(
            TunnelError::DuplicatePort { tunnel_id: "t1".into(), port: 80 }.to_string(),
            "port 80 already forwarded on tunnel t1"
        );
        assert_eq!(TunnelError::InvalidPort.to_string(), "port number must be non-zero");
        assert!(TunnelError::InvalidName("bad".into()).to_string().contains("bad"));
    }

    #[test]
    fn tunnel_port_builder_success() {
        let port = TunnelPortBuilder::new(8080)
            .protocol("http")
            .label("web")
            .build()
            .unwrap();
        assert_eq!(port.port, 8080);
        assert_eq!(port.protocol, "http");
        assert_eq!(port.label.as_deref(), Some("web"));
    }

    #[test]
    fn tunnel_port_builder_zero_port() {
        let result = TunnelPortBuilder::new(0).build();
        assert_eq!(result, Err(TunnelError::InvalidPort));
    }

    #[test]
    fn tunnel_port_display() {
        let p1 = TunnelPort { port: 443, protocol: "https".into(), label: Some("api".into()) };
        assert_eq!(p1.to_string(), "https:443 (api)");
        let p2 = TunnelPort { port: 80, protocol: "http".into(), label: None };
        assert_eq!(p2.to_string(), "http:80");
    }

    #[test]
    fn tunnel_port_is_secure() {
        let secure = TunnelPort { port: 443, protocol: "https".into(), label: None };
        assert!(secure.is_secure());
        let insecure = TunnelPort { port: 80, protocol: "http".into(), label: None };
        assert!(!insecure.is_secure());
    }

    #[test]
    fn tunnel_info_helpers() {
        let info = TunnelInfo {
            id: "t1".into(),
            name: "test".into(),
            status: TunnelStatus::Connected,
            access: TunnelAccess::Private,
            uri: None,
            ports: vec![
                TunnelPort { port: 443, protocol: "https".into(), label: None },
                TunnelPort { port: 80, protocol: "http".into(), label: None },
            ],
        };
        assert!(info.is_connected());
        assert!(!info.is_error());
        assert_eq!(info.port_count(), 2);
        assert_eq!(info.secure_ports().len(), 1);
    }

    #[test]
    fn validate_name_ok_and_bad() {
        assert!(TunnelInfo::validate_name("my-tunnel_1").is_ok());
        assert!(TunnelInfo::validate_name("").is_err());
        assert!(TunnelInfo::validate_name("bad name!").is_err());
    }

    #[test]
    fn create_validated_rejects_duplicates() {
        let mut svc = TunnelService::new();
        assert!(svc.create_validated("unique", TunnelAccess::Private).is_ok());
        let err = svc.create_validated("unique", TunnelAccess::Private).unwrap_err();
        assert_eq!(err, TunnelError::DuplicateName("unique".into()));
    }

    #[test]
    fn create_validated_rejects_bad_name() {
        let mut svc = TunnelService::new();
        assert!(svc.create_validated("", TunnelAccess::Private).is_err());
        assert!(svc.create_validated("has space", TunnelAccess::Private).is_err());
    }

    #[test]
    fn add_port_validated_errors() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t", TunnelAccess::Private);

        // zero port
        let zero = TunnelPort { port: 0, protocol: "http".into(), label: None };
        assert_eq!(svc.add_port_validated(&id, zero), Err(TunnelError::InvalidPort));

        // not-found tunnel
        let p = TunnelPort { port: 80, protocol: "http".into(), label: None };
        assert!(matches!(svc.add_port_validated("nope", p), Err(TunnelError::NotFound(_))));

        // duplicate
        let p1 = TunnelPort { port: 8080, protocol: "http".into(), label: None };
        svc.add_port_validated(&id, p1).unwrap();
        let p2 = TunnelPort { port: 8080, protocol: "https".into(), label: None };
        assert!(matches!(svc.add_port_validated(&id, p2), Err(TunnelError::DuplicatePort { .. })));
    }

    #[test]
    fn connect_checked_state_guard() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t", TunnelAccess::Private);
        svc.connect_checked(&id).unwrap();
        // already connected – should fail
        let err = svc.connect_checked(&id).unwrap_err();
        assert!(matches!(err, TunnelError::InvalidState { .. }));
        // not found
        assert!(matches!(svc.connect_checked("bad"), Err(TunnelError::NotFound(_))));
    }

    #[test]
    fn tunnels_by_access_filter() {
        let mut svc = TunnelService::new();
        svc.create_tunnel("priv1", TunnelAccess::Private);
        svc.create_tunnel("pub1", TunnelAccess::Public);
        svc.create_tunnel("priv2", TunnelAccess::Private);
        assert_eq!(svc.tunnels_by_access(&TunnelAccess::Private).len(), 2);
        assert_eq!(svc.tunnels_by_access(&TunnelAccess::Public).len(), 1);
        assert_eq!(svc.tunnels_by_access(&TunnelAccess::Organization).len(), 0);
    }

    #[test]
    fn service_display_and_debug() {
        let mut svc = TunnelService::new();
        assert_eq!(svc.to_string(), "TunnelService(0 tunnels, 0 active)");
        let id = svc.create_tunnel("t", TunnelAccess::Private);
        svc.connect(&id);
        assert_eq!(svc.to_string(), "TunnelService(1 tunnels, 1 active)");
        let dbg = format!("{:?}", svc);
        assert!(dbg.contains("TunnelService"));
    }

    #[test]
    fn summary_output() {
        let mut svc = TunnelService::new();
        assert_eq!(svc.summary(), "No tunnels configured.");
        svc.create_tunnel("web", TunnelAccess::Public);
        let s = svc.summary();
        assert!(s.contains("web"));
        assert!(s.contains("Public"));
        assert!(s.contains("Disconnected"));
    }

    #[test]
    fn tunnel_stats_new_defaults() {
        let stats = TunnelStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn tunnel_stats_record_success() {
        let mut stats = TunnelStats::new();
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
    fn tunnel_stats_record_failure() {
        let mut stats = TunnelStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn tunnel_stats_reset() {
        let mut stats = TunnelStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn tunnel_stats_merge() {
        let mut a = TunnelStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = TunnelStats::new();
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
    fn tunnel_stats_display() {
        let mut stats = TunnelStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn tunnel_stats_default() {
        let stats = TunnelStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn tunnel_validator_accepts_valid_name() {
        let v = TunnelValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn tunnel_validator_rejects_empty() {
        let v = TunnelValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn tunnel_validator_rejects_too_long() {
        let v = TunnelValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn tunnel_validator_forbidden_prefix() {
        let v = TunnelValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn tunnel_validator_allowed_chars() {
        let v = TunnelValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn tunnel_validator_range() {
        let v = TunnelValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn tunnel_sanitize_removes_control() {
        let result = TunnelValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn tunnel_truncate_short_string() {
        assert_eq!(TunnelValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn tunnel_truncate_long_string() {
        let result = TunnelValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn tunnel_is_ascii_printable() {
        assert!(TunnelValidator::is_ascii_printable("Hello World 123"));
        assert!(!TunnelValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- TunnelHealthCheck --

    #[test]
    fn health_check_initial_healthy() {
        let hc = TunnelHealthCheck::new(10, 100, 500);
        assert_eq!(hc.status(), HealthStatus::Healthy);
        assert_eq!(hc.sample_count(), 0);
        assert_eq!(hc.average_latency(), None);
    }

    #[test]
    fn health_check_records_latency() {
        let mut hc = TunnelHealthCheck::new(5, 100, 500);
        hc.record_latency(50);
        hc.record_latency(70);
        assert_eq!(hc.sample_count(), 2);
        assert_eq!(hc.average_latency(), Some(60));
        assert_eq!(hc.status(), HealthStatus::Healthy);
    }

    #[test]
    fn health_check_degraded() {
        let mut hc = TunnelHealthCheck::new(5, 100, 500);
        hc.record_latency(150);
        hc.record_latency(200);
        assert_eq!(hc.status(), HealthStatus::Degraded);
    }

    #[test]
    fn health_check_unhealthy_latency() {
        let mut hc = TunnelHealthCheck::new(5, 100, 500);
        hc.record_latency(600);
        hc.record_latency(700);
        assert_eq!(hc.status(), HealthStatus::Unhealthy);
    }

    #[test]
    fn health_check_unhealthy_failures() {
        let mut hc = TunnelHealthCheck::new(5, 100, 500).with_failure_limit(3);
        hc.record_failure();
        hc.record_failure();
        assert_ne!(hc.status(), HealthStatus::Unhealthy);
        hc.record_failure();
        assert_eq!(hc.status(), HealthStatus::Unhealthy);
    }

    #[test]
    fn health_check_sliding_window() {
        let mut hc = TunnelHealthCheck::new(3, 100, 500);
        hc.record_latency(50);
        hc.record_latency(60);
        hc.record_latency(70);
        hc.record_latency(80);
        assert_eq!(hc.sample_count(), 3);
        assert_eq!(hc.average_latency(), Some(70));
    }

    #[test]
    fn health_check_reset() {
        let mut hc = TunnelHealthCheck::new(5, 100, 500);
        hc.record_latency(200);
        hc.record_failure();
        hc.reset();
        assert_eq!(hc.sample_count(), 0);
        assert_eq!(hc.status(), HealthStatus::Healthy);
    }

    #[test]
    fn tunnel_service_names() {
        let mut svc = TunnelService::new();
        svc.create_tunnel("my-tunnel", TunnelAccess::Private);
        let names = svc.tunnel_names();
        assert_eq!(names, vec!["my-tunnel"]);
    }

    #[test]
    fn tunnel_total_port_count() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t1", TunnelAccess::Private);
        svc.add_port(&id, TunnelPort { port: 8080, protocol: "https".into(), label: Some("web".into()) });
        assert_eq!(svc.total_port_count(), 1);
    }

    #[test]
    fn tunnel_access_helpers() {
        assert!(TunnelAccess::Public.is_public());
        assert!(!TunnelAccess::Private.is_public());
        assert_eq!(TunnelAccess::Organization.short_label(), "org");
    }

    #[test]
    fn tunnel_status_helpers() {
        assert!(TunnelStatus::Connected.is_connectable());
        assert!(!TunnelStatus::Connecting.is_connectable());
        assert!(TunnelStatus::Disconnected.is_terminal());
        assert!(TunnelStatus::Error("x".into()).is_terminal());
    }

    #[test]
    fn tunnel_info_status_line() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t", TunnelAccess::Public);
        let info = svc.get_tunnel(&id).unwrap();
        let line = info.status_line();
        assert!(line.contains("t"));
    }

    #[test]
    fn tunnel_health_min_max() {
        let mut hc = TunnelHealthCheck::new(10, 100, 500);
        hc.record_latency(50);
        hc.record_latency(200);
        hc.record_latency(100);
        assert_eq!(hc.min_latency(), Some(50));
        assert_eq!(hc.max_latency(), Some(200));
    }

    #[test]
    fn tunnel_has_errors() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t", TunnelAccess::Private);
        assert!(!svc.has_errors());
        svc.set_error(&id, "boom".to_string());
        assert!(svc.has_errors());
    }

    // ── Tunnel address resolution ─────────────────────────────────

    #[test]
    fn tunnel_address_parse_full_uri() {
        let addr = TunnelAddress::parse("https://example.com:8080").unwrap();
        assert_eq!(addr.host, "example.com");
        assert_eq!(addr.port, 8080);
        assert_eq!(addr.scheme, "https");
        assert!(addr.is_secure());
    }

    #[test]
    fn tunnel_address_parse_host_port() {
        let addr = TunnelAddress::parse("localhost:3000").unwrap();
        assert_eq!(addr.host, "localhost");
        assert_eq!(addr.port, 3000);
        assert_eq!(addr.scheme, "https");
    }

    #[test]
    fn tunnel_address_parse_host_only() {
        let addr = TunnelAddress::parse("http://myhost").unwrap();
        assert_eq!(addr.host, "myhost");
        assert_eq!(addr.port, 80);
        assert!(!addr.is_secure());
    }

    #[test]
    fn tunnel_address_to_uri() {
        let addr = TunnelAddress::parse("ssh://server:22").unwrap();
        assert_eq!(addr.to_uri(), "ssh://server:22");
        assert_eq!(format!("{}", addr), "ssh://server:22");
    }

    #[test]
    fn tunnel_address_parse_empty_returns_none() {
        assert!(TunnelAddress::parse("").is_none());
        assert!(TunnelAddress::parse("://host").is_none());
    }

    // ── Connection log ────────────────────────────────────────────

    #[test]
    fn connection_log_records_and_queries() {
        let mut log = TunnelConnectionLog::new();
        log.record("t1", TunnelEventType::Connected, 100, None);
        log.record("t1", TunnelEventType::Error, 200, Some("timeout"));
        log.record("t2", TunnelEventType::Connected, 150, None);

        assert_eq!(log.total_events(), 3);
        assert_eq!(log.error_count(), 1);
        assert_eq!(log.events_for_tunnel("t1").len(), 2);
        let last = log.last_event("t1").unwrap();
        assert_eq!(last.event_type, TunnelEventType::Error);
        assert_eq!(last.message.as_deref(), Some("timeout"));
    }

    #[test]
    fn tunnel_event_type_display() {
        assert_eq!(format!("{}", TunnelEventType::Connected), "connected");
        assert_eq!(format!("{}", TunnelEventType::Reconnecting), "reconnecting");
    }

    // ── Bandwidth stats ───────────────────────────────────────────

    #[test]
    fn bandwidth_stats_accumulates() {
        let mut stats = TunnelBandwidthStats::new();
        stats.record_inbound("t1", 1000);
        stats.record_inbound("t1", 500);
        stats.record_outbound("t1", 200);
        assert_eq!(stats.inbound("t1"), 1500);
        assert_eq!(stats.outbound("t1"), 200);
        assert_eq!(stats.total_bytes(), 1700);
    }

    #[test]
    fn bandwidth_stats_unknown_tunnel_zero() {
        let stats = TunnelBandwidthStats::new();
        assert_eq!(stats.inbound("missing"), 0);
        assert_eq!(stats.outbound("missing"), 0);
    }

    // ── TunnelPortMapping ─────────────────────────────────────────

    #[test]
    fn port_mapping_basic() {
        let mapping = TunnelPortMapping::new(3000, 443, PortProtocol::Https)
            .with_label("web");
        assert_eq!(mapping.local_port, 3000);
        assert_eq!(mapping.remote_port, 443);
        assert_eq!(mapping.label.as_deref(), Some("web"));
        assert!(!mapping.is_identity());
    }

    #[test]
    fn port_mapping_identity() {
        let mapping = TunnelPortMapping::new(8080, 8080, PortProtocol::Http);
        assert!(mapping.is_identity());
    }

    #[test]
    fn port_mapping_url() {
        let mapping = TunnelPortMapping::new(3000, 443, PortProtocol::Https);
        assert_eq!(mapping.url("example.com"), "https://example.com:443");
    }

    #[test]
    fn port_mapping_display() {
        let mapping = TunnelPortMapping::new(3000, 443, PortProtocol::Https)
            .with_label("web");
        let display = format!("{}", mapping);
        assert!(display.contains("web"));
        assert!(display.contains("3000"));
        assert!(display.contains("443"));
    }

    // ── TunnelPortRouter ──────────────────────────────────────────

    #[test]
    fn port_router_resolve() {
        let mut router = TunnelPortRouter::new();
        router.add_mapping(TunnelPortMapping::new(3000, 443, PortProtocol::Https));
        router.add_mapping(TunnelPortMapping::new(5000, 80, PortProtocol::Http));
        assert_eq!(router.resolve_local(3000), Some(443));
        assert_eq!(router.resolve_local(9999), None);
        assert_eq!(router.resolve_remote(80), Some(5000));
        assert_eq!(router.count(), 2);
    }

    #[test]
    fn port_router_by_protocol() {
        let mut router = TunnelPortRouter::new();
        router.add_mapping(TunnelPortMapping::new(3000, 443, PortProtocol::Https));
        router.add_mapping(TunnelPortMapping::new(5000, 80, PortProtocol::Http));
        router.add_mapping(TunnelPortMapping::new(2222, 22, PortProtocol::Tcp));
        assert_eq!(router.by_protocol(PortProtocol::Https).len(), 1);
        assert_eq!(router.by_protocol(PortProtocol::Http).len(), 1);
    }

    #[test]
    fn port_router_remove() {
        let mut router = TunnelPortRouter::new();
        router.add_mapping(TunnelPortMapping::new(3000, 443, PortProtocol::Https));
        assert!(router.has_local_port(3000));
        assert!(router.remove_by_local_port(3000));
        assert!(!router.has_local_port(3000));
        assert_eq!(router.count(), 0);
    }

    // ── Port classification & validation ──────────────────────────

    #[test]
    fn classify_port_ranges() {
        assert_eq!(classify_port(0), PortCategory::Invalid);
        assert_eq!(classify_port(1), PortCategory::WellKnown);
        assert_eq!(classify_port(80), PortCategory::WellKnown);
        assert_eq!(classify_port(443), PortCategory::WellKnown);
        assert_eq!(classify_port(1023), PortCategory::WellKnown);
        assert_eq!(classify_port(1024), PortCategory::Registered);
        assert_eq!(classify_port(8080), PortCategory::Registered);
        assert_eq!(classify_port(49151), PortCategory::Registered);
        assert_eq!(classify_port(49152), PortCategory::Dynamic);
        assert_eq!(classify_port(65535), PortCategory::Dynamic);
    }

    #[test]
    fn privileged_and_dev_ports() {
        assert!(is_privileged_port(22));
        assert!(is_privileged_port(443));
        assert!(!is_privileged_port(0));
        assert!(!is_privileged_port(8080));

        assert!(is_common_dev_port(3000));
        assert!(is_common_dev_port(8080));
        assert!(!is_common_dev_port(12345));
    }

    #[test]
    fn port_category_display() {
        assert_eq!(PortCategory::WellKnown.to_string(), "well-known");
        assert_eq!(PortCategory::Registered.to_string(), "registered");
        assert_eq!(PortCategory::Dynamic.to_string(), "dynamic");
        assert_eq!(PortCategory::Invalid.to_string(), "invalid");
    }

    // ── Protocol detection ────────────────────────────────────────

    #[test]
    fn detect_protocol_known_ports() {
        assert_eq!(detect_protocol(22), "ssh");
        assert_eq!(detect_protocol(80), "http");
        assert_eq!(detect_protocol(443), "https");
        assert_eq!(detect_protocol(3306), "mysql");
        assert_eq!(detect_protocol(5432), "postgres");
        assert_eq!(detect_protocol(6379), "redis");
        assert_eq!(detect_protocol(27017), "mongodb");
        assert_eq!(detect_protocol(12345), "tcp");
    }

    // ── URL construction ──────────────────────────────────────────

    #[test]
    fn forwarded_port_url_with_subdomain() {
        let url = forwarded_port_url("https://abc.devtunnels.ms", 3000);
        assert_eq!(url, "https://abc-3000.devtunnels.ms");
    }

    #[test]
    fn forwarded_port_url_no_dot() {
        let url = forwarded_port_url("https://localhost", 8080);
        assert_eq!(url, "https://localhost:8080");
    }

    #[test]
    fn forwarded_port_url_no_scheme() {
        let url = forwarded_port_url("myhost", 9000);
        assert_eq!(url, "myhost:9000");
    }

    // ── Label formatting ──────────────────────────────────────────

    #[test]
    fn format_port_label_with_and_without() {
        assert_eq!(format_port_label(8080, Some("Web UI")), "Web UI (:8080)");
        assert_eq!(format_port_label(3000, None), "Port 3000");
    }

    #[test]
    fn format_tunnel_badge_variants() {
        assert_eq!(
            format_tunnel_badge("dev", &TunnelStatus::Connected, 2),
            "● dev [2 ports]"
        );
        assert_eq!(
            format_tunnel_badge("dev", &TunnelStatus::Connected, 1),
            "● dev [1 port]"
        );
        assert_eq!(
            format_tunnel_badge("dev", &TunnelStatus::Disconnected, 0),
            "○ dev"
        );
        assert_eq!(
            format_tunnel_badge("dev", &TunnelStatus::Error("x".into()), 0),
            "✖ dev"
        );
        assert_eq!(
            format_tunnel_badge("dev", &TunnelStatus::Connecting, 3),
            "◌ dev [3 ports]"
        );
    }

    // ── Port conflict detection ───────────────────────────────────

    #[test]
    fn find_available_port_skips_allocated() {
        let allocated = vec![3000, 3001, 3002];
        assert_eq!(find_available_port(&allocated, 3000), Some(3003));
        assert_eq!(find_available_port(&allocated, 3001), Some(3003));
        assert_eq!(find_available_port(&[], 8080), Some(8080));
    }

    #[test]
    fn detect_port_conflicts_across_tunnels() {
        let mut svc = TunnelService::new();
        let t1 = svc.create_tunnel("web", TunnelAccess::Private);
        let t2 = svc.create_tunnel("api", TunnelAccess::Private);
        svc.add_port(&t1, TunnelPort { port: 8080, protocol: "http".into(), label: None });
        svc.add_port(&t2, TunnelPort { port: 8080, protocol: "http".into(), label: None });
        svc.add_port(&t2, TunnelPort { port: 3000, protocol: "http".into(), label: None });

        let conflicts = detect_port_conflicts(&svc);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].0, 8080);
        assert_eq!(conflicts[0].1.len(), 2);
    }

    // ── Connection pool ───────────────────────────────────────────

    #[test]
    fn connection_pool_acquire_release() {
        let mut pool = ConnectionPool::new(2);
        assert_eq!(pool.available(), 2);
        assert!(pool.acquire("conn-1"));
        assert!(pool.acquire("conn-2"));
        assert!(!pool.acquire("conn-3")); // full
        assert!(pool.is_full());
        assert!(pool.contains("conn-1"));

        assert!(pool.release("conn-1"));
        assert!(!pool.is_full());
        assert_eq!(pool.active_count(), 1);
        assert_eq!(pool.available(), 1);

        assert!(pool.acquire("conn-3"));
        assert_eq!(pool.to_string(), "ConnectionPool(2/2 active)");
    }

    #[test]
    fn connection_pool_drain() {
        let mut pool = ConnectionPool::new(3);
        pool.acquire("a");
        pool.acquire("b");
        let drained = pool.drain_all();
        assert_eq!(drained.len(), 2);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn connection_pool_duplicate_rejected() {
        let mut pool = ConnectionPool::new(5);
        assert!(pool.acquire("x"));
        assert!(!pool.acquire("x")); // already held
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn connection_pool_release_unknown() {
        let mut pool = ConnectionPool::new(5);
        assert!(!pool.release("nonexistent"));
    }

    #[test] fn tmetrics_send() { let mut m = TunnelMetrics::new("t1"); m.record_send(100); assert_eq!(m.bytes_sent, 100); }
    #[test] fn tmetrics_avg() { let mut m = TunnelMetrics::new("t"); m.record_latency(10); m.record_latency(20); assert!((m.avg_latency() - 15.0).abs() < 1e-9); }
    #[test] fn tmetrics_err_rate() { let mut m = TunnelMetrics::new("t"); m.record_send(10); m.record_send(10); m.record_error(); assert!((m.error_rate() - 0.5).abs() < 1e-9); }
    #[test] fn tmetrics_reset() { let mut m = TunnelMetrics::new("t"); m.record_send(100); m.reset(); assert_eq!(m.bytes_sent, 0); }
    #[test] fn tmetrics_display() { assert!(format!("{}", TunnelMetrics::new("t1")).contains("t1")); }
    #[test] fn tmetrics_coll() { let mut c = TunnelMetricsCollector::new(); c.get_or_create("a").record_send(50); c.get_or_create("b").record_send(30); assert_eq!(c.total_bytes_all(), 80); }
    #[test] fn tproto_neg_ok() { let n = TunnelProtocolNegotiator::default(); assert!(n.negotiate(&[TunnelProtocol::Http2, TunnelProtocol::Raw]).is_ok()); }
    #[test] fn tproto_neg_fail() { let n = TunnelProtocolNegotiator::new(vec![TunnelProtocol::Ssh]); assert!(n.negotiate(&[TunnelProtocol::Raw]).is_err()); }
    #[test] fn tproto_add_rm() { let mut n = TunnelProtocolNegotiator::new(vec![]); n.add(TunnelProtocol::Ssh); assert!(n.supports(TunnelProtocol::Ssh)); n.remove(TunnelProtocol::Ssh); assert!(!n.supports(TunnelProtocol::Ssh)); }
    #[test] fn tproto_display() { assert_eq!(format!("{}", TunnelProtocol::WebSocket), "ws"); }
    #[test] fn tmetrics_minmax() { let mut m = TunnelMetrics::new("t"); m.record_latency(5); m.record_latency(15); assert_eq!(m.min_latency(), 5); assert_eq!(m.max_latency(), 15); }
    #[test] fn tmetrics_total() { let mut m = TunnelMetrics::new("t"); m.record_send(100); m.record_recv(200); assert_eq!(m.total_bytes(), 300); }


    #[test] fn tunnelMetrics_cfg_default() {
        let c = TunnelMetricsConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_refresh);
    }
    #[test] fn tunnelMetrics_cfg_builder() {
        let c = TunnelMetricsConfig::new().with_max_entries(500).with_auto_refresh(false);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_refresh);
    }
    #[test] fn tunnelMetrics_cfg_labels() {
        let mut c = TunnelMetricsConfig::new();
        c.set_label("x", "y");
        assert_eq!(c.get_label("x"), Some("y"));
    }
    #[test] fn tunnelMetrics_cfg_refresh_due() {
        let c = TunnelMetricsConfig::new();
        assert!(!c.is_refresh_due(1000));
        assert!(c.is_refresh_due(6000));
    }
    #[test] fn tunnelMetrics_cfg_display() {
        assert!(format!("{}", TunnelMetricsConfig::new()).contains("Config"));
    }
    #[test] fn tunnelMetricsCollector_stats_success() {
        let mut st = TunnelMetricsCollectorStats::new();
        st.record_success(10);
        st.record_success(20);
        st.record_failure(5);
        assert_eq!(st.total_operations, 3);
        assert!((st.success_rate() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn tunnelMetricsCollector_stats_avg_dur() {
        let mut st = TunnelMetricsCollectorStats::new();
        st.record_success(10);
        st.record_success(30);
        assert!((st.avg_duration_ms() - 20.0).abs() < 1e-9);
    }
    #[test] fn tunnelMetricsCollector_stats_merge() {
        let mut a = TunnelMetricsCollectorStats::new();
        a.record_success(10);
        let mut b = TunnelMetricsCollectorStats::new();
        b.record_success(20);
        a.merge(&b);
        assert_eq!(a.total_operations, 2);
    }
    #[test] fn tunnelMetricsCollector_stats_concurrent() {
        let mut st = TunnelMetricsCollectorStats::new();
        st.update_concurrent(5);
        st.update_concurrent(3);
        assert_eq!(st.peak_concurrent, 5);
    }
    #[test] fn tunnelMetricsCollector_stats_display() {
        assert!(format!("{}", TunnelMetricsCollectorStats::new()).contains("Stats"));
    }
    #[test] fn tunnelMetrics_event_new() {
        let e = TunnelMetricsEvent::new(TunnelMetricsEventKind::Created, 100);
        assert_eq!(e.kind, TunnelMetricsEventKind::Created);
        assert!(!e.is_error());
    }
    #[test] fn tunnelMetrics_event_detail() {
        let e = TunnelMetricsEvent::new(TunnelMetricsEventKind::Error, 0).with_detail("oops");
        assert!(e.is_error());
        assert_eq!(e.detail.unwrap(), "oops");
    }
    #[test] fn tunnelMetrics_event_display() {
        let e = TunnelMetricsEvent::new(TunnelMetricsEventKind::Updated, 50);
        assert!(format!("{}", e).contains("updated"));
    }
    #[test] fn tunnelMetrics_event_kind_display() {
        assert_eq!(format!("{}", TunnelMetricsEventKind::Refreshed), "refreshed");
    }

}
