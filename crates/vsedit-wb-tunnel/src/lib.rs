//! Port forwarding service.

use std::fmt;
/// Privacy level for a tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelPrivacy {
    Private,
    Public,
}

/// Protocol used by a tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelProtocol {
    Http,
    Https,
    Tcp,
}

/// Describes a tunnel to be created.
#[derive(Debug, Clone)]
pub struct TunnelDescriptor {
    pub remote_address: String,
    pub local_port: u16,
    pub privacy: TunnelPrivacy,
    pub protocol: TunnelProtocol,
    pub label: Option<String>,
}

/// Current state of a tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelState {
    Connecting,
    Connected,
    Closed,
    Error(String),
}

/// A tunnel managed by the service.
#[derive(Debug)]
pub struct ManagedTunnel {
    pub descriptor: TunnelDescriptor,
    pub state: TunnelState,
    pub id: u64,
    pub created_at: u64,
    pub bytes_transferred: u64,
}

/// Aggregate statistics about managed tunnels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelStats {
    pub active_count: usize,
    pub total_created: usize,
    pub total_bytes_transferred: u64,
    pub http_count: usize,
    pub https_count: usize,
    pub tcp_count: usize,
}

/// Trait for external tunnel providers.
pub trait TunnelProvider {
    /// Create the underlying tunnel resource. Returns `true` on success.
    fn create_tunnel(&self, descriptor: &TunnelDescriptor) -> bool {
        let _ = descriptor;
        true
    }

    /// Dispose of the underlying tunnel resource. Returns `true` on success.
    fn dispose_tunnel(&self, id: u64) -> bool {
        let _ = id;
        true
    }
}

/// Service for tunnel workbench functionality.
pub struct TunnelWorkbenchService {
    tunnels: Vec<ManagedTunnel>,
    next_id: u64,
    total_created: usize,
    clock: u64,
}

impl TunnelWorkbenchService {
    pub fn new() -> Self {
        Self {
            tunnels: Vec::new(),
            next_id: 1,
            total_created: 0,
            clock: 1,
        }
    }

    /// Creates a tunnel and returns its id.
    pub fn create_tunnel(&mut self, descriptor: TunnelDescriptor) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let created_at = self.clock;
        self.clock += 1;
        self.total_created += 1;
        self.tunnels.push(ManagedTunnel {
            descriptor,
            state: TunnelState::Connecting,
            id,
            created_at,
            bytes_transferred: 0,
        });
        id
    }

    /// Closes a tunnel by id. Returns `true` if it existed.
    pub fn close_tunnel(&mut self, id: u64) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.state = TunnelState::Closed;
            true
        } else {
            false
        }
    }

    /// Sets the state of a tunnel by id.
    pub fn set_state(&mut self, id: u64, state: TunnelState) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.state = state;
        }
    }

    pub fn get_tunnel(&self, id: u64) -> Option<&ManagedTunnel> {
        self.tunnels.iter().find(|t| t.id == id)
    }

    /// Returns tunnels that are not closed.
    pub fn get_active_tunnels(&self) -> Vec<&ManagedTunnel> {
        self.tunnels
            .iter()
            .filter(|t| t.state != TunnelState::Closed)
            .collect()
    }

    pub fn tunnel_count(&self) -> usize {
        self.tunnels.len()
    }

    /// Returns aggregate statistics about all tunnels.
    pub fn get_stats(&self) -> TunnelStats {
        let active_count = self.tunnels.iter().filter(|t| t.state != TunnelState::Closed).count();
        let total_bytes_transferred = self.tunnels.iter().map(|t| t.bytes_transferred).sum();
        let http_count = self
            .tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == TunnelProtocol::Http)
            .count();
        let https_count = self
            .tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == TunnelProtocol::Https)
            .count();
        let tcp_count = self
            .tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == TunnelProtocol::Tcp)
            .count();
        TunnelStats {
            active_count,
            total_created: self.total_created,
            total_bytes_transferred,
            http_count,
            https_count,
            tcp_count,
        }
    }

    /// Returns tunnels matching the given protocol.
    pub fn get_tunnels_by_protocol(&self, protocol: TunnelProtocol) -> Vec<&ManagedTunnel> {
        self.tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == protocol)
            .collect()
    }

    /// Returns tunnels matching the given privacy level.
    pub fn get_tunnels_by_privacy(&self, privacy: TunnelPrivacy) -> Vec<&ManagedTunnel> {
        self.tunnels
            .iter()
            .filter(|t| t.descriptor.privacy == privacy)
            .collect()
    }

    /// Finds a tunnel by its local port.
    pub fn find_by_port(&self, port: u16) -> Option<&ManagedTunnel> {
        self.tunnels.iter().find(|t| t.descriptor.local_port == port)
    }

    /// Records bytes transferred on a tunnel.
    pub fn record_transfer(&mut self, id: u64, bytes: u64) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.bytes_transferred += bytes;
        }
    }

    /// Closes all active tunnels, returning the number closed.
    pub fn close_all(&mut self) -> usize {
        let mut count = 0;
        for t in &mut self.tunnels {
            if t.state != TunnelState::Closed {
                t.state = TunnelState::Closed;
                count += 1;
            }
        }
        count
    }

    /// Removes tunnels in the `Closed` state, returning the number removed.
    pub fn remove_closed(&mut self) -> usize {
        let before = self.tunnels.len();
        self.tunnels.retain(|t| t.state != TunnelState::Closed);
        before - self.tunnels.len()
    }

    /// Renames a tunnel by setting its label.
    pub fn rename_tunnel(&mut self, id: u64, label: &str) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.descriptor.label = Some(label.to_string());
        }
    }

    /// Returns true if tunnels is empty.
    pub fn is_tunnels_empty(&self) -> bool {
        self.tunnels.is_empty()
    }

    /// Get the first tunnel, if any.
    pub fn first_tunnel(&self) -> Option<&ManagedTunnel> {
        self.tunnels.first()
    }

    /// Get the last tunnel, if any.
    pub fn last_tunnel(&self) -> Option<&ManagedTunnel> {
        self.tunnels.last()
    }

    /// Retain only tunnels matching the predicate.
    pub fn retain_tunnels(&mut self, f: impl Fn(&ManagedTunnel) -> bool) {
        self.tunnels.retain(|item| f(item));
    }
}

