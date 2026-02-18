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

// ---------------------------------------------------------------------------
// Remote URI construction and parsing
// ---------------------------------------------------------------------------

/// A parsed remote URI of the form `scheme://[user@]host[:port]/path`.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteUri {
    pub scheme: String,
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
}

impl RemoteUri {
    /// Parse a URI string into its components.
    ///
    /// Accepted formats:
    /// - `ssh://user@host:22/home/user`
    /// - `ssh://host/path`
    /// - `wsl://distro/path`
    pub fn parse(uri: &str) -> Result<Self, RemoteError> {
        let (scheme, rest) = uri
            .split_once("://")
            .ok_or_else(|| RemoteError::InvalidHost("missing scheme".into()))?;
        if scheme.is_empty() {
            return Err(RemoteError::InvalidHost("empty scheme".into()));
        }

        let (authority, path) = match rest.find('/') {
            Some(idx) => (&rest[..idx], &rest[idx..]),
            None => (rest, "/"),
        };

        let (userhost, port) = if let Some(idx) = authority.rfind(':') {
            if let Ok(p) = authority[idx + 1..].parse::<u16>() {
                (&authority[..idx], Some(p))
            } else {
                (authority, None)
            }
        } else {
            (authority, None)
        };

        let (user, host) = if let Some(idx) = userhost.find('@') {
            (Some(userhost[..idx].to_string()), &userhost[idx + 1..])
        } else {
            (None, userhost)
        };

        if host.is_empty() {
            return Err(RemoteError::InvalidHost("empty host".into()));
        }

        Ok(Self {
            scheme: scheme.to_string(),
            user,
            host: host.to_string(),
            port,
            path: path.to_string(),
        })
    }

    /// Reconstruct the URI string.
    pub fn to_uri_string(&self) -> String {
        let mut s = format!("{}://", self.scheme);
        if let Some(ref user) = self.user {
            s.push_str(user);
            s.push('@');
        }
        s.push_str(&self.host);
        if let Some(p) = self.port {
            s.push(':');
            s.push_str(&p.to_string());
        }
        s.push_str(&self.path);
        s
    }

    /// Derive a `RemoteAuthority` from the scheme.
    pub fn authority_type(&self) -> RemoteAuthority {
        match self.scheme.to_lowercase().as_str() {
            "ssh" => RemoteAuthority::SSH,
            "wsl" => RemoteAuthority::WSL,
            "container" | "docker" => RemoteAuthority::Container,
            "tunnel" => RemoteAuthority::Tunnel,
            other => RemoteAuthority::Custom(other.to_string()),
        }
    }

    /// Build a `ConnectionConfig` from this URI using the path basename as label.
    pub fn to_connection_config(&self) -> Result<ConnectionConfig, RemoteError> {
        let label = self
            .path
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or(&self.host);
        ConnectionConfig::new(&self.host, self.port, label, self.authority_type())
    }
}

impl fmt::Display for RemoteUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uri_string())
    }
}

// ---------------------------------------------------------------------------
// Reconnection strategy
// ---------------------------------------------------------------------------

/// Configurable reconnection strategy with exponential backoff.
#[derive(Debug, Clone)]
pub struct ReconnectionStrategy {
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_attempts: u32,
    current_attempt: u32,
}

impl ReconnectionStrategy {
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, max_attempts: u32) -> Self {
        Self {
            base_delay_ms,
            max_delay_ms,
            max_attempts,
            current_attempt: 0,
        }
    }

    /// Returns the next delay in ms and increments the attempt counter,
    /// or `None` if max attempts have been exhausted.
    pub fn next_delay(&mut self) -> Option<u64> {
        if self.current_attempt >= self.max_attempts {
            return None;
        }
        let delay = compute_backoff(self.current_attempt, self.base_delay_ms, self.max_delay_ms);
        self.current_attempt += 1;
        Some(delay)
    }

    /// How many attempts have been made so far.
    pub fn attempts_made(&self) -> u32 {
        self.current_attempt
    }

    /// How many attempts remain.
    pub fn attempts_remaining(&self) -> u32 {
        self.max_attempts.saturating_sub(self.current_attempt)
    }

    /// Whether all attempts have been exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.current_attempt >= self.max_attempts
    }

    /// Reset the attempt counter for a fresh reconnection cycle.
    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }
}

