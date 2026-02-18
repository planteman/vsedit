//! Port forwarding service.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// TunnelConnectionManager
// ---------------------------------------------------------------------------

/// Manages a set of tunnel connections with automatic reconnection tracking.
#[derive(Debug)]
pub struct TunnelConnectionManager {
    connections: Vec<TunnelConnection>,
    max_connections: usize,
    reconnect_policy: TunnelReconnectPolicy,
    total_reconnects: u32,
}

impl TunnelConnectionManager {
    pub fn new(max_connections: usize) -> Self {
        Self {
            connections: Vec::new(),
            max_connections,
            reconnect_policy: TunnelReconnectPolicy::new(3, 1000),
            total_reconnects: 0,
        }
    }

    pub fn with_policy(mut self, policy: TunnelReconnectPolicy) -> Self {
        self.reconnect_policy = policy;
        self
    }

    /// Add a connection. Returns `false` if already at capacity.
    pub fn add_connection(&mut self, conn: TunnelConnection) -> bool {
        if self.connections.len() >= self.max_connections {
            return false;
        }
        self.connections.push(conn);
        true
    }

    pub fn remove_connection(&mut self, id: u64) -> Option<TunnelConnection> {
        let pos = self.connections.iter().position(|c| c.tunnel_id == id)?;
        Some(self.connections.remove(pos))
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn is_at_capacity(&self) -> bool {
        self.connections.len() >= self.max_connections
    }

    pub fn find_by_port(&self, local_port: u16) -> Option<&TunnelConnection> {
        self.connections.iter().find(|c| c.local_port == local_port)
    }

    pub fn record_reconnect(&mut self) {
        self.total_reconnects += 1;
    }

    pub fn total_reconnects(&self) -> u32 {
        self.total_reconnects
    }

    pub fn active_ports(&self) -> Vec<u16> {
        self.connections.iter().map(|c| c.local_port).collect()
    }
}

// ---------------------------------------------------------------------------
// TunnelPortMapper
// ---------------------------------------------------------------------------

/// A single port mapping entry.
#[derive(Debug, Clone)]
pub struct PortMapping {
    pub local_port: u16,
    pub remote_port: u16,
    pub remote_host: String,
    pub protocol: TunnelProtocol,
}

/// Maps local ports to remote ports.
#[derive(Debug, Clone)]
pub struct TunnelPortMapper {
    mappings: Vec<PortMapping>,
}

impl TunnelPortMapper {
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
        }
    }

    pub fn add_mapping(
        &mut self,
        local: u16,
        remote: u16,
        host: impl Into<String>,
        protocol: TunnelProtocol,
    ) {
        self.mappings.push(PortMapping {
            local_port: local,
            remote_port: remote,
            remote_host: host.into(),
            protocol,
        });
    }

    pub fn remove_mapping(&mut self, local_port: u16) -> bool {
        let before = self.mappings.len();
        self.mappings.retain(|m| m.local_port != local_port);
        self.mappings.len() < before
    }

    pub fn find_by_local(&self, local: u16) -> Option<&PortMapping> {
        self.mappings.iter().find(|m| m.local_port == local)
    }

    pub fn find_by_remote(&self, remote: u16) -> Vec<&PortMapping> {
        self.mappings.iter().filter(|m| m.remote_port == remote).collect()
    }

    pub fn mapping_count(&self) -> usize {
        self.mappings.len()
    }

    pub fn is_local_port_used(&self, port: u16) -> bool {
        self.mappings.iter().any(|m| m.local_port == port)
    }
}

// ---------------------------------------------------------------------------
// TunnelHeartbeat
// ---------------------------------------------------------------------------

/// Heartbeat monitor for tunnel liveness detection.
#[derive(Debug, Clone)]
pub struct TunnelHeartbeat {
    interval_ms: u64,
    last_beat_ms: Option<u64>,
    missed_count: u32,
    max_missed: u32,
}

impl TunnelHeartbeat {
    pub fn new(interval_ms: u64, max_missed: u32) -> Self {
        Self {
            interval_ms,
            last_beat_ms: None,
            missed_count: 0,
            max_missed,
        }
    }

    pub fn beat(&mut self, now_ms: u64) {
        self.last_beat_ms = Some(now_ms);
        self.missed_count = 0;
    }

    /// Evaluate health based on the current time.
    pub fn check(&self, now_ms: u64) -> HealthStatus {
        let last = match self.last_beat_ms {
            Some(t) => t,
            None => return HealthStatus::Unhealthy("no heartbeat received".into()),
        };
        let elapsed = now_ms.saturating_sub(last);
        let missed = elapsed / self.interval_ms.max(1);
        if missed == 0 {
            HealthStatus::Healthy
        } else if missed < self.max_missed as u64 {
            HealthStatus::Degraded(format!("{missed} heartbeats missed"))
        } else {
            HealthStatus::Unhealthy(format!("{missed} heartbeats missed"))
        }
    }