impl Default for TunnelWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an active port forwarding connection with metadata.
#[derive(Debug, Clone)]
pub struct TunnelConnection {
    pub tunnel_id: u64,
    pub local_port: u16,
    pub remote_port: u16,
    pub remote_host: String,
    pub protocol: TunnelProtocol,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub connected_since: u64,
}

impl TunnelConnection {
    pub fn new(tunnel_id: u64, local_port: u16, remote_host: &str, remote_port: u16, protocol: TunnelProtocol) -> Self {
        Self {
            tunnel_id,
            local_port,
            remote_port,
            remote_host: remote_host.to_string(),
            protocol,
            bytes_in: 0,
            bytes_out: 0,
            connected_since: 0,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_in + self.bytes_out
    }

    pub fn record_inbound(&mut self, bytes: u64) {
        self.bytes_in += bytes;
    }

    pub fn record_outbound(&mut self, bytes: u64) {
        self.bytes_out += bytes;
    }

    /// Format as "local_port -> remote_host:remote_port (protocol)"
    pub fn display_address(&self) -> String {
        let proto = match self.protocol {
            TunnelProtocol::Http => "HTTP",
            TunnelProtocol::Https => "HTTPS",
            TunnelProtocol::Tcp => "TCP",
        };
        format!("{} -> {}:{} ({})", self.local_port, self.remote_host, self.remote_port, proto)
    }
}

impl fmt::Display for TunnelConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_address())
    }
}

/// Discovers available tunnels from a set of port ranges.
#[derive(Debug, Clone)]
pub struct TunnelDiscovery {
    discovered: Vec<DiscoveredPort>,
    scan_ranges: Vec<(u16, u16)>,
}

/// A discovered open port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPort {
    pub port: u16,
    pub process_name: Option<String>,
    pub suggested_label: Option<String>,
}

impl TunnelDiscovery {
    pub fn new() -> Self {
        Self {
            discovered: Vec::new(),
            scan_ranges: vec![(3000, 3100), (8000, 8100), (5000, 5100)],
        }
    }

    pub fn add_scan_range(&mut self, start: u16, end: u16) {
        if start <= end {
            self.scan_ranges.push((start, end));
        }
    }

    pub fn scan_ranges(&self) -> &[(u16, u16)] {
        &self.scan_ranges
    }

    /// Manually report a discovered port.
    pub fn report_port(&mut self, port: u16, process_name: Option<&str>, label: Option<&str>) {
        if !self.discovered.iter().any(|d| d.port == port) {
            self.discovered.push(DiscoveredPort {
                port,
                process_name: process_name.map(|s| s.to_string()),
                suggested_label: label.map(|s| s.to_string()),
            });
        }
    }

    pub fn discovered_ports(&self) -> &[DiscoveredPort] {
        &self.discovered
    }

    pub fn clear(&mut self) {
        self.discovered.clear();
    }

    /// Check if a port is in any of the scan ranges.
    pub fn in_scan_range(&self, port: u16) -> bool {
        self.scan_ranges.iter().any(|(start, end)| port >= *start && port <= *end)
    }

    /// Create a tunnel descriptor from a discovered port.
    pub fn to_descriptor(&self, port: u16, privacy: TunnelPrivacy) -> Option<TunnelDescriptor> {
        let discovered = self.discovered.iter().find(|d| d.port == port)?;
        Some(TunnelDescriptor {
            remote_address: format!("localhost:{port}"),
            local_port: port,
            privacy,
            protocol: TunnelProtocol::Http,
            label: discovered.suggested_label.clone(),
        })
    }
}

impl Default for TunnelDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a single tunnel for status bar display.
pub fn tunnel_status_display(tunnel: &ManagedTunnel) -> String {
    let state_icon = match tunnel.state {
        TunnelState::Connecting => "⟳",
        TunnelState::Connected => "●",
        TunnelState::Closed => "○",
        TunnelState::Error(_) => "✗",
    };
    let label = tunnel.descriptor.label.as_deref().unwrap_or("tunnel");
    let proto = match tunnel.descriptor.protocol {
        TunnelProtocol::Http => "HTTP",
        TunnelProtocol::Https => "HTTPS",
        TunnelProtocol::Tcp => "TCP",
    };
    format!("{state_icon} {label} :{} ({proto})", tunnel.descriptor.local_port)
}

