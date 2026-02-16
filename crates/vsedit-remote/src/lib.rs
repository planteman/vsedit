//! Remote connection management.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteAuthority {
    SSH,
    WSL,
    Container,
    Tunnel,
    Custom(String),
}

impl fmt::Display for RemoteAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteAuthority::SSH => write!(f, "SSH"),
            RemoteAuthority::WSL => write!(f, "WSL"),
            RemoteAuthority::Container => write!(f, "Container"),
            RemoteAuthority::Tunnel => write!(f, "Tunnel"),
            RemoteAuthority::Custom(name) => write!(f, "Custom({})", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionStatus::Disconnected => write!(f, "Disconnected"),
            ConnectionStatus::Connecting => write!(f, "Connecting"),
            ConnectionStatus::Connected => write!(f, "Connected"),
            ConnectionStatus::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoteConnection {
    pub authority: RemoteAuthority,
    pub host: String,
    pub port: Option<u16>,
    pub label: String,
    pub status: ConnectionStatus,
}

impl RemoteConnection {
    /// Returns a human-readable name like "label (host:port)".
    pub fn display_name(&self) -> String {
        match self.port {
            Some(p) => format!("{} ({}:{})", self.label, self.host, p),
            None => format!("{} ({})", self.label, self.host),
        }
    }

    pub fn connected(&self) -> bool {
        self.status == ConnectionStatus::Connected
    }
}

pub struct RemoteService {
    connections: Vec<RemoteConnection>,
    active: Option<usize>,
}

impl RemoteService {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            active: None,
        }
    }

    pub fn add_connection(&mut self, conn: RemoteConnection) {
        self.connections.push(conn);
    }

    pub fn connect(&mut self, index: usize) -> bool {
        if let Some(conn) = self.connections.get_mut(index) {
            conn.status = ConnectionStatus::Connected;
            self.active = Some(index);
            true
        } else {
            false
        }
    }

    pub fn disconnect(&mut self, index: usize) {
        if let Some(conn) = self.connections.get_mut(index) {
            conn.status = ConnectionStatus::Disconnected;
            if self.active == Some(index) {
                self.active = None;
            }
        }
    }

    pub fn get_active(&self) -> Option<&RemoteConnection> {
        self.active
            .and_then(|i| self.connections.get(i))
            .filter(|c| c.status == ConnectionStatus::Connected)
    }

    pub fn is_remote(&self) -> bool {
        self.active.is_some()
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn remove_connection(&mut self, index: usize) -> bool {
        if index >= self.connections.len() {
            return false;
        }
        self.connections.remove(index);
        // Adjust active index after removal.
        match self.active {
            Some(a) if a == index => self.active = None,
            Some(a) if a > index => self.active = Some(a - 1),
            _ => {}
        }
        true
    }

    pub fn get_connection(&self, index: usize) -> Option<&RemoteConnection> {
        self.connections.get(index)
    }

    pub fn find_by_host(&self, host: &str) -> Option<(usize, &RemoteConnection)> {
        self.connections
            .iter()
            .enumerate()
            .find(|(_, c)| c.host == host)
    }

    pub fn find_by_authority(&self, authority: &RemoteAuthority) -> Vec<(usize, &RemoteConnection)> {
        self.connections
            .iter()
            .enumerate()
            .filter(|(_, c)| c.authority == *authority)
            .collect()
    }

    pub fn disconnect_all(&mut self) {
        for conn in &mut self.connections {
            conn.status = ConnectionStatus::Disconnected;
        }
        self.active = None;
    }

    pub fn connected_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|c| c.status == ConnectionStatus::Connected)
            .count()
    }

    /// Returns stats about the current set of connections.
    pub fn connection_stats(&self) -> ConnectionStats {
        let total = self.connections.len();
        let connected = self.connected_count();
        let disconnected = self.connections.iter().filter(|c| c.status == ConnectionStatus::Disconnected).count();
        let errored = self.connections.iter().filter(|c| matches!(c.status, ConnectionStatus::Error(_))).count();
        ConnectionStats { total, connected, disconnected, errored }
    }

    /// Find a connection by label (first match).
    pub fn find_connection(&self, label: &str) -> Option<(usize, &RemoteConnection)> {
        self.connections
            .iter()
            .enumerate()
            .find(|(_, c)| c.label == label)
    }

    /// Returns references to all currently connected connections.
    pub fn active_connections(&self) -> Vec<&RemoteConnection> {
        self.connections
            .iter()
            .filter(|c| c.status == ConnectionStatus::Connected)
            .collect()
    }
}

impl Default for RemoteService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConnectionState – a simplified tri-state for higher-level consumers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Open,
    Closed,
    Failed,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Open => write!(f, "Open"),
            ConnectionState::Closed => write!(f, "Closed"),
            ConnectionState::Failed => write!(f, "Failed"),
        }
    }
}

impl ConnectionState {
    pub fn is_open(self) -> bool {
        self == ConnectionState::Open
    }
}

// ---------------------------------------------------------------------------
// RemoteError – typed errors for connection operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteError {
    InvalidHost(String),
    InvalidPort(u16),
    EmptyLabel,
    ConnectionFailed(String),
    AlreadyConnected,
    NotConnected,
}

impl fmt::Display for RemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteError::InvalidHost(h) => write!(f, "invalid host: {}", h),
            RemoteError::InvalidPort(p) => write!(f, "invalid port: {}", p),
            RemoteError::EmptyLabel => write!(f, "label must not be empty"),
            RemoteError::ConnectionFailed(msg) => write!(f, "connection failed: {}", msg),
            RemoteError::AlreadyConnected => write!(f, "already connected"),
            RemoteError::NotConnected => write!(f, "not connected"),
        }
    }
}

// ---------------------------------------------------------------------------
// ConnectionConfig – validated connection parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: Option<u16>,
    pub label: String,
    pub authority: RemoteAuthority,
}

impl ConnectionConfig {
    /// Validate and create a new config.
    pub fn new(
        host: impl Into<String>,
        port: Option<u16>,
        label: impl Into<String>,
        authority: RemoteAuthority,
    ) -> Result<Self, RemoteError> {
        let host = host.into();
        let label = label.into();
        if host.is_empty() {
            return Err(RemoteError::InvalidHost(host));
        }
        if label.is_empty() {
            return Err(RemoteError::EmptyLabel);
        }
        if let Some(p) = port {
            if p == 0 {
                return Err(RemoteError::InvalidPort(p));
            }
        }
        Ok(Self { host, port, label, authority })
    }

