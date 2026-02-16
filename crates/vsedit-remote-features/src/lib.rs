//! Remote features.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteFeature {
    FileSystem,
    Terminal,
    Debugging,
    Extensions,
    PortForwarding,
}

impl fmt::Display for RemoteFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileSystem => write!(f, "File System"),
            Self::Terminal => write!(f, "Terminal"),
            Self::Debugging => write!(f, "Debugging"),
            Self::Extensions => write!(f, "Extensions"),
            Self::PortForwarding => write!(f, "Port Forwarding"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PortProtocol {
    Http,
    Https,
    Tcp,
}

impl fmt::Display for PortProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http => write!(f, "HTTP"),
            Self::Https => write!(f, "HTTPS"),
            Self::Tcp => write!(f, "TCP"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PortVisibility {
    Private,
    Organization,
    Public,
}

#[derive(Debug, Clone)]
pub struct PortForward {
    pub local_port: u16,
    pub remote_port: u16,
    pub label: Option<String>,
    pub protocol: PortProtocol,
    pub auto_forward: bool,
    pub visibility: PortVisibility,
}

pub struct PortForwardBuilder {
    local_port: u16,
    remote_port: u16,
    label: Option<String>,
    protocol: PortProtocol,
    auto_forward: bool,
    visibility: PortVisibility,
}

impl PortForwardBuilder {
    pub fn new(local_port: u16, remote_port: u16) -> Self {
        Self {
            local_port,
            remote_port,
            label: None,
            protocol: PortProtocol::Tcp,
            auto_forward: false,
            visibility: PortVisibility::Private,
        }
    }

    pub fn local_port(mut self, port: u16) -> Self {
        self.local_port = port;
        self
    }

    pub fn remote_port(mut self, port: u16) -> Self {
        self.remote_port = port;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn protocol(mut self, protocol: PortProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn auto_forward(mut self, auto_forward: bool) -> Self {
        self.auto_forward = auto_forward;
        self
    }

    pub fn visibility(mut self, visibility: PortVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn build(self) -> PortForward {
        PortForward {
            local_port: self.local_port,
            remote_port: self.remote_port,
            label: self.label,
            protocol: self.protocol,
            auto_forward: self.auto_forward,
            visibility: self.visibility,
        }
    }
}

pub struct RemoteFeaturesService {
    features_enabled: Vec<RemoteFeature>,
    forwarded_ports: Vec<PortForward>,
}

impl RemoteFeaturesService {
    pub fn new() -> Self {
        Self {
            features_enabled: Vec::new(),
            forwarded_ports: Vec::new(),
        }
    }

    pub fn enable_feature(&mut self, feature: RemoteFeature) {
        if !self.features_enabled.contains(&feature) {
            self.features_enabled.push(feature);
        }
    }

    pub fn disable_feature(&mut self, feature: RemoteFeature) {
        self.features_enabled.retain(|f| f != &feature);
    }

    pub fn is_enabled(&self, feature: &RemoteFeature) -> bool {
        self.features_enabled.contains(feature)
    }

    pub fn add_port_forward(&mut self, forward: PortForward) {
        self.forwarded_ports.push(forward);
    }

    pub fn remove_port_forward(&mut self, local_port: u16) -> bool {
        let len = self.forwarded_ports.len();
        self.forwarded_ports.retain(|p| p.local_port != local_port);
        self.forwarded_ports.len() < len
    }

    pub fn get_forwarded_ports(&self) -> &[PortForward] {
        &self.forwarded_ports
    }

    pub fn get_auto_forward_ports(&self) -> Vec<&PortForward> {
        self.forwarded_ports.iter().filter(|p| p.auto_forward).collect()
    }

    pub fn find_port_by_local(&self, port: u16) -> Option<&PortForward> {
        self.forwarded_ports.iter().find(|p| p.local_port == port)
    }

    pub fn find_port_by_remote(&self, port: u16) -> Option<&PortForward> {
        self.forwarded_ports.iter().find(|p| p.remote_port == port)
    }

    pub fn enabled_features(&self) -> &[RemoteFeature] {
        &self.features_enabled
    }

    pub fn enable_all_defaults(&mut self) {
        for feature in [RemoteFeature::FileSystem, RemoteFeature::Terminal, RemoteFeature::Extensions] {
            self.enable_feature(feature);
        }
    }

    pub fn forwarded_port_count(&self) -> usize {
        self.forwarded_ports.len()
    }
}

impl Default for RemoteFeaturesService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PortVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Private => write!(f, "Private"),
            Self::Organization => write!(f, "Organization"),
            Self::Public => write!(f, "Public"),
        }
    }
}

impl fmt::Display for PortForward {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = self.label.as_deref().unwrap_or("unnamed");
        write!(f, "{} ({}:{} -> {})", label, self.protocol, self.local_port, self.remote_port)
    }
}

impl PartialEq for PortForward {
    fn eq(&self, other: &Self) -> bool {
        self.local_port == other.local_port
            && self.remote_port == other.remote_port
            && self.protocol == other.protocol
    }
}

/// Information about a remote connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteConnectionInfo {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub connection_type: ConnectionType,
    pub connected: bool,
}

