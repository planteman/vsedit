//! Dev tunnel management.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl fmt::Display for TunnelStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelStatus::Disconnected => write!(f, "Disconnected"),
            TunnelStatus::Connecting => write!(f, "Connecting"),
            TunnelStatus::Connected => write!(f, "Connected"),
            TunnelStatus::Error(msg) => write!(f, "Error: {msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelAccess {
    Private,
    Organization,
    Public,
}

impl fmt::Display for TunnelAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelAccess::Private => write!(f, "Private"),
            TunnelAccess::Organization => write!(f, "Organization"),
            TunnelAccess::Public => write!(f, "Public"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TunnelPort {
    pub port: u16,
    pub protocol: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TunnelInfo {
    pub id: String,
    pub name: String,
    pub status: TunnelStatus,
    pub access: TunnelAccess,
    pub uri: Option<String>,
    pub ports: Vec<TunnelPort>,
}

impl fmt::Display for TunnelInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.status)
    }
}

pub struct TunnelService {
    tunnels: Vec<TunnelInfo>,
    next_id: u64,
}

impl TunnelService {
    pub fn new() -> Self {
        Self {
            tunnels: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_tunnel(&mut self, name: impl Into<String>, access: TunnelAccess) -> String {
        let id = format!("tunnel-{}", self.next_id);
        self.next_id += 1;
        self.tunnels.push(TunnelInfo {
            id: id.clone(),
            name: name.into(),
            status: TunnelStatus::Disconnected,
            access,
            uri: None,
            ports: Vec::new(),
        });
        id
    }

    pub fn connect(&mut self, id: &str) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.status = TunnelStatus::Connected;
            true
        } else {
            false
        }
    }

    pub fn disconnect(&mut self, id: &str) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.status = TunnelStatus::Disconnected;
        }
    }

    pub fn get_tunnel(&self, id: &str) -> Option<&TunnelInfo> {
        self.tunnels.iter().find(|t| t.id == id)
    }

    pub fn remove_tunnel(&mut self, id: &str) -> bool {
        let len = self.tunnels.len();
        self.tunnels.retain(|t| t.id != id);
        self.tunnels.len() < len
    }

    pub fn active_count(&self) -> usize {
        self.tunnels
            .iter()
            .filter(|t| t.status == TunnelStatus::Connected)
            .count()
    }

    pub fn add_port(&mut self, tunnel_id: &str, port: TunnelPort) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == tunnel_id) {
            if t.ports.iter().any(|p| p.port == port.port) {
                return false;
            }
            t.ports.push(port);
            true
        } else {
            false
        }
    }

    pub fn remove_port(&mut self, tunnel_id: &str, port_num: u16) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == tunnel_id) {
            let len = t.ports.len();
            t.ports.retain(|p| p.port != port_num);
            t.ports.len() < len
        } else {
            false
        }
    }

    pub fn get_all_tunnels(&self) -> &[TunnelInfo] {
        &self.tunnels
    }

    pub fn tunnel_count(&self) -> usize {
        self.tunnels.len()
    }

    pub fn set_error(&mut self, id: &str, message: String) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.status = TunnelStatus::Error(message);
        }
    }

    pub fn set_uri(&mut self, id: &str, uri: String) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.uri = Some(uri);
        }
    }

    pub fn find_by_name(&self, name: &str) -> Option<&TunnelInfo> {
        self.tunnels.iter().find(|t| t.name == name)
    }

    pub fn disconnect_all(&mut self) {
        for t in &mut self.tunnels {
            if t.status == TunnelStatus::Connected || t.status == TunnelStatus::Connecting {
                t.status = TunnelStatus::Disconnected;
            }
        }
    }
}

impl Default for TunnelService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_connect() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("my-tunnel", TunnelAccess::Private);
        assert_eq!(svc.active_count(), 0);
        assert!(svc.connect(&id));
        assert_eq!(svc.active_count(), 1);
    }

    #[test]
    fn disconnect_and_remove() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t1", TunnelAccess::Public);
        svc.connect(&id);
        svc.disconnect(&id);
        assert_eq!(svc.active_count(), 0);
        assert!(svc.remove_tunnel(&id));
        assert!(svc.get_tunnel(&id).is_none());
    }

    #[test]
    fn connect_invalid() {
        let mut svc = TunnelService::new();
        assert!(!svc.connect("nonexistent"));
        assert!(!svc.remove_tunnel("nonexistent"));
    }

    #[test]
    fn add_and_remove_port() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("port-test", TunnelAccess::Private);
        let port = TunnelPort { port: 8080, protocol: "https".into(), label: Some("web".into()) };
        assert!(svc.add_port(&id, port));
        assert_eq!(svc.get_tunnel(&id).unwrap().ports.len(), 1);
        // duplicate port number rejected
        let dup = TunnelPort { port: 8080, protocol: "http".into(), label: None };
        assert!(!svc.add_port(&id, dup));
        assert!(svc.remove_port(&id, 8080));
        assert!(svc.get_tunnel(&id).unwrap().ports.is_empty());
    }

    #[test]
    fn port_on_nonexistent_tunnel() {
        let mut svc = TunnelService::new();
        let port = TunnelPort { port: 3000, protocol: "http".into(), label: None };
        assert!(!svc.add_port("bad-id", port));
        assert!(!svc.remove_port("bad-id", 3000));
    }

    #[test]
    fn tunnel_count_and_get_all() {
        let mut svc = TunnelService::new();
        assert_eq!(svc.tunnel_count(), 0);
        svc.create_tunnel("a", TunnelAccess::Private);
        svc.create_tunnel("b", TunnelAccess::Public);
        assert_eq!(svc.tunnel_count(), 2);
        assert_eq!(svc.get_all_tunnels().len(), 2);
    }

    #[test]
    fn set_error_and_uri() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("err-test", TunnelAccess::Private);
        svc.set_error(&id, "connection refused".into());
        assert_eq!(svc.get_tunnel(&id).unwrap().status, TunnelStatus::Error("connection refused".into()));
        svc.set_uri(&id, "https://example.devtunnels.ms".into());
        assert_eq!(svc.get_tunnel(&id).unwrap().uri.as_deref(), Some("https://example.devtunnels.ms"));
    }

    #[test]
    fn find_by_name_and_disconnect_all() {
        let mut svc = TunnelService::new();
        let id1 = svc.create_tunnel("alpha", TunnelAccess::Private);
        let id2 = svc.create_tunnel("beta", TunnelAccess::Organization);
        svc.connect(&id1);
        svc.connect(&id2);
        assert_eq!(svc.active_count(), 2);
        assert!(svc.find_by_name("alpha").is_some());
        assert!(svc.find_by_name("missing").is_none());
        svc.disconnect_all();
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn display_impls() {
        assert_eq!(TunnelStatus::Connected.to_string(), "Connected");
        assert_eq!(TunnelStatus::Disconnected.to_string(), "Disconnected");
        assert_eq!(TunnelStatus::Connecting.to_string(), "Connecting");
        assert_eq!(TunnelStatus::Error("fail".into()).to_string(), "Error: fail");
        assert_eq!(TunnelAccess::Private.to_string(), "Private");
        assert_eq!(TunnelAccess::Organization.to_string(), "Organization");
        assert_eq!(TunnelAccess::Public.to_string(), "Public");

        let info = TunnelInfo {
            id: "t1".into(),
            name: "my-tunnel".into(),
            status: TunnelStatus::Connected,
            access: TunnelAccess::Public,
            uri: None,
            ports: Vec::new(),
        };
        assert_eq!(info.to_string(), "my-tunnel (Connected)");
    }
}
