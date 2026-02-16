//! Dev tunnel management.

use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during tunnel operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TunnelError {
    /// The referenced tunnel does not exist.
    NotFound(String),
    /// A tunnel with the given name already exists.
    DuplicateName(String),
    /// The requested port is already forwarded on this tunnel.
    DuplicatePort { tunnel_id: String, port: u16 },
    /// A port number of zero is not valid.
    InvalidPort,
    /// The tunnel name is empty or contains invalid characters.
    InvalidName(String),
    /// The tunnel is not in the expected state for this operation.
    InvalidState { expected: TunnelStatus, actual: TunnelStatus },
}

impl fmt::Display for TunnelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelError::NotFound(id) => write!(f, "tunnel not found: {id}"),
            TunnelError::DuplicateName(n) => write!(f, "duplicate tunnel name: {n}"),
            TunnelError::DuplicatePort { tunnel_id, port } => {
                write!(f, "port {port} already forwarded on tunnel {tunnel_id}")
            }
            TunnelError::InvalidPort => write!(f, "port number must be non-zero"),
            TunnelError::InvalidName(reason) => write!(f, "invalid tunnel name: {reason}"),
            TunnelError::InvalidState { expected, actual } => {
                write!(f, "expected state {expected}, got {actual}")
            }
        }
    }
}

impl std::error::Error for TunnelError {}

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

impl fmt::Display for TunnelPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.label {
            Some(lbl) => write!(f, "{}:{} ({})", self.protocol, self.port, lbl),
            None => write!(f, "{}:{}", self.protocol, self.port),
        }
    }
}

impl TunnelPort {
    /// Validate that the port configuration is usable.
    pub fn validate(&self) -> Result<(), TunnelError> {
        if self.port == 0 {
            return Err(TunnelError::InvalidPort);
        }
        Ok(())
    }

    /// Returns `true` if this port uses a secure protocol.
    pub fn is_secure(&self) -> bool {
        matches!(self.protocol.as_str(), "https" | "ssh" | "tls")
    }
}

// ---------------------------------------------------------------------------
// TunnelPort builder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`TunnelPort`] with validation.
#[derive(Debug, Clone)]
pub struct TunnelPortBuilder {
    port: u16,
    protocol: String,
    label: Option<String>,
}

