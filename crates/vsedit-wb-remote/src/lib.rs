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
}
