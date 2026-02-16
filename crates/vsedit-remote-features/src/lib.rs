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

/// Result of a feature capability negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityNegotiationResult {
    pub feature: String,
    pub supported: bool,
    pub reason: Option<String>,
}

/// Negotiate which features are supported given a set of required features.
pub fn negotiate_capabilities(
    service: &RemoteFeaturesService,
    required: &[RemoteFeature],
) -> Vec<CapabilityNegotiationResult> {
    required
        .iter()
        .map(|f| {
            let supported = service.is_enabled(f);
            CapabilityNegotiationResult {
                feature: f.to_string(),
                supported,
                reason: if supported { None } else { Some("not enabled".to_string()) },
            }
        })
        .collect()
}

/// Checks if a client version is compatible with a minimum required version.
pub fn is_version_compatible(client_version: (u32, u32, u32), min_version: (u32, u32, u32)) -> bool {
    client_version >= min_version
}

/// A node in a feature dependency graph.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureDependency {
    pub feature: RemoteFeature,
    pub depends_on: Vec<RemoteFeature>,
}

/// Checks if all dependencies of a feature are enabled.
pub fn check_feature_dependencies(
    service: &RemoteFeaturesService,
    dep: &FeatureDependency,
) -> bool {
    dep.depends_on.iter().all(|d| service.is_enabled(d))
}

/// Tracks activation state of features with timestamps.
#[derive(Debug, Clone)]
pub struct FeatureActivationRecord {
    pub feature: RemoteFeature,
    pub activated: bool,
    pub activation_order: u32,
}

/// Tracks the order in which features were activated.
#[derive(Debug, Clone, Default)]
pub struct FeatureActivationTracker {
    records: Vec<FeatureActivationRecord>,
    next_order: u32,
}

impl FeatureActivationTracker {
    pub fn new() -> Self {
        Self { records: Vec::new(), next_order: 0 }
    }

    pub fn activate(&mut self, feature: RemoteFeature) {
        if !self.records.iter().any(|r| r.feature == feature && r.activated) {
            self.records.push(FeatureActivationRecord {
                feature,
                activated: true,
                activation_order: self.next_order,
            });
            self.next_order += 1;
        }
    }

    pub fn deactivate(&mut self, feature: &RemoteFeature) {
        if let Some(r) = self.records.iter_mut().find(|r| &r.feature == feature && r.activated) {
            r.activated = false;
        }
    }

    pub fn is_active(&self, feature: &RemoteFeature) -> bool {
        self.records.iter().any(|r| &r.feature == feature && r.activated)
    }

    pub fn active_count(&self) -> usize {
        self.records.iter().filter(|r| r.activated).count()
    }

    pub fn activation_history(&self) -> &[FeatureActivationRecord] {
        &self.records
    }
}

// ---------------------------------------------------------------------------
// Remote connection health
// ---------------------------------------------------------------------------

/// Tracks the health of a remote connection.
#[derive(Debug, Clone)]
pub struct RemoteConnectionHealth {
    /// Round-trip latency in milliseconds of the last successful ping.
    pub latency_ms: u64,
    /// Monotonic timestamp (arbitrary epoch) of the last ping attempt.
    pub last_ping_time: u64,
    /// Number of consecutive failed pings.
    pub consecutive_failures: u32,
    /// Maximum allowed consecutive failures before unhealthy.
    pub max_failures: u32,
}

impl RemoteConnectionHealth {
    pub fn new(max_failures: u32) -> Self {
        Self {
            latency_ms: 0,
            last_ping_time: 0,
            consecutive_failures: 0,
            max_failures,
        }
    }

    /// Returns `true` if the connection is considered healthy.
    pub fn is_healthy(&self) -> bool {
        self.consecutive_failures < self.max_failures
    }

    /// Record a successful ping with the measured latency.
    pub fn record_success(&mut self, latency_ms: u64, time: u64) {
        self.latency_ms = latency_ms;
        self.last_ping_time = time;
        self.consecutive_failures = 0;
    }