    /// Build a `RemoteConnection` from this config.
    pub fn into_connection(self) -> RemoteConnection {
        RemoteConnection {
            authority: self.authority,
            host: self.host,
            port: self.port,
            label: self.label,
            status: ConnectionStatus::Disconnected,
        }
    }
}

// ---------------------------------------------------------------------------
// ConnectionStats – summary returned by `RemoteService::connection_stats`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionStats {
    pub total: usize,
    pub connected: usize,
    pub disconnected: usize,
    pub errored: usize,
}

impl fmt::Display for ConnectionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "total={}, connected={}, disconnected={}, errored={}",
            self.total, self.connected, self.disconnected, self.errored
        )
    }
}

// ---------------------------------------------------------------------------
// ConnectionHistory – tracks connection attempt outcomes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConnectionAttempt {
    pub host: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionHistory {
    attempts: Vec<ConnectionAttempt>,
}

impl ConnectionHistory {
    pub fn new() -> Self {
        Self { attempts: Vec::new() }
    }

    pub fn record(&mut self, host: impl Into<String>, success: bool, message: impl Into<String>) {
        self.attempts.push(ConnectionAttempt {
            host: host.into(),
            success,
            message: message.into(),
        });
    }

    pub fn total(&self) -> usize {
        self.attempts.len()
    }

    pub fn successes(&self) -> usize {
        self.attempts.iter().filter(|a| a.success).count()
    }

    pub fn failures(&self) -> usize {
        self.attempts.iter().filter(|a| !a.success).count()
    }

    pub fn last(&self) -> Option<&ConnectionAttempt> {
        self.attempts.last()
    }

    pub fn attempts_for_host(&self, host: &str) -> Vec<&ConnectionAttempt> {
        self.attempts.iter().filter(|a| a.host == host).collect()
    }

    pub fn clear(&mut self) {
        self.attempts.clear();
    }
}

// ---------------------------------------------------------------------------
// ConnectionInfo – human-readable summary of a connection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub label: String,
    pub host: String,
    pub port: Option<u16>,
    pub authority: String,
    pub state: ConnectionState,
}

impl ConnectionInfo {
    pub fn from_connection(conn: &RemoteConnection) -> Self {
        let state = match &conn.status {
            ConnectionStatus::Connected => ConnectionState::Open,
            ConnectionStatus::Error(_) => ConnectionState::Failed,
            _ => ConnectionState::Closed,
        };
        Self {
            label: conn.label.clone(),
            host: conn.host.clone(),
            port: conn.port,
            authority: format!("{}", conn.authority),
            state,
        }
    }
}

impl fmt::Display for ConnectionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.port {
            Some(p) => write!(f, "[{}] {}@{}:{} ({})", self.state, self.label, self.host, p, self.authority),
            None => write!(f, "[{}] {}@{} ({})", self.state, self.label, self.host, self.authority),
        }
    }
}

// ---------------------------------------------------------------------------
// Connection latency tracking
// ---------------------------------------------------------------------------

/// Tracks latency samples for a connection.
#[derive(Debug, Clone, Default)]
pub struct LatencyTracker {
    samples: Vec<u64>,
}

impl LatencyTracker {
    pub fn new() -> Self {
        Self { samples: Vec::new() }
    }

    /// Record a latency sample in milliseconds.
    pub fn record(&mut self, ms: u64) {
        self.samples.push(ms);
    }

    /// Average latency in milliseconds, or 0 if no samples.
    pub fn average_ms(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        self.samples.iter().sum::<u64>() / self.samples.len() as u64
    }

    /// Minimum latency sample.
    pub fn min_ms(&self) -> Option<u64> {
        self.samples.iter().copied().min()
    }

    /// Maximum latency sample.
    pub fn max_ms(&self) -> Option<u64> {
        self.samples.iter().copied().max()
    }

    /// Number of recorded samples.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Clear all samples.
    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

// ---------------------------------------------------------------------------
// Reconnection backoff computation
// ---------------------------------------------------------------------------

/// Compute exponential backoff delay for reconnection attempts.
///
/// Returns delay in milliseconds, capped at `max_delay_ms`.
pub fn compute_backoff(attempt: u32, base_ms: u64, max_delay_ms: u64) -> u64 {
    let delay = base_ms.saturating_mul(2u64.saturating_pow(attempt));
    delay.min(max_delay_ms)
}

// ---------------------------------------------------------------------------
// Connection capability negotiation
// ---------------------------------------------------------------------------

/// Capabilities that a remote connection endpoint may support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionCapabilities {
    pub supports_file_system: bool,
    pub supports_terminal: bool,
    pub supports_port_forwarding: bool,
    pub supports_search: bool,
}

impl Default for ConnectionCapabilities {
    fn default() -> Self {
        Self {
            supports_file_system: true,
            supports_terminal: false,
            supports_port_forwarding: false,
            supports_search: false,
        }
    }
}

impl ConnectionCapabilities {
    /// Negotiate capabilities by taking the intersection of two sets.
    pub fn negotiate(&self, other: &Self) -> Self {
        Self {
            supports_file_system: self.supports_file_system && other.supports_file_system,
            supports_terminal: self.supports_terminal && other.supports_terminal,
            supports_port_forwarding: self.supports_port_forwarding && other.supports_port_forwarding,
            supports_search: self.supports_search && other.supports_search,
        }
    }

    /// Return the number of supported capabilities.
    pub fn supported_count(&self) -> usize {
        [
            self.supports_file_system,
            self.supports_terminal,
            self.supports_port_forwarding,
            self.supports_search,
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }
}

// ---------------------------------------------------------------------------
// SSH authentication methods
// ---------------------------------------------------------------------------

/// Authentication method for SSH connections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAuthMethod {
    Password,
    PublicKey { key_path: String },
    Agent,
}

impl fmt::Display for SshAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SshAuthMethod::Password => write!(f, "Password"),
            SshAuthMethod::PublicKey { key_path } => write!(f, "PublicKey({})", key_path),
            SshAuthMethod::Agent => write!(f, "Agent"),
        }
    }
}

// ---------------------------------------------------------------------------
// RemoteConnectionManager – named connection management by id
// ---------------------------------------------------------------------------

/// Manages named remote connections by string identifier.
pub struct RemoteConnectionManager {
    connections: std::collections::HashMap<String, RemoteConnection>,
}

