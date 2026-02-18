//! Remote connection service.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// RemoteConnectionPool — managing multiple connections
// ---------------------------------------------------------------------------

/// A managed connection with health monitoring and retry state.
#[derive(Debug, Clone)]
pub struct ManagedConnection {
    pub name: String,
    pub authority: RemoteAuthority,
    pub state: ConnectionState,
    pub retry_count: u32,
    pub last_health_check_ms: Option<u64>,
    pub healthy: bool,
}

impl ManagedConnection {
    pub fn new(name: impl Into<String>, authority: RemoteAuthority) -> Self {
        Self {
            name: name.into(),
            authority,
            state: ConnectionState::Disconnected,
            retry_count: 0,
            last_health_check_ms: None,
            healthy: false,
        }
    }

    /// Whether this connection is currently connected.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Whether this connection needs reconnection.
    pub fn needs_reconnect(&self) -> bool {
        matches!(self.state, ConnectionState::Error(_) | ConnectionState::Disconnected)
            && self.healthy
    }

    /// Record a health check result.
    pub fn record_health(&mut self, latency_ms: u64, is_healthy: bool) {
        self.last_health_check_ms = Some(latency_ms);
        self.healthy = is_healthy;
    }
}

impl fmt::Display for ManagedConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ManagedConnection({}, {}, state={})",
            self.name, self.authority, self.state
        )
    }
}

/// Pool of managed remote connections.
pub struct RemoteConnectionPool {
    connections: Vec<ManagedConnection>,
    max_connections: usize,
    retry_policy: RetryPolicy,
}

impl RemoteConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self {
            connections: Vec::new(),
            max_connections,
            retry_policy: RetryPolicy::default(),
        }
    }

    /// Create with a custom retry policy.
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Add a new connection to the pool.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        authority: RemoteAuthority,
    ) -> Result<(), RemoteError> {
        if self.connections.len() >= self.max_connections {
            return Err(RemoteError::Other("connection pool full".into()));
        }
        let name = name.into();
        if self.connections.iter().any(|c| c.name == name) {
            return Err(RemoteError::Other(format!("connection '{}' already exists", name)));
        }
        self.connections.push(ManagedConnection::new(name, authority));
        Ok(())
    }

    /// Remove a connection from the pool by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.connections.len();
        self.connections.retain(|c| c.name != name);
        self.connections.len() < before
    }

    /// Get a connection by name.
    pub fn get(&self, name: &str) -> Option<&ManagedConnection> {
        self.connections.iter().find(|c| c.name == name)
    }

    /// Get a mutable connection by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ManagedConnection> {
        self.connections.iter_mut().find(|c| c.name == name)
    }

    /// Connect a named connection.
    pub fn connect(&mut self, name: &str) -> Result<(), RemoteError> {
        let conn = self.connections.iter_mut().find(|c| c.name == name)
            .ok_or(RemoteError::NotConnected)?;
        conn.state = ConnectionState::Connected;
        conn.retry_count = 0;
        Ok(())
    }

    /// Disconnect a named connection.
    pub fn disconnect(&mut self, name: &str) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.name == name) {
            conn.state = ConnectionState::Disconnected;
        }
    }

    /// Set a connection to error state and increment retry counter.
    pub fn set_error(&mut self, name: &str, message: String) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.name == name) {
            conn.state = ConnectionState::Error(message);
            conn.retry_count += 1;
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

    /// Number of currently connected connections.
    pub fn connected_count(&self) -> usize {
        self.connections.iter().filter(|c| c.is_connected()).count()
    }

    /// Number of connections in error state.
    pub fn error_count(&self) -> usize {
        self.connections.iter().filter(|c| matches!(c.state, ConnectionState::Error(_))).count()
    }

    /// All connections.
    pub fn connections(&self) -> &[ManagedConnection] {
        &self.connections
    }

    /// Check health for all connections, updating their status.
    pub fn check_health(&mut self, results: &[(String, u64, bool)]) {
        for (name, latency, healthy) in results {
            if let Some(conn) = self.connections.iter_mut().find(|c| &c.name == name) {
                conn.record_health(*latency, *healthy);
            }
        }
    }

    /// Get the list of connections that need reconnection and are within retry limits.
    pub fn connections_needing_reconnect(&self) -> Vec<&ManagedConnection> {
        self.connections.iter().filter(|c| {
            c.needs_reconnect() && self.retry_policy.should_retry(c.retry_count)
        }).collect()
    }

    /// Compute the retry delay for a connection based on its retry count.
    pub fn retry_delay_for(&self, name: &str) -> Option<u64> {
        self.connections.iter().find(|c| c.name == name).map(|c| {
            self.retry_policy.delay_for_attempt(c.retry_count)
        })
    }

    /// Disconnect all connections.
    pub fn disconnect_all(&mut self) {
        for conn in &mut self.connections {
            conn.state = ConnectionState::Disconnected;
        }
    }

    /// Get a summary of pool status.
    pub fn summary(&self) -> ConnectionPoolSummary {
        ConnectionPoolSummary {
            total: self.connections.len(),
            connected: self.connected_count(),
            error: self.error_count(),
            disconnected: self.connections.iter()
                .filter(|c| c.state == ConnectionState::Disconnected)
                .count(),
        }
    }
}

/// Summary of connection pool status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPoolSummary {
    pub total: usize,
    pub connected: usize,
    pub error: usize,
    pub disconnected: usize,
}

impl fmt::Display for ConnectionPoolSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Pool(total={}, connected={}, error={}, disconnected={})",
            self.total, self.connected, self.error, self.disconnected
        )
    }
}

// ---------------------------------------------------------------------------
// Reconnection strategy with exponential backoff
// ---------------------------------------------------------------------------

/// Tracks reconnection attempts and computes backoff delays.
#[derive(Debug, Clone)]
pub struct ReconnectionStrategy {
    pub policy: RetryPolicy,
    pub current_attempt: u32,
    pub total_attempts: u32,
    pub last_delay_ms: u64,
}

impl ReconnectionStrategy {
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            current_attempt: 0,
            total_attempts: 0,
            last_delay_ms: 0,
        }
    }

    /// Record a failed attempt and compute the next delay.
    /// Returns Some(delay_ms) if another retry is allowed, None if exhausted.
    pub fn next_delay(&mut self) -> Option<u64> {
        if !self.policy.should_retry(self.current_attempt) {
            return None;
        }
        let delay = self.policy.delay_for_attempt(self.current_attempt);
        self.current_attempt += 1;
        self.total_attempts += 1;
        self.last_delay_ms = delay;
        Some(delay)
    }

    /// Record a successful connection (resets the attempt counter).
    pub fn record_success(&mut self) {
        self.current_attempt = 0;
        self.last_delay_ms = 0;
    }

    /// Whether another retry is allowed.
    pub fn can_retry(&self) -> bool {
        self.policy.should_retry(self.current_attempt)
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.current_attempt = 0;
        self.total_attempts = 0;
        self.last_delay_ms = 0;
    }

    /// Total number of attempts made (including across resets from success).
    pub fn total_attempts(&self) -> u32 {
        self.total_attempts
    }
}