    /// Record a failed ping.
    pub fn record_failure(&mut self, time: u64) {
        self.last_ping_time = time;
        self.consecutive_failures += 1;
    }

    /// Reset the health tracker to its initial state.
    pub fn reset(&mut self) {
        self.latency_ms = 0;
        self.last_ping_time = 0;
        self.consecutive_failures = 0;
    }
}

/// Runtime environment for containers.
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerRuntime {
    Docker,
    Podman,
    Devcontainer,
}

impl fmt::Display for ContainerRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Docker => write!(f, "docker"),
            Self::Podman => write!(f, "podman"),
            Self::Devcontainer => write!(f, "devcontainer"),
        }
    }
}

/// Describes a remote connection target.
#[derive(Debug, Clone, PartialEq)]
pub enum RemoteConnection {
    Ssh {
        host: String,
        port: u16,
        user: String,
        identity_file: Option<String>,
    },
    Tunnel {
        tunnel_id: String,
        endpoint: String,
    },
    Container {
        container_id: String,
        container_name: String,
        runtime: ContainerRuntime,
    },
}

impl RemoteConnection {
    /// Human-readable name for this connection.
    pub fn display_name(&self) -> String {
        match self {
            Self::Ssh { host, user, .. } => format!("{user}@{host}"),
            Self::Tunnel { tunnel_id, .. } => format!("tunnel:{tunnel_id}"),
            Self::Container {
                container_name,
                runtime,
                ..
            } => format!("{runtime}:{container_name}"),
        }
    }

    /// Connection string suitable for establishing the connection.
    pub fn connection_string(&self) -> String {
        match self {
            Self::Ssh {
                host,
                port,
                user,
                identity_file,
            } => match identity_file {
                Some(key) => format!("ssh -i {key} -p {port} {user}@{host}"),
                None => format!("ssh -p {port} {user}@{host}"),
            },
            Self::Tunnel {
                tunnel_id,
                endpoint,
            } => format!("tunnel://{tunnel_id}@{endpoint}"),
            Self::Container {
                container_id,
                runtime,
                ..
            } => format!("{runtime}://{container_id}"),
        }
    }

    pub fn is_ssh(&self) -> bool {
        matches!(self, Self::Ssh { .. })
    }

    pub fn is_tunnel(&self) -> bool {
        matches!(self, Self::Tunnel { .. })
    }

    pub fn is_container(&self) -> bool {
        matches!(self, Self::Container { .. })
    }
}

impl fmt::Display for RemoteConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// An entry in a remote file system listing.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteFsEntry {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_epoch: u64,
}

/// Provides file-system operations over a remote connection.
#[derive(Debug, Clone)]
pub struct RemoteFileSystem {
    #[allow(dead_code)]
    connection_id: String,
    #[allow(dead_code)]
    root_path: String,
    entries: Vec<RemoteFsEntry>,
}