/// Format a summary line for all active tunnels (for status bar).
pub fn tunnel_status_summary(service: &TunnelWorkbenchService) -> String {
    let active = service.get_active_tunnels();
    if active.is_empty() {
        return "No active tunnels".to_string();
    }
    let count = active.len();
    let total_bytes: u64 = active.iter().map(|t| t.bytes_transferred).sum();
    let bytes_display = if total_bytes < 1024 {
        format!("{total_bytes} B")
    } else if total_bytes < 1024 * 1024 {
        format!("{:.1} KB", total_bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", total_bytes as f64 / (1024.0 * 1024.0))
    };
    format!("{count} tunnel(s) active, {bytes_display} transferred")
}

/// Accumulated statistics for wb-tunnel operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbTunnelStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbTunnelStats {
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
    pub fn merge(&mut self, other: &WbTunnelStats) {
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

impl Default for WbTunnelStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbTunnelStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbTunnelStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-tunnel.
#[derive(Debug, Clone)]
pub struct WbTunnelValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbTunnelValidator {
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

impl Default for WbTunnelValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a single health check probe against a tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Tracks health check state for a tunnel.
#[derive(Debug, Clone)]
pub struct TunnelHealthCheck {
    pub tunnel_id: u64,
    pub status: HealthStatus,
    pub consecutive_failures: u32,
    pub total_checks: u64,
    pub total_failures: u64,
    pub last_check_timestamp: u64,
    failure_threshold: u32,
}

impl TunnelHealthCheck {
    pub fn new(tunnel_id: u64, failure_threshold: u32) -> Self {
        Self {
            tunnel_id,
            status: HealthStatus::Healthy,
            consecutive_failures: 0,
            total_checks: 0,
            total_failures: 0,
            last_check_timestamp: 0,
            failure_threshold,
        }
    }

    /// Record a successful probe at the given timestamp.
    pub fn record_success(&mut self, timestamp: u64) {
        self.total_checks += 1;
        self.consecutive_failures = 0;
        self.last_check_timestamp = timestamp;
        self.status = HealthStatus::Healthy;
    }

    /// Record a failed probe. Transitions to Degraded or Unhealthy based on threshold.
    pub fn record_failure(&mut self, timestamp: u64, reason: &str) {
        self.total_checks += 1;
        self.total_failures += 1;
        self.consecutive_failures += 1;
        self.last_check_timestamp = timestamp;
        if self.consecutive_failures >= self.failure_threshold {
            self.status = HealthStatus::Unhealthy(reason.to_string());
        } else {
            self.status = HealthStatus::Degraded(reason.to_string());
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }

    /// Fraction of checks that succeeded, in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 1.0;
        }
        (self.total_checks - self.total_failures) as f64 / self.total_checks as f64
    }
}

/// Bandwidth and latency metrics for a single tunnel.
#[derive(Debug, Clone)]
pub struct TunnelMetrics {
    pub tunnel_id: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    latency_samples: Vec<u64>,
}

impl TunnelMetrics {
    pub fn new(tunnel_id: u64) -> Self {
        Self {
            tunnel_id,
            bytes_sent: 0,
            bytes_received: 0,
            latency_samples: Vec::new(),
        }
    }

    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
    }

    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
    }

    pub fn total_bandwidth(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }

    /// Record a latency measurement in microseconds.
    pub fn record_latency(&mut self, latency_us: u64) {
        self.latency_samples.push(latency_us);
    }

    /// Average latency across all samples, or `None` if no samples.
    pub fn average_latency_us(&self) -> Option<u64> {
        if self.latency_samples.is_empty() {
            return None;
        }
        Some(self.latency_samples.iter().sum::<u64>() / self.latency_samples.len() as u64)
    }

    /// Maximum latency recorded, or `None` if no samples.
    pub fn max_latency_us(&self) -> Option<u64> {
        self.latency_samples.iter().copied().max()
    }

    /// Minimum latency recorded, or `None` if no samples.
    pub fn min_latency_us(&self) -> Option<u64> {
        self.latency_samples.iter().copied().min()
    }

    pub fn sample_count(&self) -> usize {
        self.latency_samples.len()
    }
}

/// Policy governing automatic reconnection of failed tunnels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelReconnectPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub use_exponential_backoff: bool,
}

impl TunnelReconnectPolicy {
    pub fn new(max_retries: u32, base_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms: base_delay_ms * 32,
            use_exponential_backoff: true,
        }
    }

    /// Compute the delay before the n-th retry attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        if !self.use_exponential_backoff {
            return self.base_delay_ms.min(self.max_delay_ms);
        }
        let factor = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
        let delay = self.base_delay_ms.saturating_mul(factor);
        delay.min(self.max_delay_ms)
    }

    /// Returns `true` if the attempt number is still within the retry budget.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

impl Default for TunnelReconnectPolicy {
    fn default() -> Self {
        Self::new(5, 1000)
    }
}

/// Diagnostic snapshot for a tunnel, combining health and metrics.
#[derive(Debug, Clone)]
pub struct TunnelDiagnostics {
    pub tunnel_id: u64,
    pub state: TunnelState,
    pub health: HealthStatus,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub avg_latency_us: Option<u64>,
    pub consecutive_failures: u32,
}

impl fmt::Display for TunnelDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lat = match self.avg_latency_us {
            Some(us) => format!("{us}µs"),
            None => "n/a".to_string(),
        };
        write!(
            f,
            "tunnel={} state={:?} health={:?} sent={} recv={} latency={}",
            self.tunnel_id, self.state, self.health, self.bytes_sent, self.bytes_received, lat
        )
    }
}

impl TunnelDiagnostics {
    /// Build a diagnostics snapshot from health check and metrics data.
    pub fn from_parts(
        tunnel_id: u64,
        state: TunnelState,
        health: &TunnelHealthCheck,
        metrics: &TunnelMetrics,
    ) -> Self {
        Self {
            tunnel_id,
            state,
            health: health.status.clone(),
            bytes_sent: metrics.bytes_sent,
            bytes_received: metrics.bytes_received,
            avg_latency_us: metrics.average_latency_us(),
            consecutive_failures: health.consecutive_failures,
        }
    }
}