impl RemoteConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: std::collections::HashMap::new(),
        }
    }

    /// Register a connection under the given id. Returns `false` if the id is already taken.
    pub fn connect(&mut self, id: impl Into<String>, conn: RemoteConnection) -> bool {
        let id = id.into();
        if self.connections.contains_key(&id) {
            return false;
        }
        self.connections.insert(id, conn);
        true
    }

    /// Remove and return the connection with the given id.
    pub fn disconnect(&mut self, id: &str) -> Option<RemoteConnection> {
        self.connections.remove(id)
    }

    /// List all connection ids.
    pub fn list_connections(&self) -> Vec<&str> {
        self.connections.keys().map(|s| s.as_str()).collect()
    }

    /// Get a reference to a connection by id.
    pub fn get(&self, id: &str) -> Option<&RemoteConnection> {
        self.connections.get(id)
    }

    /// Number of managed connections.
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}

impl Default for RemoteConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RemoteEnvironment – describes the remote OS environment
// ---------------------------------------------------------------------------

/// Information about the remote machine's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEnvironment {
    pub os: String,
    pub home_dir: String,
    pub temp_dir: String,
}

impl RemoteEnvironment {
    pub fn new(os: impl Into<String>, home_dir: impl Into<String>, temp_dir: impl Into<String>) -> Self {
        Self {
            os: os.into(),
            home_dir: home_dir.into(),
            temp_dir: temp_dir.into(),
        }
    }
}

impl fmt::Display for RemoteEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OS={}, home={}, tmp={}", self.os, self.home_dir, self.temp_dir)
    }
}

// ---------------------------------------------------------------------------
// PortForwardingConfig – describes a single port forwarding rule
// ---------------------------------------------------------------------------

/// Configuration for a single port forwarding rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortForwardingConfig {
    pub local_port: u16,
    pub remote_port: u16,
    pub remote_host: String,
}

impl PortForwardingConfig {
    pub fn new(local_port: u16, remote_port: u16, remote_host: impl Into<String>) -> Result<Self, RemoteError> {
        if local_port == 0 {
            return Err(RemoteError::InvalidPort(local_port));
        }
        if remote_port == 0 {
            return Err(RemoteError::InvalidPort(remote_port));
        }
        Ok(Self {
            local_port,
            remote_port,
            remote_host: remote_host.into(),
        })
    }
}

impl fmt::Display for PortForwardingConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "localhost:{} -> {}:{}", self.local_port, self.remote_host, self.remote_port)
    }
}

// ---------------------------------------------------------------------------
// Session duration tracking
// ---------------------------------------------------------------------------

/// Tracks a session's start and optional end time (as epoch seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDuration {
    pub start_secs: u64,
    pub end_secs: Option<u64>,
}

impl SessionDuration {
    pub fn start(start_secs: u64) -> Self {
        Self { start_secs, end_secs: None }
    }

    pub fn stop(&mut self, end_secs: u64) {
        self.end_secs = Some(end_secs);
    }

    /// Duration in seconds, or time since start relative to `now` if still running.
    pub fn elapsed(&self, now: u64) -> u64 {
        let end = self.end_secs.unwrap_or(now);
        end.saturating_sub(self.start_secs)
    }

    pub fn is_running(&self) -> bool {
        self.end_secs.is_none()
    }
}

// ---------------------------------------------------------------------------
// Connection summary & iteration helpers
// ---------------------------------------------------------------------------

impl RemoteService {
    /// Returns a summary of the service state.
    pub fn service_summary(&self) -> String {
        let total = self.connection_count();
        let connected = self.connected_count();
        let active = if self.is_remote() { "yes" } else { "no" };
        format!(
            "RemoteService: {} connections ({} connected), remote={}",
            total, connected, active,
        )
    }

    /// Iterate over all connections.
    pub fn iter(&self) -> std::slice::Iter<'_, RemoteConnection> {
        self.connections.iter()
    }

    /// Get a mutable reference to a connection by index.
    pub fn get_connection_mut(&mut self, index: usize) -> Option<&mut RemoteConnection> {
        self.connections.get_mut(index)
    }

    /// Returns all hosts as strings.
    pub fn all_hosts(&self) -> Vec<&str> {
        self.connections.iter().map(|c| c.host.as_str()).collect()
    }

    /// Returns the index of the active connection, if any.
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }
}

impl fmt::Display for RemoteConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}, {})", self.label, self.host, self.status)
    }
}

impl RemoteAuthority {
    /// Returns true if this authority type supports port forwarding.
    pub fn supports_port_forwarding(&self) -> bool {
        matches!(self, RemoteAuthority::SSH | RemoteAuthority::Tunnel)
    }

    /// Returns the default port for this authority type, if applicable.
    pub fn default_port(&self) -> Option<u16> {
        match self {
            RemoteAuthority::SSH => Some(22),
            _ => None,
        }
    }
}

impl SessionDuration {
    /// Return a human-readable elapsed time string.
    pub fn display_elapsed(&self, now: u64) -> String {
        let secs = self.elapsed(now);
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }
}

impl ConnectionHistory {
    /// Success rate as a percentage (0.0 to 100.0).
    pub fn success_rate(&self) -> f64 {
        if self.total() == 0 {
            return 0.0;
        }
        (self.successes() as f64 / self.total() as f64) * 100.0
    }

    /// Returns attempts in reverse chronological order.
    pub fn recent_first(&self) -> Vec<&ConnectionAttempt> {
        self.attempts.iter().rev().collect()
    }
}

// ---------------------------------------------------------------------------
// Connection pool tracking
// ---------------------------------------------------------------------------

/// A pool of remote connections with a configurable maximum size.
#[derive(Debug)]
pub struct ConnectionPool {
    connections: Vec<RemoteConnection>,
    max_size: usize,
}

impl ConnectionPool {
    /// Create a new pool with the given maximum capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            connections: Vec::new(),
            max_size,
        }
    }

    /// Add a connection to the pool. Returns `Err` if the pool is full.
    pub fn add(&mut self, conn: RemoteConnection) -> Result<(), RemoteError> {
        if self.connections.len() >= self.max_size {
            return Err(RemoteError::ConnectionFailed("pool is full".into()));
        }
        self.connections.push(conn);
        Ok(())
    }

    /// Remove and return the connection at `index`.
    pub fn remove(&mut self, index: usize) -> Option<RemoteConnection> {
        if index < self.connections.len() {
            Some(self.connections.remove(index))
        } else {
            None
        }
    }

    /// Number of connections in the pool.
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.connections.len() >= self.max_size
    }

    /// Available slots remaining.
    pub fn available(&self) -> usize {
        self.max_size.saturating_sub(self.connections.len())
    }

    /// Drain all disconnected connections from the pool, returning them.
    pub fn drain_disconnected(&mut self) -> Vec<RemoteConnection> {
        let mut drained = Vec::new();
        self.connections.retain(|c| {
            if c.status == ConnectionStatus::Disconnected {
                drained.push(c.clone());
                false
            } else {
                true
            }
        });
        drained
    }

    /// Get a reference to the pool's connections.
    pub fn connections(&self) -> &[RemoteConnection] {
        &self.connections
    }
}