/// Type of remote connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Ssh,
    Wsl,
    Container,
    Tunnel,
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ssh => write!(f, "SSH"),
            Self::Wsl => write!(f, "WSL"),
            Self::Container => write!(f, "Container"),
            Self::Tunnel => write!(f, "Tunnel"),
        }
    }
}

impl RemoteConnectionInfo {
    pub fn new(host: impl Into<String>, port: u16, conn_type: ConnectionType) -> Self {
        Self {
            host: host.into(),
            port,
            username: None,
            connection_type: conn_type,
            connected: false,
        }
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn connect(&mut self) {
        self.connected = true;
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn display_address(&self) -> String {
        match &self.username {
            Some(user) => format!("{}@{}:{}", user, self.host, self.port),
            None => format!("{}:{}", self.host, self.port),
        }
    }
}

impl fmt::Display for RemoteConnectionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.connected { "connected" } else { "disconnected" };
        write!(f, "{} {} ({})", self.connection_type, self.display_address(), status)
    }
}

/// A range of ports for batch operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

impl PortRange {
    pub fn new(start: u16, end: u16) -> Self {
        Self {
            start: start.min(end),
            end: start.max(end),
        }
    }

    pub fn contains(&self, port: u16) -> bool {
        port >= self.start && port <= self.end
    }

    pub fn len(&self) -> u16 {
        self.end - self.start + 1
    }

    pub fn is_empty(&self) -> bool {
        false // A range always has at least one port
    }

    pub fn overlaps(&self, other: &PortRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    pub fn ports(&self) -> Vec<u16> {
        (self.start..=self.end).collect()
    }
}

impl fmt::Display for PortRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

impl RemoteFeaturesService {
    /// Find ports by protocol.
    pub fn ports_by_protocol(&self, protocol: &PortProtocol) -> Vec<&PortForward> {
        self.forwarded_ports.iter().filter(|p| &p.protocol == protocol).collect()
    }

    /// Check if a local port is already forwarded.
    pub fn is_port_forwarded(&self, local_port: u16) -> bool {
        self.forwarded_ports.iter().any(|p| p.local_port == local_port)
    }

    /// Get all ports within a given range.
    pub fn ports_in_range(&self, range: &PortRange) -> Vec<&PortForward> {
        self.forwarded_ports
            .iter()
            .filter(|p| range.contains(p.local_port))
            .collect()
    }

    /// Get forwarded ports with a specific visibility.
    pub fn ports_by_visibility(&self, visibility: &PortVisibility) -> Vec<&PortForward> {
        self.forwarded_ports.iter().filter(|p| &p.visibility == visibility).collect()
    }