// ---------------------------------------------------------------------------
// Connection timeout configuration
// ---------------------------------------------------------------------------

/// Timeout settings for different phases of a remote connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutConfig {
    pub connect_ms: u64,
    pub handshake_ms: u64,
    pub idle_ms: u64,
}

impl TimeoutConfig {
    pub fn new(connect_ms: u64, handshake_ms: u64, idle_ms: u64) -> Self {
        Self {
            connect_ms,
            handshake_ms,
            idle_ms,
        }
    }

    /// Check whether a given duration exceeds the connect timeout.
    pub fn is_connect_timeout(&self, elapsed_ms: u64) -> bool {
        elapsed_ms > self.connect_ms
    }

    /// Check whether a given idle duration exceeds the idle timeout.
    pub fn is_idle_timeout(&self, idle_ms: u64) -> bool {
        idle_ms > self.idle_ms
    }

    /// Total maximum time allowed for establishing a connection
    /// (connect + handshake).
    pub fn total_setup_ms(&self) -> u64 {
        self.connect_ms.saturating_add(self.handshake_ms)
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect_ms: 10_000,
            handshake_ms: 5_000,
            idle_ms: 300_000,
        }
    }
}

// ---------------------------------------------------------------------------
// SSH config host entry parsing
// ---------------------------------------------------------------------------

/// A single SSH config host entry with common fields.
#[derive(Debug, Clone, PartialEq)]
pub struct SshHostEntry {
    pub host_pattern: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
}

impl SshHostEntry {
    /// Parse a simplified SSH config block (lines of `Key Value`).
    ///
    /// The first line must be `Host <pattern>`. Subsequent indented lines
    /// set fields like `HostName`, `User`, `Port`, `IdentityFile`.
    pub fn parse_block(block: &str) -> Option<Self> {
        let mut lines = block.lines();
        let first = lines.next()?.trim();
        let host_pattern = first.strip_prefix("Host ")?.trim().to_string();
        if host_pattern.is_empty() {
            return None;
        }

        let mut entry = SshHostEntry {
            host_pattern,
            hostname: None,
            user: None,
            port: None,
            identity_file: None,
        };

        for line in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once(char::is_whitespace) {
                let value = value.trim();
                match key {
                    "HostName" | "Hostname" => entry.hostname = Some(value.to_string()),
                    "User" => entry.user = Some(value.to_string()),
                    "Port" => entry.port = value.parse().ok(),
                    "IdentityFile" => entry.identity_file = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        Some(entry)
    }

    /// The effective hostname (falls back to host_pattern if HostName is unset).
    pub fn effective_hostname(&self) -> &str {
        self.hostname.as_deref().unwrap_or(&self.host_pattern)
    }

    /// Build a `ConnectionConfig` from this entry.
    pub fn to_connection_config(&self) -> Result<ConnectionConfig, RemoteError> {
        ConnectionConfig::new(
            self.effective_hostname(),
            self.port,
            &self.host_pattern,
            RemoteAuthority::SSH,
        )
    }
}

// ---------------------------------------------------------------------------
// Remote environment detection helpers
// ---------------------------------------------------------------------------

/// Detect the `RemoteAuthority` type from well-known environment variable
/// names that remote hosts typically set.
pub fn detect_authority_from_env_vars(vars: &[(&str, &str)]) -> Option<RemoteAuthority> {
    for &(key, _value) in vars {
        match key {
            "SSH_CONNECTION" | "SSH_CLIENT" | "SSH_TTY" => return Some(RemoteAuthority::SSH),
            "WSL_DISTRO_NAME" | "WSL_INTEROP" => return Some(RemoteAuthority::WSL),
            "container" | "DOCKER_CONTAINER_ID" => return Some(RemoteAuthority::Container),
            _ => {}
        }
    }
    None
}

/// Check whether the given environment variables suggest we are running
/// inside a remote (non-local) environment.
pub fn is_remote_environment(vars: &[(&str, &str)]) -> bool {
    detect_authority_from_env_vars(vars).is_some()
}

// ---------------------------------------------------------------------------
// RemoteConnectionPoolManager
// ---------------------------------------------------------------------------

/// Health status of a pooled connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PooledConnectionHealth {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

impl fmt::Display for PooledConnectionHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PooledConnectionHealth::Healthy => write!(f, "healthy"),
            PooledConnectionHealth::Degraded(msg) => write!(f, "degraded: {msg}"),
            PooledConnectionHealth::Unhealthy(msg) => write!(f, "unhealthy: {msg}"),
        }
    }
}

