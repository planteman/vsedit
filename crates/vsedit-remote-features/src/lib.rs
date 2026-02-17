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

// ── Port-forwarding utilities ───────────────────────────────────────────

/// Deduplicate port forwards by local port, keeping the first occurrence.
pub fn dedup_port_forwards(forwards: &[PortForward]) -> Vec<PortForward> {
    let mut seen = std::collections::HashSet::new();
    forwards
        .iter()
        .filter(|f| seen.insert(f.local_port))
        .cloned()
        .collect()
}

/// Sort port forwards by local port ascending.
pub fn sort_forwards_by_local_port(forwards: &mut [PortForward]) {
    forwards.sort_by_key(|f| f.local_port);
}

/// Return all port forwards whose local port falls within the given range.
pub fn forwards_in_port_range(forwards: &[PortForward], range: &PortRange) -> Vec<PortForward> {
    forwards
        .iter()
        .filter(|f| range.contains(f.local_port))
        .cloned()
        .collect()
}

/// Compute a summary string for a list of port forwards.
pub fn port_forwards_summary(forwards: &[PortForward]) -> String {
    if forwards.is_empty() {
        return "No active port forwards".to_string();
    }
    let auto_count = forwards.iter().filter(|f| f.auto_forward).count();
    format!(
        "{} port forward(s), {} auto-forwarded",
        forwards.len(),
        auto_count
    )
}

/// Check whether any port forward conflicts with a given local port.
pub fn has_local_port_conflict(forwards: &[PortForward], port: u16) -> bool {
    forwards.iter().any(|f| f.local_port == port)
}

/// Collect all unique protocols in use across forwarded ports.
pub fn unique_protocols(forwards: &[PortForward]) -> Vec<PortProtocol> {
    let mut protos = Vec::new();
    for f in forwards {
        if !protos.contains(&f.protocol) {
            protos.push(f.protocol.clone());
        }
    }
    protos
}

/// Partition port forwards into auto-forwarded and manual groups.
pub fn partition_forwards(forwards: &[PortForward]) -> (Vec<PortForward>, Vec<PortForward>) {
    let mut auto = Vec::new();
    let mut manual = Vec::new();
    for f in forwards {
        if f.auto_forward {
            auto.push(f.clone());
        } else {
            manual.push(f.clone());
        }
    }
    (auto, manual)
}

// ---------------------------------------------------------------------------
// Remote-features analysis utilities
// ---------------------------------------------------------------------------

/// Return the set of features that are currently enabled.
pub fn enabled_features(svc: &RemoteFeaturesService) -> Vec<RemoteFeature> {
    let all = [
        RemoteFeature::FileSystem,
        RemoteFeature::Terminal,
        RemoteFeature::Debugging,
        RemoteFeature::Extensions,
        RemoteFeature::PortForwarding,
    ];
    all.iter()
        .filter(|f| svc.is_enabled(f))
        .cloned()
        .collect()
}

/// Count ports by protocol.
pub fn count_by_protocol(ports: &[PortForward]) -> (usize, usize, usize) {
    let http = ports.iter().filter(|p| p.protocol == PortProtocol::Http).count();
    let https = ports.iter().filter(|p| p.protocol == PortProtocol::Https).count();
    let tcp = ports.iter().filter(|p| p.protocol == PortProtocol::Tcp).count();
    (http, https, tcp)
}

/// Check whether a given local port is already in use by any forwarded port.
pub fn is_port_in_use(ports: &[PortForward], local_port: u16) -> bool {
    ports.iter().any(|p| p.local_port == local_port)
}

/// Find an available local port starting from `start` that is not already
/// forwarded. Returns `None` if no port below 65535 is available.
pub fn find_available_port(ports: &[PortForward], start: u16) -> Option<u16> {
    (start..=u16::MAX).find(|&p| !is_port_in_use(ports, p))
}

/// Summarise the remote features service state.
pub fn summarise_service(svc: &RemoteFeaturesService) -> RemoteServiceSummary {
    let ports = svc.get_forwarded_ports();
    let (http, https, tcp) = count_by_protocol(ports);
    let auto_forward_count = ports.iter().filter(|p| p.auto_forward).count();
    RemoteServiceSummary {
        features_enabled: enabled_features(svc).len(),
        total_ports: ports.len(),
        http_ports: http,
        https_ports: https,
        tcp_ports: tcp,
        auto_forward_count,
    }
}