impl RemoteFileSystem {
    pub fn new(connection_id: impl Into<String>, root_path: impl Into<String>) -> Self {
        Self {
            connection_id: connection_id.into(),
            root_path: root_path.into(),
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: RemoteFsEntry) {
        self.entries.push(entry);
    }

    /// List entries whose path starts with the given directory prefix.
    pub fn list_dir(&self, path: &str) -> Vec<&RemoteFsEntry> {
        let prefix = if path.ends_with('/') {
            path.to_string()
        } else {
            format!("{path}/")
        };
        self.entries
            .iter()
            .filter(|e| e.path.starts_with(&prefix) && e.path != prefix)
            .filter(|e| {
                // Only direct children: no additional '/' after the prefix.
                let rest = &e.path[prefix.len()..];
                !rest.contains('/')
            })
            .collect()
    }

    pub fn find_entry(&self, path: &str) -> Option<&RemoteFsEntry> {
        self.entries.iter().find(|e| e.path == path)
    }

    pub fn total_size(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Remove the first entry matching `path`. Returns `true` if found.
    pub fn remove_entry(&mut self, path: &str) -> bool {
        if let Some(idx) = self.entries.iter().position(|e| e.path == path) {
            self.entries.remove(idx);
            true
        } else {
            false
        }
    }
}

/// Status of a tracked port forward.
#[derive(Debug, Clone, PartialEq)]
pub enum PortForwardStatus {
    Active,
    Inactive,
    Error(String),
}

/// A port forward together with its runtime status.
#[derive(Debug, Clone)]
pub struct TrackedPortForward {
    pub forward: PortForward,
    pub status: PortForwardStatus,
    pub bytes_transferred: u64,
}

/// Tracks active port forwards and their transfer statistics.
#[derive(Debug, Clone)]
pub struct RemotePortForwardTracker {
    forwards: Vec<TrackedPortForward>,
}

impl RemotePortForwardTracker {
    pub fn new() -> Self {
        Self {
            forwards: Vec::new(),
        }
    }

    pub fn add(&mut self, forward: PortForward) {
        self.forwards.push(TrackedPortForward {
            forward,
            status: PortForwardStatus::Active,
            bytes_transferred: 0,
        });
    }

    pub fn set_status(&mut self, local_port: u16, status: PortForwardStatus) {
        if let Some(t) = self
            .forwards
            .iter_mut()
            .find(|t| t.forward.local_port == local_port)
        {
            t.status = status;
        }
    }

    pub fn active_count(&self) -> usize {
        self.forwards
            .iter()
            .filter(|t| t.status == PortForwardStatus::Active)
            .count()
    }

    pub fn total_bytes(&self) -> u64 {
        self.forwards.iter().map(|t| t.bytes_transferred).sum()
    }

    pub fn get_by_port(&self, local_port: u16) -> Option<&TrackedPortForward> {
        self.forwards
            .iter()
            .find(|t| t.forward.local_port == local_port)
    }

    pub fn record_bytes(&mut self, local_port: u16, bytes: u64) {
        if let Some(t) = self
            .forwards
            .iter_mut()
            .find(|t| t.forward.local_port == local_port)
        {
            t.bytes_transferred += bytes;
        }
    }
}

impl Default for RemotePortForwardTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Feature capability matrix
// ---------------------------------------------------------------------------

/// Describes the capability level of a remote feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityLevel {
    /// Feature is fully supported.
    Full,
    /// Feature works but with reduced functionality.
    Partial,
    /// Feature is not available.
    None,
}

impl fmt::Display for CapabilityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "Full"),
            Self::Partial => write!(f, "Partial"),
            Self::None => write!(f, "None"),
        }
    }
}

/// A matrix describing the capability level of each feature for a given
/// connection type.
#[derive(Debug, Clone)]
pub struct CapabilityMatrix {
    entries: Vec<(ConnectionType, RemoteFeature, CapabilityLevel)>,
}

impl CapabilityMatrix {
    /// Create a new empty capability matrix.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Create a default matrix with sensible defaults for each connection type.
    pub fn with_defaults() -> Self {
        let mut m = Self::new();
        // SSH supports everything
        for feat in &[
            RemoteFeature::FileSystem,
            RemoteFeature::Terminal,
            RemoteFeature::Debugging,
            RemoteFeature::Extensions,
            RemoteFeature::PortForwarding,
        ] {
            m.set(ConnectionType::Ssh, feat.clone(), CapabilityLevel::Full);
        }
        // WSL: full except port forwarding is partial
        for feat in &[
            RemoteFeature::FileSystem,
            RemoteFeature::Terminal,
            RemoteFeature::Debugging,
            RemoteFeature::Extensions,
        ] {
            m.set(ConnectionType::Wsl, feat.clone(), CapabilityLevel::Full);
        }
        m.set(ConnectionType::Wsl, RemoteFeature::PortForwarding, CapabilityLevel::Partial);
        // Container: no debugging, partial extensions
        m.set(ConnectionType::Container, RemoteFeature::FileSystem, CapabilityLevel::Full);
        m.set(ConnectionType::Container, RemoteFeature::Terminal, CapabilityLevel::Full);
        m.set(ConnectionType::Container, RemoteFeature::Debugging, CapabilityLevel::None);
        m.set(ConnectionType::Container, RemoteFeature::Extensions, CapabilityLevel::Partial);
        m.set(ConnectionType::Container, RemoteFeature::PortForwarding, CapabilityLevel::Full);
        // Tunnel: partial debugging, partial extensions
        m.set(ConnectionType::Tunnel, RemoteFeature::FileSystem, CapabilityLevel::Full);
        m.set(ConnectionType::Tunnel, RemoteFeature::Terminal, CapabilityLevel::Full);
        m.set(ConnectionType::Tunnel, RemoteFeature::Debugging, CapabilityLevel::Partial);
        m.set(ConnectionType::Tunnel, RemoteFeature::Extensions, CapabilityLevel::Partial);
        m.set(ConnectionType::Tunnel, RemoteFeature::PortForwarding, CapabilityLevel::Full);
        m
    }