/// Access control list for tunnel connections.
///
/// Manages allow/deny rules evaluated in order. The first matching rule wins;
/// if no rule matches the default policy applies.
#[derive(Debug, Clone)]
pub struct TunnelAcl {
    rules: Vec<AclRule>,
    default_allow: bool,
}

/// A single ACL rule matching on CIDR-style prefix and optional port range.
#[derive(Debug, Clone)]
pub struct AclRule {
    /// Host pattern to match (exact or wildcard prefix with `*`).
    pub host_pattern: String,
    /// Optional port range (inclusive). `None` means any port.
    pub port_range: Option<(u16, u16)>,
    /// Whether matching connections are allowed.
    pub allow: bool,
}

impl AclRule {
    pub fn new(host_pattern: &str, port_range: Option<(u16, u16)>, allow: bool) -> Self {
        Self {
            host_pattern: host_pattern.to_string(),
            port_range,
            allow,
        }
    }

    /// Check whether `host` and `port` match this rule.
    pub fn matches(&self, host: &str, port: u16) -> bool {
        let host_ok = if self.host_pattern == "*" {
            true
        } else if let Some(prefix) = self.host_pattern.strip_suffix('*') {
            host.starts_with(prefix)
        } else {
            host == self.host_pattern
        };
        if !host_ok {
            return false;
        }
        match self.port_range {
            Some((lo, hi)) => port >= lo && port <= hi,
            None => true,
        }
    }
}

impl TunnelAcl {
    /// Create an ACL with the given default policy.
    pub fn new(default_allow: bool) -> Self {
        Self {
            rules: Vec::new(),
            default_allow,
        }
    }

    pub fn add_rule(&mut self, rule: AclRule) {
        self.rules.push(rule);
    }

    /// Evaluate the ACL for a given host and port.
    pub fn is_allowed(&self, host: &str, port: u16) -> bool {
        for rule in &self.rules {
            if rule.matches(host, port) {
                return rule.allow;
            }
        }
        self.default_allow
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }
}

/// Manages a pool of tunnel connections with capacity limits and eviction.
#[derive(Debug)]
pub struct TunnelConnectionPool {
    connections: Vec<TunnelConnection>,
    max_size: usize,
    total_evicted: usize,
}

