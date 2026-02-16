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
}