impl fmt::Display for ReconnectionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Reconnection(attempt={}/{}, last_delay={}ms)",
            self.current_attempt, self.policy.max_retries, self.last_delay_ms
        )
    }
}

// ---------------------------------------------------------------------------
// Port forwarding
// ---------------------------------------------------------------------------

/// Describes a forwarded port on a remote connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedPort {
    pub local_port: u16,
    pub remote_port: u16,
    pub label: Option<String>,
    pub protocol: PortProtocol,
}

/// Protocol for forwarded ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortProtocol {
    Tcp,
    Udp,
}

impl fmt::Display for PortProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortProtocol::Tcp => write!(f, "TCP"),
            PortProtocol::Udp => write!(f, "UDP"),
        }
    }
}

impl ForwardedPort {
    pub fn new(local: u16, remote: u16) -> Self {
        Self {
            local_port: local,
            remote_port: remote,
            label: None,
            protocol: PortProtocol::Tcp,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_protocol(mut self, proto: PortProtocol) -> Self {
        self.protocol = proto;
        self
    }

    pub fn is_same_port(&self) -> bool {
        self.local_port == self.remote_port
    }
}

impl fmt::Display for ForwardedPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = self.label.as_deref().unwrap_or("unnamed");
        write!(
            f,
            "{} {}:{} → :{}",
            self.protocol, label, self.local_port, self.remote_port
        )
    }
}

/// Manages a collection of forwarded ports for a remote session.
#[derive(Debug, Clone, Default)]
pub struct PortForwardingTable {
    ports: Vec<ForwardedPort>,
}

impl PortForwardingTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a forwarded port. Returns an error if the local port is already in use.
    pub fn add(&mut self, port: ForwardedPort) -> Result<(), RemoteError> {
        if self.ports.iter().any(|p| p.local_port == port.local_port) {
            return Err(RemoteError::Other(format!(
                "local port {} already forwarded",
                port.local_port
            )));
        }
        self.ports.push(port);
        Ok(())
    }

    /// Remove a forwarded port by local port number.
    pub fn remove(&mut self, local_port: u16) -> bool {
        let before = self.ports.len();
        self.ports.retain(|p| p.local_port != local_port);
        self.ports.len() < before
    }

    /// Find a forwarded port by local port number.
    pub fn find_by_local(&self, local_port: u16) -> Option<&ForwardedPort> {
        self.ports.iter().find(|p| p.local_port == local_port)
    }

    /// Return all forwarded ports.
    pub fn all(&self) -> &[ForwardedPort] {
        &self.ports
    }

    pub fn len(&self) -> usize {
        self.ports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ports.is_empty()
    }

    /// Remove all forwarded ports.
    pub fn clear(&mut self) {
        self.ports.clear();
    }

    /// Return a summary of forwarded ports as a display string.
    pub fn summary(&self) -> String {
        if self.ports.is_empty() {
            return "No forwarded ports".to_string();
        }
        let entries: Vec<String> = self.ports.iter().map(|p| format!("{}", p)).collect();
        entries.join(", ")
    }
}

impl fmt::Display for PortForwardingTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PortForwarding({} ports)", self.ports.len())
    }
}

// ---------------------------------------------------------------------------
// SimpleConnectionPool – lightweight pool with health checking
// ---------------------------------------------------------------------------

/// A lightweight pool of remote connections with health checking.
///
/// Unlike [`RemoteConnectionPool`], this uses a simple string-based authority
/// and has no retry policy, suitable for quick health monitoring.
#[derive(Debug, Clone)]
pub struct SimpleConnectionPool {
    connections: Vec<SimplePoolEntry>,
    max_connections: usize,
}

#[derive(Debug, Clone)]
struct SimplePoolEntry {
    name: String,
    authority: String,
    healthy: bool,
    last_check_epoch: u64,
}

impl Default for SimpleConnectionPool {
    fn default() -> Self {
        Self {
            connections: Vec::new(),
            max_connections: 16,
        }
    }
}

impl SimpleConnectionPool {
    /// Create a pool with the given maximum size.
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            ..Default::default()
        }
    }

    /// Add a connection to the pool. Returns `false` if the pool is full.
    pub fn add(&mut self, name: impl Into<String>, authority: impl Into<String>) -> bool {
        if self.connections.len() >= self.max_connections {
            return false;
        }
        self.connections.push(SimplePoolEntry {
            name: name.into(),
            authority: authority.into(),
            healthy: true,
            last_check_epoch: 0,
        });
        true
    }

    /// Remove a connection by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.connections.len();
        self.connections.retain(|e| e.name != name);
        self.connections.len() < len
    }

    /// Mark a connection as unhealthy.
    pub fn mark_unhealthy(&mut self, name: &str) {
        if let Some(e) = self.connections.iter_mut().find(|e| e.name == name) {
            e.healthy = false;
        }
    }

    /// Mark a connection as healthy.
    pub fn mark_healthy(&mut self, name: &str) {
        if let Some(e) = self.connections.iter_mut().find(|e| e.name == name) {
            e.healthy = true;
        }
    }

    /// Get all healthy connections.
    pub fn healthy_connections(&self) -> Vec<&str> {
        self.connections
            .iter()
            .filter(|e| e.healthy)
            .map(|e| e.name.as_str())
            .collect()
    }

    /// Get all unhealthy connections.
    pub fn unhealthy_connections(&self) -> Vec<&str> {
        self.connections
            .iter()
            .filter(|e| !e.healthy)
            .map(|e| e.name.as_str())
            .collect()
    }

    /// Number of connections in the pool.
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Check health of all connections, applying the checker function.
    pub fn check_health(&mut self, checker: impl Fn(&str) -> bool) {
        for entry in &mut self.connections {
            entry.healthy = checker(&entry.authority);
        }
    }
}

impl fmt::Display for SimpleConnectionPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let healthy = self.connections.iter().filter(|e| e.healthy).count();
        write!(
            f,
            "SimpleConnectionPool({}/{} healthy)",
            healthy,
            self.connections.len()
        )
    }
}

// ---------------------------------------------------------------------------
// RemotePortForwarding – port forwarding manager
// ---------------------------------------------------------------------------

/// Manages port forwarding rules for remote connections.
#[derive(Debug, Clone)]
pub struct RemotePortForwarding {
    rules: Vec<PortForwardRule>,
}