/// Return the distinct set of remote port numbers that are being forwarded.
pub fn remote_port_set(ports: &[PortForward]) -> Vec<u16> {
    let mut set: Vec<u16> = ports.iter().map(|p| p.remote_port).collect();
    set.sort_unstable();
    set.dedup();
    set
}

/// Check whether all features in `required` are enabled.
pub fn all_features_enabled(svc: &RemoteFeaturesService, required: &[RemoteFeature]) -> bool {
    required.iter().all(|f| svc.is_enabled(f))
}

// ---------------------------------------------------------------------------
// RemoteExecProxy – proxy for running commands on a remote machine
// ---------------------------------------------------------------------------

/// Proxy for executing commands on a remote host.
#[derive(Debug, Clone)]
pub struct RemoteExecProxy {
    pub host: String,
    pub working_directory: Option<String>,
    pub environment: Vec<(String, String)>,
    pub timeout_ms: u64,
}

impl RemoteExecProxy {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            working_directory: None,
            environment: Vec::new(),
            timeout_ms: 30_000,
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.working_directory = Some(cwd.into());
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn add_env(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.environment.push((key.into(), value.into()));
    }

    /// Build the command representation that would be sent to the remote host.
    pub fn build_command(&self, program: &str, args: &[&str]) -> RemoteCommand {
        RemoteCommand {
            host: self.host.clone(),
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            cwd: self.working_directory.clone(),
            env: self.environment.clone(),
            timeout_ms: self.timeout_ms,
        }
    }
}

impl fmt::Display for RemoteExecProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RemoteExecProxy({})", self.host)
    }
}

/// Represents a command to be executed remotely.
#[derive(Debug, Clone)]
pub struct RemoteCommand {
    pub host: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub timeout_ms: u64,
}

impl RemoteCommand {
    /// Build the full command string.
    pub fn command_line(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

impl fmt::Display for RemoteCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}: {}", self.program, self.host, self.command_line())
    }
}

// ---------------------------------------------------------------------------
// RemoteExtensionInstaller
// ---------------------------------------------------------------------------

/// Manages extension installation on remote hosts.
#[derive(Debug, Clone)]
pub struct RemoteExtensionInstaller {
    installed: Vec<RemoteExtension>,
}

/// An extension installed on a remote host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExtension {
    pub id: String,
    pub version: String,
    pub enabled: bool,
}

impl RemoteExtensionInstaller {
    pub fn new() -> Self {
        Self { installed: Vec::new() }
    }

    pub fn install(&mut self, id: impl Into<String>, version: impl Into<String>) {
        let ext = RemoteExtension {
            id: id.into(),
            version: version.into(),
            enabled: true,
        };
        // Replace existing version
        self.installed.retain(|e| e.id != ext.id);
        self.installed.push(ext);
    }

    pub fn uninstall(&mut self, id: &str) -> bool {
        let before = self.installed.len();
        self.installed.retain(|e| e.id != id);
        self.installed.len() < before
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.iter().any(|e| e.id == id)
    }

    pub fn get_version(&self, id: &str) -> Option<&str> {
        self.installed.iter().find(|e| e.id == id).map(|e| e.version.as_str())
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(ext) = self.installed.iter_mut().find(|e| e.id == id) {
            ext.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn installed_count(&self) -> usize {
        self.installed.len()
    }

    pub fn enabled_count(&self) -> usize {
        self.installed.iter().filter(|e| e.enabled).count()
    }
}

impl Default for RemoteExtensionInstaller {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RemoteExtensionInstaller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RemoteExtensionInstaller({} installed)", self.installed.len())
    }
}

// ---------------------------------------------------------------------------
// RemotePortScanner – scans for available ports
// ---------------------------------------------------------------------------

/// Scans a port range for available (unused) ports.
#[derive(Debug, Clone)]
pub struct RemotePortScanner {
    pub range: PortRange,
    pub used_ports: Vec<u16>,
}

impl RemotePortScanner {
    pub fn new(range: PortRange) -> Self {
        Self { range, used_ports: Vec::new() }
    }

    /// Mark a port as used.
    pub fn mark_used(&mut self, port: u16) {
        if !self.used_ports.contains(&port) {
            self.used_ports.push(port);
        }
    }

    /// Find the first available port in the range.
    pub fn find_available(&self) -> Option<u16> {
        (self.range.start..=self.range.end)
            .find(|p| !self.used_ports.contains(p))
    }

