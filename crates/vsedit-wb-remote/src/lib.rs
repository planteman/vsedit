//! Remote connection service.

use std::fmt;

/// Errors that can occur during remote operations.
#[derive(Debug, Clone, PartialEq)]
pub enum RemoteError {
    /// The authority string is empty or invalid.
    InvalidAuthority(String),
    /// Attempted an operation that requires a connection.
    NotConnected,
    /// Attempted to connect while already connected.
    AlreadyConnected,
    /// The connection timed out after the given number of seconds.
    Timeout(u64),
    /// A generic remote error with a message.
    Other(String),
}

impl fmt::Display for RemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteError::InvalidAuthority(a) => write!(f, "invalid authority: {a}"),
            RemoteError::NotConnected => write!(f, "not connected"),
            RemoteError::AlreadyConnected => write!(f, "already connected"),
            RemoteError::Timeout(secs) => write!(f, "connection timed out after {secs}s"),
            RemoteError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for RemoteError {}

#[derive(Debug, Clone, PartialEq)]
pub enum OsType {
    Linux,
    MacOS,
    Windows,
}

impl fmt::Display for OsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsType::Linux => write!(f, "Linux"),
            OsType::MacOS => write!(f, "macOS"),
            OsType::Windows => write!(f, "Windows"),
        }
    }
}

/// The current state of a remote connection.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Connecting => write!(f, "Connecting"),
            ConnectionState::Connected => write!(f, "Connected"),
            ConnectionState::Reconnecting => write!(f, "Reconnecting"),
            ConnectionState::Error(msg) => write!(f, "Error: {msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteEnvironment {
    pub os: OsType,
    pub arch: String,
    pub home_dir: String,
}

impl RemoteEnvironment {
    /// Human-readable name combining OS and architecture.
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.os, self.arch)
    }

    /// Returns the default shell path for the environment's OS.
    pub fn default_shell(&self) -> &'static str {
        match self.os {
            OsType::Linux => "/bin/bash",
            OsType::MacOS => "/bin/zsh",
            OsType::Windows => "cmd.exe",
        }
    }

    /// Returns the path separator for the environment's OS.
    pub fn path_separator(&self) -> char {
        match self.os {
            OsType::Windows => '\\',
            _ => '/',
        }
    }

    /// Joins the home directory with a relative path using the correct separator.
    pub fn resolve_home_path(&self, relative: &str) -> String {
        let sep = self.path_separator();
        let home = self.home_dir.trim_end_matches(sep);
        let rel = relative.trim_start_matches(sep).trim_start_matches('/');
        format!("{home}{sep}{rel}")
    }

    /// Returns `true` if the architecture is 64-bit.
    pub fn is_64bit(&self) -> bool {
        matches!(self.arch.as_str(), "x86_64" | "aarch64" | "arm64" | "x64")
    }
}

impl fmt::Display for RemoteEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) at {}", self.os, self.arch, self.home_dir)
    }
}

/// A record of a past connection attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionRecord {
    pub authority: String,
    pub timestamp: u64,
    pub success: bool,
}

impl fmt::Display for ConnectionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "OK" } else { "FAIL" };
        write!(f, "[{}] {} (t={})", status, self.authority, self.timestamp)
    }
}

/// Builder for constructing a [`RemoteWorkbenchService`] with validation.
pub struct RemoteWorkbenchBuilder {
    authority: Option<String>,
    max_history: usize,
}

impl RemoteWorkbenchBuilder {
    pub fn new() -> Self {
        Self {
            authority: None,
            max_history: 100,
        }
    }

    /// Set the initial authority (validated on build).
    pub fn authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = Some(authority.into());
        self
    }

    /// Set the maximum number of history records to retain.
    pub fn max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// Build the service, returning an error if the authority is invalid.
    pub fn build(self) -> Result<RemoteWorkbenchService, RemoteError> {
        if let Some(ref auth) = self.authority {
            validate_authority(auth)?;
        }
        Ok(RemoteWorkbenchService {
            authority: self.authority,
            environment: None,
            state: ConnectionState::Disconnected,
            connection_history: Vec::new(),
            max_history: self.max_history,
        })
    }
}

impl Default for RemoteWorkbenchBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates that an authority string is non-empty and contains a `+` separator.
fn validate_authority(authority: &str) -> Result<(), RemoteError> {
    if authority.is_empty() {
        return Err(RemoteError::InvalidAuthority(
            "authority must not be empty".into(),
        ));
    }
    if !authority.contains('+') {
        return Err(RemoteError::InvalidAuthority(format!(
            "authority must contain '+' separator: {authority}"
        )));
    }
    Ok(())
}