/// A single port forwarding rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortForwardRule {
    /// Local port number.
    pub local_port: u16,
    /// Remote port number.
    pub remote_port: u16,
    /// Label for the forwarded port.
    pub label: String,
    /// Whether auto-forwarding is enabled.
    pub auto_forward: bool,
}

impl Default for RemotePortForwarding {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
}

impl RemotePortForwarding {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a port forwarding rule.
    pub fn add_rule(&mut self, local_port: u16, remote_port: u16, label: impl Into<String>) {
        self.rules.push(PortForwardRule {
            local_port,
            remote_port,
            label: label.into(),
            auto_forward: false,
        });
    }

    /// Remove a rule by local port.
    pub fn remove_by_local(&mut self, local_port: u16) -> bool {
        let len = self.rules.len();
        self.rules.retain(|r| r.local_port != local_port);
        self.rules.len() < len
    }

    /// Find a rule by remote port.
    pub fn find_by_remote(&self, remote_port: u16) -> Option<&PortForwardRule> {
        self.rules.iter().find(|r| r.remote_port == remote_port)
    }

    /// Number of forwarding rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// All rules as a slice.
    pub fn rules(&self) -> &[PortForwardRule] {
        &self.rules
    }
}

// ---------------------------------------------------------------------------
// RemoteFileSync – cached remote file reads
// ---------------------------------------------------------------------------

/// Caches remote file contents to avoid repeated reads.
#[derive(Debug, Clone)]
pub struct RemoteFileSync {
    cache: std::collections::HashMap<String, CachedFile>,
    max_entries: usize,
}

#[derive(Debug, Clone)]
struct CachedFile {
    content: Vec<u8>,
    timestamp: u64,
}

impl Default for RemoteFileSync {
    fn default() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
            max_entries: 256,
        }
    }
}

impl RemoteFileSync {
    /// Create a new sync cache.
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// Store a file in the cache.
    pub fn put(&mut self, path: impl Into<String>, content: Vec<u8>, timestamp: u64) {
        if self.cache.len() >= self.max_entries {
            // Evict oldest entry
            if let Some(oldest_key) = self
                .cache
                .iter()
                .min_by_key(|(_, v)| v.timestamp)
                .map(|(k, _)| k.clone())
            {
                self.cache.remove(&oldest_key);
            }
        }
        self.cache.insert(
            path.into(),
            CachedFile { content, timestamp },
        );
    }

    /// Get a cached file if it exists and is not stale.
    pub fn get(&self, path: &str, max_age: u64, now: u64) -> Option<&[u8]> {
        self.cache.get(path).and_then(|entry| {
            if now.saturating_sub(entry.timestamp) <= max_age {
                Some(entry.content.as_slice())
            } else {
                None
            }
        })
    }

    /// Invalidate a cached file.
    pub fn invalidate(&mut self, path: &str) -> bool {
        self.cache.remove(path).is_some()
    }

    /// Number of cached files.
    pub fn cached_count(&self) -> usize {
        self.cache.len()
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

// ---------------------------------------------------------------------------
// Remote authority resolver
// ---------------------------------------------------------------------------

/// Parses and resolves remote authority URIs.
///
/// Authority format: `scheme+host` (e.g. `ssh-remote+myserver`).
/// Unlike [`RemoteAuthority`], this is a simpler parser without user/port fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAuthorityUri {
    /// The scheme part (e.g. `ssh-remote`).
    pub scheme: String,
    /// The host part (e.g. `myserver`).
    pub host: String,
}

impl RemoteAuthorityUri {
    /// Parse an authority string like `"ssh-remote+myserver"`.
    pub fn parse(authority: &str) -> Result<Self, RemoteError> {
        let authority = authority.trim();
        if authority.is_empty() {
            return Err(RemoteError::InvalidAuthority("empty".into()));
        }
        match authority.split_once('+') {
            Some((scheme, host)) if !scheme.is_empty() && !host.is_empty() => Ok(Self {
                scheme: scheme.to_string(),
                host: host.to_string(),
            }),
            _ => Err(RemoteError::InvalidAuthority(authority.to_string())),
        }
    }

    /// Reconstruct the authority string.
    pub fn to_authority_string(&self) -> String {
        format!("{}+{}", self.scheme, self.host)
    }

    /// Whether this is an SSH remote.
    pub fn is_ssh(&self) -> bool {
        self.scheme == "ssh-remote"
    }

    /// Whether this is a WSL remote.
    pub fn is_wsl(&self) -> bool {
        self.scheme == "wsl"
    }

    /// Whether this is a dev container remote.
    pub fn is_dev_container(&self) -> bool {
        self.scheme == "dev-container"
    }
}

impl fmt::Display for RemoteAuthorityUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}+{}", self.scheme, self.host)
    }
}


// === Remote Environment Detector ===

/// Remote Environment Detector implementation.
#[derive(Debug, Clone)]
pub struct RemoteEnvironmentDetector {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: RemoteEnvironmentDetectorStats,
}

/// Statistics for RemoteEnvironmentDetector.
#[derive(Debug, Clone, Default)]
pub struct RemoteEnvironmentDetectorStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl RemoteEnvironmentDetectorStats {
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

impl RemoteEnvironmentDetector {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: RemoteEnvironmentDetectorStats::default(),
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