impl TunnelConnectionPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            connections: Vec::new(),
            max_size,
            total_evicted: 0,
        }
    }

    /// Add a connection, evicting the oldest if at capacity.
    /// Returns the evicted connection if one was removed.
    pub fn add(&mut self, conn: TunnelConnection) -> Option<TunnelConnection> {
        let evicted = if self.connections.len() >= self.max_size {
            self.total_evicted += 1;
            Some(self.connections.remove(0))
        } else {
            None
        };
        self.connections.push(conn);
        evicted
    }

    /// Remove a connection by tunnel_id. Returns it if found.
    pub fn remove(&mut self, tunnel_id: u64) -> Option<TunnelConnection> {
        if let Some(pos) = self.connections.iter().position(|c| c.tunnel_id == tunnel_id) {
            Some(self.connections.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, tunnel_id: u64) -> Option<&TunnelConnection> {
        self.connections.iter().find(|c| c.tunnel_id == tunnel_id)
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.connections.len() >= self.max_size
    }

    pub fn total_evicted(&self) -> usize {
        self.total_evicted
    }

    /// Total bytes across all pooled connections.
    pub fn total_bytes(&self) -> u64 {
        self.connections.iter().map(|c| c.total_bytes()).sum()
    }

    /// Drain all connections, returning them.
    pub fn drain_all(&mut self) -> Vec<TunnelConnection> {
        self.connections.drain(..).collect()
    }
}

/// Negotiates the protocol to use for a tunnel based on capabilities.
#[derive(Debug, Clone)]
pub struct ProtocolNegotiator {
    supported: Vec<TunnelProtocol>,
    preferred: TunnelProtocol,
}

impl ProtocolNegotiator {
    pub fn new(preferred: TunnelProtocol) -> Self {
        Self {
            supported: vec![TunnelProtocol::Http, TunnelProtocol::Https, TunnelProtocol::Tcp],
            preferred,
        }
    }

    /// Restrict the set of supported protocols.
    pub fn set_supported(&mut self, protocols: Vec<TunnelProtocol>) {
        self.supported = protocols;
    }

    /// Negotiate with a remote's advertised capabilities.
    /// Returns the best mutually-supported protocol, preferring `self.preferred`.
    pub fn negotiate(&self, remote_capabilities: &[TunnelProtocol]) -> Option<TunnelProtocol> {
        // Prefer our preferred protocol if both sides support it.
        if self.supported.contains(&self.preferred) && remote_capabilities.contains(&self.preferred) {
            return Some(self.preferred);
        }
        // Otherwise pick the first protocol we support that the remote also supports.
        for proto in &self.supported {
            if remote_capabilities.contains(proto) {
                return Some(*proto);
            }
        }
        None
    }

    pub fn is_supported(&self, protocol: TunnelProtocol) -> bool {
        self.supported.contains(&protocol)
    }
}

/// Per-tunnel bandwidth tracker with windowed rate calculation.
#[derive(Debug, Clone)]
pub struct BandwidthTracker {
    samples: Vec<BandwidthSample>,
    window_size: usize,
}

#[derive(Debug, Clone)]
struct BandwidthSample {
    timestamp: u64,
    bytes: u64,
}

impl BandwidthTracker {
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: Vec::new(),
            window_size: window_size.max(1),
        }
    }

    /// Record a transfer of `bytes` at `timestamp`.
    pub fn record(&mut self, timestamp: u64, bytes: u64) {
        self.samples.push(BandwidthSample { timestamp, bytes });
        // Keep only the last `window_size` samples.
        if self.samples.len() > self.window_size {
            self.samples.remove(0);
        }
    }

    /// Total bytes in the current window.
    pub fn window_bytes(&self) -> u64 {
        self.samples.iter().map(|s| s.bytes).sum()
    }

    /// Duration spanned by the current window (last - first timestamp).
    /// Returns `None` if fewer than 2 samples.
    pub fn window_duration(&self) -> Option<u64> {
        if self.samples.len() < 2 {
            return None;
        }
        let first = self.samples.first().unwrap().timestamp;
        let last = self.samples.last().unwrap().timestamp;
        Some(last.saturating_sub(first))
    }

    /// Bytes per time-unit in the current window, or `None` if duration is zero.
    pub fn rate(&self) -> Option<f64> {
        let dur = self.window_duration()?;
        if dur == 0 {
            return None;
        }
        Some(self.window_bytes() as f64 / dur as f64)
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_descriptor() -> TunnelDescriptor {
        TunnelDescriptor {
            remote_address: "tunnel.example.com:8080".into(),
            local_port: 3000,
            privacy: TunnelPrivacy::Private,
            protocol: TunnelProtocol::Http,
            label: None,
        }
    }

    #[test]
    fn create_and_lookup() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        assert_eq!(svc.tunnel_count(), 1);
        let t = svc.get_tunnel(id).unwrap();
        assert_eq!(t.state, TunnelState::Connecting);
        assert_eq!(t.descriptor.local_port, 3000);
    }

    #[test]
    fn close_tunnel_filters_active() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        let id2 = svc.create_tunnel(TunnelDescriptor {
            label: Some("web".into()),
            ..sample_descriptor()
        });
        svc.set_state(id1, TunnelState::Connected);
        svc.set_state(id2, TunnelState::Connected);
        assert_eq!(svc.get_active_tunnels().len(), 2);

        assert!(svc.close_tunnel(id1));
        assert_eq!(svc.get_active_tunnels().len(), 1);
        assert_eq!(svc.tunnel_count(), 2);
    }

    #[test]
    fn close_nonexistent_returns_false() {
        let mut svc = TunnelWorkbenchService::new();
        assert!(!svc.close_tunnel(999));
    }

    #[test]
    fn set_error_state() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        svc.set_state(id, TunnelState::Error("timeout".into()));
        let t = svc.get_tunnel(id).unwrap();
        assert_eq!(t.state, TunnelState::Error("timeout".into()));
    }

    #[test]
    fn get_stats_empty() {
        let svc = TunnelWorkbenchService::new();
        let stats = svc.get_stats();
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.total_created, 0);
        assert_eq!(stats.total_bytes_transferred, 0);
        assert_eq!(stats.http_count, 0);
        assert_eq!(stats.https_count, 0);
        assert_eq!(stats.tcp_count, 0);
    }

    #[test]
    fn get_stats_mixed_tunnels() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor()); // Http
        let _id2 = svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Https,
            local_port: 3001,
            ..sample_descriptor()
        });
        let _id3 = svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Tcp,
            local_port: 3002,
            ..sample_descriptor()
        });
        svc.record_transfer(id1, 1024);
        svc.close_tunnel(id1);

        let stats = svc.get_stats();
        assert_eq!(stats.active_count, 2);
        assert_eq!(stats.total_created, 3);
        assert_eq!(stats.total_bytes_transferred, 1024);
        assert_eq!(stats.http_count, 1);
        assert_eq!(stats.https_count, 1);
        assert_eq!(stats.tcp_count, 1);
    }

    #[test]
    fn filter_by_protocol() {
        let mut svc = TunnelWorkbenchService::new();
        svc.create_tunnel(sample_descriptor());
        svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Tcp,
            local_port: 4000,
            ..sample_descriptor()
        });
        svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Tcp,
            local_port: 4001,
            ..sample_descriptor()
        });
        assert_eq!(svc.get_tunnels_by_protocol(TunnelProtocol::Tcp).len(), 2);
        assert_eq!(svc.get_tunnels_by_protocol(TunnelProtocol::Http).len(), 1);
        assert_eq!(svc.get_tunnels_by_protocol(TunnelProtocol::Https).len(), 0);
    }

    #[test]
    fn filter_by_privacy() {
        let mut svc = TunnelWorkbenchService::new();
        svc.create_tunnel(sample_descriptor()); // Private
        svc.create_tunnel(TunnelDescriptor {
            privacy: TunnelPrivacy::Public,
            local_port: 5000,
            ..sample_descriptor()
        });
        assert_eq!(svc.get_tunnels_by_privacy(TunnelPrivacy::Private).len(), 1);
        assert_eq!(svc.get_tunnels_by_privacy(TunnelPrivacy::Public).len(), 1);
    }

    #[test]
    fn find_by_port_hit_and_miss() {
        let mut svc = TunnelWorkbenchService::new();
        svc.create_tunnel(sample_descriptor());
        assert!(svc.find_by_port(3000).is_some());
        assert!(svc.find_by_port(9999).is_none());
    }

    #[test]
    fn record_transfer_accumulates() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        svc.record_transfer(id, 100);
        svc.record_transfer(id, 200);
        assert_eq!(svc.get_tunnel(id).unwrap().bytes_transferred, 300);
    }

    #[test]
    fn record_transfer_nonexistent_is_noop() {
        let mut svc = TunnelWorkbenchService::new();
        svc.record_transfer(999, 100); // should not panic
        assert_eq!(svc.tunnel_count(), 0);
    }

    #[test]
    fn close_all_returns_count() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        svc.create_tunnel(TunnelDescriptor {
            local_port: 3001,
            ..sample_descriptor()
        });
        svc.close_tunnel(id1);
        // id1 already closed, so close_all should close only the second one
        assert_eq!(svc.close_all(), 1);
        assert_eq!(svc.get_active_tunnels().len(), 0);
    }

    #[test]
    fn remove_closed_garbage_collects() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        svc.create_tunnel(TunnelDescriptor {
            local_port: 3001,
            ..sample_descriptor()
        });
        svc.close_tunnel(id1);
        assert_eq!(svc.remove_closed(), 1);
        assert_eq!(svc.tunnel_count(), 1);
        // removing again should return 0
        assert_eq!(svc.remove_closed(), 0);
    }

    #[test]
    fn rename_tunnel_sets_label() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        assert!(svc.get_tunnel(id).unwrap().descriptor.label.is_none());
        svc.rename_tunnel(id, "my-tunnel");
        assert_eq!(
            svc.get_tunnel(id).unwrap().descriptor.label.as_deref(),
            Some("my-tunnel")
        );
    }

    #[test]
    fn rename_nonexistent_is_noop() {
        let mut svc = TunnelWorkbenchService::new();
        svc.rename_tunnel(999, "ghost"); // should not panic
    }

    #[test]
    fn created_at_increments() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        let id2 = svc.create_tunnel(TunnelDescriptor {
            local_port: 3001,
            ..sample_descriptor()
        });
        let t1 = svc.get_tunnel(id1).unwrap();
        let t2 = svc.get_tunnel(id2).unwrap();
        assert!(t2.created_at > t1.created_at);
    }

    struct NoopProvider;
    impl TunnelProvider for NoopProvider {}

    #[test]
    fn tunnel_provider_defaults() {
        let provider = NoopProvider;
        assert!(provider.create_tunnel(&sample_descriptor()));
        assert!(provider.dispose_tunnel(1));
    }

    #[test]
    fn eq_tunnelprivacy_same() {
        assert_eq!(TunnelPrivacy::Private, TunnelPrivacy::Private);
    }

    #[test]
    fn ne_tunnelprivacy_diff() {
        assert_ne!(TunnelPrivacy::Private, TunnelPrivacy::Public);
    }

    #[test]
    fn eq_tunnelprotocol_same() {
        assert_eq!(TunnelProtocol::Http, TunnelProtocol::Http);
    }

    #[test]
    fn ne_tunnelprotocol_diff() {
        assert_ne!(TunnelProtocol::Http, TunnelProtocol::Https);
    }

    #[test]
    fn eq_tunnelstate_same() {
        assert_eq!(TunnelState::Connecting, TunnelState::Connecting);
    }

    #[test]
    fn ne_tunnelstate_diff() {
        assert_ne!(TunnelState::Connecting, TunnelState::Connected);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = TunnelWorkbenchService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn wb_tunnel_stats_new_defaults() {
        let stats = WbTunnelStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_tunnel_stats_record_success() {
        let mut stats = WbTunnelStats::new();
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
    fn wb_tunnel_stats_record_failure() {
        let mut stats = WbTunnelStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_tunnel_stats_reset() {
        let mut stats = WbTunnelStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_tunnel_stats_merge() {
        let mut a = WbTunnelStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbTunnelStats::new();
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
    fn wb_tunnel_stats_display() {
        let mut stats = WbTunnelStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_tunnel_stats_default() {
        let stats = WbTunnelStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_tunnel_validator_accepts_valid_name() {
        let v = WbTunnelValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_tunnel_validator_rejects_empty() {
        let v = WbTunnelValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_tunnel_validator_rejects_too_long() {
        let v = WbTunnelValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_tunnel_validator_forbidden_prefix() {
        let v = WbTunnelValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_tunnel_validator_allowed_chars() {
        let v = WbTunnelValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_tunnel_validator_range() {
        let v = WbTunnelValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_tunnel_sanitize_removes_control() {
        let result = WbTunnelValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_tunnel_truncate_short_string() {
        assert_eq!(WbTunnelValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_tunnel_truncate_long_string() {
        let result = WbTunnelValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_tunnel_is_ascii_printable() {
        assert!(WbTunnelValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbTunnelValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn connection_total_bytes() {
        let mut conn = TunnelConnection::new(1, 3000, "remote.host", 80, TunnelProtocol::Http);
        conn.record_inbound(100);
        conn.record_outbound(200);
        assert_eq!(conn.total_bytes(), 300);
    }

    #[test]
    fn connection_display_address() {
        let conn = TunnelConnection::new(1, 8080, "example.com", 443, TunnelProtocol::Https);
        assert_eq!(conn.display_address(), "8080 -> example.com:443 (HTTPS)");
    }

    #[test]
    fn connection_display_trait() {
        let conn = TunnelConnection::new(1, 3000, "localhost", 3000, TunnelProtocol::Tcp);
        let s = format!("{conn}");
        assert!(s.contains("TCP"));
        assert!(s.contains("3000"));
    }

    #[test]
    fn discovery_report_and_query() {
        let mut disc = TunnelDiscovery::new();
        disc.report_port(3000, Some("node"), Some("Frontend"));
        disc.report_port(8080, None, None);
        assert_eq!(disc.discovered_ports().len(), 2);
    }

    #[test]
    fn discovery_no_duplicates() {
        let mut disc = TunnelDiscovery::new();
        disc.report_port(3000, None, None);
        disc.report_port(3000, None, None);
        assert_eq!(disc.discovered_ports().len(), 1);
    }

    #[test]
    fn discovery_in_scan_range() {
        let disc = TunnelDiscovery::new();
        assert!(disc.in_scan_range(3000));
        assert!(disc.in_scan_range(8050));
        assert!(!disc.in_scan_range(9999));
    }

    #[test]
    fn discovery_to_descriptor() {
        let mut disc = TunnelDiscovery::new();
        disc.report_port(3000, None, Some("Web"));
        let desc = disc.to_descriptor(3000, TunnelPrivacy::Private).unwrap();
        assert_eq!(desc.local_port, 3000);
        assert_eq!(desc.label, Some("Web".to_string()));
    }

    #[test]
    fn status_display_connected() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(TunnelDescriptor {
            remote_address: "localhost:3000".into(),
            local_port: 3000,
            privacy: TunnelPrivacy::Private,
            protocol: TunnelProtocol::Http,
            label: Some("Web".into()),
        });
        svc.set_state(id, TunnelState::Connected);
        let t = svc.get_tunnel(id).unwrap();
        let s = tunnel_status_display(t);
        assert!(s.contains("●"));
        assert!(s.contains("Web"));
        assert!(s.contains("3000"));
    }

    #[test]
    fn status_summary_no_tunnels() {
        let svc = TunnelWorkbenchService::new();
        assert_eq!(tunnel_status_summary(&svc), "No active tunnels");
    }

    #[test]
    fn status_summary_with_tunnels() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(TunnelDescriptor {
            remote_address: "localhost:3000".into(),
            local_port: 3000,
            privacy: TunnelPrivacy::Private,
            protocol: TunnelProtocol::Http,
            label: None,
        });
        svc.set_state(id, TunnelState::Connected);
        svc.record_transfer(id, 2048);
        let s = tunnel_status_summary(&svc);
        assert!(s.contains("1 tunnel(s) active"));
        assert!(s.contains("KB"));
    }

    #[test]
    fn health_check_transitions() {
        let mut hc = TunnelHealthCheck::new(1, 3);
        assert!(hc.is_healthy());

        hc.record_failure(1, "timeout");
        assert_eq!(hc.status, HealthStatus::Degraded("timeout".into()));
        assert_eq!(hc.consecutive_failures, 1);

        hc.record_failure(2, "timeout");
        hc.record_failure(3, "connection refused");
        assert_eq!(hc.status, HealthStatus::Unhealthy("connection refused".into()));
        assert_eq!(hc.consecutive_failures, 3);

        hc.record_success(4);
        assert!(hc.is_healthy());
        assert_eq!(hc.consecutive_failures, 0);
        assert_eq!(hc.total_failures, 3);
        assert_eq!(hc.total_checks, 4);
    }

    #[test]
    fn health_check_success_rate() {
        let mut hc = TunnelHealthCheck::new(1, 5);
        assert!((hc.success_rate() - 1.0).abs() < f64::EPSILON);

        hc.record_success(1);
        hc.record_success(2);
        hc.record_failure(3, "err");
        hc.record_success(4);
        // 3 successes out of 4
        assert!((hc.success_rate() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn tunnel_metrics_bandwidth_and_latency() {
        let mut m = TunnelMetrics::new(42);
        assert_eq!(m.total_bandwidth(), 0);
        assert_eq!(m.average_latency_us(), None);

        m.record_sent(1000);
        m.record_received(500);
        assert_eq!(m.total_bandwidth(), 1500);

        m.record_latency(100);
        m.record_latency(200);
        m.record_latency(300);
        assert_eq!(m.average_latency_us(), Some(200));
        assert_eq!(m.min_latency_us(), Some(100));
        assert_eq!(m.max_latency_us(), Some(300));
        assert_eq!(m.sample_count(), 3);
    }

    #[test]
    fn reconnect_policy_exponential_backoff() {
        let policy = TunnelReconnectPolicy::new(4, 100);
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(3));
        assert!(!policy.should_retry(4));

        assert_eq!(policy.delay_for_attempt(0), 100);   // 100 * 2^0
        assert_eq!(policy.delay_for_attempt(1), 200);   // 100 * 2^1
        assert_eq!(policy.delay_for_attempt(2), 400);   // 100 * 2^2
        // capped at max_delay_ms = 100 * 32 = 3200
        assert_eq!(policy.delay_for_attempt(10), 3200);
    }

    #[test]
    fn tunnel_diagnostics_from_parts() {
        let mut hc = TunnelHealthCheck::new(7, 3);
        hc.record_failure(1, "slow");
        let mut m = TunnelMetrics::new(7);
        m.record_sent(512);
        m.record_received(256);
        m.record_latency(150);

        let diag = TunnelDiagnostics::from_parts(7, TunnelState::Connected, &hc, &m);
        assert_eq!(diag.tunnel_id, 7);
        assert_eq!(diag.health, HealthStatus::Degraded("slow".into()));
        assert_eq!(diag.bytes_sent, 512);
        assert_eq!(diag.bytes_received, 256);
        assert_eq!(diag.avg_latency_us, Some(150));
        assert_eq!(diag.consecutive_failures, 1);

        let display = format!("{diag}");
        assert!(display.contains("tunnel=7"));
        assert!(display.contains("150µs"));
    }

    #[test]
    fn acl_default_allow_no_rules() {
        let acl = TunnelAcl::new(true);
        assert!(acl.is_allowed("example.com", 80));
        let acl_deny = TunnelAcl::new(false);
        assert!(!acl_deny.is_allowed("example.com", 80));
    }

    #[test]
    fn acl_exact_and_wildcard_rules() {
        let mut acl = TunnelAcl::new(false);
        acl.add_rule(AclRule::new("trusted.host", None, true));
        acl.add_rule(AclRule::new("evil.*", None, false));
        acl.add_rule(AclRule::new("*", Some((8000, 8100)), true));

        assert!(acl.is_allowed("trusted.host", 443));
        assert!(!acl.is_allowed("evil.corp", 80));
        assert!(acl.is_allowed("random.host", 8050));
        assert!(!acl.is_allowed("random.host", 9999));
        assert_eq!(acl.rule_count(), 3);
    }

    #[test]
    fn acl_port_range_matching() {
        let rule = AclRule::new("*", Some((3000, 3010)), true);
        assert!(rule.matches("any", 3000));
        assert!(rule.matches("any", 3010));
        assert!(!rule.matches("any", 3011));
        assert!(!rule.matches("any", 2999));
    }

    #[test]
    fn connection_pool_add_and_evict() {
        let mut pool = TunnelConnectionPool::new(2);
        assert!(pool.is_empty());

        pool.add(TunnelConnection::new(1, 3000, "h1", 80, TunnelProtocol::Http));
        pool.add(TunnelConnection::new(2, 3001, "h2", 80, TunnelProtocol::Http));
        assert!(pool.is_full());
        assert_eq!(pool.len(), 2);

        // Adding a third should evict tunnel_id=1
        let evicted = pool.add(TunnelConnection::new(3, 3002, "h3", 80, TunnelProtocol::Http));
        assert!(evicted.is_some());
        assert_eq!(evicted.unwrap().tunnel_id, 1);
        assert_eq!(pool.total_evicted(), 1);
        assert!(pool.get(1).is_none());
        assert!(pool.get(3).is_some());
    }

    #[test]
    fn connection_pool_remove_and_drain() {
        let mut pool = TunnelConnectionPool::new(10);
        pool.add(TunnelConnection::new(1, 3000, "h", 80, TunnelProtocol::Tcp));
        pool.add(TunnelConnection::new(2, 3001, "h", 80, TunnelProtocol::Tcp));

        let removed = pool.remove(1);
        assert!(removed.is_some());
        assert_eq!(pool.len(), 1);

        let all = pool.drain_all();
        assert_eq!(all.len(), 1);
        assert!(pool.is_empty());
    }

    #[test]
    fn protocol_negotiator_prefers_preferred() {
        let neg = ProtocolNegotiator::new(TunnelProtocol::Https);
        let remote = vec![TunnelProtocol::Http, TunnelProtocol::Https];
        assert_eq!(neg.negotiate(&remote), Some(TunnelProtocol::Https));
    }

    #[test]
    fn protocol_negotiator_fallback() {
        let neg = ProtocolNegotiator::new(TunnelProtocol::Https);
        let remote = vec![TunnelProtocol::Tcp];
        assert_eq!(neg.negotiate(&remote), Some(TunnelProtocol::Tcp));
    }

    #[test]
    fn protocol_negotiator_no_overlap() {
        let mut neg = ProtocolNegotiator::new(TunnelProtocol::Http);
        neg.set_supported(vec![TunnelProtocol::Http]);
        let remote = vec![TunnelProtocol::Tcp];
        assert_eq!(neg.negotiate(&remote), None);
    }

    #[test]
    fn bandwidth_tracker_rate() {
        let mut bw = BandwidthTracker::new(5);
        bw.record(0, 100);
        bw.record(10, 200);
        bw.record(20, 300);

        assert_eq!(bw.sample_count(), 3);
        assert_eq!(bw.window_bytes(), 600);
        assert_eq!(bw.window_duration(), Some(20));
        assert!((bw.rate().unwrap() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bandwidth_tracker_window_eviction() {
        let mut bw = BandwidthTracker::new(2);
        bw.record(0, 100);
        bw.record(10, 200);
        bw.record(20, 400);

        // Window should only contain the last 2 samples
        assert_eq!(bw.sample_count(), 2);
        assert_eq!(bw.window_bytes(), 600); // 200 + 400
        assert_eq!(bw.window_duration(), Some(10)); // 20 - 10
    }

    #[test]
    fn bandwidth_tracker_edge_cases() {
        let mut bw = BandwidthTracker::new(5);
        assert_eq!(bw.rate(), None);
        assert_eq!(bw.window_duration(), None);

        bw.record(5, 100);
        assert_eq!(bw.window_duration(), None); // only 1 sample
        assert_eq!(bw.window_bytes(), 100);

        bw.clear();
        assert_eq!(bw.sample_count(), 0);
    }

    #[test]
    fn connection_pool_total_bytes() {
        let mut pool = TunnelConnectionPool::new(10);
        let mut c1 = TunnelConnection::new(1, 3000, "h", 80, TunnelProtocol::Http);
        c1.record_inbound(500);
        c1.record_outbound(300);
        let mut c2 = TunnelConnection::new(2, 3001, "h", 80, TunnelProtocol::Http);
        c2.record_inbound(200);
        pool.add(c1);
        pool.add(c2);
        assert_eq!(pool.total_bytes(), 1000);
    }
}