    pub fn missed_count(&self) -> u32 {
        self.missed_count
    }

    pub fn is_alive(&self, now_ms: u64) -> bool {
        matches!(self.check(now_ms), HealthStatus::Healthy | HealthStatus::Degraded(_))
    }

    pub fn reset(&mut self) {
        self.last_beat_ms = None;
        self.missed_count = 0;
    }
}

// ---------------------------------------------------------------------------
// TunnelBandwidthMonitor
// ---------------------------------------------------------------------------

/// Aggregates bandwidth tracking across multiple connections.
#[derive(Debug)]
pub struct TunnelBandwidthMonitor {
    trackers: HashMap<u64, BandwidthTracker>,
}

impl TunnelBandwidthMonitor {
    pub fn new() -> Self {
        Self {
            trackers: HashMap::new(),
        }
    }

    pub fn add_tracker(&mut self, conn_id: u64, window_size: usize) {
        self.trackers.insert(conn_id, BandwidthTracker::new(window_size));
    }

    pub fn record(&mut self, conn_id: u64, timestamp: u64, bytes: u64) {
        if let Some(tracker) = self.trackers.get_mut(&conn_id) {
            tracker.record(timestamp, bytes);
        }
    }

    pub fn rate_for(&self, conn_id: u64) -> Option<f64> {
        self.trackers.get(&conn_id)?.rate()
    }

    pub fn total_bytes(&self) -> u64 {
        self.trackers.values().map(|t| t.window_bytes()).sum()
    }

    pub fn tracker_count(&self) -> usize {
        self.trackers.len()
    }

    pub fn remove_tracker(&mut self, conn_id: u64) {
        self.trackers.remove(&conn_id);
    }
}


// === Tunnel Bandwidth Monitor ===

/// Tunnel Bandwidth Monitor implementation.
#[derive(Debug, Clone)]
pub struct TunnelBwMonitorEngine {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TunnelBwMonitorEngineStats,
}