/// Service for remote workbench functionality.
pub struct RemoteWorkbenchService {
    authority: Option<String>,
    environment: Option<RemoteEnvironment>,
    state: ConnectionState,
    connection_history: Vec<ConnectionRecord>,
    max_history: usize,
}

impl RemoteWorkbenchService {
    pub fn new() -> Self {
        Self {
            authority: None,
            environment: None,
            state: ConnectionState::Disconnected,
            connection_history: Vec::new(),
            max_history: 100,
        }
    }

    /// Create a builder for more controlled construction.
    pub fn builder() -> RemoteWorkbenchBuilder {
        RemoteWorkbenchBuilder::new()
    }

    /// Set the authority, validating the format.
    pub fn set_authority(&mut self, authority: String) {
        self.authority = Some(authority);
    }

    /// Set the authority with validation, returning an error for invalid values.
    pub fn set_authority_checked(&mut self, authority: String) -> Result<(), RemoteError> {
        validate_authority(&authority)?;
        self.authority = Some(authority);
        Ok(())
    }

    pub fn get_authority(&self) -> Option<&str> {
        self.authority.as_deref()
    }

    /// Extracts the scheme portion (before `+`) from the authority.
    pub fn authority_scheme(&self) -> Option<&str> {
        self.authority.as_deref().and_then(|a| a.split('+').next())
    }

    /// Extracts the host portion (after `+`) from the authority.
    pub fn authority_host(&self) -> Option<&str> {
        self.authority.as_deref().and_then(|a| a.split('+').nth(1))
    }

    pub fn connect(&mut self, env: RemoteEnvironment) {
        self.environment = Some(env);
        self.state = ConnectionState::Connected;
        if let Some(auth) = &self.authority {
            self.connection_history.push(ConnectionRecord {
                authority: auth.clone(),
                timestamp: 0,
                success: true,
            });
            self.trim_history();
        }
    }

    /// Attempt a checked connection: fails if already connected or no authority set.
    pub fn try_connect(&mut self, env: RemoteEnvironment) -> Result<(), RemoteError> {
        if self.is_connected() {
            return Err(RemoteError::AlreadyConnected);
        }
        if self.authority.is_none() {
            return Err(RemoteError::InvalidAuthority(
                "no authority set".into(),
            ));
        }
        self.connect(env);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.environment = None;
        self.state = ConnectionState::Disconnected;
    }

    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Returns `true` if the service is in an error state.
    pub fn has_error(&self) -> bool {
        matches!(self.state, ConnectionState::Error(_))
    }

    /// Returns the error message if the service is in an error state.
    pub fn error_message(&self) -> Option<&str> {
        match &self.state {
            ConnectionState::Error(msg) => Some(msg.as_str()),
            _ => None,
        }
    }

    pub fn get_environment(&self) -> Option<&RemoteEnvironment> {
        self.environment.as_ref()
    }

    pub fn get_state(&self) -> &ConnectionState {
        &self.state
    }

    /// Transition to the `Reconnecting` state.
    pub fn reconnect(&mut self) {
        self.state = ConnectionState::Reconnecting;
    }

    /// Transition to the `Error` state with the given message.
    pub fn set_error(&mut self, message: String) {
        self.state = ConnectionState::Error(message);
    }

    pub fn get_history(&self) -> &[ConnectionRecord] {
        &self.connection_history
    }

    /// Returns the number of successful connections in the history.
    pub fn successful_connection_count(&self) -> usize {
        self.connection_history.iter().filter(|r| r.success).count()
    }

    /// Returns the number of failed connections in the history.
    pub fn failed_connection_count(&self) -> usize {
        self.connection_history.iter().filter(|r| !r.success).count()
    }

    /// Records a failed connection attempt for the current authority.
    pub fn record_failure(&mut self, timestamp: u64) {
        if let Some(auth) = &self.authority {
            self.connection_history.push(ConnectionRecord {
                authority: auth.clone(),
                timestamp,
                success: false,
            });
            self.trim_history();
        }
    }

    /// Clears connection history.
    pub fn clear_history(&mut self) {
        self.connection_history.clear();
    }