impl TunnelPortBuilder {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            protocol: "https".to_string(),
            label: None,
        }
    }

    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = protocol.into();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Build the `TunnelPort`, returning an error if validation fails.
    pub fn build(self) -> Result<TunnelPort, TunnelError> {
        let tp = TunnelPort {
            port: self.port,
            protocol: self.protocol,
            label: self.label,
        };
        tp.validate()?;
        Ok(tp)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TunnelInfo {
    pub id: String,
    pub name: String,
    pub status: TunnelStatus,
    pub access: TunnelAccess,
    pub uri: Option<String>,
    pub ports: Vec<TunnelPort>,
}

impl TunnelInfo {
    /// Returns `true` when the tunnel is actively connected.
    pub fn is_connected(&self) -> bool {
        self.status == TunnelStatus::Connected
    }

    /// Returns `true` when the tunnel is in an error state.
    pub fn is_error(&self) -> bool {
        matches!(self.status, TunnelStatus::Error(_))
    }

    /// Count of forwarded ports.
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    /// Returns all secure ports forwarded on this tunnel.
    pub fn secure_ports(&self) -> Vec<&TunnelPort> {
        self.ports.iter().filter(|p| p.is_secure()).collect()
    }

    /// Validate the tunnel name (non-empty, ASCII alphanumeric / hyphens only).
    pub fn validate_name(name: &str) -> Result<(), TunnelError> {
        if name.is_empty() {
            return Err(TunnelError::InvalidName("name must not be empty".into()));
        }
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(TunnelError::InvalidName(
                "name must contain only ASCII alphanumerics, hyphens, or underscores".into(),
            ));
        }
        Ok(())
    }
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

    /// Create a tunnel with name validation, rejecting duplicates.
    pub fn create_validated(
        &mut self,
        name: impl Into<String>,
        access: TunnelAccess,
    ) -> Result<String, TunnelError> {
        let name = name.into();
        TunnelInfo::validate_name(&name)?;
        if self.tunnels.iter().any(|t| t.name == name) {
            return Err(TunnelError::DuplicateName(name));
        }
        Ok(self.create_tunnel(name, access))
    }

    /// Add a port with full validation, returning a typed error.
    pub fn add_port_validated(
        &mut self,
        tunnel_id: &str,
        port: TunnelPort,
    ) -> Result<(), TunnelError> {
        port.validate()?;
        let tunnel = self
            .tunnels
            .iter_mut()
            .find(|t| t.id == tunnel_id)
            .ok_or_else(|| TunnelError::NotFound(tunnel_id.to_string()))?;
        if tunnel.ports.iter().any(|p| p.port == port.port) {
            return Err(TunnelError::DuplicatePort {
                tunnel_id: tunnel_id.to_string(),
                port: port.port,
            });
        }
        tunnel.ports.push(port);
        Ok(())
    }

    /// Connect a tunnel only if it is currently disconnected.
    pub fn connect_checked(&mut self, id: &str) -> Result<(), TunnelError> {
        let tunnel = self
            .tunnels
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TunnelError::NotFound(id.to_string()))?;
        if tunnel.status != TunnelStatus::Disconnected {
            return Err(TunnelError::InvalidState {
                expected: TunnelStatus::Disconnected,
                actual: tunnel.status.clone(),
            });
        }
        tunnel.status = TunnelStatus::Connected;
        Ok(())
    }

    /// Return tunnels filtered by access level.
    pub fn tunnels_by_access(&self, access: &TunnelAccess) -> Vec<&TunnelInfo> {
        self.tunnels.iter().filter(|t| &t.access == access).collect()
    }

    /// Summary string listing every tunnel and its status.
    pub fn summary(&self) -> String {
        if self.tunnels.is_empty() {
            return "No tunnels configured.".to_string();
        }
        self.tunnels
            .iter()
            .map(|t| format!("  {} ({}) [{}] - {} ({} ports)", t.id, t.name, t.access, t.status, t.port_count()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Debug for TunnelService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TunnelService")
            .field("tunnel_count", &self.tunnels.len())
            .field("next_id", &self.next_id)
            .finish()
    }
}

impl fmt::Display for TunnelService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TunnelService({} tunnels, {} active)",
            self.tunnel_count(),
            self.active_count()
        )
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

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn tunnel_error_display() {
        assert_eq!(
            TunnelError::NotFound("x".into()).to_string(),
            "tunnel not found: x"
        );
        assert_eq!(
            TunnelError::DuplicateName("dup".into()).to_string(),
            "duplicate tunnel name: dup"
        );
        assert_eq!(
            TunnelError::DuplicatePort { tunnel_id: "t1".into(), port: 80 }.to_string(),
            "port 80 already forwarded on tunnel t1"
        );
        assert_eq!(TunnelError::InvalidPort.to_string(), "port number must be non-zero");
        assert!(TunnelError::InvalidName("bad".into()).to_string().contains("bad"));
    }

    #[test]
    fn tunnel_port_builder_success() {
        let port = TunnelPortBuilder::new(8080)
            .protocol("http")
            .label("web")
            .build()
            .unwrap();
        assert_eq!(port.port, 8080);
        assert_eq!(port.protocol, "http");
        assert_eq!(port.label.as_deref(), Some("web"));
    }

    #[test]
    fn tunnel_port_builder_zero_port() {
        let result = TunnelPortBuilder::new(0).build();
        assert_eq!(result, Err(TunnelError::InvalidPort));
    }

    #[test]
    fn tunnel_port_display() {
        let p1 = TunnelPort { port: 443, protocol: "https".into(), label: Some("api".into()) };
        assert_eq!(p1.to_string(), "https:443 (api)");
        let p2 = TunnelPort { port: 80, protocol: "http".into(), label: None };
        assert_eq!(p2.to_string(), "http:80");
    }

    #[test]
    fn tunnel_port_is_secure() {
        let secure = TunnelPort { port: 443, protocol: "https".into(), label: None };
        assert!(secure.is_secure());
        let insecure = TunnelPort { port: 80, protocol: "http".into(), label: None };
        assert!(!insecure.is_secure());
    }

    #[test]
    fn tunnel_info_helpers() {
        let info = TunnelInfo {
            id: "t1".into(),
            name: "test".into(),
            status: TunnelStatus::Connected,
            access: TunnelAccess::Private,
            uri: None,
            ports: vec![
                TunnelPort { port: 443, protocol: "https".into(), label: None },
                TunnelPort { port: 80, protocol: "http".into(), label: None },
            ],
        };
        assert!(info.is_connected());
        assert!(!info.is_error());
        assert_eq!(info.port_count(), 2);
        assert_eq!(info.secure_ports().len(), 1);
    }

    #[test]
    fn validate_name_ok_and_bad() {
        assert!(TunnelInfo::validate_name("my-tunnel_1").is_ok());
        assert!(TunnelInfo::validate_name("").is_err());
        assert!(TunnelInfo::validate_name("bad name!").is_err());
    }

    #[test]
    fn create_validated_rejects_duplicates() {
        let mut svc = TunnelService::new();
        assert!(svc.create_validated("unique", TunnelAccess::Private).is_ok());
        let err = svc.create_validated("unique", TunnelAccess::Private).unwrap_err();
        assert_eq!(err, TunnelError::DuplicateName("unique".into()));
    }

    #[test]
    fn create_validated_rejects_bad_name() {
        let mut svc = TunnelService::new();
        assert!(svc.create_validated("", TunnelAccess::Private).is_err());
        assert!(svc.create_validated("has space", TunnelAccess::Private).is_err());
    }

    #[test]
    fn add_port_validated_errors() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t", TunnelAccess::Private);

        // zero port
        let zero = TunnelPort { port: 0, protocol: "http".into(), label: None };
        assert_eq!(svc.add_port_validated(&id, zero), Err(TunnelError::InvalidPort));

        // not-found tunnel
        let p = TunnelPort { port: 80, protocol: "http".into(), label: None };
        assert!(matches!(svc.add_port_validated("nope", p), Err(TunnelError::NotFound(_))));

        // duplicate
        let p1 = TunnelPort { port: 8080, protocol: "http".into(), label: None };
        svc.add_port_validated(&id, p1).unwrap();
        let p2 = TunnelPort { port: 8080, protocol: "https".into(), label: None };
        assert!(matches!(svc.add_port_validated(&id, p2), Err(TunnelError::DuplicatePort { .. })));
    }

    #[test]
    fn connect_checked_state_guard() {
        let mut svc = TunnelService::new();
        let id = svc.create_tunnel("t", TunnelAccess::Private);
        svc.connect_checked(&id).unwrap();
        // already connected – should fail
        let err = svc.connect_checked(&id).unwrap_err();
        assert!(matches!(err, TunnelError::InvalidState { .. }));
        // not found
        assert!(matches!(svc.connect_checked("bad"), Err(TunnelError::NotFound(_))));
    }

    #[test]
    fn tunnels_by_access_filter() {
        let mut svc = TunnelService::new();
        svc.create_tunnel("priv1", TunnelAccess::Private);
        svc.create_tunnel("pub1", TunnelAccess::Public);
        svc.create_tunnel("priv2", TunnelAccess::Private);
        assert_eq!(svc.tunnels_by_access(&TunnelAccess::Private).len(), 2);
        assert_eq!(svc.tunnels_by_access(&TunnelAccess::Public).len(), 1);
        assert_eq!(svc.tunnels_by_access(&TunnelAccess::Organization).len(), 0);
    }

    #[test]
    fn service_display_and_debug() {
        let mut svc = TunnelService::new();
        assert_eq!(svc.to_string(), "TunnelService(0 tunnels, 0 active)");
        let id = svc.create_tunnel("t", TunnelAccess::Private);
        svc.connect(&id);
        assert_eq!(svc.to_string(), "TunnelService(1 tunnels, 1 active)");
        let dbg = format!("{:?}", svc);
        assert!(dbg.contains("TunnelService"));
    }

    #[test]
    fn summary_output() {
        let mut svc = TunnelService::new();
        assert_eq!(svc.summary(), "No tunnels configured.");
        svc.create_tunnel("web", TunnelAccess::Public);
        let s = svc.summary();
        assert!(s.contains("web"));
        assert!(s.contains("Public"));
        assert!(s.contains("Disconnected"));
    }
}