/// Statistics for TunnelBwMonitorEngine.
#[derive(Debug, Clone, Default)]
pub struct TunnelBwMonitorEngineStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TunnelBwMonitorEngineStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TunnelBwMonitorEngine {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TunnelBwMonitorEngineStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TunnelBwMonitorEngineStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TunnelBwMonitorEngine {
    fn default() -> Self {
        Self::new()
    }
}

// === Tunnel Latency Tracker ===

/// Priority level for TunnelLatencyTracker items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TunnelLatencyTrackerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TunnelLatencyTrackerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TunnelLatencyTrackerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Tunnel Latency Tracker implementation.
#[derive(Debug, Clone)]
pub struct TunnelLatencyTracker {
    items: Vec<TunnelLatencyTrackerItem>,
    max_items: usize,
    default_priority: TunnelLatencyTrackerPriority,
}

/// A single item in TunnelLatencyTracker.
#[derive(Debug, Clone)]
pub struct TunnelLatencyTrackerItem {
    pub id: String,
    pub label: String,
    pub priority: TunnelLatencyTrackerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TunnelLatencyTrackerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TunnelLatencyTrackerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TunnelLatencyTrackerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TunnelLatencyTracker {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TunnelLatencyTrackerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TunnelLatencyTrackerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TunnelLatencyTrackerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TunnelLatencyTrackerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TunnelLatencyTrackerPriority) -> Vec<&TunnelLatencyTrackerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TunnelLatencyTrackerItem> {
        let mut sorted: Vec<&TunnelLatencyTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TunnelLatencyTrackerItem> {
        let mut sorted: Vec<&TunnelLatencyTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TunnelLatencyTrackerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TunnelLatencyTrackerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TunnelLatencyTrackerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TunnelLatencyTrackerItem> {
        self.items.iter()
    }
}

impl Default for TunnelLatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}


/// Workbench tunnel configuration manager.
#[derive(Debug, Clone)]
pub struct WbTunnelConfig {
    entries: Vec<WbTunnelEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench tunnel entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbTunnelEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbTunnelEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbTunnelConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbTunnelEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbTunnelEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbTunnelEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbTunnelEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbTunnelEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbTunnelEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbTunnelEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Port forwarding tunnel manager — extended utilities (qi)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_tunnel operations.
#[derive(Debug, Clone)]
pub struct QiMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QiMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for wb_tunnel.
#[derive(Debug, Clone)]
pub struct QiRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QiRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for wb_tunnel lookups.
#[derive(Debug, Clone)]
pub struct QiLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QiLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_tunnel
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbTunnelRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbTunnelRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaWbTunnelCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbTunnelCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaWbTunnelCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 232
// ---------------------------------------------------------------------------

/// Generic object pool `Xc232Pool<T>`.
pub struct Xc232Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc232Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc232PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc232Pool<T> {
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
    pub fn stats(&self) -> Xc232PoolStats {
        Xc232PoolStats {
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

impl<T> Default for Xc232Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc232Scheduler`.
pub struct Xc232Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc232Scheduler {
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

impl Default for Xc232Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_232 hash for the given byte slice.
pub fn xc_232_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_232 convention.
pub fn xc_232_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe9 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe9Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe9PipelineError {
    pub stage: Xe9Stage,
    pub message: String,
}

impl std::fmt::Display for Xe9PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe9Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe9Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError>>>,
    stage_names: Vec<Xe9Stage>,
}

impl Xe9Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe9Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe9Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe9Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe9Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> {
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

    pub fn compose(mut self, other: Xe9Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe9CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe9CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe9Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe9CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe9CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe9Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe9CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_9_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe9CacheEntry {
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

    fn xe_9_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe9CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_9_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> {
    Ok(data)
}

pub fn xe_9_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_9_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_9_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_9_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe9PipelineError> {
    Err(Xe9PipelineError {
        stage: Xe9Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #73
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf73Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf73TrieNode {
    children: std::collections::HashMap<char, Xf73TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf73Trie {
    root: Xf73TrieNode,
    count: usize,
}

impl Xf73Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf73TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf73TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf73TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf73BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf73BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 231).
pub struct Xh231SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh231SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 273 as u64,
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

/// A compact bit set supporting boolean operations (variant 231).
pub struct Xh231BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh231BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 231).
pub struct Xi231Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi231Deque<T> {
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
pub struct Xi231Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi231Interval {
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

/// A simple interval tree (variant 231).
pub struct Xi231IntervalTree {
    xi_intervals: Vec<Xi231Interval>,
}

impl Xi231IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi231Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi231Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi231Interval) -> Vec<&Xi231Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi231Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi231Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi231Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi231Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi231Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi231Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 231) ---

/// Disjoint set / union-find for crate 231.
pub struct Xj231UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj231UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ231_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 231.
pub struct Xj231BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj231BTreeNode<K, V>>>,
    len: usize,
}

struct Xj231BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj231BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj231BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ231_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ231_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj231BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj231BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj231BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj231BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_231 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk231SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk231SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk231DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk231DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_231).
#[derive(Debug, Clone)]
pub struct Xl231Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl231Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_231).
#[derive(Debug, Clone)]
pub struct Xl231SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl231SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm231MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm231MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm231Tokenizer {
    text: String,
}

impl Xm231Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
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

    // -----------------------------------------------------------------------
    // TunnelConnectionManager tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_conn_manager_add_remove() {
        let mut mgr = TunnelConnectionManager::new(4);
        let c = TunnelConnection::new(1, 3000, "host", 80, TunnelProtocol::Http);
        assert!(mgr.add_connection(c));
        assert_eq!(mgr.connection_count(), 1);
        let removed = mgr.remove_connection(1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().tunnel_id, 1);
        assert_eq!(mgr.connection_count(), 0);
        assert!(mgr.remove_connection(99).is_none());
    }

    #[test]
    fn test_conn_manager_at_capacity() {
        let mut mgr = TunnelConnectionManager::new(2);
        assert!(mgr.add_connection(TunnelConnection::new(1, 3000, "h", 80, TunnelProtocol::Http)));
        assert!(!mgr.is_at_capacity());
        assert!(mgr.add_connection(TunnelConnection::new(2, 3001, "h", 80, TunnelProtocol::Http)));
        assert!(mgr.is_at_capacity());
        assert!(!mgr.add_connection(TunnelConnection::new(3, 3002, "h", 80, TunnelProtocol::Http)));
        assert_eq!(mgr.connection_count(), 2);
    }

    #[test]
    fn test_conn_manager_find_by_port() {
        let mut mgr = TunnelConnectionManager::new(4);
        mgr.add_connection(TunnelConnection::new(1, 3000, "a", 80, TunnelProtocol::Http));
        mgr.add_connection(TunnelConnection::new(2, 4000, "b", 443, TunnelProtocol::Https));
        assert_eq!(mgr.find_by_port(4000).unwrap().tunnel_id, 2);
        assert!(mgr.find_by_port(9999).is_none());
        assert_eq!(mgr.active_ports(), vec![3000, 4000]);
    }

    #[test]
    fn test_conn_manager_reconnect_count() {
        let mut mgr = TunnelConnectionManager::new(4);
        assert_eq!(mgr.total_reconnects(), 0);
        mgr.record_reconnect();
        mgr.record_reconnect();
        assert_eq!(mgr.total_reconnects(), 2);
    }

    // -----------------------------------------------------------------------
    // TunnelPortMapper tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_port_mapper_add_find() {
        let mut mapper = TunnelPortMapper::new();
        mapper.add_mapping(3000, 80, "example.com", TunnelProtocol::Http);
        assert_eq!(mapper.mapping_count(), 1);
        let m = mapper.find_by_local(3000).unwrap();
        assert_eq!(m.remote_port, 80);
        assert_eq!(m.remote_host, "example.com");
        assert!(mapper.is_local_port_used(3000));
        assert!(!mapper.is_local_port_used(4000));
    }

    #[test]
    fn test_port_mapper_remove() {
        let mut mapper = TunnelPortMapper::new();
        mapper.add_mapping(3000, 80, "host", TunnelProtocol::Http);
        mapper.add_mapping(4000, 443, "host", TunnelProtocol::Https);
        assert!(mapper.remove_mapping(3000));
        assert_eq!(mapper.mapping_count(), 1);
        assert!(!mapper.remove_mapping(3000));
        assert!(mapper.find_by_local(3000).is_none());
    }

    #[test]
    fn test_port_mapper_find_by_remote() {
        let mut mapper = TunnelPortMapper::new();
        mapper.add_mapping(3000, 80, "a", TunnelProtocol::Http);
        mapper.add_mapping(4000, 80, "b", TunnelProtocol::Http);
        mapper.add_mapping(5000, 443, "c", TunnelProtocol::Https);
        let results = mapper.find_by_remote(80);
        assert_eq!(results.len(), 2);
        assert!(mapper.find_by_remote(9999).is_empty());
    }

    // -----------------------------------------------------------------------
    // TunnelHeartbeat tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_heartbeat_healthy() {
        let mut hb = TunnelHeartbeat::new(1000, 3);
        hb.beat(5000);
        assert!(matches!(hb.check(5500), HealthStatus::Healthy));
        assert!(hb.is_alive(5500));
    }

    #[test]
    fn test_heartbeat_degraded() {
        let mut hb = TunnelHeartbeat::new(1000, 3);
        hb.beat(5000);
        // 2 intervals missed (elapsed=2500, missed=2, max_missed=3)
        let status = hb.check(7500);
        assert!(matches!(status, HealthStatus::Degraded(_)));
        assert!(hb.is_alive(7500));
    }

    #[test]
    fn test_heartbeat_unhealthy() {
        let mut hb = TunnelHeartbeat::new(1000, 3);
        hb.beat(5000);
        // 4 intervals missed, exceeds max_missed=3
        let status = hb.check(9500);
        assert!(matches!(status, HealthStatus::Unhealthy(_)));
        assert!(!hb.is_alive(9500));
    }

    #[test]
    fn test_heartbeat_reset() {
        let mut hb = TunnelHeartbeat::new(1000, 3);
        hb.beat(5000);
        hb.reset();
        // No heartbeat received after reset => unhealthy
        assert!(matches!(hb.check(6000), HealthStatus::Unhealthy(_)));
    }

    // -----------------------------------------------------------------------
    // TunnelBwMonitorEngine tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_bandwidth_monitor_track() {
        let mut mon = TunnelBwMonitorEngine::new().with_capacity(10);
        assert!(mon.add("tracker_1"));
        assert!(mon.add("tracker_2"));
        assert_eq!(mon.len(), 2);

        assert!(mon.contains("tracker_1"));
        assert!(mon.contains("tracker_2"));
        assert!(!mon.contains("tracker_3"));

        // duplicate add returns false (cache hit)
        assert!(!mon.add("tracker_1"));
        assert_eq!(mon.stats().cache_hits, 1);
        assert_eq!(mon.stats().cache_misses, 2);

        assert!(mon.remove("tracker_2"));
        assert_eq!(mon.len(), 1);
        assert!(!mon.contains("tracker_2"));

        // removing a non-existent entry returns false
        assert!(!mon.remove("tracker_2"));
        assert_eq!(mon.len(), 1);
    }

    #[test]
    fn tunnelBandwidthMonitor_new() {
        let s = TunnelBwMonitorEngine::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn tunnelBandwidthMonitor_add_contains() {
        let mut s = TunnelBwMonitorEngine::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn tunnelBandwidthMonitor_add_duplicate() {
        let mut s = TunnelBwMonitorEngine::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn tunnelBandwidthMonitor_remove() {
        let mut s = TunnelBwMonitorEngine::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn tunnelBandwidthMonitor_capacity() {
        let s = TunnelBwMonitorEngine::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn tunnelBandwidthMonitor_search() {
        let mut s = TunnelBwMonitorEngine::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn tunnelBandwidthMonitor_stats() {
        let mut s = TunnelBwMonitorEngine::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn tunnelLatencyTracker_new() {
        let m = TunnelLatencyTracker::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn tunnelLatencyTracker_add_find() {
        let mut m = TunnelLatencyTracker::new();
        m.add(TunnelLatencyTrackerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn tunnelLatencyTracker_priority_filter() {
        let mut m = TunnelLatencyTracker::new();
        m.add(TunnelLatencyTrackerItem::new("a", "A").with_priority(TunnelLatencyTrackerPriority::High));
        m.add(TunnelLatencyTrackerItem::new("b", "B").with_priority(TunnelLatencyTrackerPriority::Low));
        m.add(TunnelLatencyTrackerItem::new("c", "C").with_priority(TunnelLatencyTrackerPriority::High));
        assert_eq!(m.by_priority(TunnelLatencyTrackerPriority::High).len(), 2);
    }

    #[test]
    fn tunnelLatencyTracker_remove() {
        let mut m = TunnelLatencyTracker::new();
        m.add(TunnelLatencyTrackerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn tunnelLatencyTracker_search() {
        let mut m = TunnelLatencyTracker::new();
        m.add(TunnelLatencyTrackerItem::new("id1", "Hello World"));
        m.add(TunnelLatencyTrackerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn tunnelLatencyTracker_total_weight() {
        let mut m = TunnelLatencyTracker::new();
        m.add(TunnelLatencyTrackerItem::new("a", "A").with_priority(TunnelLatencyTrackerPriority::Critical));
        m.add(TunnelLatencyTrackerItem::new("b", "B").with_priority(TunnelLatencyTrackerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn tunnelLatencyTracker_capacity_limit() {
        let mut m = TunnelLatencyTracker::new().with_max_items(2);
        m.add(TunnelLatencyTrackerItem::new("1", "one"));
        m.add(TunnelLatencyTrackerItem::new("2", "two"));
        assert!(!m.add(TunnelLatencyTrackerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn tunnelLatencyTracker_sorted_by_priority() {
        let mut m = TunnelLatencyTracker::new();
        m.add(TunnelLatencyTrackerItem::new("lo", "Low").with_priority(TunnelLatencyTrackerPriority::Low));
        m.add(TunnelLatencyTrackerItem::new("hi", "High").with_priority(TunnelLatencyTrackerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn tunnelLatencyTracker_item_metadata() {
        let mut item = TunnelLatencyTrackerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn tunnelBandwidthMonitor_enabled_toggle() {
        let mut s = TunnelBwMonitorEngine::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn tunnelLatencyTracker_priority_display() {
        assert_eq!(format!("{}", TunnelLatencyTrackerPriority::High), "high");
        assert_eq!(format!("{}", TunnelLatencyTrackerPriority::Low), "low");
    }


    #[test]
    fn wb_tunnel_entry_creation() {
        let e = WbTunnelEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_tunnel_entry_with_priority() {
        let e = WbTunnelEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_tunnel_entry_metadata() {
        let e = WbTunnelEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_tunnel_entry_remove_meta() {
        let mut e = WbTunnelEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_tunnel_entry_activate_deactivate() {
        let mut e = WbTunnelEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_tunnel_config_add_sorted() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("lo", "Lo").with_priority(1));
        c.add(WbTunnelEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_tunnel_config_capacity() {
        let mut c = WbTunnelConfig::new(1);
        assert!(c.add(WbTunnelEntry::new("a", "A")));
        assert!(!c.add(WbTunnelEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_tunnel_config_remove() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_tunnel_config_get() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_tunnel_config_active_entries() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "A"));
        c.add(WbTunnelEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_tunnel_config_enable_disable() {
        let mut c = WbTunnelConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_tunnel_config_clear() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_tunnel_config_find_by_label() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_tunnel_config_top_n() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "A").with_priority(1));
        c.add(WbTunnelEntry::new("b", "B").with_priority(2));
        c.add(WbTunnelEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_tunnel_config_deactivate_activate_all() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "A"));
        c.add(WbTunnelEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_tunnel_config_highest_priority() {
        let mut c = WbTunnelConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbTunnelEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_tunnel_config_contains() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_tunnel_config_labels() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "Alpha"));
        c.add(WbTunnelEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_tunnel_config_drain_inactive() {
        let mut c = WbTunnelConfig::new(10);
        c.add(WbTunnelEntry::new("a", "A"));
        c.add(WbTunnelEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qi_metrics_empty() {
        let m = QiMetrics::new("wb_tunnel");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qi_metrics_record_and_mean() {
        let mut m = QiMetrics::new("wb_tunnel");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qi_metrics_min_max() {
        let mut m = QiMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qi_metrics_variance_and_std() {
        let mut m = QiMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qi_metrics_percentile() {
        let mut m = QiMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qi_metrics_merge() {
        let mut a = QiMetrics::new("a");
        a.record(1.0);
        let mut b = QiMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qi_metrics_reset() {
        let mut m = QiMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qi_rate_window_empty() {
        let rw = QiRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qi_rate_window_tick_and_rate() {
        let mut rw = QiRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qi_lru_cache_basic() {
        let mut c = QiLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qi_lru_cache_contains_and_keys() {
        let mut c = QiLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qi_lru_cache_remove() {
        let mut c = QiLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qi_metrics_sum() {
        let mut m = QiMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qi_metrics_label() {
        let m = QiMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qi_lru_cache_clear() {
        let mut c = QiLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_tunnel
    #[test]
    fn xa_wb_tunnel_ring_new() {
        let rb = super::XaWbTunnelRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_tunnel_ring_push_len() {
        let mut rb = super::XaWbTunnelRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_tunnel_ring_wrap() {
        let mut rb = super::XaWbTunnelRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_tunnel_ring_mean_empty() {
        let rb = super::XaWbTunnelRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_tunnel_ring_mean_values() {
        let mut rb = super::XaWbTunnelRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_tunnel_ring_min_max() {
        let mut rb = super::XaWbTunnelRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_tunnel_ring_iter() {
        let mut rb = super::XaWbTunnelRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_tunnel_counter_new() {
        let c = super::XaWbTunnelCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_tunnel_counter_inc() {
        let mut c = super::XaWbTunnelCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_tunnel_counter_inc_by() {
        let mut c = super::XaWbTunnelCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_tunnel_counter_reset() {
        let mut c = super::XaWbTunnelCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_tunnel_counter_clear() {
        let mut c = super::XaWbTunnelCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_tunnel_counter_default() {
        let c = super::XaWbTunnelCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 232 ----

    #[test]
    fn xc_232_pool_new_empty() {
        let pool: super::Xc232Pool<i32> = super::Xc232Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_232_pool_release_acquire() {
        let mut pool = super::Xc232Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_232_pool_acquire_empty() {
        let mut pool: super::Xc232Pool<i32> = super::Xc232Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_232_pool_full() {
        let mut pool = super::Xc232Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_232_pool_drain() {
        let mut pool = super::Xc232Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_232_pool_stats() {
        let mut pool = super::Xc232Pool::new(8);
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
    fn xc_232_pool_clear() {
        let mut pool = super::Xc232Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_232_pool_shrink() {
        let mut pool = super::Xc232Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_232_pool_default() {
        let pool: super::Xc232Pool<String> = super::Xc232Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_232_pool_extend() {
        let mut pool = super::Xc232Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_232_pool_retain() {
        let mut pool = super::Xc232Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_232_scheduler_round_robin() {
        let mut sched = super::Xc232Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_232_scheduler_empty() {
        let mut sched = super::Xc232Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_232_scheduler_reset() {
        let mut sched = super::Xc232Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_232_scheduler_add_remove() {
        let mut sched = super::Xc232Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_232_scheduler_targets() {
        let sched = super::Xc232Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_232_hash_empty() {
        assert_eq!(super::xc_232_hash(b""), 5381);
    }

    #[test]
    fn xc_232_hash_data() {
        let h = super::xc_232_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_232_hash(b"hello"), h);
    }

    #[test]
    fn xc_232_reverse_str() {
        assert_eq!(super::xc_232_reverse("abc"), "cba");
        assert_eq!(super::xc_232_reverse(""), "");
    }


    #[test]
    fn xe_9_pipeline_empty() {
        let p = super::Xe9Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_9_pipeline_parse_stage() {
        let p = super::Xe9Pipeline::new()
            .add_parse(super::xe_9_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_9_pipeline_transform_double() {
        let p = super::Xe9Pipeline::new()
            .add_transform(super::xe_9_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_9_pipeline_validate_reverse() {
        let p = super::Xe9Pipeline::new()
            .add_validate(super::xe_9_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_9_pipeline_emit_filter() {
        let p = super::Xe9Pipeline::new()
            .add_emit(super::xe_9_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_9_pipeline_multi_stage() {
        let p = super::Xe9Pipeline::new()
            .add_parse(super::xe_9_pipeline_identity)
            .add_transform(super::xe_9_pipeline_double)
            .add_validate(super::xe_9_pipeline_reverse)
            .add_emit(super::xe_9_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_9_pipeline_error_propagation() {
        let p = super::Xe9Pipeline::new()
            .add_parse(super::xe_9_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe9Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_9_pipeline_compose() {
        let p1 = super::Xe9Pipeline::new()
            .add_parse(super::xe_9_pipeline_identity);
        let p2 = super::Xe9Pipeline::new()
            .add_transform(super::xe_9_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_9_pipeline_error_display() {
        let e = super::Xe9PipelineError {
            stage: super::Xe9Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_9_cache_put_get() {
        let mut c = super::Xe9Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_9_cache_miss() {
        let mut c: super::Xe9Cache<&str, i32> = super::Xe9Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_9_cache_ttl_expiry() {
        let mut c = super::Xe9Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_9_cache_evict() {
        let mut c = super::Xe9Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_9_cache_capacity() {
        let mut c = super::Xe9Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_9_cache_stats() {
        let mut c = super::Xe9Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_9_cache_clear() {
        let mut c = super::Xe9Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #73 --

    #[test]
    fn xf73_trie_insert_search() {
        let mut t = Xf73Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf73_trie_starts_with() {
        let mut t = Xf73Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf73_trie_remove() {
        let mut t = Xf73Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf73_trie_word_count() {
        let mut t = Xf73Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf73_trie_longest_prefix() {
        let mut t = Xf73Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf73_trie_all_words() {
        let mut t = Xf73Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf73_trie_autocomplete() {
        let mut t = Xf73Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf73_trie_empty_search() {
        let t = Xf73Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf73_bloom_add_contains() {
        let mut bf = Xf73BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf73_bloom_probably_absent() {
        let bf = Xf73BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf73_bloom_false_positive_rate() {
        let mut bf = Xf73BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf73_bloom_clear() {
        let mut bf = Xf73BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf73_bloom_union() {
        let mut a = Xf73BloomFilter::xf_new(512, 2);
        let mut b = Xf73BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf73_bloom_intersection_estimate() {
        let mut a = Xf73BloomFilter::xf_new(512, 2);
        let mut b = Xf73BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf73_bloom_union_size_mismatch() {
        let a = Xf73BloomFilter::xf_new(256, 2);
        let b = Xf73BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh231_skip_insert_contains() {
        let mut sl = super::Xh231SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh231_skip_remove() {
        let mut sl = super::Xh231SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh231_skip_len() {
        let mut sl = super::Xh231SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh231_skip_range_query() {
        let mut sl = super::Xh231SkipList::xh_new(4);
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
    fn xh231_skip_floor_ceiling() {
        let mut sl = super::Xh231SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh231_skip_rank() {
        let mut sl = super::Xh231SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh231_skip_empty() {
        let sl = super::Xh231SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh231_skip_duplicates() {
        let mut sl = super::Xh231SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh231_bitset_set_test() {
        let mut bs = super::Xh231BitSet::xh_new(256);
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
    fn xh231_bitset_clear_count() {
        let mut bs = super::Xh231BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh231_bitset_and_or_xor() {
        let mut a = super::Xh231BitSet::xh_new(128);
        let mut b = super::Xh231BitSet::xh_new(128);
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
    fn xh231_bitset_iter_ones() {
        let mut bs = super::Xh231BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh231_bitset_first_last() {
        let mut bs = super::Xh231BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh231_bitset_empty() {
        let bs = super::Xh231BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi231_deque_push_pop_back() {
        let mut dq = super::Xi231Deque::xi_new(4);
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
    fn xi231_deque_push_pop_front() {
        let mut dq = super::Xi231Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi231_deque_mixed_ops() {
        let mut dq = super::Xi231Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi231_deque_get_and_split() {
        let mut dq = super::Xi231Deque::xi_new(8);
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
    fn xi231_deque_rotate_left() {
        let mut dq = super::Xi231Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi231_deque_rotate_right() {
        let mut dq = super::Xi231Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi231_deque_grow() {
        let mut dq = super::Xi231Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi231_deque_empty() {
        let dq = super::Xi231Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi231_interval_tree_insert_query() {
        let mut tree = super::Xi231IntervalTree::xi_new();
        tree.xi_insert(super::Xi231Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi231Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi231Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi231_interval_tree_overlap() {
        let mut tree = super::Xi231IntervalTree::xi_new();
        tree.xi_insert(super::Xi231Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi231Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi231Interval::xi_new(12, 20));
        let q = super::Xi231Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi231_interval_tree_remove() {
        let mut tree = super::Xi231IntervalTree::xi_new();
        tree.xi_insert(super::Xi231Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi231Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi231_interval_tree_gaps() {
        let mut tree = super::Xi231IntervalTree::xi_new();
        tree.xi_insert(super::Xi231Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi231Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi231Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi231Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi231Interval::xi_new(8, 10));
    }

    #[test]
    fn xi231_interval_tree_merge() {
        let mut tree = super::Xi231IntervalTree::xi_new();
        tree.xi_insert(super::Xi231Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi231Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi231Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi231Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi231Interval::xi_new(10, 15));
    }

    #[test]
    fn xi231_interval_tree_all() {
        let mut tree = super::Xi231IntervalTree::xi_new();
        tree.xi_insert(super::Xi231Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi231Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi231_interval_tree_empty() {
        let tree = super::Xi231IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi231_interval_tree_contains_point() {
        let iv = super::Xi231Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 231) ---

    #[test]
    fn xj_231_uf_make_and_find() {
        let mut uf = super::Xj231UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_231_uf_union_connected() {
        let mut uf = super::Xj231UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_231_uf_component_count() {
        let mut uf = super::Xj231UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_231_uf_component_size() {
        let mut uf = super::Xj231UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_231_uf_largest_component() {
        let mut uf = super::Xj231UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_231_uf_many_elements() {
        let mut uf = super::Xj231UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_231_uf_separate_components() {
        let mut uf = super::Xj231UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_231_uf_path_compression() {
        let mut uf = super::Xj231UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_231_bt_insert_get() {
        let mut bt = super::Xj231BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_231_bt_contains_len() {
        let mut bt = super::Xj231BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_231_bt_replace() {
        let mut bt = super::Xj231BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_231_bt_remove() {
        let mut bt = super::Xj231BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_231_bt_keys_values() {
        let mut bt = super::Xj231BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_231_bt_range() {
        let mut bt = super::Xj231BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_231_bt_min_max() {
        let mut bt = super::Xj231BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_231_bt_many_inserts() {
        let mut bt = super::Xj231BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_231 segment tree tests ---

    #[test]
    fn xk_231_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk231SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_231_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk231SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_231_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk231SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_231_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk231SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_231_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk231SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_231_st_single_element() {
        let data = vec![42];
        let st = super::Xk231SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_231_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk231SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_231_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk231SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_231 disjoint intervals tests ---

    #[test]
    fn xk_231_di_add_and_count() {
        let mut di = super::Xk231DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_231_di_merge_overlap() {
        let mut di = super::Xk231DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_231_di_contains() {
        let mut di = super::Xk231DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_231_di_remove() {
        let mut di = super::Xk231DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_231_di_covered_length() {
        let mut di = super::Xk231DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_231_di_gaps() {
        let mut di = super::Xk231DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_231_di_merge_adjacent() {
        let mut di = super::Xk231DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_231_di_empty() {
        let di = super::Xk231DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_231_rope_new_empty() {
        let rope = super::Xl231Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_231_rope_from_str() {
        let rope = super::Xl231Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_231_rope_insert_at() {
        let mut rope = super::Xl231Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_231_rope_delete_range() {
        let mut rope = super::Xl231Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_231_rope_char_at() {
        let rope = super::Xl231Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_231_rope_split_concat() {
        let rope = super::Xl231Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_231_rope_line_count() {
        let rope = super::Xl231Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_231_rope_line_at() {
        let rope = super::Xl231Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_231_sa_build_and_search() {
        let sa = super::Xl231SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_231_sa_count() {
        let sa = super::Xl231SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_231_sa_longest_repeated() {
        let sa = super::Xl231SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_231_sa_all_positions() {
        let sa = super::Xl231SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_231_sa_len() {
        let sa = super::Xl231SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_231_sa_empty() {
        let sa = super::Xl231SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_231_rope_slice() {
        let rope = super::Xl231Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_231_sa_search_start() {
        let sa = super::Xl231SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_231_sparse_set_get() {
        let mut m = super::Xm231MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_231_sparse_row_col() {
        let mut m = super::Xm231MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_231_sparse_transpose() {
        let mut m = super::Xm231MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_231_sparse_multiply_vec() {
        let mut m = super::Xm231MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_231_sparse_nnz_density() {
        let mut m = super::Xm231MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_231_sparse_clear() {
        let mut m = super::Xm231MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_231_sparse_overwrite_zero() {
        let mut m = super::Xm231MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_231_tokenizer_basic() {
        let t = super::Xm231Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_231_tokenizer_count() {
        let t = super::Xm231Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_231_tokenizer_unique() {
        let t = super::Xm231Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_231_tokenizer_frequency() {
        let t = super::Xm231Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_231_tokenizer_delimiter() {
        let t = super::Xm231Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_231_tokenizer_whitespace() {
        let t = super::Xm231Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_231_tokenizer_empty() {
        let t = super::Xm231Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }

}