    /// Keeps history within the configured maximum.
    fn trim_history(&mut self) {
        if self.connection_history.len() > self.max_history {
            let excess = self.connection_history.len() - self.max_history;
            self.connection_history.drain(..excess);
        }
    }
}

impl Default for RemoteWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RemoteWorkbenchService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let auth = self.authority.as_deref().unwrap_or("<none>");
        write!(f, "RemoteWorkbenchService(authority={auth}, state={}", self.state)?;
        if let Some(env) = &self.environment {
            write!(f, ", env={env}")?;
        }
        write!(f, ")")
    }
}

// ---------------------------------------------------------------------------
// Connection health monitoring
// ---------------------------------------------------------------------------

/// Health check result for a remote connection.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthCheckResult {
    pub latency_ms: u64,
    pub is_healthy: bool,
    pub message: String,
}

impl fmt::Display for HealthCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_healthy { "healthy" } else { "unhealthy" };
        write!(f, "{} ({}ms): {}", status, self.latency_ms, self.message)
    }
}

/// Monitors connection health over time by tracking check results.
pub struct ConnectionHealthMonitor {
    results: Vec<HealthCheckResult>,
    max_results: usize,
}

impl ConnectionHealthMonitor {
    pub fn new(max_results: usize) -> Self {
        Self { results: Vec::new(), max_results }
    }

    pub fn record(&mut self, result: HealthCheckResult) {
        self.results.push(result);
        if self.results.len() > self.max_results {
            self.results.remove(0);
        }
    }

    pub fn average_latency(&self) -> Option<u64> {
        if self.results.is_empty() {
            return None;
        }
        let sum: u64 = self.results.iter().map(|r| r.latency_ms).sum();
        Some(sum / self.results.len() as u64)
    }

    pub fn failure_rate(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let failures = self.results.iter().filter(|r| !r.is_healthy).count();
        failures as f64 / self.results.len() as f64
    }

    pub fn latest(&self) -> Option<&HealthCheckResult> {
        self.results.last()
    }

    pub fn result_count(&self) -> usize {
        self.results.len()
    }
}

// ---------------------------------------------------------------------------
// Connection retry policy
// ---------------------------------------------------------------------------

/// Configuration for connection retry behaviour.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_factor: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Compute the delay in milliseconds for the given attempt number (0-based).
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let delay = self.initial_delay_ms as f64 * self.backoff_factor.powi(attempt as i32);
        (delay as u64).min(self.max_delay_ms)
    }

    /// Returns `true` if another retry is allowed at the given attempt number.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

// ---------------------------------------------------------------------------
// Bandwidth estimation
// ---------------------------------------------------------------------------

/// Simple bandwidth estimator based on transfer samples.
pub struct BandwidthEstimator {
    samples: Vec<(u64, u64)>, // (bytes, duration_ms)
    max_samples: usize,
}

impl BandwidthEstimator {
    pub fn new(max_samples: usize) -> Self {
        Self { samples: Vec::new(), max_samples }
    }