/// A single entry in the connection pool.
#[derive(Debug, Clone)]
pub struct PooledConnectionEntry {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub health: PooledConnectionHealth,
    pub created_at: u64,
    pub last_used: u64,
    pub use_count: u64,
}

impl PooledConnectionEntry {
    pub fn new(id: impl Into<String>, host: impl Into<String>, port: u16, created_at: u64) -> Self {
        Self {
            id: id.into(),
            host: host.into(),
            port,
            health: PooledConnectionHealth::Healthy,
            created_at,
            last_used: created_at,
            use_count: 0,
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.health == PooledConnectionHealth::Healthy
    }

    pub fn idle_time(&self, now: u64) -> u64 {
        now.saturating_sub(self.last_used)
    }
}

impl fmt::Display for PooledConnectionEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}:{} [{}] uses={}", self.id, self.host, self.port, self.health, self.use_count)
    }
}

/// Manages a pool of remote connections with health checks and idle eviction.
pub struct RemoteConnectionPoolManager {
    pool: Vec<PooledConnectionEntry>,
    max_size: usize,
    max_idle_secs: u64,
}

impl RemoteConnectionPoolManager {
    pub fn new(max_size: usize, max_idle_secs: u64) -> Self {
        Self { pool: Vec::new(), max_size, max_idle_secs }
    }

    pub fn add(&mut self, entry: PooledConnectionEntry) -> Result<(), String> {
        if self.pool.len() >= self.max_size {
            return Err("pool is full".into());
        }
        if self.pool.iter().any(|e| e.id == entry.id) {
            return Err(format!("duplicate id: {}", entry.id));
        }
        self.pool.push(entry);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.pool.len();
        self.pool.retain(|e| e.id != id);
        self.pool.len() < before
    }

    pub fn get(&self, id: &str) -> Option<&PooledConnectionEntry> {
        self.pool.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut PooledConnectionEntry> {
        self.pool.iter_mut().find(|e| e.id == id)
    }

    pub fn acquire(&mut self, id: &str, now: u64) -> Option<&PooledConnectionEntry> {
        if let Some(entry) = self.pool.iter_mut().find(|e| e.id == id && e.is_healthy()) {
            entry.last_used = now;
            entry.use_count += 1;
            Some(entry)
        } else {
            None
        }
    }

    pub fn size(&self) -> usize {
        self.pool.len()
    }

    pub fn healthy_count(&self) -> usize {
        self.pool.iter().filter(|e| e.is_healthy()).count()
    }

    pub fn evict_idle(&mut self, now: u64) -> usize {
        let before = self.pool.len();
        self.pool.retain(|e| e.idle_time(now) <= self.max_idle_secs);
        before - self.pool.len()
    }

    pub fn set_health(&mut self, id: &str, health: PooledConnectionHealth) -> bool {
        if let Some(entry) = self.pool.iter_mut().find(|e| e.id == id) {
            entry.health = health;
            true
        } else {
            false
        }
    }

    pub fn unhealthy_entries(&self) -> Vec<&PooledConnectionEntry> {
        self.pool.iter().filter(|e| matches!(e.health, PooledConnectionHealth::Unhealthy(_))).collect()
    }

    /// Least-recently-used healthy connection.
    pub fn least_recently_used(&self) -> Option<&PooledConnectionEntry> {
        self.pool.iter().filter(|e| e.is_healthy()).min_by_key(|e| e.last_used)
    }
}

impl fmt::Display for RemoteConnectionPoolManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RemoteConnectionPoolManager({}/{} conns, idle_max={}s)",
            self.pool.len(), self.max_size, self.max_idle_secs)
    }
}