    /// Return a summary of the service state.
    pub fn summary(&self) -> RemoteServiceSummary {
        let http_count = self.ports_by_protocol(&PortProtocol::Http).len();
        let https_count = self.ports_by_protocol(&PortProtocol::Https).len();
        let tcp_count = self.ports_by_protocol(&PortProtocol::Tcp).len();
        RemoteServiceSummary {
            features_enabled: self.features_enabled.len(),
            total_ports: self.forwarded_ports.len(),
            http_ports: http_count,
            https_ports: https_count,
            tcp_ports: tcp_count,
            auto_forward_count: self.get_auto_forward_ports().len(),
        }
    }

    /// Update the label of a forwarded port. Returns true if found and updated.
    pub fn update_port_label(&mut self, local_port: u16, new_label: impl Into<String>) -> bool {
        if let Some(pf) = self.forwarded_ports.iter_mut().find(|p| p.local_port == local_port) {
            pf.label = Some(new_label.into());
            true
        } else {
            false
        }
    }

    /// Remove all ports with auto_forward disabled.
    pub fn remove_non_auto_forwards(&mut self) -> usize {
        let before = self.forwarded_ports.len();
        self.forwarded_ports.retain(|p| p.auto_forward);
        before - self.forwarded_ports.len()
    }
}

/// Summary of the remote features service state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteServiceSummary {
    pub features_enabled: usize,
    pub total_ports: usize,
    pub http_ports: usize,
    pub https_ports: usize,
    pub tcp_ports: usize,
    pub auto_forward_count: usize,
}