    pub fn record_transfer(&mut self, bytes: u64, duration_ms: u64) {
        if duration_ms == 0 {
            return;
        }
        self.samples.push((bytes, duration_ms));
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    /// Returns estimated bytes per second, or `None` if no samples exist.
    pub fn estimated_bps(&self) -> Option<u64> {
        if self.samples.is_empty() {
            return None;
        }
        let total_bytes: u64 = self.samples.iter().map(|(b, _)| *b).sum();
        let total_ms: u64 = self.samples.iter().map(|(_, d)| *d).sum();
        if total_ms == 0 {
            return None;
        }
        Some(total_bytes * 1000 / total_ms)
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

// ---------------------------------------------------------------------------
// Connection pool management
// ---------------------------------------------------------------------------

/// A pool of named remote connections with their states.
pub struct ConnectionPool {
    connections: Vec<(String, ConnectionState)>,
    max_connections: usize,
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self { connections: Vec::new(), max_connections }
    }

    pub fn add(&mut self, name: String) -> Result<(), RemoteError> {
        if self.connections.len() >= self.max_connections {
            return Err(RemoteError::Other("connection pool full".into()));
        }
        self.connections.push((name, ConnectionState::Disconnected));
        Ok(())
    }

    pub fn connect(&mut self, name: &str) -> Result<(), RemoteError> {
        for (n, state) in &mut self.connections {
            if n == name {
                *state = ConnectionState::Connected;
                return Ok(());
            }
        }
        Err(RemoteError::NotConnected)
    }

    pub fn disconnect(&mut self, name: &str) {
        for (n, state) in &mut self.connections {
            if n == name {
                *state = ConnectionState::Disconnected;
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.connections.iter().filter(|(_, s)| *s == ConnectionState::Connected).count()
    }

    pub fn total_count(&self) -> usize {
        self.connections.len()
    }

    pub fn get_state(&self, name: &str) -> Option<&ConnectionState> {
        self.connections.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }
}

/// Parsed remote authority with scheme and host components.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAuthority {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
    pub user: Option<String>,
}

impl RemoteAuthority {
    /// Parse an authority string like "ssh+remote+myhost" or "wsl+Ubuntu".
    /// Format: scheme+host or scheme+host+port or user@scheme+host
    pub fn parse(authority: &str) -> Result<Self, RemoteError> {
        if authority.is_empty() {
            return Err(RemoteError::InvalidAuthority("empty authority".into()));
        }
        let (user, rest) = if let Some(at_pos) = authority.find('@') {
            (Some(authority[..at_pos].to_string()), &authority[at_pos+1..])
        } else {
            (None, authority)
        };

        let parts: Vec<&str> = rest.splitn(2, '+').collect();
        if parts.len() < 2 {
            return Err(RemoteError::InvalidAuthority(
                format!("authority must contain '+' separator: {authority}")
            ));
        }

        let scheme = parts[0].to_string();
        let host_part = parts[1].to_string();

        // Check for port in host (host:port format)
        let (host, port) = if let Some(colon_pos) = host_part.rfind(':') {
            if let Ok(p) = host_part[colon_pos+1..].parse::<u16>() {
                (host_part[..colon_pos].to_string(), Some(p))
            } else {
                (host_part, None)
            }
        } else {
            (host_part, None)
        };

        Ok(Self { scheme, host, port, user })
    }

    /// Check if this is an SSH remote.
    pub fn is_ssh(&self) -> bool {
        self.scheme == "ssh" || self.scheme == "ssh-remote"
    }

    /// Check if this is a WSL remote.
    pub fn is_wsl(&self) -> bool {
        self.scheme == "wsl"
    }

    /// Check if this is a dev container remote.
    pub fn is_dev_container(&self) -> bool {
        self.scheme == "dev-container" || self.scheme == "attached-container"
    }

    /// Reconstruct the authority string.
    pub fn to_authority_string(&self) -> String {
        let mut s = String::new();
        if let Some(ref user) = self.user {
            s.push_str(user);
            s.push('@');
        }
        s.push_str(&self.scheme);
        s.push('+');
        s.push_str(&self.host);
        if let Some(port) = self.port {
            s.push(':');
            s.push_str(&port.to_string());
        }
        s
    }
}

impl fmt::Display for RemoteAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_authority_string())
    }
}

/// Generate a human-readable label for a remote connection.
pub fn remote_label(authority: &RemoteAuthority) -> String {
    match authority.scheme.as_str() {
        "ssh" | "ssh-remote" => {
            let user_prefix = authority.user.as_ref()
                .map(|u| format!("{u}@"))
                .unwrap_or_default();
            let port_suffix = authority.port
                .map(|p| format!(":{p}"))
                .unwrap_or_default();
            format!("SSH: {user_prefix}{host}{port_suffix}", host = authority.host)
        }
        "wsl" => format!("WSL: {}", authority.host),
        "dev-container" | "attached-container" => format!("Dev Container: {}", authority.host),
        "tunnel" => format!("Tunnel: {}", authority.host),
        other => format!("{}: {}", other, authority.host),
    }
}

/// Generate a short label suitable for the status bar.
pub fn remote_label_short(authority: &RemoteAuthority) -> String {
    match authority.scheme.as_str() {
        "ssh" | "ssh-remote" => authority.host.clone(),
        "wsl" => format!("WSL: {}", authority.host),
        "dev-container" | "attached-container" => "Container".to_string(),
        "tunnel" => "Tunnel".to_string(),
        _ => authority.host.clone(),
    }
}

/// Represents the remote indicator shown in the status bar.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteIndicator {
    pub label: String,
    pub tooltip: String,
    pub icon: RemoteIcon,
    pub is_connected: bool,
}

/// Icon to display for the remote indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteIcon {
    Cloud,
    Terminal,
    Container,
    Globe,
    Disconnected,
}

