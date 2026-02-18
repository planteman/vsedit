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


// ---------------------------------------------------------------------------
// vsedit-tunnel: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl TunnelXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for TunnelXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct TunnelXRegistry {
    entries: Vec<TunnelXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl TunnelXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: TunnelXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&TunnelXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut TunnelXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<TunnelXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&TunnelXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&TunnelXConfig> {
        let mut sorted: Vec<&TunnelXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&TunnelXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> TunnelXIterator<'_> {
        TunnelXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct TunnelXIterator<'a> {
    inner: std::slice::Iter<'a, TunnelXConfig>,
}

impl<'a> Iterator for TunnelXIterator<'a> {
    type Item = &'a TunnelXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct TunnelXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl TunnelXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct TunnelXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl TunnelXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &TunnelXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &TunnelXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &TunnelXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for TunnelXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct TunnelXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl TunnelXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &TunnelXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &TunnelXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for TunnelXValidator {
    fn default() -> Self {
        Self::new()
    }
}



// ---------------------------------------------------------------------------
// tunnel – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for tunnel connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YTunnelTunnelProtocol {
    Tcp,
    WebSocket,
    Ssh,
    Http,
}

impl YTunnelTunnelProtocol {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Tcp => 0,
            Self::WebSocket => 1,
            Self::Ssh => 2,
            Self::Http => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tcp => "Tcp",
            Self::WebSocket => "WebSocket",
            Self::Ssh => "Ssh",
            Self::Http => "Http",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YTunnelTunnelProtocol] {
        &[
            YTunnelTunnelProtocol::Tcp,
            YTunnelTunnelProtocol::WebSocket,
            YTunnelTunnelProtocol::Ssh,
            YTunnelTunnelProtocol::Http,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YTunnelTunnelProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks tunnel metrics data.
#[derive(Debug, Clone)]
pub struct YTunnelTunnelMetrics {
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub uptime_ms: u64,
}

impl YTunnelTunnelMetrics {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            bytes_sent: 0,
            bytes_recv: 0,
            uptime_ms: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YTunnelTunnelMetrics({}: {:?})", "bytes_sent", self.bytes_sent)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_tunnel_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_tunnel_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_tunnel_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_tunnel_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_tunnel_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_tunnel_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_tunnel_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_tunnel_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// tunnel – Extended tunnel health check helpers
// ---------------------------------------------------------------------------

/// Priority levels for tunnel health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZTunnelPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZTunnelPriority {
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
    pub fn all_asc() -> [ZTunnelPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZTunnelPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks tunnel health check data.
#[derive(Debug, Clone)]
pub struct ZTunnelTunnelHealthCheck {
    pub check_results: Vec<(u64, bool)>,
    pub interval_ms: u64,
    pub consecutive_failures: u32,
}

impl ZTunnelTunnelHealthCheck {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            check_results: Vec::new(),
            interval_ms: 0,
            consecutive_failures: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.check_results.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.check_results.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.check_results.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZTunnelTunnelHealthCheck[interval_ms={:?}, consecutive_failures={:?}]", self.interval_ms, self.consecutive_failures)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for tunnel health check.
pub fn z_tunnel_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_tunnel_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_tunnel_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_tunnel_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_tunnel_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_tunnel_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_tunnel_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 187
// ---------------------------------------------------------------------------

/// Generic object pool `Xc187Pool<T>`.
pub struct Xc187Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc187Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc187PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc187Pool<T> {
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
    pub fn stats(&self) -> Xc187PoolStats {
        Xc187PoolStats {
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

impl<T> Default for Xc187Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc187Scheduler`.
pub struct Xc187Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc187Scheduler {
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

impl Default for Xc187Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_187 hash for the given byte slice.
pub fn xc_187_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_187 convention.
pub fn xc_187_reverse(s: &str) -> String {
    s.chars().rev().collect()
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


    #[test]
    fn tunnel_x_config_new() {
        let c = TunnelXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn tunnel_x_config_builder() {
        let c = TunnelXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn tunnel_x_config_display() {
        let c = TunnelXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn tunnel_x_registry_insert_get() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn tunnel_x_registry_duplicate() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a")).unwrap();
        assert!(reg.insert(TunnelXConfig::new("a")).is_err());
    }

    #[test]
    fn tunnel_x_registry_remove() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a")).unwrap();
        reg.insert(TunnelXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn tunnel_x_registry_active_entries() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a")).unwrap();
        reg.insert(TunnelXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn tunnel_x_registry_by_weight() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(TunnelXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn tunnel_x_registry_tags() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(TunnelXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn tunnel_x_registry_total_weight() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(TunnelXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn tunnel_x_registry_iterator() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a")).unwrap();
        reg.insert(TunnelXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn tunnel_x_cache_put_get() {
        let mut cache = TunnelXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn tunnel_x_cache_eviction() {
        let mut cache = TunnelXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn tunnel_x_cache_lru_order() {
        let mut cache = TunnelXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn tunnel_x_cache_most_least_recent() {
        let mut cache = TunnelXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn tunnel_x_formatter_entry() {
        let e = TunnelXConfig::new("k").with_value("v");
        let fmt = TunnelXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn tunnel_x_formatter_summary() {
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("a").with_weight(5)).unwrap();
        let fmt = TunnelXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn tunnel_x_validator_valid() {
        let v = TunnelXValidator::new();
        let c = TunnelXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn tunnel_x_validator_empty_key() {
        let v = TunnelXValidator::new();
        let c = TunnelXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn tunnel_x_validator_require_value() {
        let v = TunnelXValidator::new().require_value(true);
        let c = TunnelXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn tunnel_x_validator_allowed_tags() {
        let v = TunnelXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = TunnelXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn tunnel_x_validator_validate_all() {
        let v = TunnelXValidator::new();
        let mut reg = TunnelXRegistry::new();
        reg.insert(TunnelXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // -- tunnel extended domain tests ----------------------------------------

    #[test]
    fn y_tunnel_enum_index() {
        assert_eq!(YTunnelTunnelProtocol::Tcp.index(), 0);
        assert_eq!(YTunnelTunnelProtocol::WebSocket.index(), 1);
        assert_eq!(YTunnelTunnelProtocol::Ssh.index(), 2);
        assert_eq!(YTunnelTunnelProtocol::Http.index(), 3);
    }

    #[test]
    fn y_tunnel_enum_label() {
        assert_eq!(YTunnelTunnelProtocol::Tcp.label(), "Tcp");
        assert_eq!(YTunnelTunnelProtocol::WebSocket.label(), "WebSocket");
        assert_eq!(YTunnelTunnelProtocol::Ssh.label(), "Ssh");
        assert_eq!(YTunnelTunnelProtocol::Http.label(), "Http");
    }

    #[test]
    fn y_tunnel_enum_all() {
        let all = YTunnelTunnelProtocol::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_tunnel_enum_is_default() {
        assert!(YTunnelTunnelProtocol::Tcp.is_default());
        assert!(!YTunnelTunnelProtocol::Http.is_default());
    }

    #[test]
    fn y_tunnel_enum_display() {
        assert_eq!(format!("{}", YTunnelTunnelProtocol::Tcp), "Tcp");
    }

    #[test]
    fn y_tunnel_struct_new() {
        let s = YTunnelTunnelMetrics::new();
        let _ = s.summary();
    }

    #[test]
    fn y_tunnel_fingerprint_deterministic() {
        let h1 = y_tunnel_fingerprint("hello");
        let h2 = y_tunnel_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_tunnel_fingerprint("a"), y_tunnel_fingerprint("b"));
    }

    #[test]
    fn y_tunnel_truncate_short() {
        assert_eq!(y_tunnel_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_tunnel_truncate_long() {
        let r = y_tunnel_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_tunnel_normalize_key_basic() {
        assert_eq!(y_tunnel_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_tunnel_split_path_basic() {
        let parts = y_tunnel_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_tunnel_count_occurrences_basic() {
        assert_eq!(y_tunnel_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_tunnel_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_tunnel_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_tunnel_in_range_basic() {
        assert!(y_tunnel_in_range(5, 1, 10));
        assert!(y_tunnel_in_range(1, 1, 10));
        assert!(y_tunnel_in_range(10, 1, 10));
        assert!(!y_tunnel_in_range(0, 1, 10));
        assert!(!y_tunnel_in_range(11, 1, 10));
    }

    #[test]
    fn y_tunnel_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_tunnel_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_tunnel_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_tunnel_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- tunnel Z-extended tests -----------------------------------------------

    #[test]
    fn z_tunnel_priority_weight() {
        assert_eq!(ZTunnelPriority::Idle.weight(), 0);
        assert_eq!(ZTunnelPriority::Normal.weight(), 2);
        assert_eq!(ZTunnelPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_tunnel_priority_label() {
        assert_eq!(ZTunnelPriority::Low.label(), "low");
        assert_eq!(ZTunnelPriority::High.label(), "high");
    }

    #[test]
    fn z_tunnel_priority_is_elevated() {
        assert!(!ZTunnelPriority::Normal.is_elevated());
        assert!(ZTunnelPriority::High.is_elevated());
        assert!(ZTunnelPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_tunnel_priority_display() {
        assert_eq!(format!("{}", ZTunnelPriority::Idle), "idle");
    }

    #[test]
    fn z_tunnel_priority_all_asc() {
        let all = ZTunnelPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZTunnelPriority::Idle);
        assert_eq!(all[4], ZTunnelPriority::Realtime);
    }

    #[test]
    fn z_tunnel_struct_new() {
        let s = ZTunnelTunnelHealthCheck::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_tunnel_struct_toggled_clone() {
        let s = ZTunnelTunnelHealthCheck::new();
        let t = s.toggled_clone();
        let _ = t.consecutive_failures;
    }

    #[test]
    fn z_tunnel_rolling_hash_deterministic() {
        let h1 = z_tunnel_rolling_hash(b"test");
        let h2 = z_tunnel_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_tunnel_rolling_hash(b"a"), z_tunnel_rolling_hash(b"b"));
    }

    #[test]
    fn z_tunnel_pad_to_basic() {
        assert_eq!(z_tunnel_pad_to("hi", 5), "hi   ");
        assert_eq!(z_tunnel_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_tunnel_is_identifier_basic() {
        assert!(z_tunnel_is_identifier("foo_bar"));
        assert!(z_tunnel_is_identifier("abc123"));
        assert!(!z_tunnel_is_identifier(""));
        assert!(!z_tunnel_is_identifier("has space"));
    }

    #[test]
    fn z_tunnel_levenshtein_basic() {
        assert_eq!(z_tunnel_levenshtein("", ""), 0);
        assert_eq!(z_tunnel_levenshtein("abc", "abc"), 0);
        assert_eq!(z_tunnel_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_tunnel_unique_words_basic() {
        let w = z_tunnel_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_tunnel_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_tunnel_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_tunnel_common_prefix_basic() {
        assert_eq!(z_tunnel_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_tunnel_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_tunnel_struct_clear() {
        let mut s = ZTunnelTunnelHealthCheck::new();
        s.check_results.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_tunnel_rolling_hash_empty() {
        let h = z_tunnel_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 187 ----

    #[test]
    fn xc_187_pool_new_empty() {
        let pool: super::Xc187Pool<i32> = super::Xc187Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_187_pool_release_acquire() {
        let mut pool = super::Xc187Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_187_pool_acquire_empty() {
        let mut pool: super::Xc187Pool<i32> = super::Xc187Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_187_pool_full() {
        let mut pool = super::Xc187Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_187_pool_drain() {
        let mut pool = super::Xc187Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_187_pool_stats() {
        let mut pool = super::Xc187Pool::new(8);
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
    fn xc_187_pool_clear() {
        let mut pool = super::Xc187Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_187_pool_shrink() {
        let mut pool = super::Xc187Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_187_pool_default() {
        let pool: super::Xc187Pool<String> = super::Xc187Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_187_pool_extend() {
        let mut pool = super::Xc187Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_187_pool_retain() {
        let mut pool = super::Xc187Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_187_scheduler_round_robin() {
        let mut sched = super::Xc187Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_187_scheduler_empty() {
        let mut sched = super::Xc187Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_187_scheduler_reset() {
        let mut sched = super::Xc187Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_187_scheduler_add_remove() {
        let mut sched = super::Xc187Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_187_scheduler_targets() {
        let sched = super::Xc187Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_187_hash_empty() {
        assert_eq!(super::xc_187_hash(b""), 5381);
    }

    #[test]
    fn xc_187_hash_data() {
        let h = super::xc_187_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_187_hash(b"hello"), h);
    }

    #[test]
    fn xc_187_reverse_str() {
        assert_eq!(super::xc_187_reverse("abc"), "cba");
        assert_eq!(super::xc_187_reverse(""), "");
    }

}
