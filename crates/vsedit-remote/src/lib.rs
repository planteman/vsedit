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


// ---------------------------------------------------------------------------
// xa_ extended helpers for remote
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaRemoteRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaRemoteRingBuf {
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
pub struct XaRemoteCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaRemoteCounter {
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

impl Default for XaRemoteCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 149
// ---------------------------------------------------------------------------

/// Generic object pool `Xc149Pool<T>`.
pub struct Xc149Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc149Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc149PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc149Pool<T> {
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
    pub fn stats(&self) -> Xc149PoolStats {
        Xc149PoolStats {
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

impl<T> Default for Xc149Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc149Scheduler`.
pub struct Xc149Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc149Scheduler {
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

impl Default for Xc149Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_149 hash for the given byte slice.
pub fn xc_149_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_149 convention.
pub fn xc_149_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_70 deepening: state machine + event bus ---

/// States for the Xd70 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd70State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd70State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd70Transition {
    pub from: Xd70State,
    pub to: Xd70State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd70StateMachine {
    current: Xd70State,
    history: Vec<Xd70Transition>,
    step_counter: usize,
}

impl Xd70StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd70State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd70State {
        self.current
    }

    pub fn history(&self) -> &[Xd70Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd70State) -> Result<Xd70State, String> {
        let allowed = match (self.current, target) {
            (Xd70State::Idle, Xd70State::Running) => true,
            (Xd70State::Running, Xd70State::Paused) => true,
            (Xd70State::Running, Xd70State::Done) => true,
            (Xd70State::Paused, Xd70State::Running) => true,
            (Xd70State::Paused, Xd70State::Done) => true,
            (Xd70State::Done, Xd70State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_70: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd70Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd70SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd70State> {
        let prefix = "Xd70SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd70State::Idle),
            "Running" => Some(Xd70State::Running),
            "Paused" => Some(Xd70State::Paused),
            "Done" => Some(Xd70State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd70State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd70 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd70Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd70Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd70HandlerFn = Box<dyn Fn(&Xd70Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd70EventBus {
    handlers: Vec<(usize, Option<String>, Xd70HandlerFn)>,
    next_id: usize,
    published: Vec<Xd70Event>,
}

impl Xd70EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd70Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd70Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd70Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd70Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #79
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf79Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf79TrieNode {
    children: std::collections::HashMap<char, Xf79TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf79Trie {
    root: Xf79TrieNode,
    count: usize,
}

impl Xf79Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf79TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf79TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf79TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf79BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf79BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 148).
pub struct Xh148SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh148SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 190 as u64,
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

/// A compact bit set supporting boolean operations (variant 148).
pub struct Xh148BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh148BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 148).
pub struct Xi148Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi148Deque<T> {
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
pub struct Xi148Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi148Interval {
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

/// A simple interval tree (variant 148).
pub struct Xi148IntervalTree {
    xi_intervals: Vec<Xi148Interval>,
}

impl Xi148IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi148Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi148Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi148Interval) -> Vec<&Xi148Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi148Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi148Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi148Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi148Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi148Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi148Interval> = Vec::new();
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


    // xa_ extended tests for remote
    #[test]
    fn xa_remote_ring_new() {
        let rb = super::XaRemoteRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_remote_ring_push_len() {
        let mut rb = super::XaRemoteRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_remote_ring_wrap() {
        let mut rb = super::XaRemoteRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_remote_ring_mean_empty() {
        let rb = super::XaRemoteRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_remote_ring_mean_values() {
        let mut rb = super::XaRemoteRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_remote_ring_min_max() {
        let mut rb = super::XaRemoteRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_remote_ring_iter() {
        let mut rb = super::XaRemoteRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_remote_counter_new() {
        let c = super::XaRemoteCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_remote_counter_inc() {
        let mut c = super::XaRemoteCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_remote_counter_inc_by() {
        let mut c = super::XaRemoteCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_remote_counter_reset() {
        let mut c = super::XaRemoteCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_remote_counter_clear() {
        let mut c = super::XaRemoteCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_remote_counter_default() {
        let c = super::XaRemoteCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 149 ----

    #[test]
    fn xc_149_pool_new_empty() {
        let pool: super::Xc149Pool<i32> = super::Xc149Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_149_pool_release_acquire() {
        let mut pool = super::Xc149Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_149_pool_acquire_empty() {
        let mut pool: super::Xc149Pool<i32> = super::Xc149Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_149_pool_full() {
        let mut pool = super::Xc149Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_149_pool_drain() {
        let mut pool = super::Xc149Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_149_pool_stats() {
        let mut pool = super::Xc149Pool::new(8);
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
    fn xc_149_pool_clear() {
        let mut pool = super::Xc149Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_149_pool_shrink() {
        let mut pool = super::Xc149Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_149_pool_default() {
        let pool: super::Xc149Pool<String> = super::Xc149Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_149_pool_extend() {
        let mut pool = super::Xc149Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_149_pool_retain() {
        let mut pool = super::Xc149Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_149_scheduler_round_robin() {
        let mut sched = super::Xc149Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_149_scheduler_empty() {
        let mut sched = super::Xc149Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_149_scheduler_reset() {
        let mut sched = super::Xc149Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_149_scheduler_add_remove() {
        let mut sched = super::Xc149Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_149_scheduler_targets() {
        let sched = super::Xc149Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_149_hash_empty() {
        assert_eq!(super::xc_149_hash(b""), 5381);
    }

    #[test]
    fn xc_149_hash_data() {
        let h = super::xc_149_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_149_hash(b"hello"), h);
    }

    #[test]
    fn xc_149_reverse_str() {
        assert_eq!(super::xc_149_reverse("abc"), "cba");
        assert_eq!(super::xc_149_reverse(""), "");
    }


    // --- xd_70 deepening tests ---

    #[test]
    fn xd_70_sm_initial_state() {
        let sm = Xd70StateMachine::new();
        assert_eq!(sm.current_state(), Xd70State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_70_sm_valid_idle_to_running() {
        let mut sm = Xd70StateMachine::new();
        assert!(sm.transition(Xd70State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd70State::Running);
    }

    #[test]
    fn xd_70_sm_valid_running_to_paused() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        assert!(sm.transition(Xd70State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd70State::Paused);
    }

    #[test]
    fn xd_70_sm_valid_running_to_done() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        assert!(sm.transition(Xd70State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd70State::Done);
    }

    #[test]
    fn xd_70_sm_valid_paused_to_running() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        sm.transition(Xd70State::Paused).unwrap();
        assert!(sm.transition(Xd70State::Running).is_ok());
    }

    #[test]
    fn xd_70_sm_valid_done_to_idle() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        sm.transition(Xd70State::Done).unwrap();
        assert!(sm.transition(Xd70State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd70State::Idle);
    }

    #[test]
    fn xd_70_sm_invalid_idle_to_done() {
        let mut sm = Xd70StateMachine::new();
        assert!(sm.transition(Xd70State::Done).is_err());
    }

    #[test]
    fn xd_70_sm_invalid_idle_to_paused() {
        let mut sm = Xd70StateMachine::new();
        assert!(sm.transition(Xd70State::Paused).is_err());
    }

    #[test]
    fn xd_70_sm_history_tracking() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        sm.transition(Xd70State::Paused).unwrap();
        sm.transition(Xd70State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd70State::Idle);
        assert_eq!(sm.history()[0].to, Xd70State::Running);
        assert_eq!(sm.history()[1].from, Xd70State::Running);
        assert_eq!(sm.history()[2].to, Xd70State::Done);
    }

    #[test]
    fn xd_70_sm_serialize_deserialize() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd70StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd70State::Running));
    }

    #[test]
    fn xd_70_sm_deserialize_invalid() {
        assert_eq!(Xd70StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_70_sm_reset() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd70State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_70_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd70EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd70Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_70_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd70EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd70Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd70Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_70_bus_unsubscribe() {
        let mut bus = Xd70EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_70_event_kind_and_payload() {
        let e = Xd70Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd70Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_70_bus_clear_history() {
        let mut bus = Xd70EventBus::new();
        bus.publish(Xd70Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_70_sm_step_counter_increments() {
        let mut sm = Xd70StateMachine::new();
        sm.transition(Xd70State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd70State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #79 --

    #[test]
    fn xf79_trie_insert_search() {
        let mut t = Xf79Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf79_trie_starts_with() {
        let mut t = Xf79Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf79_trie_remove() {
        let mut t = Xf79Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf79_trie_word_count() {
        let mut t = Xf79Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf79_trie_longest_prefix() {
        let mut t = Xf79Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf79_trie_all_words() {
        let mut t = Xf79Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf79_trie_autocomplete() {
        let mut t = Xf79Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf79_trie_empty_search() {
        let t = Xf79Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf79_bloom_add_contains() {
        let mut bf = Xf79BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf79_bloom_probably_absent() {
        let bf = Xf79BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf79_bloom_false_positive_rate() {
        let mut bf = Xf79BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf79_bloom_clear() {
        let mut bf = Xf79BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf79_bloom_union() {
        let mut a = Xf79BloomFilter::xf_new(512, 2);
        let mut b = Xf79BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf79_bloom_intersection_estimate() {
        let mut a = Xf79BloomFilter::xf_new(512, 2);
        let mut b = Xf79BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf79_bloom_union_size_mismatch() {
        let a = Xf79BloomFilter::xf_new(256, 2);
        let b = Xf79BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh148_skip_insert_contains() {
        let mut sl = super::Xh148SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh148_skip_remove() {
        let mut sl = super::Xh148SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh148_skip_len() {
        let mut sl = super::Xh148SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh148_skip_range_query() {
        let mut sl = super::Xh148SkipList::xh_new(4);
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
    fn xh148_skip_floor_ceiling() {
        let mut sl = super::Xh148SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh148_skip_rank() {
        let mut sl = super::Xh148SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh148_skip_empty() {
        let sl = super::Xh148SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh148_skip_duplicates() {
        let mut sl = super::Xh148SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh148_bitset_set_test() {
        let mut bs = super::Xh148BitSet::xh_new(256);
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
    fn xh148_bitset_clear_count() {
        let mut bs = super::Xh148BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh148_bitset_and_or_xor() {
        let mut a = super::Xh148BitSet::xh_new(128);
        let mut b = super::Xh148BitSet::xh_new(128);
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
    fn xh148_bitset_iter_ones() {
        let mut bs = super::Xh148BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh148_bitset_first_last() {
        let mut bs = super::Xh148BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh148_bitset_empty() {
        let bs = super::Xh148BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi148_deque_push_pop_back() {
        let mut dq = super::Xi148Deque::xi_new(4);
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
    fn xi148_deque_push_pop_front() {
        let mut dq = super::Xi148Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi148_deque_mixed_ops() {
        let mut dq = super::Xi148Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi148_deque_get_and_split() {
        let mut dq = super::Xi148Deque::xi_new(8);
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
    fn xi148_deque_rotate_left() {
        let mut dq = super::Xi148Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi148_deque_rotate_right() {
        let mut dq = super::Xi148Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi148_deque_grow() {
        let mut dq = super::Xi148Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi148_deque_empty() {
        let dq = super::Xi148Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi148_interval_tree_insert_query() {
        let mut tree = super::Xi148IntervalTree::xi_new();
        tree.xi_insert(super::Xi148Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi148Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi148Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi148_interval_tree_overlap() {
        let mut tree = super::Xi148IntervalTree::xi_new();
        tree.xi_insert(super::Xi148Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi148Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi148Interval::xi_new(12, 20));
        let q = super::Xi148Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi148_interval_tree_remove() {
        let mut tree = super::Xi148IntervalTree::xi_new();
        tree.xi_insert(super::Xi148Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi148Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi148_interval_tree_gaps() {
        let mut tree = super::Xi148IntervalTree::xi_new();
        tree.xi_insert(super::Xi148Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi148Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi148Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi148Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi148Interval::xi_new(8, 10));
    }

    #[test]
    fn xi148_interval_tree_merge() {
        let mut tree = super::Xi148IntervalTree::xi_new();
        tree.xi_insert(super::Xi148Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi148Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi148Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi148Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi148Interval::xi_new(10, 15));
    }

    #[test]
    fn xi148_interval_tree_all() {
        let mut tree = super::Xi148IntervalTree::xi_new();
        tree.xi_insert(super::Xi148Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi148Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi148_interval_tree_empty() {
        let tree = super::Xi148IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi148_interval_tree_contains_point() {
        let iv = super::Xi148Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