impl fmt::Display for RemoteServiceSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} features, {} ports ({} HTTP, {} HTTPS, {} TCP), {} auto-forward",
            self.features_enabled, self.total_ports,
            self.http_ports, self.https_ports, self.tcp_ports,
            self.auto_forward_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_and_disable_feature() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_feature(RemoteFeature::Terminal);
        assert!(svc.is_enabled(&RemoteFeature::Terminal));
        svc.disable_feature(RemoteFeature::Terminal);
        assert!(!svc.is_enabled(&RemoteFeature::Terminal));
    }

    #[test]
    fn no_duplicate_features() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_feature(RemoteFeature::FileSystem);
        svc.enable_feature(RemoteFeature::FileSystem);
        svc.disable_feature(RemoteFeature::FileSystem);
        assert!(!svc.is_enabled(&RemoteFeature::FileSystem));
    }

    #[test]
    fn port_forwarding() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForward {
            local_port: 3000,
            remote_port: 3000,
            label: Some("web".into()),
            protocol: PortProtocol::Http,
            auto_forward: true,
            visibility: PortVisibility::Private,
        });
        assert_eq!(svc.get_forwarded_ports().len(), 1);
        assert!(svc.remove_port_forward(3000));
        assert!(svc.get_forwarded_ports().is_empty());
        assert!(!svc.remove_port_forward(3000));
    }

    #[test]
    fn port_forward_builder() {
        let pf = PortForwardBuilder::new(8080, 80)
            .label("api")
            .protocol(PortProtocol::Https)
            .auto_forward(true)
            .visibility(PortVisibility::Organization)
            .build();
        assert_eq!(pf.local_port, 8080);
        assert_eq!(pf.remote_port, 80);
        assert_eq!(pf.label, Some("api".into()));
        assert_eq!(pf.protocol, PortProtocol::Https);
        assert!(pf.auto_forward);
        assert_eq!(pf.visibility, PortVisibility::Organization);
    }

    #[test]
    fn port_forward_builder_defaults() {
        let pf = PortForwardBuilder::new(3000, 3000).build();
        assert_eq!(pf.protocol, PortProtocol::Tcp);
        assert!(!pf.auto_forward);
        assert_eq!(pf.visibility, PortVisibility::Private);
        assert!(pf.label.is_none());
    }

    #[test]
    fn get_auto_forward_ports() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).auto_forward(true).build());
        svc.add_port_forward(PortForwardBuilder::new(5000, 5000).auto_forward(false).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).auto_forward(true).build());
        let auto = svc.get_auto_forward_ports();
        assert_eq!(auto.len(), 2);
        assert_eq!(auto[0].local_port, 3000);
        assert_eq!(auto[1].local_port, 8080);
    }

    #[test]
    fn find_ports() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 80).label("web").build());
        svc.add_port_forward(PortForwardBuilder::new(5432, 5432).label("db").build());
        assert_eq!(svc.find_port_by_local(3000).unwrap().remote_port, 80);
        assert_eq!(svc.find_port_by_remote(5432).unwrap().local_port, 5432);
        assert!(svc.find_port_by_local(9999).is_none());
        assert!(svc.find_port_by_remote(9999).is_none());
    }

    #[test]
    fn enable_all_defaults_and_enabled_features() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_all_defaults();
        let features = svc.enabled_features();
        assert_eq!(features.len(), 3);
        assert!(svc.is_enabled(&RemoteFeature::FileSystem));
        assert!(svc.is_enabled(&RemoteFeature::Terminal));
        assert!(svc.is_enabled(&RemoteFeature::Extensions));
        assert!(!svc.is_enabled(&RemoteFeature::Debugging));
        assert!(!svc.is_enabled(&RemoteFeature::PortForwarding));
    }

    #[test]
    fn forwarded_port_count() {
        let mut svc = RemoteFeaturesService::new();
        assert_eq!(svc.forwarded_port_count(), 0);
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).build());
        assert_eq!(svc.forwarded_port_count(), 2);
        svc.remove_port_forward(3000);
        assert_eq!(svc.forwarded_port_count(), 1);
    }

    #[test]
    fn display_impls() {
        assert_eq!(RemoteFeature::FileSystem.to_string(), "File System");
        assert_eq!(RemoteFeature::Terminal.to_string(), "Terminal");
        assert_eq!(RemoteFeature::Debugging.to_string(), "Debugging");
        assert_eq!(RemoteFeature::Extensions.to_string(), "Extensions");
        assert_eq!(RemoteFeature::PortForwarding.to_string(), "Port Forwarding");
        assert_eq!(PortProtocol::Http.to_string(), "HTTP");
        assert_eq!(PortProtocol::Https.to_string(), "HTTPS");
        assert_eq!(PortProtocol::Tcp.to_string(), "TCP");
    }

    #[test]
    fn port_visibility_display() {
        assert_eq!(PortVisibility::Private.to_string(), "Private");
        assert_eq!(PortVisibility::Organization.to_string(), "Organization");
        assert_eq!(PortVisibility::Public.to_string(), "Public");
    }

    #[test]
    fn port_forward_display() {
        let pf = PortForwardBuilder::new(8080, 80)
            .label("web")
            .protocol(PortProtocol::Http)
            .build();
        let display = pf.to_string();
        assert!(display.contains("web"));
        assert!(display.contains("8080"));
    }

    #[test]
    fn port_forward_equality() {
        let a = PortForwardBuilder::new(8080, 80).protocol(PortProtocol::Http).build();
        let b = PortForwardBuilder::new(8080, 80).protocol(PortProtocol::Http).build();
        let c = PortForwardBuilder::new(9090, 80).protocol(PortProtocol::Http).build();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn connection_info_new() {
        let conn = RemoteConnectionInfo::new("server.local", 22, ConnectionType::Ssh);
        assert_eq!(conn.host, "server.local");
        assert_eq!(conn.port, 22);
        assert!(!conn.connected);
    }

    #[test]
    fn connection_info_with_username() {
        let conn = RemoteConnectionInfo::new("host", 22, ConnectionType::Ssh)
            .with_username("admin");
        assert_eq!(conn.display_address(), "admin@host:22");
    }

    #[test]
    fn connection_info_connect_disconnect() {
        let mut conn = RemoteConnectionInfo::new("host", 22, ConnectionType::Ssh);
        conn.connect();
        assert!(conn.connected);
        conn.disconnect();
        assert!(!conn.connected);
    }

    #[test]
    fn connection_info_display() {
        let mut conn = RemoteConnectionInfo::new("host", 22, ConnectionType::Ssh);
        conn.connect();
        let display = conn.to_string();
        assert!(display.contains("SSH"));
        assert!(display.contains("connected"));
    }

    #[test]
    fn connection_type_display() {
        assert_eq!(ConnectionType::Ssh.to_string(), "SSH");
        assert_eq!(ConnectionType::Wsl.to_string(), "WSL");
        assert_eq!(ConnectionType::Container.to_string(), "Container");
        assert_eq!(ConnectionType::Tunnel.to_string(), "Tunnel");
    }

    #[test]
    fn port_range_basic() {
        let range = PortRange::new(3000, 3010);
        assert!(range.contains(3000));
        assert!(range.contains(3010));
        assert!(range.contains(3005));
        assert!(!range.contains(2999));
        assert!(!range.contains(3011));
        assert_eq!(range.len(), 11);
    }

    #[test]
    fn port_range_reversed_input() {
        let range = PortRange::new(3010, 3000);
        assert_eq!(range.start, 3000);
        assert_eq!(range.end, 3010);
    }

    #[test]
    fn port_range_overlaps() {
        let a = PortRange::new(3000, 3010);
        let b = PortRange::new(3005, 3020);
        let c = PortRange::new(4000, 5000);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn port_range_display() {
        assert_eq!(PortRange::new(3000, 3010).to_string(), "3000-3010");
    }

    #[test]
    fn ports_by_protocol() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).protocol(PortProtocol::Http).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).protocol(PortProtocol::Https).build());
        svc.add_port_forward(PortForwardBuilder::new(5432, 5432).protocol(PortProtocol::Tcp).build());
        assert_eq!(svc.ports_by_protocol(&PortProtocol::Http).len(), 1);
        assert_eq!(svc.ports_by_protocol(&PortProtocol::Https).len(), 1);
    }

    #[test]
    fn is_port_forwarded() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).build());
        assert!(svc.is_port_forwarded(3000));
        assert!(!svc.is_port_forwarded(9999));
    }

    #[test]
    fn ports_in_range() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).build());
        svc.add_port_forward(PortForwardBuilder::new(3005, 3005).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).build());
        let range = PortRange::new(3000, 3010);
        assert_eq!(svc.ports_in_range(&range).len(), 2);
    }

    #[test]
    fn service_summary() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_all_defaults();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).protocol(PortProtocol::Http).auto_forward(true).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).protocol(PortProtocol::Tcp).build());
        let summary = svc.summary();
        assert_eq!(summary.features_enabled, 3);
        assert_eq!(summary.total_ports, 2);
        assert_eq!(summary.http_ports, 1);
        assert_eq!(summary.tcp_ports, 1);
        assert_eq!(summary.auto_forward_count, 1);
    }

    #[test]
    fn service_summary_display() {
        let summary = RemoteServiceSummary {
            features_enabled: 3, total_ports: 5,
            http_ports: 2, https_ports: 1, tcp_ports: 2,
            auto_forward_count: 3,
        };
        let display = summary.to_string();
        assert!(display.contains("3 features"));
        assert!(display.contains("5 ports"));
    }

    #[test]
    fn update_port_label() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).label("old").build());
        assert!(svc.update_port_label(3000, "new"));
        assert_eq!(svc.find_port_by_local(3000).unwrap().label.as_deref(), Some("new"));
        assert!(!svc.update_port_label(9999, "x"));
    }

    #[test]
    fn remove_non_auto_forwards() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).auto_forward(true).build());
        svc.add_port_forward(PortForwardBuilder::new(5000, 5000).auto_forward(false).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).auto_forward(false).build());
        let removed = svc.remove_non_auto_forwards();
        assert_eq!(removed, 2);
        assert_eq!(svc.forwarded_port_count(), 1);
    }

    #[test]
    fn ports_by_visibility() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).visibility(PortVisibility::Public).build());
        svc.add_port_forward(PortForwardBuilder::new(5000, 5000).visibility(PortVisibility::Private).build());
        assert_eq!(svc.ports_by_visibility(&PortVisibility::Public).len(), 1);
        assert_eq!(svc.ports_by_visibility(&PortVisibility::Private).len(), 1);
    }
}