impl fmt::Display for RemoteIcon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteIcon::Cloud => write!(f, "☁"),
            RemoteIcon::Terminal => write!(f, ">_"),
            RemoteIcon::Container => write!(f, "⬡"),
            RemoteIcon::Globe => write!(f, "🌐"),
            RemoteIcon::Disconnected => write!(f, "⊘"),
        }
    }
}

impl RemoteIndicator {
    /// Create a remote indicator from a parsed authority and connection state.
    pub fn from_authority(authority: &RemoteAuthority, connected: bool) -> Self {
        let label = if connected {
            remote_label_short(authority)
        } else {
            "Disconnected".to_string()
        };

        let tooltip = if connected {
            remote_label(authority)
        } else {
            format!("Disconnected from {}", remote_label(authority))
        };

        let icon = if !connected {
            RemoteIcon::Disconnected
        } else if authority.is_ssh() {
            RemoteIcon::Terminal
        } else if authority.is_wsl() {
            RemoteIcon::Cloud
        } else if authority.is_dev_container() {
            RemoteIcon::Container
        } else {
            RemoteIcon::Globe
        };

        Self { label, tooltip, icon, is_connected: connected }
    }

    /// Render the indicator as a status bar string: "icon label"
    pub fn render(&self) -> String {
        format!("{} {}", self.icon, self.label)
    }
}