// ---------------------------------------------------------------------------
// Connection health monitoring
// ---------------------------------------------------------------------------

/// Health status derived from latency and error rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Healthy – low latency and low error rate.
    Healthy,
    /// Degraded – elevated latency or moderate error rate.
    Degraded,
    /// Unhealthy – high latency or high error rate.
    Unhealthy,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded => write!(f, "Degraded"),
            HealthStatus::Unhealthy => write!(f, "Unhealthy"),
        }
    }
}

/// Assess connection health based on latency tracker and connection history.
///
/// Thresholds:
/// - Unhealthy if avg latency > `unhealthy_ms` or success rate < 50%
/// - Degraded if avg latency > `degraded_ms` or success rate < 80%
/// - Healthy otherwise
pub fn assess_health(
    latency: &LatencyTracker,
    history: &ConnectionHistory,
    degraded_ms: u64,
    unhealthy_ms: u64,
) -> HealthStatus {
    let avg = latency.average_ms();
    let rate = history.success_rate();
    if avg > unhealthy_ms || rate < 50.0 {
        HealthStatus::Unhealthy
    } else if avg > degraded_ms || rate < 80.0 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    }
}

// ---------------------------------------------------------------------------
// Remote file path mapping (local <-> remote)
// ---------------------------------------------------------------------------

/// Maps local file paths to remote file paths and vice versa.
#[derive(Debug, Clone)]
pub struct PathMapping {
    local_root: String,
    remote_root: String,
}

impl PathMapping {
    pub fn new(local_root: impl Into<String>, remote_root: impl Into<String>) -> Self {
        Self {
            local_root: local_root.into(),
            remote_root: remote_root.into(),
        }
    }

    /// Convert a local path to its remote equivalent.
    /// Returns `None` if the path does not start with the local root.
    pub fn to_remote(&self, local_path: &str) -> Option<String> {
        local_path
            .strip_prefix(&self.local_root)
            .map(|suffix| format!("{}{}", self.remote_root, suffix))
    }

    /// Convert a remote path to its local equivalent.
    /// Returns `None` if the path does not start with the remote root.
    pub fn to_local(&self, remote_path: &str) -> Option<String> {
        remote_path
            .strip_prefix(&self.remote_root)
            .map(|suffix| format!("{}{}", self.local_root, suffix))
    }

    pub fn local_root(&self) -> &str {
        &self.local_root
    }

    pub fn remote_root(&self) -> &str {
        &self.remote_root
    }
}

/// A registry of multiple path mappings used to translate paths between
/// local and remote file systems.
#[derive(Debug, Clone, Default)]
pub struct PathMappingRegistry {
    mappings: Vec<PathMapping>,
}

impl PathMappingRegistry {
    pub fn new() -> Self {
        Self { mappings: Vec::new() }
    }

    pub fn add(&mut self, mapping: PathMapping) {
        self.mappings.push(mapping);
    }

    /// Translate a local path using the first matching mapping.
    pub fn to_remote(&self, local_path: &str) -> Option<String> {
        self.mappings.iter().find_map(|m| m.to_remote(local_path))
    }

    /// Translate a remote path using the first matching mapping.
    pub fn to_local(&self, remote_path: &str) -> Option<String> {
        self.mappings.iter().find_map(|m| m.to_local(remote_path))
    }

    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Heartbeat tracker for remote sessions
// ---------------------------------------------------------------------------

/// Tracks heartbeat timestamps for a remote session and determines liveness.
#[derive(Debug, Clone)]
pub struct HeartbeatTracker {
    /// Epoch seconds of each received heartbeat.
    timestamps: Vec<u64>,
    /// Maximum interval (seconds) before the session is considered stale.
    timeout_secs: u64,
}

impl HeartbeatTracker {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            timestamps: Vec::new(),
            timeout_secs,
        }
    }

    /// Record a heartbeat at the given epoch time.
    pub fn beat(&mut self, now_secs: u64) {
        self.timestamps.push(now_secs);
    }

    /// Returns `true` if the session is alive (last heartbeat within timeout).
    pub fn is_alive(&self, now_secs: u64) -> bool {
        self.timestamps
            .last()
            .map_or(false, |&last| now_secs.saturating_sub(last) <= self.timeout_secs)
    }

    /// Seconds since the last heartbeat, or `None` if no beats recorded.
    pub fn seconds_since_last(&self, now_secs: u64) -> Option<u64> {
        self.timestamps.last().map(|&last| now_secs.saturating_sub(last))
    }

    /// Number of heartbeats received.
    pub fn count(&self) -> usize {
        self.timestamps.len()
    }

    /// Average interval between consecutive heartbeats in seconds, or `None`.
    pub fn average_interval(&self) -> Option<u64> {
        if self.timestamps.len() < 2 {
            return None;
        }
        let total: u64 = self
            .timestamps
            .windows(2)
            .map(|w| w[1].saturating_sub(w[0]))
            .sum();
        Some(total / (self.timestamps.len() as u64 - 1))
    }

    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

// ---------------------------------------------------------------------------
// Port forwarding rule manager
// ---------------------------------------------------------------------------

/// Manages a collection of port forwarding rules, ensuring no local port
/// conflicts and providing lookup helpers.
#[derive(Debug, Clone, Default)]
pub struct PortForwardingManager {
    rules: Vec<PortForwardingConfig>,
}

impl PortForwardingManager {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule. Returns `Err` if the local port is already forwarded.
    pub fn add_rule(&mut self, rule: PortForwardingConfig) -> Result<(), RemoteError> {
        if self.rules.iter().any(|r| r.local_port == rule.local_port) {
            return Err(RemoteError::ConnectionFailed(format!(
                "local port {} already forwarded",
                rule.local_port
            )));
        }
        self.rules.push(rule);
        Ok(())
    }

