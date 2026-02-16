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
}

impl Default for RemoteService {
    fn default() -> Self {
        Self::new()
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
}