// ---------------------------------------------------------------------------
// RemoteHeartbeatMonitor
// ---------------------------------------------------------------------------

/// A heartbeat record.
#[derive(Debug, Clone, PartialEq)]
pub struct HeartbeatRecord {
    pub connection_id: String,
    pub timestamp: u64,
    pub latency_ms: u32,
    pub success: bool,
}

impl HeartbeatRecord {
    pub fn success(conn_id: impl Into<String>, timestamp: u64, latency_ms: u32) -> Self {
        Self { connection_id: conn_id.into(), timestamp, latency_ms, success: true }
    }

    pub fn failure(conn_id: impl Into<String>, timestamp: u64) -> Self {
        Self { connection_id: conn_id.into(), timestamp, latency_ms: 0, success: false }
    }
}

impl fmt::Display for HeartbeatRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "ok" } else { "fail" };
        write!(f, "heartbeat[{}] {} {}ms t={}", self.connection_id, status, self.latency_ms, self.timestamp)
    }
}

/// Monitors remote connection health via periodic heartbeat tracking.
pub struct RemoteHeartbeatMonitor {
    records: Vec<HeartbeatRecord>,
    max_records: usize,
    failure_threshold: u32,
}

impl RemoteHeartbeatMonitor {
    pub fn new(max_records: usize, failure_threshold: u32) -> Self {
        Self { records: Vec::new(), max_records, failure_threshold }
    }

    pub fn record_heartbeat(&mut self, record: HeartbeatRecord) {
        self.records.push(record);
        if self.records.len() > self.max_records {
            self.records.remove(0);
        }
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Returns consecutive failure count for a connection (from most recent).
    pub fn consecutive_failures(&self, conn_id: &str) -> u32 {
        let mut count = 0u32;
        for r in self.records.iter().rev() {
            if r.connection_id != conn_id {
                continue;
            }
            if r.success {
                break;
            }
            count += 1;
        }
        count
    }

    /// Whether a connection should be considered dead (exceeded failure threshold).
    pub fn is_connection_dead(&self, conn_id: &str) -> bool {
        self.consecutive_failures(conn_id) >= self.failure_threshold
    }

    /// Average latency for successful heartbeats of a connection.
    pub fn avg_latency(&self, conn_id: &str) -> Option<f64> {
        let successes: Vec<u32> = self
            .records
            .iter()
            .filter(|r| r.connection_id == conn_id && r.success)
            .map(|r| r.latency_ms)
            .collect();
        if successes.is_empty() {
            None
        } else {
            let sum: u32 = successes.iter().sum();
            Some(sum as f64 / successes.len() as f64)
        }
    }

    /// Last successful heartbeat timestamp for a connection.
    pub fn last_success(&self, conn_id: &str) -> Option<u64> {
        self.records
            .iter()
            .rev()
            .find(|r| r.connection_id == conn_id && r.success)
            .map(|r| r.timestamp)
    }

    /// All unique connection IDs that have been monitored.
    pub fn monitored_connections(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for r in &self.records {
            if seen.insert(r.connection_id.clone()) {
                result.push(r.connection_id.clone());
            }
        }
        result
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl fmt::Display for RemoteHeartbeatMonitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RemoteHeartbeatMonitor({} records, threshold={})",
            self.records.len(), self.failure_threshold)
    }
}