    /// Remove the rule forwarding the given local port. Returns `true` if found.
    pub fn remove_by_local_port(&mut self, local_port: u16) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.local_port != local_port);
        self.rules.len() < before
    }

    /// Find the rule for a given local port.
    pub fn find_by_local_port(&self, local_port: u16) -> Option<&PortForwardingConfig> {
        self.rules.iter().find(|r| r.local_port == local_port)
    }

    /// All rules targeting a specific remote host.
    pub fn rules_for_host(&self, host: &str) -> Vec<&PortForwardingConfig> {
        self.rules.iter().filter(|r| r.remote_host == host).collect()
    }

    pub fn rules(&self) -> &[PortForwardingConfig] {
        &self.rules
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_conn() -> RemoteConnection {
        RemoteConnection {
            authority: RemoteAuthority::SSH,
            host: "example.com".into(),
            port: Some(22),
            label: "dev-server".into(),
            status: ConnectionStatus::Disconnected,
        }
    }

    #[test]
    fn add_and_count() {
        let mut svc = RemoteService::new();
        assert_eq!(svc.connection_count(), 0);
        svc.add_connection(sample_conn());
        assert_eq!(svc.connection_count(), 1);
    }

    #[test]
    fn connect_and_disconnect() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        assert!(!svc.is_remote());
        assert!(svc.connect(0));
        assert!(svc.is_remote());
        assert!(svc.get_active().unwrap().connected());
        svc.disconnect(0);
        assert!(!svc.is_remote());
    }

    #[test]
    fn connect_invalid_index() {
        let mut svc = RemoteService::new();
        assert!(!svc.connect(5));
        assert!(!svc.is_remote());
    }

    #[test]
    fn remove_connection() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        svc.add_connection(RemoteConnection {
            authority: RemoteAuthority::WSL,
            host: "localhost".into(),
            port: None,
            label: "wsl".into(),
            status: ConnectionStatus::Disconnected,
        });
        assert_eq!(svc.connection_count(), 2);
        assert!(svc.remove_connection(0));
        assert_eq!(svc.connection_count(), 1);
        assert_eq!(svc.get_connection(0).unwrap().host, "localhost");
        assert!(!svc.remove_connection(5));
    }

    #[test]
    fn remove_adjusts_active_index() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        svc.add_connection(RemoteConnection {
            authority: RemoteAuthority::Tunnel,
            host: "tunnel.example.com".into(),
            port: Some(443),
            label: "tunnel".into(),
            status: ConnectionStatus::Disconnected,
        });
        svc.connect(1);
        assert!(svc.remove_connection(0));
        // Active index should have shifted from 1 to 0.
        assert!(svc.get_active().is_some());
        assert_eq!(svc.get_active().unwrap().host, "tunnel.example.com");
    }

    #[test]
    fn find_by_host() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        let found = svc.find_by_host("example.com");
        assert!(found.is_some());
        let (idx, conn) = found.unwrap();
        assert_eq!(idx, 0);
        assert_eq!(conn.host, "example.com");
        assert!(svc.find_by_host("nonexistent").is_none());
    }

    #[test]
    fn find_by_authority() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        svc.add_connection(RemoteConnection {
            authority: RemoteAuthority::SSH,
            host: "other.com".into(),
            port: Some(2222),
            label: "other-ssh".into(),
            status: ConnectionStatus::Disconnected,
        });
        svc.add_connection(RemoteConnection {
            authority: RemoteAuthority::Container,
            host: "container-host".into(),
            port: None,
            label: "docker".into(),
            status: ConnectionStatus::Disconnected,
        });
        let ssh_conns = svc.find_by_authority(&RemoteAuthority::SSH);
        assert_eq!(ssh_conns.len(), 2);
        let container_conns = svc.find_by_authority(&RemoteAuthority::Container);
        assert_eq!(container_conns.len(), 1);
        let wsl_conns = svc.find_by_authority(&RemoteAuthority::WSL);
        assert!(wsl_conns.is_empty());
    }

    #[test]
    fn disconnect_all_and_connected_count() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        svc.add_connection(RemoteConnection {
            authority: RemoteAuthority::WSL,
            host: "localhost".into(),
            port: None,
            label: "wsl".into(),
            status: ConnectionStatus::Disconnected,
        });
        svc.connect(0);
        svc.connections[1].status = ConnectionStatus::Connected;
        assert_eq!(svc.connected_count(), 2);
        svc.disconnect_all();
        assert_eq!(svc.connected_count(), 0);
        assert!(!svc.is_remote());
    }

    #[test]
    fn display_name_with_and_without_port() {
        let conn = sample_conn();
        assert_eq!(conn.display_name(), "dev-server (example.com:22)");
        let no_port = RemoteConnection {
            authority: RemoteAuthority::WSL,
            host: "localhost".into(),
            port: None,
            label: "wsl".into(),
            status: ConnectionStatus::Disconnected,
        };
        assert_eq!(no_port.display_name(), "wsl (localhost)");
    }

    #[test]
    fn display_traits() {
        assert_eq!(format!("{}", RemoteAuthority::SSH), "SSH");
        assert_eq!(format!("{}", RemoteAuthority::WSL), "WSL");
        assert_eq!(format!("{}", RemoteAuthority::Container), "Container");
        assert_eq!(format!("{}", RemoteAuthority::Tunnel), "Tunnel");
        assert_eq!(
            format!("{}", RemoteAuthority::Custom("myproto".into())),
            "Custom(myproto)"
        );
        assert_eq!(format!("{}", ConnectionStatus::Disconnected), "Disconnected");
        assert_eq!(format!("{}", ConnectionStatus::Connecting), "Connecting");
        assert_eq!(format!("{}", ConnectionStatus::Connected), "Connected");
        assert_eq!(
            format!("{}", ConnectionStatus::Error("timeout".into())),
            "Error: timeout"
        );
    }

    #[test]
    fn connection_status_field() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        assert_eq!(svc.get_connection(0).unwrap().status, ConnectionStatus::Disconnected);
        svc.connect(0);
        assert_eq!(svc.get_connection(0).unwrap().status, ConnectionStatus::Connected);
        svc.disconnect(0);
        assert_eq!(svc.get_connection(0).unwrap().status, ConnectionStatus::Disconnected);
    }

    // ---------------------------------------------------------------
    // New tests
    // ---------------------------------------------------------------

    #[test]
    fn connection_state_display_and_helpers() {
        assert_eq!(format!("{}", ConnectionState::Open), "Open");
        assert_eq!(format!("{}", ConnectionState::Closed), "Closed");
        assert_eq!(format!("{}", ConnectionState::Failed), "Failed");
        assert!(ConnectionState::Open.is_open());
        assert!(!ConnectionState::Closed.is_open());
        assert!(!ConnectionState::Failed.is_open());
    }

    #[test]
    fn remote_error_display() {
        assert_eq!(format!("{}", RemoteError::InvalidHost("".into())), "invalid host: ");
        assert_eq!(format!("{}", RemoteError::InvalidPort(0)), "invalid port: 0");
        assert_eq!(format!("{}", RemoteError::EmptyLabel), "label must not be empty");
        assert_eq!(format!("{}", RemoteError::ConnectionFailed("timeout".into())), "connection failed: timeout");
        assert_eq!(format!("{}", RemoteError::AlreadyConnected), "already connected");
        assert_eq!(format!("{}", RemoteError::NotConnected), "not connected");
    }

    #[test]
    fn connection_config_valid() {
        let cfg = ConnectionConfig::new("host.io", Some(22), "myhost", RemoteAuthority::SSH);
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.host, "host.io");
        assert_eq!(cfg.port, Some(22));
    }

    #[test]
    fn connection_config_empty_host() {
        let cfg = ConnectionConfig::new("", Some(22), "label", RemoteAuthority::SSH);
        assert_eq!(cfg, Err(RemoteError::InvalidHost("".into())));
    }

    #[test]
    fn connection_config_empty_label() {
        let cfg = ConnectionConfig::new("host.io", Some(22), "", RemoteAuthority::SSH);
        assert_eq!(cfg, Err(RemoteError::EmptyLabel));
    }

    #[test]
    fn connection_config_zero_port() {
        let cfg = ConnectionConfig::new("host.io", Some(0), "lab", RemoteAuthority::SSH);
        assert_eq!(cfg, Err(RemoteError::InvalidPort(0)));
    }

    #[test]
    fn connection_config_none_port_ok() {
        let cfg = ConnectionConfig::new("host.io", None, "lab", RemoteAuthority::WSL);
        assert!(cfg.is_ok());
        assert_eq!(cfg.unwrap().port, None);
    }

    #[test]
    fn connection_config_into_connection() {
        let cfg = ConnectionConfig::new("h.io", Some(80), "web", RemoteAuthority::Tunnel).unwrap();
        let conn = cfg.into_connection();
        assert_eq!(conn.host, "h.io");
        assert_eq!(conn.port, Some(80));
        assert_eq!(conn.label, "web");
        assert_eq!(conn.status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn connection_stats_all_states() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        svc.add_connection(RemoteConnection {
            authority: RemoteAuthority::WSL,
            host: "wsl".into(),
            port: None,
            label: "wsl".into(),
            status: ConnectionStatus::Error("fail".into()),
        });
        svc.connect(0);
        let stats = svc.connection_stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.connected, 1);
        assert_eq!(stats.disconnected, 0);
        assert_eq!(stats.errored, 1);
    }

    #[test]
    fn connection_stats_display() {
        let stats = ConnectionStats { total: 3, connected: 1, disconnected: 1, errored: 1 };
        assert_eq!(format!("{}", stats), "total=3, connected=1, disconnected=1, errored=1");
    }

    #[test]
    fn find_connection_by_label() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        let found = svc.find_connection("dev-server");
        assert!(found.is_some());
        assert_eq!(found.unwrap().0, 0);
        assert!(svc.find_connection("nope").is_none());
    }

    #[test]
    fn active_connections_returns_only_connected() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        svc.add_connection(RemoteConnection {
            authority: RemoteAuthority::WSL,
            host: "wsl".into(),
            port: None,
            label: "wsl".into(),
            status: ConnectionStatus::Disconnected,
        });
        assert!(svc.active_connections().is_empty());
        svc.connect(0);
        let active = svc.active_connections();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].host, "example.com");
    }

    #[test]
    fn connection_history_record_and_query() {
        let mut history = ConnectionHistory::new();
        assert_eq!(history.total(), 0);
        history.record("h1", true, "ok");
        history.record("h1", false, "timeout");
        history.record("h2", true, "ok");
        assert_eq!(history.total(), 3);
        assert_eq!(history.successes(), 2);
        assert_eq!(history.failures(), 1);
        assert_eq!(history.attempts_for_host("h1").len(), 2);
        assert_eq!(history.attempts_for_host("h2").len(), 1);
        assert!(history.last().unwrap().success);
    }

    #[test]
    fn connection_history_clear() {
        let mut history = ConnectionHistory::new();
        history.record("h1", true, "ok");
        history.clear();
        assert_eq!(history.total(), 0);
        assert!(history.last().is_none());
    }

    #[test]
    fn connection_info_from_connection_and_display() {
        let conn = RemoteConnection {
            authority: RemoteAuthority::SSH,
            host: "myhost.io".into(),
            port: Some(22),
            label: "prod".into(),
            status: ConnectionStatus::Connected,
        };
        let info = ConnectionInfo::from_connection(&conn);
        assert_eq!(info.state, ConnectionState::Open);
        assert_eq!(format!("{}", info), "[Open] prod@myhost.io:22 (SSH)");

        let conn2 = RemoteConnection {
            authority: RemoteAuthority::WSL,
            host: "localhost".into(),
            port: None,
            label: "wsl".into(),
            status: ConnectionStatus::Error("bad".into()),
        };
        let info2 = ConnectionInfo::from_connection(&conn2);
        assert_eq!(info2.state, ConnectionState::Failed);
        assert_eq!(format!("{}", info2), "[Failed] wsl@localhost (WSL)");
    }

    #[test]
    fn connection_info_closed_state() {
        let conn = RemoteConnection {
            authority: RemoteAuthority::Tunnel,
            host: "t.io".into(),
            port: Some(443),
            label: "tun".into(),
            status: ConnectionStatus::Disconnected,
        };
        let info = ConnectionInfo::from_connection(&conn);
        assert_eq!(info.state, ConnectionState::Closed);

        let conn2 = RemoteConnection {
            authority: RemoteAuthority::Tunnel,
            host: "t.io".into(),
            port: Some(443),
            label: "tun".into(),
            status: ConnectionStatus::Connecting,
        };
        let info2 = ConnectionInfo::from_connection(&conn2);
        assert_eq!(info2.state, ConnectionState::Closed);
    }

    #[test]
    fn latency_tracker_basic() {
        let mut lt = LatencyTracker::new();
        assert_eq!(lt.average_ms(), 0);
        assert_eq!(lt.sample_count(), 0);
        lt.record(10);
        lt.record(20);
        lt.record(30);
        assert_eq!(lt.average_ms(), 20);
        assert_eq!(lt.min_ms(), Some(10));
        assert_eq!(lt.max_ms(), Some(30));
        assert_eq!(lt.sample_count(), 3);
        lt.reset();
        assert_eq!(lt.sample_count(), 0);
    }

    #[test]
    fn compute_backoff_exponential() {
        assert_eq!(compute_backoff(0, 100, 10000), 100);
        assert_eq!(compute_backoff(1, 100, 10000), 200);
        assert_eq!(compute_backoff(2, 100, 10000), 400);
        assert_eq!(compute_backoff(3, 100, 10000), 800);
        // Capped at max_delay.
        assert_eq!(compute_backoff(10, 100, 5000), 5000);
    }

    #[test]
    fn capabilities_negotiation() {
        let client = ConnectionCapabilities {
            supports_file_system: true,
            supports_terminal: true,
            supports_port_forwarding: false,
            supports_search: true,
        };
        let server = ConnectionCapabilities {
            supports_file_system: true,
            supports_terminal: false,
            supports_port_forwarding: true,
            supports_search: true,
        };
        let result = client.negotiate(&server);
        assert!(result.supports_file_system);
        assert!(!result.supports_terminal);
        assert!(!result.supports_port_forwarding);
        assert!(result.supports_search);
        assert_eq!(result.supported_count(), 2);
    }

    #[test]
    fn capabilities_default_and_count() {
        let caps = ConnectionCapabilities::default();
        assert!(caps.supports_file_system);
        assert!(!caps.supports_terminal);
        assert_eq!(caps.supported_count(), 1);
    }

    #[test]
    fn session_duration_tracking() {
        let mut session = SessionDuration::start(1000);
        assert!(session.is_running());
        assert_eq!(session.elapsed(1500), 500);
        session.stop(2000);
        assert!(!session.is_running());
        assert_eq!(session.elapsed(9999), 1000);
    }

    // ---------------------------------------------------------------
    // SSH auth, connection manager, remote env, port forwarding tests
    // ---------------------------------------------------------------

    #[test]
    fn ssh_auth_method_display() {
        assert_eq!(format!("{}", SshAuthMethod::Password), "Password");
        assert_eq!(
            format!("{}", SshAuthMethod::PublicKey { key_path: "~/.ssh/id_rsa".into() }),
            "PublicKey(~/.ssh/id_rsa)"
        );
        assert_eq!(format!("{}", SshAuthMethod::Agent), "Agent");
    }

    #[test]
    fn connection_manager_connect_disconnect() {
        let mut mgr = RemoteConnectionManager::new();
        assert!(mgr.is_empty());
        assert!(mgr.connect("dev", sample_conn()));
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.connect("dev", sample_conn())); // duplicate id
        assert_eq!(mgr.len(), 1);
        assert!(mgr.get("dev").is_some());
        let removed = mgr.disconnect("dev");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn connection_manager_list_connections() {
        let mut mgr = RemoteConnectionManager::new();
        mgr.connect("a", sample_conn());
        mgr.connect("b", sample_conn());
        let mut ids = mgr.list_connections();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn connection_manager_disconnect_unknown() {
        let mut mgr = RemoteConnectionManager::new();
        assert!(mgr.disconnect("nope").is_none());
    }

    #[test]
    fn remote_environment_new_and_display() {
        let env = RemoteEnvironment::new("Linux", "/home/user", "/tmp");
        assert_eq!(env.os, "Linux");
        assert_eq!(env.home_dir, "/home/user");
        assert_eq!(env.temp_dir, "/tmp");
        let s = format!("{env}");
        assert!(s.contains("Linux"));
        assert!(s.contains("/home/user"));
        assert!(s.contains("/tmp"));
    }

    #[test]
    fn port_forwarding_valid() {
        let pf = PortForwardingConfig::new(8080, 80, "remote.io").unwrap();
        assert_eq!(pf.local_port, 8080);
        assert_eq!(pf.remote_port, 80);
        assert_eq!(pf.remote_host, "remote.io");
        let s = format!("{pf}");
        assert!(s.contains("localhost:8080"));
        assert!(s.contains("remote.io:80"));
    }

    #[test]
    fn port_forwarding_zero_local_port() {
        let pf = PortForwardingConfig::new(0, 80, "remote.io");
        assert_eq!(pf, Err(RemoteError::InvalidPort(0)));
    }

    #[test]
    fn port_forwarding_zero_remote_port() {
        let pf = PortForwardingConfig::new(8080, 0, "remote.io");
        assert_eq!(pf, Err(RemoteError::InvalidPort(0)));
    }

    #[test]
    fn connection_manager_default() {
        let mgr = RemoteConnectionManager::default();
        assert!(mgr.is_empty());
    }

    #[test]
    fn remote_service_summary() {
        let svc = RemoteService::new();
        let summary = svc.service_summary();
        assert!(summary.contains("0 connections"));
        assert!(summary.contains("remote=no"));
    }

    #[test]
    fn remote_connection_display() {
        let conn = sample_conn();
        let s = conn.to_string();
        assert!(s.contains("dev-server"));
        assert!(s.contains("example.com"));
    }

    #[test]
    fn remote_authority_port_forwarding() {
        assert!(RemoteAuthority::SSH.supports_port_forwarding());
        assert!(RemoteAuthority::Tunnel.supports_port_forwarding());
        assert!(!RemoteAuthority::WSL.supports_port_forwarding());
        assert!(!RemoteAuthority::Container.supports_port_forwarding());
    }

    #[test]
    fn remote_authority_default_port() {
        assert_eq!(RemoteAuthority::SSH.default_port(), Some(22));
        assert_eq!(RemoteAuthority::WSL.default_port(), None);
    }

    #[test]
    fn session_duration_display_elapsed() {
        let s = SessionDuration::start(0);
        assert_eq!(s.display_elapsed(45), "45s");
        assert_eq!(s.display_elapsed(125), "2m 5s");
        assert_eq!(s.display_elapsed(3661), "1h 1m");
    }

    #[test]
    fn connection_history_success_rate() {
        let mut h = ConnectionHistory::new();
        h.record("host1", true, "ok");
        h.record("host2", false, "err");
        assert!((h.success_rate() - 50.0).abs() < 0.01);
    }

    #[test]
    fn remote_all_hosts() {
        let mut svc = RemoteService::new();
        svc.add_connection(sample_conn());
        let hosts = svc.all_hosts();
        assert_eq!(hosts, vec!["example.com"]);
    }

    #[test]
    fn connection_pool_add_and_full() {
        let mut pool = ConnectionPool::new(2);
        assert!(pool.is_empty());
        assert_eq!(pool.available(), 2);
        assert!(pool.add(sample_conn()).is_ok());
        assert!(pool.add(sample_conn()).is_ok());
        assert!(pool.is_full());
        assert!(pool.add(sample_conn()).is_err());
    }

    #[test]
    fn connection_pool_drain_disconnected() {
        let mut pool = ConnectionPool::new(5);
        let mut c1 = sample_conn();
        c1.status = ConnectionStatus::Connected;
        let c2 = sample_conn(); // Disconnected
        pool.add(c1).unwrap();
        pool.add(c2).unwrap();
        let drained = pool.drain_disconnected();
        assert_eq!(drained.len(), 1);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn connection_pool_remove() {
        let mut pool = ConnectionPool::new(5);
        pool.add(sample_conn()).unwrap();
        assert!(pool.remove(0).is_some());
        assert!(pool.remove(0).is_none());
        assert!(pool.is_empty());
    }

    #[test]
    fn assess_health_healthy() {
        let mut latency = LatencyTracker::new();
        latency.record(10);
        latency.record(20);
        let mut history = ConnectionHistory::new();
        history.record("h", true, "ok");
        history.record("h", true, "ok");
        assert_eq!(assess_health(&latency, &history, 100, 500), HealthStatus::Healthy);
    }

    #[test]
    fn assess_health_degraded() {
        let mut latency = LatencyTracker::new();
        latency.record(200);
        let mut history = ConnectionHistory::new();
        history.record("h", true, "ok");
        assert_eq!(assess_health(&latency, &history, 100, 500), HealthStatus::Degraded);
    }

    #[test]
    fn assess_health_unhealthy_latency() {
        let mut latency = LatencyTracker::new();
        latency.record(1000);
        let mut history = ConnectionHistory::new();
        history.record("h", true, "ok");
        assert_eq!(assess_health(&latency, &history, 100, 500), HealthStatus::Unhealthy);
    }

    #[test]
    fn assess_health_unhealthy_error_rate() {
        let latency = LatencyTracker::new();
        let mut history = ConnectionHistory::new();
        history.record("h", false, "err");
        history.record("h", false, "err");
        history.record("h", true, "ok");
        // 33% success rate < 50%
        assert_eq!(assess_health(&latency, &history, 100, 500), HealthStatus::Unhealthy);
    }

    // ---------------------------------------------------------------
    // Path mapping tests
    // ---------------------------------------------------------------

    #[test]
    fn path_mapping_local_to_remote_and_back() {
        let m = PathMapping::new("/home/user/project", "/workspace/project");
        assert_eq!(
            m.to_remote("/home/user/project/src/main.rs"),
            Some("/workspace/project/src/main.rs".into())
        );
        assert_eq!(
            m.to_local("/workspace/project/src/main.rs"),
            Some("/home/user/project/src/main.rs".into())
        );
        // Non-matching prefix returns None.
        assert_eq!(m.to_remote("/other/path"), None);
        assert_eq!(m.to_local("/other/path"), None);
    }

    #[test]
    fn path_mapping_registry_multiple_mappings() {
        let mut reg = PathMappingRegistry::new();
        assert!(reg.is_empty());
        reg.add(PathMapping::new("/home/a", "/remote/a"));
        reg.add(PathMapping::new("/home/b", "/remote/b"));
        assert_eq!(reg.len(), 2);
        assert_eq!(
            reg.to_remote("/home/b/file.txt"),
            Some("/remote/b/file.txt".into())
        );
        assert_eq!(
            reg.to_local("/remote/a/lib.rs"),
            Some("/home/a/lib.rs".into())
        );
        assert_eq!(reg.to_remote("/nomatch"), None);
    }

    // ---------------------------------------------------------------
    // Heartbeat tracker tests
    // ---------------------------------------------------------------

    #[test]
    fn heartbeat_tracker_liveness() {
        let mut hb = HeartbeatTracker::new(30);
        // No beats yet – not alive.
        assert!(!hb.is_alive(100));
        assert_eq!(hb.seconds_since_last(100), None);

        hb.beat(100);
        assert!(hb.is_alive(120)); // 20s < 30s timeout
        assert!(!hb.is_alive(200)); // 100s > 30s timeout
        assert_eq!(hb.seconds_since_last(110), Some(10));
        assert_eq!(hb.count(), 1);
    }

    #[test]
    fn heartbeat_average_interval() {
        let mut hb = HeartbeatTracker::new(60);
        hb.beat(100);
        assert_eq!(hb.average_interval(), None); // need >=2 samples
        hb.beat(110);
        hb.beat(130);
        // intervals: 10, 20 → avg = 15
        assert_eq!(hb.average_interval(), Some(15));
        hb.reset();
        assert_eq!(hb.count(), 0);
    }

    // ---------------------------------------------------------------
    // Port forwarding manager tests
    // ---------------------------------------------------------------

    #[test]
    fn port_forwarding_manager_add_find_remove() {
        let mut mgr = PortForwardingManager::new();
        assert!(mgr.is_empty());

        let r1 = PortForwardingConfig::new(3000, 80, "server.io").unwrap();
        let r2 = PortForwardingConfig::new(3001, 443, "server.io").unwrap();
        mgr.add_rule(r1).unwrap();
        mgr.add_rule(r2).unwrap();
        assert_eq!(mgr.len(), 2);

        // Duplicate local port rejected.
        let dup = PortForwardingConfig::new(3000, 8080, "other.io").unwrap();
        assert!(mgr.add_rule(dup).is_err());

        // Lookup by local port.
        let found = mgr.find_by_local_port(3000).unwrap();
        assert_eq!(found.remote_port, 80);

        // Lookup by host.
        assert_eq!(mgr.rules_for_host("server.io").len(), 2);
        assert_eq!(mgr.rules_for_host("unknown").len(), 0);

        // Remove.
        assert!(mgr.remove_by_local_port(3000));
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.remove_by_local_port(9999));

        mgr.clear();
        assert!(mgr.is_empty());
    }

    #[test]
    fn path_mapping_roots_accessors() {
        let m = PathMapping::new("/local", "/remote");
        assert_eq!(m.local_root(), "/local");
        assert_eq!(m.remote_root(), "/remote");
    }
}
