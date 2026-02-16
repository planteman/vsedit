//! Remote connection service.

use std::fmt;

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

#[derive(Debug, Clone)]
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
}

/// A record of a past connection attempt.
#[derive(Debug, Clone)]
pub struct ConnectionRecord {
    pub authority: String,
    pub timestamp: u64,
    pub success: bool,
}

/// Service for remote workbench functionality.
pub struct RemoteWorkbenchService {
    authority: Option<String>,
    environment: Option<RemoteEnvironment>,
    state: ConnectionState,
    connection_history: Vec<ConnectionRecord>,
}

impl RemoteWorkbenchService {
    pub fn new() -> Self {
        Self {
            authority: None,
            environment: None,
            state: ConnectionState::Disconnected,
            connection_history: Vec::new(),
        }
    }

    pub fn set_authority(&mut self, authority: String) {
        self.authority = Some(authority);
    }

    pub fn get_authority(&self) -> Option<&str> {
        self.authority.as_deref()
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
        }
    }

    pub fn disconnect(&mut self) {
        self.environment = None;
        self.state = ConnectionState::Disconnected;
    }

    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
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
}

impl Default for RemoteWorkbenchService {
    fn default() -> Self {
        Self::new()
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
}