    /// Find N available ports.
    pub fn find_n_available(&self, n: usize) -> Vec<u16> {
        (self.range.start..=self.range.end)
            .filter(|p| !self.used_ports.contains(p))
            .take(n)
            .collect()
    }

    /// Number of available ports in the range.
    pub fn available_count(&self) -> usize {
        (self.range.start..=self.range.end)
            .filter(|p| !self.used_ports.contains(p))
            .count()
    }

    /// Whether a specific port is available.
    pub fn is_available(&self, port: u16) -> bool {
        self.range.contains(port) && !self.used_ports.contains(&port)
    }
}

impl fmt::Display for RemotePortScanner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PortScanner({}-{}, {} used)",
            self.range.start, self.range.end, self.used_ports.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Remote connection diagnostics
// ---------------------------------------------------------------------------

/// Diagnostic check result for remote connections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticStatus {
    Ok,
    Warning(String),
    Error(String),
}

impl fmt::Display for DiagnosticStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Warning(msg) => write!(f, "Warning: {msg}"),
            Self::Error(msg) => write!(f, "Error: {msg}"),
        }
    }
}

/// A single diagnostic check for remote connection health.
#[derive(Debug, Clone)]
pub struct RemoteDiagnosticCheck {
    pub name: String,
    pub status: DiagnosticStatus,
}

/// Runs a series of diagnostic checks on a remote connection.
#[derive(Debug, Clone)]
pub struct RemoteConnectionDiagnostics {
    checks: Vec<RemoteDiagnosticCheck>,
}

impl RemoteConnectionDiagnostics {
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    pub fn add_check(&mut self, name: impl Into<String>, status: DiagnosticStatus) {
        self.checks.push(RemoteDiagnosticCheck {
            name: name.into(),
            status,
        });
    }

    /// Whether all checks passed.
    pub fn all_ok(&self) -> bool {
        self.checks.iter().all(|c| c.status == DiagnosticStatus::Ok)
    }

    /// Count of errors.
    pub fn error_count(&self) -> usize {
        self.checks.iter().filter(|c| matches!(c.status, DiagnosticStatus::Error(_))).count()
    }

    /// Count of warnings.
    pub fn warning_count(&self) -> usize {
        self.checks.iter().filter(|c| matches!(c.status, DiagnosticStatus::Warning(_))).count()
    }

    pub fn check_count(&self) -> usize {
        self.checks.len()
    }

    /// Get a summary string.
    pub fn summary(&self) -> String {
        format!(
            "{} checks: {} ok, {} warnings, {} errors",
            self.checks.len(),
            self.checks.iter().filter(|c| c.status == DiagnosticStatus::Ok).count(),
            self.warning_count(),
            self.error_count()
        )
    }
}

impl Default for RemoteConnectionDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RemoteConnectionDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// RemoteFilesystemCacheManager
// ---------------------------------------------------------------------------

/// A cached filesystem entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFsEntry {
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_ts: u64,
    pub cached_at: u64,
}

impl CachedFsEntry {
    pub fn file(path: impl Into<String>, size: u64, modified: u64, cached_at: u64) -> Self {
        Self { path: path.into(), is_dir: false, size_bytes: size, modified_ts: modified, cached_at }
    }

    pub fn directory(path: impl Into<String>, modified: u64, cached_at: u64) -> Self {
        Self { path: path.into(), is_dir: true, size_bytes: 0, modified_ts: modified, cached_at }
    }

    pub fn is_stale(&self, now: u64, ttl: u64) -> bool {
        now.saturating_sub(self.cached_at) > ttl
    }
}

impl std::fmt::Display for CachedFsEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.is_dir { "dir" } else { "file" };
        write!(f, "{} [{}] {}B mod={}", self.path, kind, self.size_bytes, self.modified_ts)
    }
}

/// Caches remote filesystem entries locally for faster lookups.
pub struct RemoteFilesystemCacheManager {
    entries: std::collections::HashMap<String, CachedFsEntry>,
    ttl: u64,
    max_entries: usize,
}