    /// Set the capability level for a (connection_type, feature) pair.
    pub fn set(&mut self, conn: ConnectionType, feature: RemoteFeature, level: CapabilityLevel) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|(c, f, _)| *c == conn && *f == feature)
        {
            entry.2 = level;
        } else {
            self.entries.push((conn, feature, level));
        }
    }

    /// Look up the capability level for a specific connection type and feature.
    pub fn get(&self, conn: &ConnectionType, feature: &RemoteFeature) -> CapabilityLevel {
        self.entries
            .iter()
            .find(|(c, f, _)| c == conn && f == feature)
            .map(|(_, _, l)| *l)
            .unwrap_or(CapabilityLevel::None)
    }

    /// Return all features that have at least `Partial` support for a
    /// connection type.
    pub fn supported_features(&self, conn: &ConnectionType) -> Vec<&RemoteFeature> {
        self.entries
            .iter()
            .filter(|(c, _, l)| c == conn && *l != CapabilityLevel::None)
            .map(|(_, f, _)| f)
            .collect()
    }

    /// Return all features with `Full` support for a connection type.
    pub fn fully_supported_features(&self, conn: &ConnectionType) -> Vec<&RemoteFeature> {
        self.entries
            .iter()
            .filter(|(c, _, l)| c == conn && *l == CapabilityLevel::Full)
            .map(|(_, f, _)| f)
            .collect()
    }

    /// Total number of entries in the matrix.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for CapabilityMatrix {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Feature version requirements
// ---------------------------------------------------------------------------

/// Maps a feature to the minimum server version required to use it.
#[derive(Debug, Clone)]
pub struct FeatureVersionRequirement {
    pub feature: RemoteFeature,
    pub min_version: (u32, u32, u32),
}

/// Check which features from `requirements` are available given
/// `server_version`.
pub fn available_features_for_version(
    server_version: (u32, u32, u32),
    requirements: &[FeatureVersionRequirement],
) -> Vec<RemoteFeature> {
    requirements
        .iter()
        .filter(|r| is_version_compatible(server_version, r.min_version))
        .map(|r| r.feature.clone())
        .collect()
}