impl fmt::Display for RemoteIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_and_disconnect() {
        let mut svc = RemoteWorkbenchService::new();
        assert!(!svc.is_connected());
        let env = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        svc.connect(env);
        assert!(svc.is_connected());
        assert_eq!(svc.get_environment().unwrap().os, OsType::Linux);
        svc.disconnect();
        assert!(!svc.is_connected());
        assert!(svc.get_environment().is_none());
    }

    #[test]
    fn authority_management() {
        let mut svc = RemoteWorkbenchService::new();
        assert!(svc.get_authority().is_none());
        svc.set_authority("ssh-remote+myhost".into());
        assert_eq!(svc.get_authority(), Some("ssh-remote+myhost"));
    }

    #[test]
    fn environment_details() {
        let mut svc = RemoteWorkbenchService::new();
        let env = RemoteEnvironment {
            os: OsType::MacOS,
            arch: "aarch64".into(),
            home_dir: "/Users/dev".into(),
        };
        svc.connect(env);
        let e = svc.get_environment().unwrap();
        assert_eq!(e.arch, "aarch64");
        assert_eq!(e.home_dir, "/Users/dev");
    }

    #[test]
    fn connection_state_transitions() {
        let mut svc = RemoteWorkbenchService::new();
        assert_eq!(*svc.get_state(), ConnectionState::Disconnected);

        let env = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        svc.connect(env);
        assert_eq!(*svc.get_state(), ConnectionState::Connected);

        svc.reconnect();
        assert_eq!(*svc.get_state(), ConnectionState::Reconnecting);
        assert!(!svc.is_connected());

        svc.set_error("timeout".into());
        assert_eq!(
            *svc.get_state(),
            ConnectionState::Error("timeout".into())
        );

        svc.disconnect();
        assert_eq!(*svc.get_state(), ConnectionState::Disconnected);
    }

    #[test]
    fn connection_state_display() {
        assert_eq!(ConnectionState::Disconnected.to_string(), "Disconnected");
        assert_eq!(ConnectionState::Connecting.to_string(), "Connecting");
        assert_eq!(ConnectionState::Connected.to_string(), "Connected");
        assert_eq!(ConnectionState::Reconnecting.to_string(), "Reconnecting");
        assert_eq!(
            ConnectionState::Error("fail".into()).to_string(),
            "Error: fail"
        );
    }

    #[test]
    fn os_type_display() {
        assert_eq!(OsType::Linux.to_string(), "Linux");
        assert_eq!(OsType::MacOS.to_string(), "macOS");
        assert_eq!(OsType::Windows.to_string(), "Windows");
    }

    #[test]
    fn remote_environment_display_name() {
        let env = RemoteEnvironment {
            os: OsType::Windows,
            arch: "x86_64".into(),
            home_dir: "C:\\Users\\dev".into(),
        };
        assert_eq!(env.display_name(), "Windows (x86_64)");
    }

    #[test]
    fn connection_history_tracking() {
        let mut svc = RemoteWorkbenchService::new();
        svc.set_authority("ssh-remote+host1".into());
        let env = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        svc.connect(env);

        let history = svc.get_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].authority, "ssh-remote+host1");
        assert!(history[0].success);

        svc.disconnect();
        svc.set_authority("ssh-remote+host2".into());
        let env2 = RemoteEnvironment {
            os: OsType::MacOS,
            arch: "aarch64".into(),
            home_dir: "/Users/dev".into(),
        };
        svc.connect(env2);

        let history = svc.get_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].authority, "ssh-remote+host2");
    }

    #[test]
    fn remote_error_display() {
        assert_eq!(
            RemoteError::InvalidAuthority("bad".into()).to_string(),
            "invalid authority: bad"
        );
        assert_eq!(RemoteError::NotConnected.to_string(), "not connected");
        assert_eq!(
            RemoteError::AlreadyConnected.to_string(),
            "already connected"
        );
        assert_eq!(
            RemoteError::Timeout(30).to_string(),
            "connection timed out after 30s"
        );
        assert_eq!(
            RemoteError::Other("boom".into()).to_string(),
            "boom"
        );
    }

    #[test]
    fn validate_authority_format() {
        let mut svc = RemoteWorkbenchService::new();
        assert!(svc.set_authority_checked("ssh-remote+host".into()).is_ok());
        assert_eq!(svc.get_authority(), Some("ssh-remote+host"));

        let mut svc2 = RemoteWorkbenchService::new();
        assert!(svc2.set_authority_checked("".into()).is_err());
        assert!(svc2.set_authority_checked("no-plus".into()).is_err());
    }

    #[test]
    fn authority_scheme_and_host() {
        let mut svc = RemoteWorkbenchService::new();
        assert!(svc.authority_scheme().is_none());
        assert!(svc.authority_host().is_none());

        svc.set_authority("ssh-remote+myserver".into());
        assert_eq!(svc.authority_scheme(), Some("ssh-remote"));
        assert_eq!(svc.authority_host(), Some("myserver"));
    }

    #[test]
    fn try_connect_errors() {
        let mut svc = RemoteWorkbenchService::new();
        let env = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        // No authority set
        assert_eq!(
            svc.try_connect(env.clone()),
            Err(RemoteError::InvalidAuthority("no authority set".into()))
        );

        svc.set_authority("ssh-remote+host".into());
        assert!(svc.try_connect(env.clone()).is_ok());

        // Already connected
        assert_eq!(svc.try_connect(env), Err(RemoteError::AlreadyConnected));
    }

    #[test]
    fn builder_pattern() {
        let svc = RemoteWorkbenchService::builder()
            .authority("ssh-remote+host")
            .max_history(5)
            .build()
            .unwrap();
        assert_eq!(svc.get_authority(), Some("ssh-remote+host"));

        let result = RemoteWorkbenchService::builder()
            .authority("invalid")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn environment_default_shell() {
        let linux = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        assert_eq!(linux.default_shell(), "/bin/bash");

        let mac = RemoteEnvironment {
            os: OsType::MacOS,
            arch: "aarch64".into(),
            home_dir: "/Users/dev".into(),
        };
        assert_eq!(mac.default_shell(), "/bin/zsh");

        let win = RemoteEnvironment {
            os: OsType::Windows,
            arch: "x86_64".into(),
            home_dir: "C:\\Users\\dev".into(),
        };
        assert_eq!(win.default_shell(), "cmd.exe");
    }

    #[test]
    fn environment_resolve_home_path() {
        let linux = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        assert_eq!(linux.resolve_home_path(".config/app"), "/home/user/.config/app");

        let win = RemoteEnvironment {
            os: OsType::Windows,
            arch: "x86_64".into(),
            home_dir: "C:\\Users\\dev".into(),
        };
        assert_eq!(win.resolve_home_path("Documents"), "C:\\Users\\dev\\Documents");
    }

    #[test]
    fn environment_is_64bit() {
        let env64 = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        assert!(env64.is_64bit());

        let env32 = RemoteEnvironment {
            os: OsType::Linux,
            arch: "armv7l".into(),
            home_dir: "/home/user".into(),
        };
        assert!(!env32.is_64bit());
    }

    #[test]
    fn record_failure_and_counts() {
        let mut svc = RemoteWorkbenchService::new();
        svc.set_authority("ssh-remote+host".into());

        let env = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        svc.connect(env);
        svc.record_failure(100);

        assert_eq!(svc.successful_connection_count(), 1);
        assert_eq!(svc.failed_connection_count(), 1);
        assert_eq!(svc.get_history().len(), 2);
    }

    #[test]
    fn has_error_and_error_message() {
        let mut svc = RemoteWorkbenchService::new();
        assert!(!svc.has_error());
        assert!(svc.error_message().is_none());

        svc.set_error("connection refused".into());
        assert!(svc.has_error());
        assert_eq!(svc.error_message(), Some("connection refused"));

        svc.disconnect();
        assert!(!svc.has_error());
    }

    #[test]
    fn connection_record_display() {
        let rec = ConnectionRecord {
            authority: "ssh-remote+host".into(),
            timestamp: 42,
            success: true,
        };
        assert_eq!(rec.to_string(), "[OK] ssh-remote+host (t=42)");

        let fail = ConnectionRecord {
            authority: "ssh-remote+host".into(),
            timestamp: 99,
            success: false,
        };
        assert_eq!(fail.to_string(), "[FAIL] ssh-remote+host (t=99)");
    }

    #[test]
    fn service_display() {
        let mut svc = RemoteWorkbenchService::new();
        let display = svc.to_string();
        assert!(display.contains("<none>"));
        assert!(display.contains("Disconnected"));

        svc.set_authority("ssh-remote+host".into());
        let env = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        svc.connect(env);
        let display = svc.to_string();
        assert!(display.contains("ssh-remote+host"));
        assert!(display.contains("Connected"));
        assert!(display.contains("Linux"));
    }

    #[test]
    fn history_trimming() {
        let mut svc = RemoteWorkbenchService::builder()
            .authority("ssh-remote+host")
            .max_history(3)
            .build()
            .unwrap();

        for _ in 0..5 {
            let env = RemoteEnvironment {
                os: OsType::Linux,
                arch: "x86_64".into(),
                home_dir: "/home/user".into(),
            };
            svc.connect(env);
            svc.disconnect();
        }
        assert_eq!(svc.get_history().len(), 3);
    }

    #[test]
    fn clear_history() {
        let mut svc = RemoteWorkbenchService::new();
        svc.set_authority("ssh-remote+host".into());
        let env = RemoteEnvironment {
            os: OsType::Linux,
            arch: "x86_64".into(),
            home_dir: "/home/user".into(),
        };
        svc.connect(env);
        assert!(!svc.get_history().is_empty());
        svc.clear_history();
        assert!(svc.get_history().is_empty());
    }

    #[test]
    fn health_monitor_average_and_failure_rate() {
        let mut monitor = ConnectionHealthMonitor::new(10);
        assert!(monitor.average_latency().is_none());
        assert_eq!(monitor.failure_rate(), 0.0);
        monitor.record(HealthCheckResult { latency_ms: 100, is_healthy: true, message: "ok".into() });
        monitor.record(HealthCheckResult { latency_ms: 200, is_healthy: false, message: "timeout".into() });
        assert_eq!(monitor.average_latency(), Some(150));
        assert!((monitor.failure_rate() - 0.5).abs() < f64::EPSILON);
        assert_eq!(monitor.result_count(), 2);
        assert!(!monitor.latest().unwrap().is_healthy);
    }

    #[test]
    fn health_check_result_display() {
        let r = HealthCheckResult { latency_ms: 50, is_healthy: true, message: "ok".into() };
        let s = r.to_string();
        assert!(s.contains("healthy"));
        assert!(s.contains("50ms"));
    }

    #[test]
    fn retry_policy_delays() {
        let policy = RetryPolicy::default();
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        assert_eq!(policy.delay_for_attempt(0), 1000);
        assert_eq!(policy.delay_for_attempt(1), 2000);
        // Capped at max_delay_ms
        let big = policy.delay_for_attempt(100);
        assert!(big <= policy.max_delay_ms);
    }

    #[test]
    fn bandwidth_estimator_basic() {
        let mut est = BandwidthEstimator::new(5);
        assert!(est.estimated_bps().is_none());
        est.record_transfer(1000, 100); // 10,000 bytes/sec
        assert_eq!(est.estimated_bps(), Some(10000));
        est.record_transfer(0, 0); // ignored
        assert_eq!(est.sample_count(), 1);
        est.record_transfer(2000, 200);
        assert_eq!(est.estimated_bps(), Some(10000));
    }

    #[test]
    fn connection_pool_lifecycle() {
        let mut pool = ConnectionPool::new(2);
        assert!(pool.add("host1".into()).is_ok());
        assert!(pool.add("host2".into()).is_ok());
        assert!(pool.add("host3".into()).is_err());
        assert_eq!(pool.total_count(), 2);
        assert_eq!(pool.active_count(), 0);
        assert!(pool.connect("host1").is_ok());
        assert_eq!(pool.active_count(), 1);
        assert_eq!(*pool.get_state("host1").unwrap(), ConnectionState::Connected);
        pool.disconnect("host1");
        assert_eq!(pool.active_count(), 0);
        assert!(pool.connect("nonexistent").is_err());
    }

    #[test]
    fn remote_authority_parse_ssh() {
        let auth = RemoteAuthority::parse("ssh+myserver").unwrap();
        assert_eq!(auth.scheme, "ssh");
        assert_eq!(auth.host, "myserver");
        assert!(auth.is_ssh());
        assert!(!auth.is_wsl());
    }

    #[test]
    fn remote_authority_parse_wsl() {
        let auth = RemoteAuthority::parse("wsl+Ubuntu-22.04").unwrap();
        assert_eq!(auth.scheme, "wsl");
        assert_eq!(auth.host, "Ubuntu-22.04");
        assert!(auth.is_wsl());
    }

    #[test]
    fn remote_authority_parse_with_user() {
        let auth = RemoteAuthority::parse("root@ssh+server").unwrap();
        assert_eq!(auth.user, Some("root".into()));
        assert_eq!(auth.scheme, "ssh");
        assert_eq!(auth.host, "server");
    }

    #[test]
    fn remote_authority_parse_with_port() {
        let auth = RemoteAuthority::parse("ssh+myhost:2222").unwrap();
        assert_eq!(auth.host, "myhost");
        assert_eq!(auth.port, Some(2222));
    }

    #[test]
    fn remote_authority_parse_invalid() {
        assert!(RemoteAuthority::parse("").is_err());
        assert!(RemoteAuthority::parse("no-plus-sign").is_err());
    }

    #[test]
    fn remote_authority_to_string_roundtrip() {
        let auth = RemoteAuthority::parse("ssh+myserver").unwrap();
        assert_eq!(auth.to_authority_string(), "ssh+myserver");
    }

    #[test]
    fn remote_label_ssh() {
        let auth = RemoteAuthority::parse("ssh+prod-server").unwrap();
        let label = remote_label(&auth);
        assert!(label.contains("SSH"));
        assert!(label.contains("prod-server"));
    }

    #[test]
    fn remote_label_wsl() {
        let auth = RemoteAuthority::parse("wsl+Ubuntu").unwrap();
        assert_eq!(remote_label(&auth), "WSL: Ubuntu");
    }

    #[test]
    fn remote_label_short_ssh() {
        let auth = RemoteAuthority::parse("ssh+myserver").unwrap();
        assert_eq!(remote_label_short(&auth), "myserver");
    }

    #[test]
    fn remote_indicator_connected_ssh() {
        let auth = RemoteAuthority::parse("ssh+prod").unwrap();
        let ind = RemoteIndicator::from_authority(&auth, true);
        assert!(ind.is_connected);
        assert_eq!(ind.icon, RemoteIcon::Terminal);
        assert_eq!(ind.label, "prod");
    }

    #[test]
    fn remote_indicator_disconnected() {
        let auth = RemoteAuthority::parse("ssh+prod").unwrap();
        let ind = RemoteIndicator::from_authority(&auth, false);
        assert!(!ind.is_connected);
        assert_eq!(ind.icon, RemoteIcon::Disconnected);
        assert_eq!(ind.label, "Disconnected");
    }

    #[test]
    fn remote_indicator_render() {
        let auth = RemoteAuthority::parse("wsl+Ubuntu").unwrap();
        let ind = RemoteIndicator::from_authority(&auth, true);
        let rendered = ind.render();
        assert!(rendered.contains("WSL"));
    }

    #[test]
    fn remote_icon_display() {
        assert!(!format!("{}", RemoteIcon::Cloud).is_empty());
        assert!(!format!("{}", RemoteIcon::Disconnected).is_empty());
    }

    #[test]
    fn remote_authority_dev_container() {
        let auth = RemoteAuthority::parse("dev-container+abc123").unwrap();
        assert!(auth.is_dev_container());
        let label = remote_label(&auth);
        assert!(label.contains("Dev Container"));
    }
}
