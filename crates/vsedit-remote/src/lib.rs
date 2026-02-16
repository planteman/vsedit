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
}
