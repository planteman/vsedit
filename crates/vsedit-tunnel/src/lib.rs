//! Dev tunnel management.

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
}