    pub fn stats(&self) -> &RemoteEnvironmentDetectorStats {
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

impl Default for RemoteEnvironmentDetector {
    fn default() -> Self {
        Self::new()
    }
}

// === Remote Capability Checker ===

/// Priority level for RemoteCapabilityChecker items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RemoteCapabilityCheckerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl RemoteCapabilityCheckerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for RemoteCapabilityCheckerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Remote Capability Checker implementation.
#[derive(Debug, Clone)]
pub struct RemoteCapabilityChecker {
    items: Vec<RemoteCapabilityCheckerItem>,
    max_items: usize,
    default_priority: RemoteCapabilityCheckerPriority,
}

/// A single item in RemoteCapabilityChecker.
#[derive(Debug, Clone)]
pub struct RemoteCapabilityCheckerItem {
    pub id: String,
    pub label: String,
    pub priority: RemoteCapabilityCheckerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl RemoteCapabilityCheckerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: RemoteCapabilityCheckerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: RemoteCapabilityCheckerPriority) -> Self {
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

impl RemoteCapabilityChecker {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: RemoteCapabilityCheckerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: RemoteCapabilityCheckerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<RemoteCapabilityCheckerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&RemoteCapabilityCheckerItem> {
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

    pub fn by_priority(&self, priority: RemoteCapabilityCheckerPriority) -> Vec<&RemoteCapabilityCheckerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&RemoteCapabilityCheckerItem> {
        let mut sorted: Vec<&RemoteCapabilityCheckerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&RemoteCapabilityCheckerItem> {
        let mut sorted: Vec<&RemoteCapabilityCheckerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&RemoteCapabilityCheckerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: RemoteCapabilityCheckerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> RemoteCapabilityCheckerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &RemoteCapabilityCheckerItem> {
        self.items.iter()
    }
}

impl Default for RemoteCapabilityChecker {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// wb_remote – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbRemoteLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbRemotePanelState {
    pub region: XWbRemoteLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbRemotePanelState {
    pub fn new(region: XWbRemoteLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_remote_total_visible_area(panels: &[XWbRemotePanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_remote_count_in_region(
    panels: &[XWbRemotePanelState],
    region: XWbRemoteLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_remote_widest_panel(panels: &[XWbRemotePanelState]) -> Option<&XWbRemotePanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_remote_collapse_region(
    panels: &mut [XWbRemotePanelState],
    region: XWbRemoteLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbRemoteLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbRemoteLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// wb_remote – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench remote connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbRemoteRemoteConnectionKind {
    Ssh,
    Tunnel,
    Container,
    Wsl,
}

impl YWbRemoteRemoteConnectionKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Ssh => 0,
            Self::Tunnel => 1,
            Self::Container => 2,
            Self::Wsl => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ssh => "Ssh",
            Self::Tunnel => "Tunnel",
            Self::Container => "Container",
            Self::Wsl => "Wsl",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbRemoteRemoteConnectionKind] {
        &[
            YWbRemoteRemoteConnectionKind::Ssh,
            YWbRemoteRemoteConnectionKind::Tunnel,
            YWbRemoteRemoteConnectionKind::Container,
            YWbRemoteRemoteConnectionKind::Wsl,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbRemoteRemoteConnectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks remote session data.
#[derive(Debug, Clone)]
pub struct YWbRemoteRemoteSessionInfo {
    pub host: String,
    pub latency_ms: u32,
    pub connected: bool,
}

impl YWbRemoteRemoteSessionInfo {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            host: String::new(),
            latency_ms: 0,
            connected: false,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbRemoteRemoteSessionInfo({}: {:?})", "host", self.host)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_remote_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_remote_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_remote_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_remote_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_remote_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_remote_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_remote_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_remote_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_remote – Extended remote latency tracker helpers
// ---------------------------------------------------------------------------

/// Priority levels for remote latency tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbRemotePriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbRemotePriority {
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
    pub fn all_asc() -> [ZWbRemotePriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbRemotePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks remote latency tracker data.
#[derive(Debug, Clone)]
pub struct ZWbRemoteRemoteLatencyTracker {
    pub samples_ms: Vec<u32>,
    pub window_size: usize,
    pub alarm_threshold_ms: u32,
}

impl ZWbRemoteRemoteLatencyTracker {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            samples_ms: Vec::new(),
            window_size: 0,
            alarm_threshold_ms: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.samples_ms.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.samples_ms.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.samples_ms.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbRemoteRemoteLatencyTracker[window_size={:?}, alarm_threshold_ms={:?}]", self.window_size, self.alarm_threshold_ms)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for remote latency tracker.
pub fn z_wb_remote_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_remote_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_remote_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_remote_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_wb_remote_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_remote_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_remote_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 80
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer80 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer80 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_80(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_80<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_80<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_80(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_80(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 223
// ---------------------------------------------------------------------------

/// Generic object pool `Xc223Pool<T>`.
pub struct Xc223Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc223Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc223PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc223Pool<T> {
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
    pub fn stats(&self) -> Xc223PoolStats {
        Xc223PoolStats {
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

impl<T> Default for Xc223Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc223Scheduler`.
pub struct Xc223Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc223Scheduler {
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

impl Default for Xc223Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_223 hash for the given byte slice.
pub fn xc_223_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_223 convention.
pub fn xc_223_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe93 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe93Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe93PipelineError {
    pub stage: Xe93Stage,
    pub message: String,
}

impl std::fmt::Display for Xe93PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe93Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe93Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError>>>,
    stage_names: Vec<Xe93Stage>,
}

impl Xe93Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe93Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe93Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe93Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe93Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> {
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

    pub fn compose(mut self, other: Xe93Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe93CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe93CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe93Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe93CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe93CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe93Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe93CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_93_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe93CacheEntry {
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

    fn xe_93_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe93CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_93_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> {
    Ok(data)
}

pub fn xe_93_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_93_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_93_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_93_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe93PipelineError> {
    Err(Xe93PipelineError {
        stage: Xe93Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_91: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg91Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg91Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg91Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_91: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg91Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg91Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg91Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg91Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 222).
pub struct Xh222SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh222SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 264 as u64,
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

/// A compact bit set supporting boolean operations (variant 222).
pub struct Xh222BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh222BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 222).
pub struct Xi222Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi222Deque<T> {
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
pub struct Xi222Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi222Interval {
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

/// A simple interval tree (variant 222).
pub struct Xi222IntervalTree {
    xi_intervals: Vec<Xi222Interval>,
}

impl Xi222IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi222Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi222Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi222Interval) -> Vec<&Xi222Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi222Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi222Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi222Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi222Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi222Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi222Interval> = Vec::new();
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
    fn clear_history_works() {
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

    // ---- RemoteConnectionPool tests ----

    #[test]
    fn connection_pool_add_connect() {
        let mut pool = RemoteConnectionPool::new(5);
        let auth = RemoteAuthority::parse("ssh+myhost").unwrap();
        pool.add("dev", auth).unwrap();
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.connected_count(), 0);

        pool.connect("dev").unwrap();
        assert_eq!(pool.connected_count(), 1);
        assert!(pool.get("dev").unwrap().is_connected());
    }

    #[test]
    fn connection_pool_full() {
        let mut pool = RemoteConnectionPool::new(1);
        let auth1 = RemoteAuthority::parse("ssh+host1").unwrap();
        let auth2 = RemoteAuthority::parse("ssh+host2").unwrap();
        pool.add("a", auth1).unwrap();
        let err = pool.add("b", auth2).unwrap_err();
        assert!(matches!(err, RemoteError::Other(_)));
    }

    #[test]
    fn connection_pool_error_and_summary() {
        let mut pool = RemoteConnectionPool::new(3);
        let auth = RemoteAuthority::parse("ssh+h1").unwrap();
        pool.add("conn1", auth.clone()).unwrap();
        pool.add("conn2", auth).unwrap();

        pool.connect("conn1").unwrap();
        pool.set_error("conn2", "timeout".into());

        let summary = pool.summary();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.connected, 1);
        assert_eq!(summary.error, 1);
    }

    #[test]
    fn connection_pool_health_check() {
        let mut pool = RemoteConnectionPool::new(5);
        let auth = RemoteAuthority::parse("ssh+h").unwrap();
        pool.add("test", auth).unwrap();

        pool.check_health(&[("test".to_string(), 50, true)]);
        let conn = pool.get("test").unwrap();
        assert!(conn.healthy);
        assert_eq!(conn.last_health_check_ms, Some(50));
    }

    // ---- ReconnectionStrategy tests ----

    #[test]
    fn reconnection_strategy_backoff() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            backoff_factor: 2.0,
        };
        let mut strategy = ReconnectionStrategy::new(policy);
        let d1 = strategy.next_delay().unwrap();
        assert_eq!(d1, 100);
        let d2 = strategy.next_delay().unwrap();
        assert_eq!(d2, 200);
        let d3 = strategy.next_delay().unwrap();
        assert_eq!(d3, 400);
        assert!(strategy.next_delay().is_none()); // exhausted
    }

    #[test]
    fn reconnection_strategy_reset_on_success() {
        let policy = RetryPolicy::default();
        let mut strategy = ReconnectionStrategy::new(policy);
        strategy.next_delay();
        strategy.next_delay();
        assert_eq!(strategy.current_attempt, 2);
        strategy.record_success();
        assert_eq!(strategy.current_attempt, 0);
        assert!(strategy.can_retry());
    }

    #[test]
    fn connection_pool_disconnect_all() {
        let mut pool = RemoteConnectionPool::new(5);
        let auth = RemoteAuthority::parse("ssh+h").unwrap();
        pool.add("a", auth.clone()).unwrap();
        pool.add("b", auth).unwrap();
        pool.connect("a").unwrap();
        pool.connect("b").unwrap();
        assert_eq!(pool.connected_count(), 2);
        pool.disconnect_all();
        assert_eq!(pool.connected_count(), 0);
    }

    // --- new tests ---

    #[test]
    fn forwarded_port_creation() {
        let port = ForwardedPort::new(3000, 3000)
            .with_label("dev server")
            .with_protocol(PortProtocol::Tcp);
        assert!(port.is_same_port());
        assert_eq!(port.label.as_deref(), Some("dev server"));
        let display = format!("{}", port);
        assert!(display.contains("3000"));
        assert!(display.contains("TCP"));
    }

    #[test]
    fn port_forwarding_table_add_remove() {
        let mut table = PortForwardingTable::new();
        assert!(table.is_empty());
        table.add(ForwardedPort::new(8080, 80)).unwrap();
        table.add(ForwardedPort::new(3000, 3000)).unwrap();
        assert_eq!(table.len(), 2);
        // duplicate local port
        assert!(table.add(ForwardedPort::new(8080, 9090)).is_err());
        // find
        assert!(table.find_by_local(8080).is_some());
        assert!(table.find_by_local(9999).is_none());
        // remove
        assert!(table.remove(8080));
        assert_eq!(table.len(), 1);
        assert!(!table.remove(8080));
    }

    #[test]
    fn port_forwarding_table_summary() {
        let table = PortForwardingTable::new();
        assert_eq!(table.summary(), "No forwarded ports");
        let mut table2 = PortForwardingTable::new();
        table2.add(ForwardedPort::new(8080, 80).with_label("web")).unwrap();
        let summary = table2.summary();
        assert!(summary.contains("8080"));
    }

    #[test]
    fn port_forwarding_table_clear() {
        let mut table = PortForwardingTable::new();
        table.add(ForwardedPort::new(1111, 2222)).unwrap();
        table.add(ForwardedPort::new(3333, 4444)).unwrap();
        table.clear();
        assert!(table.is_empty());
    }

    #[test]
    fn port_protocol_display() {
        assert_eq!(format!("{}", PortProtocol::Tcp), "TCP");
        assert_eq!(format!("{}", PortProtocol::Udp), "UDP");
    }

    #[test]
    fn forwarded_port_different_ports() {
        let port = ForwardedPort::new(3000, 8080);
        assert!(!port.is_same_port());
    }

    // -- SimpleConnectionPool tests --

    #[test]
    fn simple_pool_add_and_remove() {
        let mut pool = SimpleConnectionPool::new(4);
        assert!(pool.add("dev", "ssh-remote+dev"));
        assert_eq!(pool.len(), 1);
        assert!(pool.remove("dev"));
        assert!(pool.is_empty());
    }

    #[test]
    fn simple_pool_capacity() {
        let mut pool = SimpleConnectionPool::new(2);
        assert!(pool.add("a", "auth_a"));
        assert!(pool.add("b", "auth_b"));
        assert!(!pool.add("c", "auth_c"));
    }

    #[test]
    fn simple_pool_health() {
        let mut pool = SimpleConnectionPool::new(4);
        pool.add("a", "auth_a");
        pool.add("b", "auth_b");
        pool.mark_unhealthy("a");
        assert_eq!(pool.healthy_connections(), vec!["b"]);
        assert_eq!(pool.unhealthy_connections(), vec!["a"]);
        pool.mark_healthy("a");
        assert_eq!(pool.healthy_connections().len(), 2);
    }

    #[test]
    fn simple_pool_check_health() {
        let mut pool = SimpleConnectionPool::new(4);
        pool.add("a", "good");
        pool.add("b", "bad");
        pool.check_health(|auth| auth == "good");
        assert_eq!(pool.healthy_connections(), vec!["a"]);
    }

    // -- RemotePortForwarding tests --

    #[test]
    fn port_forwarding_add_and_find() {
        let mut pf = RemotePortForwarding::new();
        pf.add_rule(3000, 8080, "web");
        assert_eq!(pf.rule_count(), 1);
        let rule = pf.find_by_remote(8080).unwrap();
        assert_eq!(rule.local_port, 3000);
        assert_eq!(rule.label, "web");
    }

    #[test]
    fn port_forwarding_remove() {
        let mut pf = RemotePortForwarding::new();
        pf.add_rule(3000, 8080, "web");
        assert!(pf.remove_by_local(3000));
        assert_eq!(pf.rule_count(), 0);
    }

    // -- RemoteFileSync tests --

    #[test]
    fn file_sync_put_and_get() {
        let mut sync = RemoteFileSync::new(10);
        sync.put("/etc/hosts", b"127.0.0.1".to_vec(), 100);
        assert_eq!(sync.get("/etc/hosts", 50, 120), Some(b"127.0.0.1".as_slice()));
    }

    #[test]
    fn file_sync_stale() {
        let mut sync = RemoteFileSync::new(10);
        sync.put("/etc/hosts", b"data".to_vec(), 100);
        assert!(sync.get("/etc/hosts", 10, 200).is_none());
    }

    #[test]
    fn file_sync_invalidate() {
        let mut sync = RemoteFileSync::new(10);
        sync.put("/etc/hosts", b"data".to_vec(), 100);
        assert!(sync.invalidate("/etc/hosts"));
        assert_eq!(sync.cached_count(), 0);
    }

    // -- RemoteAuthorityUri tests --

    #[test]
    fn authority_uri_parse_ssh() {
        let a = RemoteAuthorityUri::parse("ssh-remote+myserver").unwrap();
        assert_eq!(a.scheme, "ssh-remote");
        assert_eq!(a.host, "myserver");
        assert!(a.is_ssh());
    }

    #[test]
    fn authority_uri_parse_wsl() {
        let a = RemoteAuthorityUri::parse("wsl+Ubuntu").unwrap();
        assert!(a.is_wsl());
    }

    #[test]
    fn authority_uri_parse_empty() {
        assert!(RemoteAuthorityUri::parse("").is_err());
    }

    #[test]
    fn authority_uri_parse_no_plus() {
        assert!(RemoteAuthorityUri::parse("noplus").is_err());
    }

    #[test]
    fn authority_uri_roundtrip() {
        let a = RemoteAuthorityUri::parse("dev-container+myapp").unwrap();
        assert_eq!(a.to_authority_string(), "dev-container+myapp");
        assert!(a.is_dev_container());
    }

    #[test]
    fn remoteEnvironmentDetector_new() {
        let s = RemoteEnvironmentDetector::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn remoteEnvironmentDetector_add_contains() {
        let mut s = RemoteEnvironmentDetector::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn remoteEnvironmentDetector_add_duplicate() {
        let mut s = RemoteEnvironmentDetector::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn remoteEnvironmentDetector_remove() {
        let mut s = RemoteEnvironmentDetector::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn remoteEnvironmentDetector_capacity() {
        let s = RemoteEnvironmentDetector::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn remoteEnvironmentDetector_search() {
        let mut s = RemoteEnvironmentDetector::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn remoteEnvironmentDetector_stats() {
        let mut s = RemoteEnvironmentDetector::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn remoteCapabilityChecker_new() {
        let m = RemoteCapabilityChecker::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn remoteCapabilityChecker_add_find() {
        let mut m = RemoteCapabilityChecker::new();
        m.add(RemoteCapabilityCheckerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn remoteCapabilityChecker_priority_filter() {
        let mut m = RemoteCapabilityChecker::new();
        m.add(RemoteCapabilityCheckerItem::new("a", "A").with_priority(RemoteCapabilityCheckerPriority::High));
        m.add(RemoteCapabilityCheckerItem::new("b", "B").with_priority(RemoteCapabilityCheckerPriority::Low));
        m.add(RemoteCapabilityCheckerItem::new("c", "C").with_priority(RemoteCapabilityCheckerPriority::High));
        assert_eq!(m.by_priority(RemoteCapabilityCheckerPriority::High).len(), 2);
    }

    #[test]
    fn remoteCapabilityChecker_remove() {
        let mut m = RemoteCapabilityChecker::new();
        m.add(RemoteCapabilityCheckerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn remoteCapabilityChecker_search() {
        let mut m = RemoteCapabilityChecker::new();
        m.add(RemoteCapabilityCheckerItem::new("id1", "Hello World"));
        m.add(RemoteCapabilityCheckerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn remoteCapabilityChecker_total_weight() {
        let mut m = RemoteCapabilityChecker::new();
        m.add(RemoteCapabilityCheckerItem::new("a", "A").with_priority(RemoteCapabilityCheckerPriority::Critical));
        m.add(RemoteCapabilityCheckerItem::new("b", "B").with_priority(RemoteCapabilityCheckerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn remoteCapabilityChecker_capacity_limit() {
        let mut m = RemoteCapabilityChecker::new().with_max_items(2);
        m.add(RemoteCapabilityCheckerItem::new("1", "one"));
        m.add(RemoteCapabilityCheckerItem::new("2", "two"));
        assert!(!m.add(RemoteCapabilityCheckerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn remoteCapabilityChecker_sorted_by_priority() {
        let mut m = RemoteCapabilityChecker::new();
        m.add(RemoteCapabilityCheckerItem::new("lo", "Low").with_priority(RemoteCapabilityCheckerPriority::Low));
        m.add(RemoteCapabilityCheckerItem::new("hi", "High").with_priority(RemoteCapabilityCheckerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn remoteCapabilityChecker_item_metadata() {
        let mut item = RemoteCapabilityCheckerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn remoteEnvironmentDetector_enabled_toggle() {
        let mut s = RemoteEnvironmentDetector::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn remoteCapabilityChecker_priority_display() {
        assert_eq!(format!("{}", RemoteCapabilityCheckerPriority::High), "high");
        assert_eq!(format!("{}", RemoteCapabilityCheckerPriority::Low), "low");
    }


    // -- wb_remote additional tests -------------------------------------------

    #[test]
    fn x_wb_remote_panel_state_new() {
        let p = XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbRemoteLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_remote_panel_area() {
        let p = XWbRemotePanelState::new(XWbRemoteLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_remote_panel_toggle() {
        let mut p = XWbRemotePanelState::new(XWbRemoteLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_remote_panel_resize() {
        let mut p = XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_remote_panel_is_narrow() {
        let mut p = XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_remote_total_visible_area_basic() {
        let panels = vec![
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "a"),
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_remote_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_remote_total_visible_area_hidden() {
        let mut panels = vec![
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "a"),
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_remote_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_remote_count_in_region_basic() {
        let panels = vec![
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "a"),
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "b"),
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_remote_count_in_region(&panels, XWbRemoteLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_remote_count_in_region(&panels, XWbRemoteLayoutRegion::Editor), 1);
        assert_eq!(x_wb_remote_count_in_region(&panels, XWbRemoteLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_remote_widest_panel_basic() {
        let mut panels = vec![
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "narrow"),
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_remote_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_remote_collapse_region_basic() {
        let mut panels = vec![
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "a"),
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Sidebar, "b"),
            XWbRemotePanelState::new(XWbRemoteLayoutRegion::Editor, "c"),
        ];
        x_wb_remote_collapse_region(&mut panels, XWbRemoteLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_remote_layout_constraint_clamp() {
        let lc = XWbRemoteLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_remote_layout_constraint_satisfied() {
        let lc = XWbRemoteLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_remote_widest_panel_empty() {
        let panels: Vec<XWbRemotePanelState> = vec![];
        assert!(x_wb_remote_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_remote_layout_region_eq() {
        assert_eq!(XWbRemoteLayoutRegion::Sidebar, XWbRemoteLayoutRegion::Sidebar);
        assert_ne!(XWbRemoteLayoutRegion::Sidebar, XWbRemoteLayoutRegion::Panel);
    }


    // -- wb_remote extended domain tests ----------------------------------------

    #[test]
    fn y_wb_remote_enum_index() {
        assert_eq!(YWbRemoteRemoteConnectionKind::Ssh.index(), 0);
        assert_eq!(YWbRemoteRemoteConnectionKind::Tunnel.index(), 1);
        assert_eq!(YWbRemoteRemoteConnectionKind::Container.index(), 2);
        assert_eq!(YWbRemoteRemoteConnectionKind::Wsl.index(), 3);
    }

    #[test]
    fn y_wb_remote_enum_label() {
        assert_eq!(YWbRemoteRemoteConnectionKind::Ssh.label(), "Ssh");
        assert_eq!(YWbRemoteRemoteConnectionKind::Tunnel.label(), "Tunnel");
        assert_eq!(YWbRemoteRemoteConnectionKind::Container.label(), "Container");
        assert_eq!(YWbRemoteRemoteConnectionKind::Wsl.label(), "Wsl");
    }

    #[test]
    fn y_wb_remote_enum_all() {
        let all = YWbRemoteRemoteConnectionKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_remote_enum_is_default() {
        assert!(YWbRemoteRemoteConnectionKind::Ssh.is_default());
        assert!(!YWbRemoteRemoteConnectionKind::Wsl.is_default());
    }

    #[test]
    fn y_wb_remote_enum_display() {
        assert_eq!(format!("{}", YWbRemoteRemoteConnectionKind::Ssh), "Ssh");
    }

    #[test]
    fn y_wb_remote_struct_new() {
        let s = YWbRemoteRemoteSessionInfo::new();
        let _ = s.summary();
    }

    #[test]
    fn y_wb_remote_fingerprint_deterministic() {
        let h1 = y_wb_remote_fingerprint("hello");
        let h2 = y_wb_remote_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_remote_fingerprint("a"), y_wb_remote_fingerprint("b"));
    }

    #[test]
    fn y_wb_remote_truncate_short() {
        assert_eq!(y_wb_remote_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_remote_truncate_long() {
        let r = y_wb_remote_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_remote_normalize_key_basic() {
        assert_eq!(y_wb_remote_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_remote_split_path_basic() {
        let parts = y_wb_remote_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_remote_count_occurrences_basic() {
        assert_eq!(y_wb_remote_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_remote_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_remote_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_remote_in_range_basic() {
        assert!(y_wb_remote_in_range(5, 1, 10));
        assert!(y_wb_remote_in_range(1, 1, 10));
        assert!(y_wb_remote_in_range(10, 1, 10));
        assert!(!y_wb_remote_in_range(0, 1, 10));
        assert!(!y_wb_remote_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_remote_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_remote_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_remote_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_remote_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_remote Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_remote_priority_weight() {
        assert_eq!(ZWbRemotePriority::Idle.weight(), 0);
        assert_eq!(ZWbRemotePriority::Normal.weight(), 2);
        assert_eq!(ZWbRemotePriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_remote_priority_label() {
        assert_eq!(ZWbRemotePriority::Low.label(), "low");
        assert_eq!(ZWbRemotePriority::High.label(), "high");
    }

    #[test]
    fn z_wb_remote_priority_is_elevated() {
        assert!(!ZWbRemotePriority::Normal.is_elevated());
        assert!(ZWbRemotePriority::High.is_elevated());
        assert!(ZWbRemotePriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_remote_priority_display() {
        assert_eq!(format!("{}", ZWbRemotePriority::Idle), "idle");
    }

    #[test]
    fn z_wb_remote_priority_all_asc() {
        let all = ZWbRemotePriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbRemotePriority::Idle);
        assert_eq!(all[4], ZWbRemotePriority::Realtime);
    }

    #[test]
    fn z_wb_remote_struct_new() {
        let s = ZWbRemoteRemoteLatencyTracker::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_remote_struct_toggled_clone() {
        let s = ZWbRemoteRemoteLatencyTracker::new();
        let t = s.toggled_clone();
        let _ = t.alarm_threshold_ms;
    }

    #[test]
    fn z_wb_remote_rolling_hash_deterministic() {
        let h1 = z_wb_remote_rolling_hash(b"test");
        let h2 = z_wb_remote_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_remote_rolling_hash(b"a"), z_wb_remote_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_remote_pad_to_basic() {
        assert_eq!(z_wb_remote_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_remote_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_remote_is_identifier_basic() {
        assert!(z_wb_remote_is_identifier("foo_bar"));
        assert!(z_wb_remote_is_identifier("abc123"));
        assert!(!z_wb_remote_is_identifier(""));
        assert!(!z_wb_remote_is_identifier("has space"));
    }

    #[test]
    fn z_wb_remote_levenshtein_basic() {
        assert_eq!(z_wb_remote_levenshtein("", ""), 0);
        assert_eq!(z_wb_remote_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_remote_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_remote_unique_words_basic() {
        let w = z_wb_remote_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_remote_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_remote_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_remote_common_prefix_basic() {
        assert_eq!(z_wb_remote_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_remote_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_remote_struct_clear() {
        let mut s = ZWbRemoteRemoteLatencyTracker::new();
        s.samples_ms.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_remote_rolling_hash_empty() {
        let h = z_wb_remote_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_80_push_and_len() {
        let mut rb = super::XbRingBuffer80::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_80_overwrite() {
        let mut rb = super::XbRingBuffer80::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_80_get_out_of_bounds() {
        let rb = super::XbRingBuffer80::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_80_drain_all() {
        let mut rb = super::XbRingBuffer80::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_80_peek_front_back() {
        let mut rb = super::XbRingBuffer80::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_80_clear() {
        let mut rb = super::XbRingBuffer80::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_80_capacity() {
        let rb = super::XbRingBuffer80::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_80_basic() {
        let h = super::xb_fnv1a_80(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_80(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_80_different_inputs() {
        let h1 = super::xb_fnv1a_80(b"abc");
        let h2 = super::xb_fnv1a_80(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_80_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_80(&data);
        let dec = super::xb_rle_decode_80(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_80_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_80(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_80(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_80_values() {
        assert!((super::xb_clamp_80(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_80(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_80(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_80_values() {
        assert!((super::xb_lerp_80(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_80(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_80(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_80_wrap_around_twice() {
        let mut rb = super::XbRingBuffer80::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 223 ----

    #[test]
    fn xc_223_pool_new_empty() {
        let pool: super::Xc223Pool<i32> = super::Xc223Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_223_pool_release_acquire() {
        let mut pool = super::Xc223Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_223_pool_acquire_empty() {
        let mut pool: super::Xc223Pool<i32> = super::Xc223Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_223_pool_full() {
        let mut pool = super::Xc223Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_223_pool_drain() {
        let mut pool = super::Xc223Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_223_pool_stats() {
        let mut pool = super::Xc223Pool::new(8);
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
    fn xc_223_pool_clear() {
        let mut pool = super::Xc223Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_223_pool_shrink() {
        let mut pool = super::Xc223Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_223_pool_default() {
        let pool: super::Xc223Pool<String> = super::Xc223Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_223_pool_extend() {
        let mut pool = super::Xc223Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_223_pool_retain() {
        let mut pool = super::Xc223Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_223_scheduler_round_robin() {
        let mut sched = super::Xc223Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_223_scheduler_empty() {
        let mut sched = super::Xc223Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_223_scheduler_reset() {
        let mut sched = super::Xc223Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_223_scheduler_add_remove() {
        let mut sched = super::Xc223Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_223_scheduler_targets() {
        let sched = super::Xc223Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_223_hash_empty() {
        assert_eq!(super::xc_223_hash(b""), 5381);
    }

    #[test]
    fn xc_223_hash_data() {
        let h = super::xc_223_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_223_hash(b"hello"), h);
    }

    #[test]
    fn xc_223_reverse_str() {
        assert_eq!(super::xc_223_reverse("abc"), "cba");
        assert_eq!(super::xc_223_reverse(""), "");
    }


    #[test]
    fn xe_93_pipeline_empty() {
        let p = super::Xe93Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_93_pipeline_parse_stage() {
        let p = super::Xe93Pipeline::new()
            .add_parse(super::xe_93_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_93_pipeline_transform_double() {
        let p = super::Xe93Pipeline::new()
            .add_transform(super::xe_93_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_93_pipeline_validate_reverse() {
        let p = super::Xe93Pipeline::new()
            .add_validate(super::xe_93_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_93_pipeline_emit_filter() {
        let p = super::Xe93Pipeline::new()
            .add_emit(super::xe_93_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_93_pipeline_multi_stage() {
        let p = super::Xe93Pipeline::new()
            .add_parse(super::xe_93_pipeline_identity)
            .add_transform(super::xe_93_pipeline_double)
            .add_validate(super::xe_93_pipeline_reverse)
            .add_emit(super::xe_93_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_93_pipeline_error_propagation() {
        let p = super::Xe93Pipeline::new()
            .add_parse(super::xe_93_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe93Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_93_pipeline_compose() {
        let p1 = super::Xe93Pipeline::new()
            .add_parse(super::xe_93_pipeline_identity);
        let p2 = super::Xe93Pipeline::new()
            .add_transform(super::xe_93_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_93_pipeline_error_display() {
        let e = super::Xe93PipelineError {
            stage: super::Xe93Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_93_cache_put_get() {
        let mut c = super::Xe93Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_93_cache_miss() {
        let mut c: super::Xe93Cache<&str, i32> = super::Xe93Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_93_cache_ttl_expiry() {
        let mut c = super::Xe93Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_93_cache_evict() {
        let mut c = super::Xe93Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_93_cache_capacity() {
        let mut c = super::Xe93Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_93_cache_stats() {
        let mut c = super::Xe93Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_93_cache_clear() {
        let mut c = super::Xe93Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_91 graph tests ------------------------------------------------

    #[test]
    fn xg_91_graph_empty() {
        let g = super::Xg91Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_91_graph_add_node() {
        let mut g = super::Xg91Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_91_graph_add_edge() {
        let mut g = super::Xg91Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_91_graph_neighbors() {
        let mut g = super::Xg91Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_91_graph_has_path() {
        let mut g = super::Xg91Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_91_graph_self_path() {
        let g = super::Xg91Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_91_graph_topo_sort() {
        let mut g = super::Xg91Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_91_graph_cycle_detect_false() {
        let mut g = super::Xg91Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_91_graph_cycle_detect_true() {
        let mut g = super::Xg91Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_91 heap tests -------------------------------------------------

    #[test]
    fn xg_91_heap_empty() {
        let h: super::Xg91Heap<i32> = super::Xg91Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_91_heap_push_pop() {
        let mut h = super::Xg91Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_91_heap_peek() {
        let mut h = super::Xg91Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_91_heap_drain_sorted() {
        let mut h = super::Xg91Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_91_heap_merge() {
        let mut a = super::Xg91Heap::new();
        let mut b = super::Xg91Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_91_heap_default() {
        let h: super::Xg91Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_91_graph_default() {
        let g: super::Xg91Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh222_skip_insert_contains() {
        let mut sl = super::Xh222SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh222_skip_remove() {
        let mut sl = super::Xh222SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh222_skip_len() {
        let mut sl = super::Xh222SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh222_skip_range_query() {
        let mut sl = super::Xh222SkipList::xh_new(4);
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
    fn xh222_skip_floor_ceiling() {
        let mut sl = super::Xh222SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh222_skip_rank() {
        let mut sl = super::Xh222SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh222_skip_empty() {
        let sl = super::Xh222SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh222_skip_duplicates() {
        let mut sl = super::Xh222SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh222_bitset_set_test() {
        let mut bs = super::Xh222BitSet::xh_new(256);
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
    fn xh222_bitset_clear_count() {
        let mut bs = super::Xh222BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh222_bitset_and_or_xor() {
        let mut a = super::Xh222BitSet::xh_new(128);
        let mut b = super::Xh222BitSet::xh_new(128);
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
    fn xh222_bitset_iter_ones() {
        let mut bs = super::Xh222BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh222_bitset_first_last() {
        let mut bs = super::Xh222BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh222_bitset_empty() {
        let bs = super::Xh222BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi222_deque_push_pop_back() {
        let mut dq = super::Xi222Deque::xi_new(4);
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
    fn xi222_deque_push_pop_front() {
        let mut dq = super::Xi222Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi222_deque_mixed_ops() {
        let mut dq = super::Xi222Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi222_deque_get_and_split() {
        let mut dq = super::Xi222Deque::xi_new(8);
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
    fn xi222_deque_rotate_left() {
        let mut dq = super::Xi222Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi222_deque_rotate_right() {
        let mut dq = super::Xi222Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi222_deque_grow() {
        let mut dq = super::Xi222Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi222_deque_empty() {
        let dq = super::Xi222Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi222_interval_tree_insert_query() {
        let mut tree = super::Xi222IntervalTree::xi_new();
        tree.xi_insert(super::Xi222Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi222Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi222Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi222_interval_tree_overlap() {
        let mut tree = super::Xi222IntervalTree::xi_new();
        tree.xi_insert(super::Xi222Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi222Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi222Interval::xi_new(12, 20));
        let q = super::Xi222Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi222_interval_tree_remove() {
        let mut tree = super::Xi222IntervalTree::xi_new();
        tree.xi_insert(super::Xi222Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi222Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi222_interval_tree_gaps() {
        let mut tree = super::Xi222IntervalTree::xi_new();
        tree.xi_insert(super::Xi222Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi222Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi222Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi222Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi222Interval::xi_new(8, 10));
    }

    #[test]
    fn xi222_interval_tree_merge() {
        let mut tree = super::Xi222IntervalTree::xi_new();
        tree.xi_insert(super::Xi222Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi222Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi222Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi222Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi222Interval::xi_new(10, 15));
    }

    #[test]
    fn xi222_interval_tree_all() {
        let mut tree = super::Xi222IntervalTree::xi_new();
        tree.xi_insert(super::Xi222Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi222Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi222_interval_tree_empty() {
        let tree = super::Xi222IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi222_interval_tree_contains_point() {
        let iv = super::Xi222Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