/// Remote connection configuration manager.
#[derive(Debug, Clone)]
pub struct RemoteConfig {
    entries: Vec<RemoteEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single remote connection entry.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl RemoteEntry {
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

impl RemoteConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: RemoteEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&RemoteEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut RemoteEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&RemoteEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&RemoteEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&RemoteEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<RemoteEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
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

    // ---------------------------------------------------------------
    // Remote URI parsing and construction tests
    // ---------------------------------------------------------------

    #[test]
    fn remote_uri_parse_full() {
        let uri = RemoteUri::parse("ssh://user@host.io:2222/home/user").unwrap();
        assert_eq!(uri.scheme, "ssh");
        assert_eq!(uri.user, Some("user".to_string()));
        assert_eq!(uri.host, "host.io");
        assert_eq!(uri.port, Some(2222));
        assert_eq!(uri.path, "/home/user");
    }

    #[test]
    fn remote_uri_parse_no_user_no_port() {
        let uri = RemoteUri::parse("wsl://Ubuntu/home/dev").unwrap();
        assert_eq!(uri.scheme, "wsl");
        assert_eq!(uri.user, None);
        assert_eq!(uri.host, "Ubuntu");
        assert_eq!(uri.port, None);
        assert_eq!(uri.path, "/home/dev");
    }

    #[test]
    fn remote_uri_parse_no_path() {
        let uri = RemoteUri::parse("ssh://host.io").unwrap();
        assert_eq!(uri.host, "host.io");
        assert_eq!(uri.path, "/");
    }

    #[test]
    fn remote_uri_parse_missing_scheme() {
        assert!(RemoteUri::parse("host.io/path").is_err());
    }

    #[test]
    fn remote_uri_parse_empty_host() {
        assert!(RemoteUri::parse("ssh:///path").is_err());
    }

    #[test]
    fn remote_uri_roundtrip() {
        let original = "ssh://admin@server.com:22/data";
        let uri = RemoteUri::parse(original).unwrap();
        assert_eq!(uri.to_uri_string(), original);
        assert_eq!(format!("{uri}"), original);
    }

    #[test]
    fn remote_uri_authority_type() {
        assert_eq!(
            RemoteUri::parse("ssh://h/p").unwrap().authority_type(),
            RemoteAuthority::SSH
        );
        assert_eq!(
            RemoteUri::parse("wsl://h/p").unwrap().authority_type(),
            RemoteAuthority::WSL
        );
        assert_eq!(
            RemoteUri::parse("docker://h/p").unwrap().authority_type(),
            RemoteAuthority::Container
        );
        assert_eq!(
            RemoteUri::parse("tunnel://h/p").unwrap().authority_type(),
            RemoteAuthority::Tunnel
        );
        assert_eq!(
            RemoteUri::parse("myproto://h/p").unwrap().authority_type(),
            RemoteAuthority::Custom("myproto".into())
        );
    }

    #[test]
    fn remote_uri_to_connection_config() {
        let uri = RemoteUri::parse("ssh://root@prod.io:22/opt/app").unwrap();
        let cfg = uri.to_connection_config().unwrap();
        assert_eq!(cfg.host, "prod.io");
        assert_eq!(cfg.port, Some(22));
        assert_eq!(cfg.label, "app");
        assert_eq!(cfg.authority, RemoteAuthority::SSH);
    }

    // ---------------------------------------------------------------
    // Reconnection strategy tests
    // ---------------------------------------------------------------

    #[test]
    fn reconnection_strategy_delays() {
        let mut strat = ReconnectionStrategy::new(100, 5000, 4);
        assert_eq!(strat.attempts_remaining(), 4);
        assert!(!strat.is_exhausted());

        assert_eq!(strat.next_delay(), Some(100));
        assert_eq!(strat.next_delay(), Some(200));
        assert_eq!(strat.next_delay(), Some(400));
        assert_eq!(strat.next_delay(), Some(800));
        assert_eq!(strat.attempts_made(), 4);
        assert!(strat.is_exhausted());
        assert_eq!(strat.next_delay(), None);

        strat.reset();
        assert_eq!(strat.attempts_made(), 0);
        assert_eq!(strat.next_delay(), Some(100));
    }

    #[test]
    fn reconnection_strategy_capped() {
        let mut strat = ReconnectionStrategy::new(1000, 3000, 10);
        strat.next_delay(); // 1000
        strat.next_delay(); // 2000
        assert_eq!(strat.next_delay(), Some(3000)); // capped
        assert_eq!(strat.next_delay(), Some(3000)); // stays capped
    }

    // ---------------------------------------------------------------
    // Timeout config tests
    // ---------------------------------------------------------------

    #[test]
    fn timeout_config_defaults_and_checks() {
        let tc = TimeoutConfig::default();
        assert_eq!(tc.connect_ms, 10_000);
        assert_eq!(tc.handshake_ms, 5_000);
        assert_eq!(tc.idle_ms, 300_000);
        assert_eq!(tc.total_setup_ms(), 15_000);

        assert!(!tc.is_connect_timeout(5_000));
        assert!(tc.is_connect_timeout(15_000));

        assert!(!tc.is_idle_timeout(100_000));
        assert!(tc.is_idle_timeout(400_000));
    }

    #[test]
    fn timeout_config_custom() {
        let tc = TimeoutConfig::new(500, 200, 1000);
        assert_eq!(tc.total_setup_ms(), 700);
        assert!(tc.is_connect_timeout(501));
        assert!(!tc.is_connect_timeout(500));
    }

    // ---------------------------------------------------------------
    // SSH host entry parsing tests
    // ---------------------------------------------------------------

    #[test]
    fn ssh_host_entry_parse_full_block() {
        let block = "\
Host myserver
    HostName 192.168.1.100
    User deploy
    Port 2222
    IdentityFile ~/.ssh/deploy_key";
        let entry = SshHostEntry::parse_block(block).unwrap();
        assert_eq!(entry.host_pattern, "myserver");
        assert_eq!(entry.hostname, Some("192.168.1.100".into()));
        assert_eq!(entry.user, Some("deploy".into()));
        assert_eq!(entry.port, Some(2222));
        assert_eq!(entry.identity_file, Some("~/.ssh/deploy_key".into()));
        assert_eq!(entry.effective_hostname(), "192.168.1.100");
    }

    #[test]
    fn ssh_host_entry_minimal_block() {
        let block = "Host devbox";
        let entry = SshHostEntry::parse_block(block).unwrap();
        assert_eq!(entry.host_pattern, "devbox");
        assert_eq!(entry.hostname, None);
        assert_eq!(entry.effective_hostname(), "devbox");
    }

    #[test]
    fn ssh_host_entry_to_connection_config() {
        let block = "\
Host prod
    HostName prod.example.com
    Port 22";
        let entry = SshHostEntry::parse_block(block).unwrap();
        let cfg = entry.to_connection_config().unwrap();
        assert_eq!(cfg.host, "prod.example.com");
        assert_eq!(cfg.port, Some(22));
        assert_eq!(cfg.label, "prod");
        assert_eq!(cfg.authority, RemoteAuthority::SSH);
    }

    #[test]
    fn ssh_host_entry_parse_invalid() {
        assert!(SshHostEntry::parse_block("NotAHost line").is_none());
        assert!(SshHostEntry::parse_block("Host ").is_none());
        assert!(SshHostEntry::parse_block("").is_none());
    }

    #[test]
    fn ssh_host_entry_skips_comments() {
        let block = "\
Host ci
    # This is a comment
    HostName ci.internal
    # Another comment
    User runner";
        let entry = SshHostEntry::parse_block(block).unwrap();
        assert_eq!(entry.hostname, Some("ci.internal".into()));
        assert_eq!(entry.user, Some("runner".into()));
    }

    // ---------------------------------------------------------------
    // Remote environment detection tests
    // ---------------------------------------------------------------

    #[test]
    fn detect_authority_ssh() {
        let vars = vec![("SSH_CONNECTION", "1.2.3.4 5678 10.0.0.1 22")];
        assert_eq!(detect_authority_from_env_vars(&vars), Some(RemoteAuthority::SSH));
    }

    #[test]
    fn detect_authority_wsl() {
        let vars = vec![("WSL_DISTRO_NAME", "Ubuntu")];
        assert_eq!(detect_authority_from_env_vars(&vars), Some(RemoteAuthority::WSL));
    }

    #[test]
    fn detect_authority_container() {
        let vars = vec![("container", "podman")];
        assert_eq!(
            detect_authority_from_env_vars(&vars),
            Some(RemoteAuthority::Container)
        );
    }

    #[test]
    fn detect_authority_none() {
        let vars = vec![("HOME", "/home/user"), ("TERM", "xterm")];
        assert_eq!(detect_authority_from_env_vars(&vars), None);
    }

    #[test]
    fn is_remote_environment_true_and_false() {
        assert!(is_remote_environment(&[("SSH_TTY", "/dev/pts/0")]));
        assert!(!is_remote_environment(&[("PATH", "/usr/bin")]));
    }

    #[test]
    fn pool_add_and_get() {
        let mut pool = RemoteConnectionPoolManager::new(10, 300);
        let entry = PooledConnectionEntry::new("c1", "host1", 22, 100);
        pool.add(entry).unwrap();
        assert_eq!(pool.size(), 1);
        assert!(pool.get("c1").is_some());
    }

    #[test]
    fn pool_add_full() {
        let mut pool = RemoteConnectionPoolManager::new(1, 300);
        pool.add(PooledConnectionEntry::new("c1", "h", 22, 1)).unwrap();
        assert!(pool.add(PooledConnectionEntry::new("c2", "h", 22, 1)).is_err());
    }

    #[test]
    fn pool_add_duplicate() {
        let mut pool = RemoteConnectionPoolManager::new(10, 300);
        pool.add(PooledConnectionEntry::new("c1", "h", 22, 1)).unwrap();
        assert!(pool.add(PooledConnectionEntry::new("c1", "h2", 22, 2)).is_err());
    }

    #[test]
    fn pool_remove() {
        let mut pool = RemoteConnectionPoolManager::new(10, 300);
        pool.add(PooledConnectionEntry::new("c1", "h", 22, 1)).unwrap();
        assert!(pool.remove("c1"));
        assert_eq!(pool.size(), 0);
        assert!(!pool.remove("c1"));
    }

    #[test]
    fn pool_acquire() {
        let mut pool = RemoteConnectionPoolManager::new(10, 300);
        pool.add(PooledConnectionEntry::new("c1", "h", 22, 100)).unwrap();
        let entry = pool.acquire("c1", 200).unwrap();
        assert_eq!(entry.use_count, 1);
        assert_eq!(entry.last_used, 200);
    }

    #[test]
    fn pool_evict_idle() {
        let mut pool = RemoteConnectionPoolManager::new(10, 100);
        pool.add(PooledConnectionEntry::new("old", "h", 22, 10)).unwrap();
        pool.add(PooledConnectionEntry::new("new", "h", 22, 500)).unwrap();
        let evicted = pool.evict_idle(500);
        assert_eq!(evicted, 1);
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn pool_health_management() {
        let mut pool = RemoteConnectionPoolManager::new(10, 300);
        pool.add(PooledConnectionEntry::new("c1", "h", 22, 1)).unwrap();
        pool.set_health("c1", PooledConnectionHealth::Unhealthy("timeout".into()));
        assert_eq!(pool.healthy_count(), 0);
        assert_eq!(pool.unhealthy_entries().len(), 1);
    }

    #[test]
    fn pool_least_recently_used() {
        let mut pool = RemoteConnectionPoolManager::new(10, 300);
        pool.add(PooledConnectionEntry::new("c1", "h", 22, 100)).unwrap();
        pool.add(PooledConnectionEntry::new("c2", "h", 22, 50)).unwrap();
        let lru = pool.least_recently_used().unwrap();
        assert_eq!(lru.id, "c2");
    }

    #[test]
    fn pool_display() {
        let pool = RemoteConnectionPoolManager::new(5, 60);
        assert!(format!("{pool}").contains("0/5"));
    }

    #[test]
    fn pooled_entry_display_and_idle() {
        let entry = PooledConnectionEntry::new("c1", "host", 22, 100);
        assert!(format!("{entry}").contains("c1"));
        assert_eq!(entry.idle_time(200), 100);
    }

    #[test]
    fn heartbeat_record_success_and_failure() {
        let s = HeartbeatRecord::success("c1", 100, 50);
        assert!(s.success);
        let f = HeartbeatRecord::failure("c1", 200);
        assert!(!f.success);
    }

    #[test]
    fn heartbeat_consecutive_failures() {
        let mut monitor = RemoteHeartbeatMonitor::new(100, 3);
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 1, 10));
        monitor.record_heartbeat(HeartbeatRecord::failure("c1", 2));
        monitor.record_heartbeat(HeartbeatRecord::failure("c1", 3));
        assert_eq!(monitor.consecutive_failures("c1"), 2);
        assert!(!monitor.is_connection_dead("c1"));
    }