impl RemoteFilesystemCacheManager {
    pub fn new(max_entries: usize, ttl: u64) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            ttl,
            max_entries,
        }
    }

    pub fn put(&mut self, entry: CachedFsEntry) {
        if self.entries.len() >= self.max_entries && !self.entries.contains_key(&entry.path) {
            // Evict oldest entry
            if let Some(oldest_key) = self
                .entries
                .values()
                .min_by_key(|e| e.cached_at)
                .map(|e| e.path.clone())
            {
                self.entries.remove(&oldest_key);
            }
        }
        self.entries.insert(entry.path.clone(), entry);
    }

    pub fn get(&self, path: &str, now: u64) -> Option<&CachedFsEntry> {
        self.entries.get(path).filter(|e| !e.is_stale(now, self.ttl))
    }

    pub fn invalidate(&mut self, path: &str) -> bool {
        self.entries.remove(path).is_some()
    }

    pub fn invalidate_prefix(&mut self, prefix: &str) -> usize {
        let keys: Vec<String> = self
            .entries
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        let count = keys.len();
        for k in keys {
            self.entries.remove(&k);
        }
        count
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn evict_stale(&mut self, now: u64) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, e| !e.is_stale(now, self.ttl));
        before - self.entries.len()
    }

    /// List cached paths matching a directory prefix.
    pub fn list_dir(&self, dir_prefix: &str, now: u64) -> Vec<&CachedFsEntry> {
        self.entries
            .values()
            .filter(|e| {
                e.path.starts_with(dir_prefix) && !e.is_stale(now, self.ttl)
            })
            .collect()
    }

    /// Total cached size in bytes (files only).
    pub fn total_cached_size(&self) -> u64 {
        self.entries.values().filter(|e| !e.is_dir).map(|e| e.size_bytes).sum()
    }
}

impl std::fmt::Display for RemoteFilesystemCacheManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemoteFilesystemCacheManager({}/{} entries, ttl={})",
            self.entries.len(), self.max_entries, self.ttl)
    }
}

// ---------------------------------------------------------------------------
// RemotePortDetector
// ---------------------------------------------------------------------------

/// A detected open port on a remote host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedPort {
    pub port: u16,
    pub protocol: String,
    pub process_name: Option<String>,
    pub detected_at: u64,
}

impl DetectedPort {
    pub fn new(port: u16, protocol: impl Into<String>, detected_at: u64) -> Self {
        Self { port, protocol: protocol.into(), process_name: None, detected_at }
    }

    pub fn with_process(mut self, name: impl Into<String>) -> Self {
        self.process_name = Some(name.into());
        self
    }
}

impl std::fmt::Display for DetectedPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.process_name {
            Some(name) => write!(f, ":{}/{} ({})", self.port, self.protocol, name),
            None => write!(f, ":{}/{}", self.port, self.protocol),
        }
    }
}

/// Detects and tracks open ports on a remote host.
pub struct RemotePortDetector {
    detected_ports: Vec<DetectedPort>,
    ignored_ports: std::collections::HashSet<u16>,
}

impl RemotePortDetector {
    pub fn new() -> Self {
        Self {
            detected_ports: Vec::new(),
            ignored_ports: std::collections::HashSet::new(),
        }
    }

    pub fn add_detected(&mut self, port: DetectedPort) {
        if self.ignored_ports.contains(&port.port) {
            return;
        }
        if !self.detected_ports.iter().any(|p| p.port == port.port) {
            self.detected_ports.push(port);
        }
    }

    pub fn ignore_port(&mut self, port: u16) {
        self.ignored_ports.insert(port);
        self.detected_ports.retain(|p| p.port != port);
    }

    pub fn unignore_port(&mut self, port: u16) {
        self.ignored_ports.remove(&port);
    }

    pub fn is_detected(&self, port: u16) -> bool {
        self.detected_ports.iter().any(|p| p.port == port)
    }

    pub fn detected_count(&self) -> usize {
        self.detected_ports.len()
    }

    pub fn all_detected(&self) -> &[DetectedPort] {
        &self.detected_ports
    }

    pub fn remove_port(&mut self, port: u16) -> bool {
        let before = self.detected_ports.len();
        self.detected_ports.retain(|p| p.port != port);
        self.detected_ports.len() < before
    }

    pub fn clear(&mut self) {
        self.detected_ports.clear();
    }

    /// Get ports in a specific range.
    pub fn ports_in_range(&self, min: u16, max: u16) -> Vec<&DetectedPort> {
        self.detected_ports
            .iter()
            .filter(|p| p.port >= min && p.port <= max)
            .collect()
    }