/// Return features that are NOT available for a given server version.
pub fn unavailable_features_for_version(
    server_version: (u32, u32, u32),
    requirements: &[FeatureVersionRequirement],
) -> Vec<(RemoteFeature, (u32, u32, u32))> {
    requirements
        .iter()
        .filter(|r| !is_version_compatible(server_version, r.min_version))
        .map(|r| (r.feature.clone(), r.min_version))
        .collect()
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

    #[test]
    fn capability_negotiation_supported() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_feature(RemoteFeature::Terminal);
        svc.enable_feature(RemoteFeature::FileSystem);
        let results = negotiate_capabilities(&svc, &[RemoteFeature::Terminal, RemoteFeature::Debugging]);
        assert!(results[0].supported);
        assert!(!results[1].supported);
        assert!(results[1].reason.is_some());
    }

    #[test]
    fn version_compatibility_check() {
        assert!(is_version_compatible((1, 2, 3), (1, 2, 0)));
        assert!(is_version_compatible((2, 0, 0), (1, 9, 9)));
        assert!(!is_version_compatible((1, 0, 0), (1, 0, 1)));
        assert!(is_version_compatible((1, 0, 0), (1, 0, 0)));
    }

    #[test]
    fn feature_dependency_check() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_feature(RemoteFeature::FileSystem);
        svc.enable_feature(RemoteFeature::Terminal);
        let dep = FeatureDependency {
            feature: RemoteFeature::Debugging,
            depends_on: vec![RemoteFeature::FileSystem, RemoteFeature::Terminal],
        };
        assert!(check_feature_dependencies(&svc, &dep));
        let dep2 = FeatureDependency {
            feature: RemoteFeature::Debugging,
            depends_on: vec![RemoteFeature::Extensions],
        };
        assert!(!check_feature_dependencies(&svc, &dep2));
    }

    #[test]
    fn activation_tracker_lifecycle() {
        let mut tracker = FeatureActivationTracker::new();
        assert_eq!(tracker.active_count(), 0);
        tracker.activate(RemoteFeature::Terminal);
        tracker.activate(RemoteFeature::FileSystem);
        assert_eq!(tracker.active_count(), 2);
        assert!(tracker.is_active(&RemoteFeature::Terminal));
        tracker.deactivate(&RemoteFeature::Terminal);
        assert!(!tracker.is_active(&RemoteFeature::Terminal));
        assert_eq!(tracker.active_count(), 1);
    }

    #[test]
    fn activation_tracker_no_double_activate() {
        let mut tracker = FeatureActivationTracker::new();
        tracker.activate(RemoteFeature::Terminal);
        tracker.activate(RemoteFeature::Terminal);
        assert_eq!(tracker.active_count(), 1);
        assert_eq!(tracker.activation_history().len(), 1);
    }

    // -- RemoteConnectionHealth tests ------------------------------------

    #[test]
    fn health_initially_healthy() {
        let h = RemoteConnectionHealth::new(3);
        assert!(h.is_healthy());
        assert_eq!(h.latency_ms, 0);
    }

    #[test]
    fn health_becomes_unhealthy_after_max_failures() {
        let mut h = RemoteConnectionHealth::new(2);
        h.record_failure(1);
        assert!(h.is_healthy());
        h.record_failure(2);
        assert!(!h.is_healthy());
    }

    #[test]
    fn health_success_resets_failures() {
        let mut h = RemoteConnectionHealth::new(3);
        h.record_failure(1);
        h.record_failure(2);
        h.record_success(50, 3);
        assert!(h.is_healthy());
        assert_eq!(h.latency_ms, 50);
        assert_eq!(h.consecutive_failures, 0);
    }

    #[test]
    fn health_reset() {
        let mut h = RemoteConnectionHealth::new(3);
        h.record_failure(1);
        h.record_success(100, 2);
        h.reset();
        assert_eq!(h.latency_ms, 0);
        assert_eq!(h.consecutive_failures, 0);
    }

    #[test]
    fn health_tracks_last_ping_time() {
        let mut h = RemoteConnectionHealth::new(5);
        h.record_success(10, 42);
        assert_eq!(h.last_ping_time, 42);
        h.record_failure(99);
        assert_eq!(h.last_ping_time, 99);
    }

    // ── RemoteConnection ──────────────────────────────────────────

    #[test]
    fn remote_connection_ssh_display_and_string() {
        let conn = RemoteConnection::Ssh {
            host: "dev.example.com".into(),
            port: 22,
            user: "alice".into(),
            identity_file: Some("/home/alice/.ssh/id_ed25519".into()),
        };
        assert_eq!(conn.display_name(), "alice@dev.example.com");
        assert_eq!(
            conn.connection_string(),
            "ssh -i /home/alice/.ssh/id_ed25519 -p 22 alice@dev.example.com"
        );
        assert!(conn.is_ssh());
        assert!(!conn.is_tunnel());
        assert!(!conn.is_container());
        assert_eq!(format!("{conn}"), "alice@dev.example.com");
    }

    #[test]
    fn remote_connection_tunnel_variant() {
        let conn = RemoteConnection::Tunnel {
            tunnel_id: "t-42".into(),
            endpoint: "relay.example.com:443".into(),
        };
        assert_eq!(conn.display_name(), "tunnel:t-42");
        assert_eq!(
            conn.connection_string(),
            "tunnel://t-42@relay.example.com:443"
        );
        assert!(conn.is_tunnel());
        assert!(!conn.is_ssh());
    }

    #[test]
    fn remote_connection_container_variant() {
        let conn = RemoteConnection::Container {
            container_id: "abc123".into(),
            container_name: "my-app".into(),
            runtime: ContainerRuntime::Podman,
        };
        assert_eq!(conn.display_name(), "podman:my-app");
        assert_eq!(conn.connection_string(), "podman://abc123");
        assert!(conn.is_container());
    }

    // ── RemoteFileSystem ──────────────────────────────────────────

    #[test]
    fn remote_fs_add_find_remove() {
        let mut fs = RemoteFileSystem::new("conn-1", "/workspace");
        fs.add_entry(RemoteFsEntry {
            path: "/workspace/src/main.rs".into(),
            is_dir: false,
            size: 1024,
            modified_epoch: 1_700_000_000,
        });
        fs.add_entry(RemoteFsEntry {
            path: "/workspace/src/lib.rs".into(),
            is_dir: false,
            size: 2048,
            modified_epoch: 1_700_000_100,
        });

        assert_eq!(fs.entry_count(), 2);
        assert_eq!(fs.total_size(), 3072);
        assert!(fs.find_entry("/workspace/src/main.rs").is_some());
        assert!(fs.find_entry("/workspace/missing.rs").is_none());

        assert!(fs.remove_entry("/workspace/src/main.rs"));
        assert_eq!(fs.entry_count(), 1);
        assert!(!fs.remove_entry("/workspace/src/main.rs"));
    }

    #[test]
    fn remote_fs_list_dir() {
        let mut fs = RemoteFileSystem::new("conn-2", "/project");
        fs.add_entry(RemoteFsEntry {
            path: "/project/src/a.rs".into(),
            is_dir: false,
            size: 100,
            modified_epoch: 0,
        });
        fs.add_entry(RemoteFsEntry {
            path: "/project/src/b.rs".into(),
            is_dir: false,
            size: 200,
            modified_epoch: 0,
        });
        fs.add_entry(RemoteFsEntry {
            path: "/project/src/sub/c.rs".into(),
            is_dir: false,
            size: 50,
            modified_epoch: 0,
        });
        fs.add_entry(RemoteFsEntry {
            path: "/project/README.md".into(),
            is_dir: false,
            size: 10,
            modified_epoch: 0,
        });

        let src_entries = fs.list_dir("/project/src");
        assert_eq!(src_entries.len(), 2); // a.rs, b.rs — not sub/c.rs
    }

    // ── RemotePortForwardTracker ──────────────────────────────────

    #[test]
    fn tracker_add_and_count() {
        let mut tracker = RemotePortForwardTracker::new();
        tracker.add(PortForwardBuilder::new(8080, 80).build());
        tracker.add(PortForwardBuilder::new(3000, 3000).build());
        assert_eq!(tracker.active_count(), 2);
    }

    #[test]
    fn tracker_set_status_and_bytes() {
        let mut tracker = RemotePortForwardTracker::new();
        tracker.add(PortForwardBuilder::new(8080, 80).build());
        tracker.add(PortForwardBuilder::new(3000, 3000).build());

        tracker.set_status(8080, PortForwardStatus::Inactive);
        assert_eq!(tracker.active_count(), 1);

        tracker.record_bytes(3000, 500);
        tracker.record_bytes(3000, 250);
        assert_eq!(tracker.total_bytes(), 750);

        let t = tracker.get_by_port(3000).unwrap();
        assert_eq!(t.bytes_transferred, 750);
        assert_eq!(t.status, PortForwardStatus::Active);
    }

    #[test]
    fn tracker_error_status() {
        let mut tracker = RemotePortForwardTracker::new();
        tracker.add(PortForwardBuilder::new(4000, 4000).build());
        tracker.set_status(4000, PortForwardStatus::Error("timeout".into()));
        assert_eq!(tracker.active_count(), 0);
        let t = tracker.get_by_port(4000).unwrap();
        assert_eq!(t.status, PortForwardStatus::Error("timeout".into()));
    }

    // ── CapabilityMatrix ──────────────────────────────────────────

    #[test]
    fn capability_matrix_defaults_ssh_full() {
        let m = CapabilityMatrix::with_defaults();
        assert_eq!(
            m.get(&ConnectionType::Ssh, &RemoteFeature::FileSystem),
            CapabilityLevel::Full
        );
        assert_eq!(
            m.get(&ConnectionType::Ssh, &RemoteFeature::Debugging),
            CapabilityLevel::Full
        );
    }

    #[test]
    fn capability_matrix_container_no_debugging() {
        let m = CapabilityMatrix::with_defaults();
        assert_eq!(
            m.get(&ConnectionType::Container, &RemoteFeature::Debugging),
            CapabilityLevel::None
        );
    }

    #[test]
    fn capability_matrix_supported_features() {
        let m = CapabilityMatrix::with_defaults();
        let supported = m.supported_features(&ConnectionType::Container);
        // Container has 4 supported (FileSystem, Terminal, Extensions partial, PortForwarding)
        assert_eq!(supported.len(), 4);
        let full = m.fully_supported_features(&ConnectionType::Container);
        // Full: FileSystem, Terminal, PortForwarding
        assert_eq!(full.len(), 3);
    }

    #[test]
    fn capability_matrix_set_override() {
        let mut m = CapabilityMatrix::new();
        m.set(ConnectionType::Ssh, RemoteFeature::Terminal, CapabilityLevel::None);
        assert_eq!(
            m.get(&ConnectionType::Ssh, &RemoteFeature::Terminal),
            CapabilityLevel::None
        );
        m.set(ConnectionType::Ssh, RemoteFeature::Terminal, CapabilityLevel::Full);
        assert_eq!(
            m.get(&ConnectionType::Ssh, &RemoteFeature::Terminal),
            CapabilityLevel::Full
        );
        assert_eq!(m.entry_count(), 1);
    }

    #[test]
    fn capability_matrix_unknown_returns_none() {
        let m = CapabilityMatrix::new();
        assert_eq!(
            m.get(&ConnectionType::Wsl, &RemoteFeature::Extensions),
            CapabilityLevel::None
        );
    }

    // ── Feature version requirements ──────────────────────────────

    #[test]
    fn available_features_filters_by_version() {
        let reqs = vec![
            FeatureVersionRequirement {
                feature: RemoteFeature::FileSystem,
                min_version: (1, 0, 0),
            },
            FeatureVersionRequirement {
                feature: RemoteFeature::Debugging,
                min_version: (2, 0, 0),
            },
            FeatureVersionRequirement {
                feature: RemoteFeature::PortForwarding,
                min_version: (1, 5, 0),
            },
        ];
        let available = available_features_for_version((1, 5, 0), &reqs);
        assert_eq!(available.len(), 2);
        assert!(available.contains(&RemoteFeature::FileSystem));
        assert!(available.contains(&RemoteFeature::PortForwarding));
        assert!(!available.contains(&RemoteFeature::Debugging));
    }

    #[test]
    fn unavailable_features_returns_missing() {
        let reqs = vec![
            FeatureVersionRequirement {
                feature: RemoteFeature::Terminal,
                min_version: (3, 0, 0),
            },
        ];
        let unavailable = unavailable_features_for_version((2, 9, 9), &reqs);
        assert_eq!(unavailable.len(), 1);
        assert_eq!(unavailable[0].0, RemoteFeature::Terminal);
        assert_eq!(unavailable[0].1, (3, 0, 0));
    }

    #[test]
    fn capability_level_display() {
        assert_eq!(format!("{}", CapabilityLevel::Full), "Full");
        assert_eq!(format!("{}", CapabilityLevel::Partial), "Partial");
        assert_eq!(format!("{}", CapabilityLevel::None), "None");
    }
}