    #[test]
    fn heartbeat_connection_dead() {
        let mut monitor = RemoteHeartbeatMonitor::new(100, 2);
        monitor.record_heartbeat(HeartbeatRecord::failure("c1", 1));
        monitor.record_heartbeat(HeartbeatRecord::failure("c1", 2));
        assert!(monitor.is_connection_dead("c1"));
    }

    #[test]
    fn heartbeat_avg_latency() {
        let mut monitor = RemoteHeartbeatMonitor::new(100, 3);
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 1, 100));
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 2, 200));
        let avg = monitor.avg_latency("c1").unwrap();
        assert!((avg - 150.0).abs() < 0.01);
    }

    #[test]
    fn heartbeat_avg_latency_none() {
        let monitor = RemoteHeartbeatMonitor::new(100, 3);
        assert!(monitor.avg_latency("nope").is_none());
    }

    #[test]
    fn heartbeat_last_success() {
        let mut monitor = RemoteHeartbeatMonitor::new(100, 3);
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 100, 10));
        monitor.record_heartbeat(HeartbeatRecord::failure("c1", 200));
        assert_eq!(monitor.last_success("c1"), Some(100));
    }

    #[test]
    fn heartbeat_monitored_connections() {
        let mut monitor = RemoteHeartbeatMonitor::new(100, 3);
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 1, 10));
        monitor.record_heartbeat(HeartbeatRecord::success("c2", 2, 20));
        let conns = monitor.monitored_connections();
        assert_eq!(conns.len(), 2);
    }

    #[test]
    fn heartbeat_max_records() {
        let mut monitor = RemoteHeartbeatMonitor::new(2, 3);
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 1, 10));
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 2, 20));
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 3, 30));
        assert_eq!(monitor.record_count(), 2);
    }

    #[test]
    fn heartbeat_display_and_clear() {
        let mut monitor = RemoteHeartbeatMonitor::new(100, 3);
        monitor.record_heartbeat(HeartbeatRecord::success("c1", 1, 10));
        assert!(format!("{monitor}").contains("1 records"));
        monitor.clear();
        assert_eq!(monitor.record_count(), 0);
    }

    #[test]
    fn heartbeat_record_display() {
        let r = HeartbeatRecord::success("c1", 100, 50);
        let s = format!("{r}");
        assert!(s.contains("c1"));
        assert!(s.contains("ok"));
        assert!(s.contains("50ms"));
    }

    #[test]
    fn pooled_conn_health_display() {
        assert_eq!(format!("{}", PooledConnectionHealth::Healthy), "healthy");
        assert!(format!("{}", PooledConnectionHealth::Degraded("slow".into())).contains("slow"));
        assert!(format!("{}", PooledConnectionHealth::Unhealthy("down".into())).contains("down"));
    }


    #[test]
    fn remote_entry_creation() {
        let e = RemoteEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn remote_entry_with_priority() {
        let e = RemoteEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn remote_entry_metadata() {
        let e = RemoteEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn remote_entry_remove_meta() {
        let mut e = RemoteEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn remote_entry_activate_deactivate() {
        let mut e = RemoteEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn remote_config_add_sorted() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("lo", "Lo").with_priority(1));
        c.add(RemoteEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn remote_config_capacity() {
        let mut c = RemoteConfig::new(1);
        assert!(c.add(RemoteEntry::new("a", "A")));
        assert!(!c.add(RemoteEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn remote_config_remove() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn remote_config_get() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn remote_config_active_entries() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "A"));
        c.add(RemoteEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn remote_config_enable_disable() {
        let mut c = RemoteConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn remote_config_clear() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn remote_config_find_by_label() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn remote_config_top_n() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "A").with_priority(1));
        c.add(RemoteEntry::new("b", "B").with_priority(2));
        c.add(RemoteEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn remote_config_deactivate_activate_all() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "A"));
        c.add(RemoteEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn remote_config_highest_priority() {
        let mut c = RemoteConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(RemoteEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn remote_config_contains() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn remote_config_labels() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "Alpha"));
        c.add(RemoteEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn remote_config_drain_inactive() {
        let mut c = RemoteConfig::new(10);
        c.add(RemoteEntry::new("a", "A"));
        c.add(RemoteEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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

}