    /// Get all detected port numbers, sorted.
    pub fn port_numbers(&self) -> Vec<u16> {
        let mut ports: Vec<u16> = self.detected_ports.iter().map(|p| p.port).collect();
        ports.sort();
        ports
    }
}

impl std::fmt::Display for RemotePortDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RemotePortDetector({} detected, {} ignored)",
            self.detected_ports.len(), self.ignored_ports.len())
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

    #[test]
    fn dedup_port_forwards_removes_dups() {
        let a = PortForwardBuilder::new(8080, 80).build();
        let b = PortForwardBuilder::new(8080, 81).build();
        let c = PortForwardBuilder::new(3000, 3000).build();
        let result = dedup_port_forwards(&[a, b, c]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].local_port, 8080);
        assert_eq!(result[1].local_port, 3000);
    }

    #[test]
    fn dedup_port_forwards_empty() {
        let result = dedup_port_forwards(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn sort_forwards_by_local_port_order() {
        let mut forwards = vec![
            PortForwardBuilder::new(9000, 90).build(),
            PortForwardBuilder::new(3000, 30).build(),
            PortForwardBuilder::new(5000, 50).build(),
        ];
        sort_forwards_by_local_port(&mut forwards);
        assert_eq!(forwards[0].local_port, 3000);
        assert_eq!(forwards[1].local_port, 5000);
        assert_eq!(forwards[2].local_port, 9000);
    }

    #[test]
    fn forwards_in_port_range_filters() {
        let forwards = vec![
            PortForwardBuilder::new(80, 80).build(),
            PortForwardBuilder::new(3000, 3000).build(),
            PortForwardBuilder::new(8080, 8080).build(),
        ];
        let range = PortRange::new(3000, 9000);
        let result = forwards_in_port_range(&forwards, &range);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].local_port, 3000);
        assert_eq!(result[1].local_port, 8080);
    }

    #[test]
    fn port_forwards_summary_formatting() {
        assert_eq!(port_forwards_summary(&[]), "No active port forwards");
        let forwards = vec![
            PortForwardBuilder::new(8080, 80).auto_forward(true).build(),
            PortForwardBuilder::new(3000, 3000).auto_forward(false).build(),
        ];
        let summary = port_forwards_summary(&forwards);
        assert!(summary.contains("2 port forward(s)"));
        assert!(summary.contains("1 auto-forwarded"));
    }

    #[test]
    fn has_local_port_conflict_detection() {
        let forwards = vec![
            PortForwardBuilder::new(8080, 80).build(),
        ];
        assert!(has_local_port_conflict(&forwards, 8080));
        assert!(!has_local_port_conflict(&forwards, 3000));
        assert!(!has_local_port_conflict(&[], 8080));
    }

    #[test]
    fn unique_protocols_collection() {
        let forwards = vec![
            PortForwardBuilder::new(80, 80).protocol(PortProtocol::Http).build(),
            PortForwardBuilder::new(443, 443).protocol(PortProtocol::Https).build(),
            PortForwardBuilder::new(8080, 8080).protocol(PortProtocol::Http).build(),
        ];
        let protos = unique_protocols(&forwards);
        assert_eq!(protos.len(), 2);
    }

    #[test]
    fn partition_forwards_splits() {
        let forwards = vec![
            PortForwardBuilder::new(80, 80).auto_forward(true).build(),
            PortForwardBuilder::new(3000, 3000).auto_forward(false).build(),
            PortForwardBuilder::new(8080, 8080).auto_forward(true).build(),
        ];
        let (auto, manual) = partition_forwards(&forwards);
        assert_eq!(auto.len(), 2);
        assert_eq!(manual.len(), 1);
        assert_eq!(manual[0].local_port, 3000);
    }

    // -- enabled_features ------------------------------------------------------

    #[test]
    fn enabled_features_empty() {
        let svc = RemoteFeaturesService::new();
        assert!(enabled_features(&svc).is_empty());
    }

    #[test]
    fn enabled_features_some() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_feature(RemoteFeature::Terminal);
        svc.enable_feature(RemoteFeature::Debugging);
        let feats = enabled_features(&svc);
        assert_eq!(feats.len(), 2);
    }

    // -- count_by_protocol -----------------------------------------------------

    #[test]
    fn count_by_protocol_basic() {
        let ports = vec![
            PortForwardBuilder::new(80, 80).protocol(PortProtocol::Http).build(),
            PortForwardBuilder::new(443, 443).protocol(PortProtocol::Https).build(),
            PortForwardBuilder::new(3000, 3000).protocol(PortProtocol::Http).build(),
        ];
        let (http, https, tcp) = count_by_protocol(&ports);
        assert_eq!(http, 2);
        assert_eq!(https, 1);
        assert_eq!(tcp, 0);
    }

    // -- is_port_in_use --------------------------------------------------------

    #[test]
    fn is_port_in_use_true() {
        let ports = vec![PortForwardBuilder::new(8080, 80).build()];
        assert!(is_port_in_use(&ports, 8080));
    }

    #[test]
    fn is_port_in_use_false() {
        let ports = vec![PortForwardBuilder::new(8080, 80).build()];
        assert!(!is_port_in_use(&ports, 9090));
    }

    // -- find_available_port ---------------------------------------------------

    #[test]
    fn find_available_port_skips_used() {
        let ports = vec![
            PortForwardBuilder::new(3000, 3000).build(),
            PortForwardBuilder::new(3001, 3001).build(),
        ];
        assert_eq!(find_available_port(&ports, 3000), Some(3002));
    }

    // -- all_features_enabled --------------------------------------------------

    #[test]
    fn all_features_enabled_checks() {
        let mut svc = RemoteFeaturesService::new();
        svc.enable_feature(RemoteFeature::Terminal);
        assert!(all_features_enabled(&svc, &[RemoteFeature::Terminal]));
        assert!(!all_features_enabled(&svc, &[RemoteFeature::Terminal, RemoteFeature::Debugging]));
    }

    // -- remote_port_set -------------------------------------------------------

    #[test]
    fn remote_port_set_deduplicates() {
        let ports = vec![
            PortForwardBuilder::new(8080, 80).build(),
            PortForwardBuilder::new(8081, 80).build(),
            PortForwardBuilder::new(443, 443).build(),
        ];
        let set = remote_port_set(&ports);
        assert_eq!(set, vec![80, 443]);
    }

    // -- RemoteExecProxy ---------------------------------------------------

    #[test]
    fn exec_proxy_build_command() {
        let proxy = RemoteExecProxy::new("remote-host")
            .with_cwd("/home/user")
            .with_timeout(5000);
        let cmd = proxy.build_command("cargo", &["build", "--release"]);
        assert_eq!(cmd.command_line(), "cargo build --release");
        assert_eq!(cmd.host, "remote-host");
        assert_eq!(cmd.timeout_ms, 5000);
    }

    #[test]
    fn exec_proxy_display() {
        let proxy = RemoteExecProxy::new("host");
        assert!(format!("{proxy}").contains("host"));
    }

    #[test]
    fn remote_command_no_args() {
        let proxy = RemoteExecProxy::new("h");
        let cmd = proxy.build_command("ls", &[]);
        assert_eq!(cmd.command_line(), "ls");
    }

    // -- RemoteExtensionInstaller ------------------------------------------

    #[test]
    fn extension_installer_install_uninstall() {
        let mut inst = RemoteExtensionInstaller::new();
        inst.install("ext.rust", "1.0.0");
        assert!(inst.is_installed("ext.rust"));
        assert_eq!(inst.get_version("ext.rust"), Some("1.0.0"));
        assert!(inst.uninstall("ext.rust"));
        assert!(!inst.is_installed("ext.rust"));
    }

    #[test]
    fn extension_installer_upgrade() {
        let mut inst = RemoteExtensionInstaller::new();
        inst.install("ext.rust", "1.0.0");
        inst.install("ext.rust", "2.0.0");
        assert_eq!(inst.installed_count(), 1);
        assert_eq!(inst.get_version("ext.rust"), Some("2.0.0"));
    }

    #[test]
    fn extension_installer_enable_disable() {
        let mut inst = RemoteExtensionInstaller::new();
        inst.install("ext.rust", "1.0.0");
        inst.set_enabled("ext.rust", false);
        assert_eq!(inst.enabled_count(), 0);
    }

    #[test]
    fn extension_installer_display() {
        let inst = RemoteExtensionInstaller::default();
        assert!(format!("{inst}").contains("0 installed"));
    }

    // -- RemotePortScanner -------------------------------------------------

    #[test]
    fn port_scanner_find_available() {
        let mut scanner = RemotePortScanner::new(PortRange::new(8080, 8085));
        scanner.mark_used(8080);
        scanner.mark_used(8081);
        assert_eq!(scanner.find_available(), Some(8082));
        assert_eq!(scanner.available_count(), 4);
    }

    #[test]
    fn port_scanner_find_n() {
        let scanner = RemotePortScanner::new(PortRange::new(3000, 3010));
        let ports = scanner.find_n_available(3);
        assert_eq!(ports.len(), 3);
        assert_eq!(ports[0], 3000);
    }

    #[test]
    fn port_scanner_is_available() {
        let mut scanner = RemotePortScanner::new(PortRange::new(80, 90));
        scanner.mark_used(80);
        assert!(!scanner.is_available(80));
        assert!(scanner.is_available(81));
        assert!(!scanner.is_available(100)); // out of range
    }

    #[test]
    fn port_scanner_display() {
        let scanner = RemotePortScanner::new(PortRange::new(80, 90));
        assert!(format!("{scanner}").contains("80-90"));
    }

    // -- RemoteConnectionDiagnostics ---------------------------------------

    #[test]
    fn diagnostics_all_ok() {
        let mut diag = RemoteConnectionDiagnostics::new();
        diag.add_check("connectivity", DiagnosticStatus::Ok);
        diag.add_check("auth", DiagnosticStatus::Ok);
        assert!(diag.all_ok());
        assert_eq!(diag.error_count(), 0);
    }

    #[test]
    fn diagnostics_with_errors() {
        let mut diag = RemoteConnectionDiagnostics::new();
        diag.add_check("connectivity", DiagnosticStatus::Ok);
        diag.add_check("auth", DiagnosticStatus::Error("timeout".into()));
        diag.add_check("fs", DiagnosticStatus::Warning("slow".into()));
        assert!(!diag.all_ok());
        assert_eq!(diag.error_count(), 1);
        assert_eq!(diag.warning_count(), 1);
    }

    #[test]
    fn diagnostics_summary() {
        let diag = RemoteConnectionDiagnostics::new();
        let s = diag.summary();
        assert!(s.contains("0 checks"));
    }

    #[test]
    fn diagnostics_display() {
        let diag = RemoteConnectionDiagnostics::default();
        let s = format!("{diag}");
        assert!(s.contains("checks"));
    }

    #[test]
    fn fs_cache_put_and_get() {
        let mut cache = RemoteFilesystemCacheManager::new(100, 1000);
        cache.put(CachedFsEntry::file("/a.txt", 100, 50, 100));
        assert!(cache.get("/a.txt", 200).is_some());
    }

    #[test]
    fn fs_cache_stale_entry() {
        let mut cache = RemoteFilesystemCacheManager::new(100, 100);
        cache.put(CachedFsEntry::file("/a.txt", 100, 50, 100));
        assert!(cache.get("/a.txt", 300).is_none());
    }

    #[test]
    fn fs_cache_invalidate() {
        let mut cache = RemoteFilesystemCacheManager::new(100, 1000);
        cache.put(CachedFsEntry::file("/a.txt", 100, 50, 100));
        assert!(cache.invalidate("/a.txt"));
        assert!(!cache.invalidate("/a.txt"));
    }

    #[test]
    fn fs_cache_invalidate_prefix() {
        let mut cache = RemoteFilesystemCacheManager::new(100, 1000);
        cache.put(CachedFsEntry::file("/src/a.rs", 100, 50, 100));
        cache.put(CachedFsEntry::file("/src/b.rs", 200, 50, 100));
        cache.put(CachedFsEntry::file("/doc/c.md", 50, 50, 100));
        assert_eq!(cache.invalidate_prefix("/src/"), 2);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn fs_cache_eviction() {
        let mut cache = RemoteFilesystemCacheManager::new(2, 1000);
        cache.put(CachedFsEntry::file("/a", 10, 1, 1));
        cache.put(CachedFsEntry::file("/b", 20, 2, 2));
        cache.put(CachedFsEntry::file("/c", 30, 3, 3));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn fs_cache_evict_stale() {
        let mut cache = RemoteFilesystemCacheManager::new(100, 100);
        cache.put(CachedFsEntry::file("/old", 10, 1, 10));
        cache.put(CachedFsEntry::file("/new", 20, 2, 500));
        let evicted = cache.evict_stale(500);
        assert_eq!(evicted, 1);
    }

    #[test]
    fn fs_cache_list_dir() {
        let mut cache = RemoteFilesystemCacheManager::new(100, 1000);
        cache.put(CachedFsEntry::file("/src/a.rs", 10, 1, 100));
        cache.put(CachedFsEntry::file("/src/b.rs", 20, 2, 100));
        cache.put(CachedFsEntry::file("/doc/c.md", 5, 3, 100));
        let listing = cache.list_dir("/src/", 200);
        assert_eq!(listing.len(), 2);
    }

    #[test]
    fn fs_cache_total_size() {
        let mut cache = RemoteFilesystemCacheManager::new(100, 1000);
        cache.put(CachedFsEntry::file("/a", 100, 1, 1));
        cache.put(CachedFsEntry::file("/b", 200, 2, 2));
        cache.put(CachedFsEntry::directory("/d", 3, 3));
        assert_eq!(cache.total_cached_size(), 300);
    }

    #[test]
    fn fs_cache_display_and_clear() {
        let mut cache = RemoteFilesystemCacheManager::new(10, 100);
        cache.put(CachedFsEntry::file("/x", 1, 1, 1));
        assert!(format!("{cache}").contains("1/10"));
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn cached_fs_entry_display() {
        let f = CachedFsEntry::file("/a.txt", 1024, 50, 100);
        let s = format!("{f}");
        assert!(s.contains("file"));
        assert!(s.contains("/a.txt"));
        let d = CachedFsEntry::directory("/dir", 50, 100);
        let s = format!("{d}");
        assert!(s.contains("dir"));
    }

    #[test]
    fn port_detector_add_and_detect() {
        let mut det = RemotePortDetector::new();
        det.add_detected(DetectedPort::new(8080, "tcp", 100));
        assert!(det.is_detected(8080));
        assert_eq!(det.detected_count(), 1);
    }

    #[test]
    fn port_detector_ignore() {
        let mut det = RemotePortDetector::new();
        det.add_detected(DetectedPort::new(8080, "tcp", 100));
        det.ignore_port(8080);
        assert!(!det.is_detected(8080));
        // Adding ignored port does nothing
        det.add_detected(DetectedPort::new(8080, "tcp", 200));
        assert!(!det.is_detected(8080));
    }

    #[test]
    fn port_detector_unignore() {
        let mut det = RemotePortDetector::new();
        det.ignore_port(3000);
        det.unignore_port(3000);
        det.add_detected(DetectedPort::new(3000, "tcp", 100));
        assert!(det.is_detected(3000));
    }

    #[test]
    fn port_detector_no_duplicates() {
        let mut det = RemotePortDetector::new();
        det.add_detected(DetectedPort::new(8080, "tcp", 100));
        det.add_detected(DetectedPort::new(8080, "tcp", 200));
        assert_eq!(det.detected_count(), 1);
    }

    #[test]
    fn port_detector_range() {
        let mut det = RemotePortDetector::new();
        det.add_detected(DetectedPort::new(80, "tcp", 1));
        det.add_detected(DetectedPort::new(443, "tcp", 2));
        det.add_detected(DetectedPort::new(8080, "tcp", 3));
        let high = det.ports_in_range(1000, 9000);
        assert_eq!(high.len(), 1);
    }

    #[test]
    fn port_detector_port_numbers_sorted() {
        let mut det = RemotePortDetector::new();
        det.add_detected(DetectedPort::new(8080, "tcp", 1));
        det.add_detected(DetectedPort::new(80, "tcp", 2));
        det.add_detected(DetectedPort::new(443, "tcp", 3));
        assert_eq!(det.port_numbers(), vec![80, 443, 8080]);
    }

    #[test]
    fn port_detector_remove_and_clear() {
        let mut det = RemotePortDetector::new();
        det.add_detected(DetectedPort::new(80, "tcp", 1));
        det.add_detected(DetectedPort::new(443, "tcp", 2));
        assert!(det.remove_port(80));
        assert!(!det.remove_port(80));
        det.clear();
        assert_eq!(det.detected_count(), 0);
    }

    #[test]
    fn port_detector_display() {
        let det = RemotePortDetector::new();
        assert!(format!("{det}").contains("0 detected"));
    }

    #[test]
    fn detected_port_display() {
        let p = DetectedPort::new(8080, "tcp", 100).with_process("node");
        let s = format!("{p}");
        assert!(s.contains("8080"));
        assert!(s.contains("node"));
        let p2 = DetectedPort::new(443, "tcp", 100);
        let s2 = format!("{p2}");
        assert!(s2.contains("443"));
    }

}
