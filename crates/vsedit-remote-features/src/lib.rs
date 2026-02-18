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



// ---------------------------------------------------------------------------
// remote_features – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XRemoteFeaturesCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XRemoteFeaturesCapabilities {
    pub fn new() -> Self {
        Self { flags: std::collections::HashSet::new() }
    }

    pub fn register(&mut self, cap: impl Into<String>) {
        self.flags.insert(cap.into());
    }

    pub fn has(&self, cap: &str) -> bool {
        self.flags.contains(cap)
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Return the intersection with another capability set.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.intersection(&other.flags).cloned().collect(),
        }
    }

    /// Return capabilities present here but not in `other`.
    pub fn diff(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.difference(&other.flags).cloned().collect(),
        }
    }

    pub fn all(&self) -> Vec<&str> {
        self.flags.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for XRemoteFeaturesCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XRemoteFeaturesServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XRemoteFeaturesServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. Returns the previous value if the key was already present.
    pub fn register(&mut self, name: impl Into<String>, descriptor: impl Into<String>) -> Option<String> {
        self.services.insert(name.into(), descriptor.into())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.services.get(name).map(|s| s.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.services.remove(name)
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

/// Sanitize a path-like string by collapsing repeated separators and removing trailing ones.
pub fn x_remote_features_sanitize_path(p: &str) -> String {
    let mut result = String::with_capacity(p.len());
    let mut last_was_sep = false;
    for ch in p.chars() {
        if ch == '/' || ch == '\\' {
            if !last_was_sep {
                result.push('/');
            }
            last_was_sep = true;
        } else {
            result.push(ch);
            last_was_sep = false;
        }
    }
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}



// ---------------------------------------------------------------------------
// remote_features – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for remote development features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YRemoteFeaturesRemoteAuthMethod {
    Token,
    Ssh,
    Certificate,
    Interactive,
}

impl YRemoteFeaturesRemoteAuthMethod {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Token => 0,
            Self::Ssh => 1,
            Self::Certificate => 2,
            Self::Interactive => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Token => "Token",
            Self::Ssh => "Ssh",
            Self::Certificate => "Certificate",
            Self::Interactive => "Interactive",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YRemoteFeaturesRemoteAuthMethod] {
        &[
            YRemoteFeaturesRemoteAuthMethod::Token,
            YRemoteFeaturesRemoteAuthMethod::Ssh,
            YRemoteFeaturesRemoteAuthMethod::Certificate,
            YRemoteFeaturesRemoteAuthMethod::Interactive,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YRemoteFeaturesRemoteAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks remote endpoint data.
#[derive(Debug, Clone)]
pub struct YRemoteFeaturesRemoteEndpoint {
    pub host: String,
    pub port: u16,
    pub latency_ms: u32,
}

impl YRemoteFeaturesRemoteEndpoint {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            host: String::new(),
            port: 0,
            latency_ms: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YRemoteFeaturesRemoteEndpoint({}: {:?})", "host", self.host)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_remote_features_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_remote_features_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_remote_features_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_remote_features_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_remote_features_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_remote_features_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_remote_features_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_remote_features_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// remote_features – Extended remote capability set helpers
// ---------------------------------------------------------------------------

/// Priority levels for remote capability set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZRemoteFeaturesPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZRemoteFeaturesPriority {
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
    pub fn all_asc() -> [ZRemoteFeaturesPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZRemoteFeaturesPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks remote capability set data.
#[derive(Debug, Clone)]
pub struct ZRemoteFeaturesRemoteCapabilitySet {
    pub caps: Vec<String>,
    pub negotiated: bool,
    pub round_trips: u32,
}

impl ZRemoteFeaturesRemoteCapabilitySet {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            caps: Vec::new(),
            negotiated: false,
            round_trips: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.caps.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.caps.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZRemoteFeaturesRemoteCapabilitySet[negotiated={:?}, round_trips={:?}]", self.negotiated, self.round_trips)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for remote capability set.
pub fn z_remote_features_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_remote_features_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_remote_features_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_remote_features_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_remote_features_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_remote_features_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_remote_features_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 59
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer59 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer59 {
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
pub fn xb_fnv1a_59(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_59<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_59<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_59(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_59(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 148
// ---------------------------------------------------------------------------

/// Generic object pool `Xc148Pool<T>`.
pub struct Xc148Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc148Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc148PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc148Pool<T> {
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
    pub fn stats(&self) -> Xc148PoolStats {
        Xc148PoolStats {
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

impl<T> Default for Xc148Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc148Scheduler`.
pub struct Xc148Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc148Scheduler {
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

impl Default for Xc148Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_148 hash for the given byte slice.
pub fn xc_148_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_148 convention.
pub fn xc_148_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe72 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe72Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe72PipelineError {
    pub stage: Xe72Stage,
    pub message: String,
}

impl std::fmt::Display for Xe72PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe72Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe72Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError>>>,
    stage_names: Vec<Xe72Stage>,
}

impl Xe72Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe72Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe72Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe72Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe72Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> {
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

    pub fn compose(mut self, other: Xe72Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe72CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe72CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe72Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe72CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe72CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe72Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe72CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_72_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe72CacheEntry {
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

    fn xe_72_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe72CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_72_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> {
    Ok(data)
}

pub fn xe_72_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_72_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_72_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_72_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe72PipelineError> {
    Err(Xe72PipelineError {
        stage: Xe72Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_70: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg70Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg70Graph {
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

impl Default for Xg70Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_70: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg70Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg70Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg70Heap<T>) {
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

impl<T: Ord> Default for Xg70Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 147).
pub struct Xh147SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh147SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 189 as u64,
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

/// A compact bit set supporting boolean operations (variant 147).
pub struct Xh147BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh147BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 147).
pub struct Xi147Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi147Deque<T> {
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
pub struct Xi147Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi147Interval {
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

/// A simple interval tree (variant 147).
pub struct Xi147IntervalTree {
    xi_intervals: Vec<Xi147Interval>,
}

impl Xi147IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi147Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi147Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi147Interval) -> Vec<&Xi147Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi147Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi147Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi147Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi147Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi147Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi147Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 147) ---

/// Disjoint set / union-find for crate 147.
pub struct Xj147UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj147UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ147_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 147.
pub struct Xj147BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj147BTreeNode<K, V>>>,
    len: usize,
}

struct Xj147BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj147BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj147BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ147_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ147_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj147BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj147BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj147BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj147BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_148 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk148SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk148SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk148DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk148DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_147).
#[derive(Debug, Clone)]
pub struct Xl147Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl147Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_147).
#[derive(Debug, Clone)]
pub struct Xl147SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl147SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm147MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm147MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm147Tokenizer {
    text: String,
}

impl Xm147Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 147.
pub struct Xn147Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn147Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 147 -----

#[derive(Debug, Clone)]
struct Xn147AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn147AvlNode<K, V>>>,
    right: Option<Box<Xn147AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 147.
#[derive(Debug, Clone)]
pub struct Xn147AVL<K, V> {
    root: Option<Box<Xn147AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn147AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn147AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn147AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn147AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn147AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn147AvlNode<K, V>>) -> Box<Xn147AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn147AvlNode<K, V>>) -> Box<Xn147AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn147AvlNode<K, V>>) -> Box<Xn147AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn147AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn147AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn147AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn147AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn147AvlNode<K, V>>) -> &Xn147AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn147AvlNode<K, V>>) -> (Box<Xn147AvlNode<K, V>>, Option<Box<Xn147AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn147AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn147AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn147AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn147AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn147AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn147AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn147AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo147RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo147Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo147RBNode<K, V> {
    key: K,
    value: V,
    color: Xo147Color,
    left: Option<Box<Xo147RBNode<K, V>>>,
    right: Option<Box<Xo147RBNode<K, V>>>,
}

/// A red-black tree map for crate 147.
#[derive(Debug, Clone)]
pub struct Xo147RedBlack<K, V> {
    root: Option<Box<Xo147RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo147RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo147Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo147RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo147RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo147RBNode {
                    key, value, color: Xo147Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo147RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo147Color::Red)
    }

    fn xo_balance(mut h: Box<Xo147RBNode<K, V>>) -> Box<Xo147RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo147Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo147RBNode<K, V>>) -> Box<Xo147RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo147Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo147RBNode<K, V>>) -> Box<Xo147RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo147Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo147RBNode<K, V>>) {
        h.color = Xo147Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo147Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo147Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo147Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo147RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo147RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo147RBNode<K, V>) -> (K, V, Option<Box<Xo147RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo147RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo147Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo147RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo147ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 147.
#[derive(Debug, Clone)]
pub struct Xo147ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo147ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo147#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo147#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
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
    fn get_auto_forward_ports_works() {
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
    fn forwarded_port_count_works() {
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
    fn ports_by_protocol_works() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).protocol(PortProtocol::Http).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).protocol(PortProtocol::Https).build());
        svc.add_port_forward(PortForwardBuilder::new(5432, 5432).protocol(PortProtocol::Tcp).build());
        assert_eq!(svc.ports_by_protocol(&PortProtocol::Http).len(), 1);
        assert_eq!(svc.ports_by_protocol(&PortProtocol::Https).len(), 1);
    }

    #[test]
    fn is_port_forwarded_works() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).build());
        assert!(svc.is_port_forwarded(3000));
        assert!(!svc.is_port_forwarded(9999));
    }

    #[test]
    fn ports_in_range_works() {
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
    fn update_port_label_works() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).label("old").build());
        assert!(svc.update_port_label(3000, "new"));
        assert_eq!(svc.find_port_by_local(3000).unwrap().label.as_deref(), Some("new"));
        assert!(!svc.update_port_label(9999, "x"));
    }

    #[test]
    fn remove_non_auto_forwards_works() {
        let mut svc = RemoteFeaturesService::new();
        svc.add_port_forward(PortForwardBuilder::new(3000, 3000).auto_forward(true).build());
        svc.add_port_forward(PortForwardBuilder::new(5000, 5000).auto_forward(false).build());
        svc.add_port_forward(PortForwardBuilder::new(8080, 80).auto_forward(false).build());
        let removed = svc.remove_non_auto_forwards();
        assert_eq!(removed, 2);
        assert_eq!(svc.forwarded_port_count(), 1);
    }

    #[test]
    fn ports_by_visibility_works() {
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


    // -- remote_features additional tests -------------------------------------------

    #[test]
    fn x_remote_features_capabilities_register_and_has() {
        let mut caps = XRemoteFeaturesCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_remote_features_capabilities_len() {
        let mut caps = XRemoteFeaturesCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_remote_features_capabilities_intersect() {
        let mut a = XRemoteFeaturesCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XRemoteFeaturesCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_remote_features_capabilities_diff() {
        let mut a = XRemoteFeaturesCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XRemoteFeaturesCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_remote_features_service_registry_basic() {
        let mut reg = XRemoteFeaturesServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_remote_features_service_registry_replace() {
        let mut reg = XRemoteFeaturesServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_remote_features_service_registry_remove() {
        let mut reg = XRemoteFeaturesServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_remote_features_service_registry_names() {
        let mut reg = XRemoteFeaturesServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_remote_features_sanitize_path_basic() {
        assert_eq!(x_remote_features_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_remote_features_sanitize_path_backslash() {
        assert_eq!(x_remote_features_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_remote_features_sanitize_path_single() {
        assert_eq!(x_remote_features_sanitize_path("/"), "/");
    }

    #[test]
    fn x_remote_features_capabilities_default() {
        let caps = XRemoteFeaturesCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_remote_features_capabilities_all() {
        let mut caps = XRemoteFeaturesCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    // -- remote_features extended domain tests ----------------------------------------

    #[test]
    fn y_remote_features_enum_index() {
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Token.index(), 0);
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Ssh.index(), 1);
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Certificate.index(), 2);
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Interactive.index(), 3);
    }

    #[test]
    fn y_remote_features_enum_label() {
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Token.label(), "Token");
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Ssh.label(), "Ssh");
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Certificate.label(), "Certificate");
        assert_eq!(YRemoteFeaturesRemoteAuthMethod::Interactive.label(), "Interactive");
    }

    #[test]
    fn y_remote_features_enum_all() {
        let all = YRemoteFeaturesRemoteAuthMethod::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_remote_features_enum_is_default() {
        assert!(YRemoteFeaturesRemoteAuthMethod::Token.is_default());
        assert!(!YRemoteFeaturesRemoteAuthMethod::Interactive.is_default());
    }

    #[test]
    fn y_remote_features_enum_display() {
        assert_eq!(format!("{}", YRemoteFeaturesRemoteAuthMethod::Token), "Token");
    }

    #[test]
    fn y_remote_features_struct_new() {
        let s = YRemoteFeaturesRemoteEndpoint::new();
        let _ = s.summary();
    }

    #[test]
    fn y_remote_features_fingerprint_deterministic() {
        let h1 = y_remote_features_fingerprint("hello");
        let h2 = y_remote_features_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_remote_features_fingerprint("a"), y_remote_features_fingerprint("b"));
    }

    #[test]
    fn y_remote_features_truncate_short() {
        assert_eq!(y_remote_features_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_remote_features_truncate_long() {
        let r = y_remote_features_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_remote_features_normalize_key_basic() {
        assert_eq!(y_remote_features_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_remote_features_split_path_basic() {
        let parts = y_remote_features_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_remote_features_count_occurrences_basic() {
        assert_eq!(y_remote_features_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_remote_features_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_remote_features_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_remote_features_in_range_basic() {
        assert!(y_remote_features_in_range(5, 1, 10));
        assert!(y_remote_features_in_range(1, 1, 10));
        assert!(y_remote_features_in_range(10, 1, 10));
        assert!(!y_remote_features_in_range(0, 1, 10));
        assert!(!y_remote_features_in_range(11, 1, 10));
    }

    #[test]
    fn y_remote_features_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_remote_features_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_remote_features_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_remote_features_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- remote_features Z-extended tests -----------------------------------------------

    #[test]
    fn z_remote_features_priority_weight() {
        assert_eq!(ZRemoteFeaturesPriority::Idle.weight(), 0);
        assert_eq!(ZRemoteFeaturesPriority::Normal.weight(), 2);
        assert_eq!(ZRemoteFeaturesPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_remote_features_priority_label() {
        assert_eq!(ZRemoteFeaturesPriority::Low.label(), "low");
        assert_eq!(ZRemoteFeaturesPriority::High.label(), "high");
    }

    #[test]
    fn z_remote_features_priority_is_elevated() {
        assert!(!ZRemoteFeaturesPriority::Normal.is_elevated());
        assert!(ZRemoteFeaturesPriority::High.is_elevated());
        assert!(ZRemoteFeaturesPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_remote_features_priority_display() {
        assert_eq!(format!("{}", ZRemoteFeaturesPriority::Idle), "idle");
    }

    #[test]
    fn z_remote_features_priority_all_asc() {
        let all = ZRemoteFeaturesPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZRemoteFeaturesPriority::Idle);
        assert_eq!(all[4], ZRemoteFeaturesPriority::Realtime);
    }

    #[test]
    fn z_remote_features_struct_new() {
        let s = ZRemoteFeaturesRemoteCapabilitySet::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_remote_features_struct_toggled_clone() {
        let s = ZRemoteFeaturesRemoteCapabilitySet::new();
        let t = s.toggled_clone();
        let _ = t.round_trips;
    }

    #[test]
    fn z_remote_features_rolling_hash_deterministic() {
        let h1 = z_remote_features_rolling_hash(b"test");
        let h2 = z_remote_features_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_remote_features_rolling_hash(b"a"), z_remote_features_rolling_hash(b"b"));
    }

    #[test]
    fn z_remote_features_pad_to_basic() {
        assert_eq!(z_remote_features_pad_to("hi", 5), "hi   ");
        assert_eq!(z_remote_features_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_remote_features_is_identifier_basic() {
        assert!(z_remote_features_is_identifier("foo_bar"));
        assert!(z_remote_features_is_identifier("abc123"));
        assert!(!z_remote_features_is_identifier(""));
        assert!(!z_remote_features_is_identifier("has space"));
    }

    #[test]
    fn z_remote_features_levenshtein_basic() {
        assert_eq!(z_remote_features_levenshtein("", ""), 0);
        assert_eq!(z_remote_features_levenshtein("abc", "abc"), 0);
        assert_eq!(z_remote_features_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_remote_features_unique_words_basic() {
        let w = z_remote_features_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_remote_features_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_remote_features_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_remote_features_common_prefix_basic() {
        assert_eq!(z_remote_features_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_remote_features_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_remote_features_struct_clear() {
        let mut s = ZRemoteFeaturesRemoteCapabilitySet::new();
        s.caps.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_remote_features_rolling_hash_empty() {
        let h = z_remote_features_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_59_push_and_len() {
        let mut rb = super::XbRingBuffer59::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_59_overwrite() {
        let mut rb = super::XbRingBuffer59::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_59_get_out_of_bounds() {
        let rb = super::XbRingBuffer59::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_59_drain_all() {
        let mut rb = super::XbRingBuffer59::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_59_peek_front_back() {
        let mut rb = super::XbRingBuffer59::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_59_clear() {
        let mut rb = super::XbRingBuffer59::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_59_capacity() {
        let rb = super::XbRingBuffer59::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_59_basic() {
        let h = super::xb_fnv1a_59(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_59(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_59_different_inputs() {
        let h1 = super::xb_fnv1a_59(b"abc");
        let h2 = super::xb_fnv1a_59(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_59_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_59(&data);
        let dec = super::xb_rle_decode_59(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_59_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_59(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_59(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_59_values() {
        assert!((super::xb_clamp_59(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_59(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_59(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_59_values() {
        assert!((super::xb_lerp_59(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_59(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_59(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_59_wrap_around_twice() {
        let mut rb = super::XbRingBuffer59::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 148 ----

    #[test]
    fn xc_148_pool_new_empty() {
        let pool: super::Xc148Pool<i32> = super::Xc148Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_148_pool_release_acquire() {
        let mut pool = super::Xc148Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_148_pool_acquire_empty() {
        let mut pool: super::Xc148Pool<i32> = super::Xc148Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_148_pool_full() {
        let mut pool = super::Xc148Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_148_pool_drain() {
        let mut pool = super::Xc148Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_148_pool_stats() {
        let mut pool = super::Xc148Pool::new(8);
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
    fn xc_148_pool_clear() {
        let mut pool = super::Xc148Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_148_pool_shrink() {
        let mut pool = super::Xc148Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_148_pool_default() {
        let pool: super::Xc148Pool<String> = super::Xc148Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_148_pool_extend() {
        let mut pool = super::Xc148Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_148_pool_retain() {
        let mut pool = super::Xc148Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_148_scheduler_round_robin() {
        let mut sched = super::Xc148Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_148_scheduler_empty() {
        let mut sched = super::Xc148Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_148_scheduler_reset() {
        let mut sched = super::Xc148Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_148_scheduler_add_remove() {
        let mut sched = super::Xc148Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_148_scheduler_targets() {
        let sched = super::Xc148Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_148_hash_empty() {
        assert_eq!(super::xc_148_hash(b""), 5381);
    }

    #[test]
    fn xc_148_hash_data() {
        let h = super::xc_148_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_148_hash(b"hello"), h);
    }

    #[test]
    fn xc_148_reverse_str() {
        assert_eq!(super::xc_148_reverse("abc"), "cba");
        assert_eq!(super::xc_148_reverse(""), "");
    }


    #[test]
    fn xe_72_pipeline_empty() {
        let p = super::Xe72Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_72_pipeline_parse_stage() {
        let p = super::Xe72Pipeline::new()
            .add_parse(super::xe_72_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_72_pipeline_transform_double() {
        let p = super::Xe72Pipeline::new()
            .add_transform(super::xe_72_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_72_pipeline_validate_reverse() {
        let p = super::Xe72Pipeline::new()
            .add_validate(super::xe_72_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_72_pipeline_emit_filter() {
        let p = super::Xe72Pipeline::new()
            .add_emit(super::xe_72_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_72_pipeline_multi_stage() {
        let p = super::Xe72Pipeline::new()
            .add_parse(super::xe_72_pipeline_identity)
            .add_transform(super::xe_72_pipeline_double)
            .add_validate(super::xe_72_pipeline_reverse)
            .add_emit(super::xe_72_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_72_pipeline_error_propagation() {
        let p = super::Xe72Pipeline::new()
            .add_parse(super::xe_72_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe72Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_72_pipeline_compose() {
        let p1 = super::Xe72Pipeline::new()
            .add_parse(super::xe_72_pipeline_identity);
        let p2 = super::Xe72Pipeline::new()
            .add_transform(super::xe_72_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_72_pipeline_error_display() {
        let e = super::Xe72PipelineError {
            stage: super::Xe72Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_72_cache_put_get() {
        let mut c = super::Xe72Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_72_cache_miss() {
        let mut c: super::Xe72Cache<&str, i32> = super::Xe72Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_72_cache_ttl_expiry() {
        let mut c = super::Xe72Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_72_cache_evict() {
        let mut c = super::Xe72Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_72_cache_capacity() {
        let mut c = super::Xe72Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_72_cache_stats() {
        let mut c = super::Xe72Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_72_cache_clear() {
        let mut c = super::Xe72Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_70 graph tests ------------------------------------------------

    #[test]
    fn xg_70_graph_empty() {
        let g = super::Xg70Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_70_graph_add_node() {
        let mut g = super::Xg70Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_70_graph_add_edge() {
        let mut g = super::Xg70Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_70_graph_neighbors() {
        let mut g = super::Xg70Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_70_graph_has_path() {
        let mut g = super::Xg70Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_70_graph_self_path() {
        let g = super::Xg70Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_70_graph_topo_sort() {
        let mut g = super::Xg70Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_70_graph_cycle_detect_false() {
        let mut g = super::Xg70Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_70_graph_cycle_detect_true() {
        let mut g = super::Xg70Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_70 heap tests -------------------------------------------------

    #[test]
    fn xg_70_heap_empty() {
        let h: super::Xg70Heap<i32> = super::Xg70Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_70_heap_push_pop() {
        let mut h = super::Xg70Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_70_heap_peek() {
        let mut h = super::Xg70Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_70_heap_drain_sorted() {
        let mut h = super::Xg70Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_70_heap_merge() {
        let mut a = super::Xg70Heap::new();
        let mut b = super::Xg70Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_70_heap_default() {
        let h: super::Xg70Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_70_graph_default() {
        let g: super::Xg70Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh147_skip_insert_contains() {
        let mut sl = super::Xh147SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh147_skip_remove() {
        let mut sl = super::Xh147SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh147_skip_len() {
        let mut sl = super::Xh147SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh147_skip_range_query() {
        let mut sl = super::Xh147SkipList::xh_new(4);
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
    fn xh147_skip_floor_ceiling() {
        let mut sl = super::Xh147SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh147_skip_rank() {
        let mut sl = super::Xh147SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh147_skip_empty() {
        let sl = super::Xh147SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh147_skip_duplicates() {
        let mut sl = super::Xh147SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh147_bitset_set_test() {
        let mut bs = super::Xh147BitSet::xh_new(256);
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
    fn xh147_bitset_clear_count() {
        let mut bs = super::Xh147BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh147_bitset_and_or_xor() {
        let mut a = super::Xh147BitSet::xh_new(128);
        let mut b = super::Xh147BitSet::xh_new(128);
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
    fn xh147_bitset_iter_ones() {
        let mut bs = super::Xh147BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh147_bitset_first_last() {
        let mut bs = super::Xh147BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh147_bitset_empty() {
        let bs = super::Xh147BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi147_deque_push_pop_back() {
        let mut dq = super::Xi147Deque::xi_new(4);
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
    fn xi147_deque_push_pop_front() {
        let mut dq = super::Xi147Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi147_deque_mixed_ops() {
        let mut dq = super::Xi147Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi147_deque_get_and_split() {
        let mut dq = super::Xi147Deque::xi_new(8);
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
    fn xi147_deque_rotate_left() {
        let mut dq = super::Xi147Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi147_deque_rotate_right() {
        let mut dq = super::Xi147Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi147_deque_grow() {
        let mut dq = super::Xi147Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi147_deque_empty() {
        let dq = super::Xi147Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi147_interval_tree_insert_query() {
        let mut tree = super::Xi147IntervalTree::xi_new();
        tree.xi_insert(super::Xi147Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi147Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi147Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi147_interval_tree_overlap() {
        let mut tree = super::Xi147IntervalTree::xi_new();
        tree.xi_insert(super::Xi147Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi147Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi147Interval::xi_new(12, 20));
        let q = super::Xi147Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi147_interval_tree_remove() {
        let mut tree = super::Xi147IntervalTree::xi_new();
        tree.xi_insert(super::Xi147Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi147Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi147_interval_tree_gaps() {
        let mut tree = super::Xi147IntervalTree::xi_new();
        tree.xi_insert(super::Xi147Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi147Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi147Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi147Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi147Interval::xi_new(8, 10));
    }

    #[test]
    fn xi147_interval_tree_merge() {
        let mut tree = super::Xi147IntervalTree::xi_new();
        tree.xi_insert(super::Xi147Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi147Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi147Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi147Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi147Interval::xi_new(10, 15));
    }

    #[test]
    fn xi147_interval_tree_all() {
        let mut tree = super::Xi147IntervalTree::xi_new();
        tree.xi_insert(super::Xi147Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi147Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi147_interval_tree_empty() {
        let tree = super::Xi147IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi147_interval_tree_contains_point() {
        let iv = super::Xi147Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 147) ---

    #[test]
    fn xj_147_uf_make_and_find() {
        let mut uf = super::Xj147UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_147_uf_union_connected() {
        let mut uf = super::Xj147UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_147_uf_component_count() {
        let mut uf = super::Xj147UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_147_uf_component_size() {
        let mut uf = super::Xj147UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_147_uf_largest_component() {
        let mut uf = super::Xj147UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_147_uf_many_elements() {
        let mut uf = super::Xj147UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_147_uf_separate_components() {
        let mut uf = super::Xj147UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_147_uf_path_compression() {
        let mut uf = super::Xj147UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_147_bt_insert_get() {
        let mut bt = super::Xj147BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_147_bt_contains_len() {
        let mut bt = super::Xj147BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_147_bt_replace() {
        let mut bt = super::Xj147BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_147_bt_remove() {
        let mut bt = super::Xj147BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_147_bt_keys_values() {
        let mut bt = super::Xj147BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_147_bt_range() {
        let mut bt = super::Xj147BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_147_bt_min_max() {
        let mut bt = super::Xj147BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_147_bt_many_inserts() {
        let mut bt = super::Xj147BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_148 segment tree tests ---

    #[test]
    fn xk_148_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk148SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_148_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk148SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_148_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk148SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_148_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk148SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_148_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk148SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_148_st_single_element() {
        let data = vec![42];
        let st = super::Xk148SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_148_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk148SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_148_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk148SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_148 disjoint intervals tests ---

    #[test]
    fn xk_148_di_add_and_count() {
        let mut di = super::Xk148DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_148_di_merge_overlap() {
        let mut di = super::Xk148DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_148_di_contains() {
        let mut di = super::Xk148DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_148_di_remove() {
        let mut di = super::Xk148DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_148_di_covered_length() {
        let mut di = super::Xk148DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_148_di_gaps() {
        let mut di = super::Xk148DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_148_di_merge_adjacent() {
        let mut di = super::Xk148DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_148_di_empty() {
        let di = super::Xk148DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_147_rope_new_empty() {
        let rope = super::Xl147Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_147_rope_from_str() {
        let rope = super::Xl147Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_147_rope_insert_at() {
        let mut rope = super::Xl147Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_147_rope_delete_range() {
        let mut rope = super::Xl147Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_147_rope_char_at() {
        let rope = super::Xl147Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_147_rope_split_concat() {
        let rope = super::Xl147Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_147_rope_line_count() {
        let rope = super::Xl147Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_147_rope_line_at() {
        let rope = super::Xl147Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_147_sa_build_and_search() {
        let sa = super::Xl147SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_147_sa_count() {
        let sa = super::Xl147SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_147_sa_longest_repeated() {
        let sa = super::Xl147SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_147_sa_all_positions() {
        let sa = super::Xl147SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_147_sa_len() {
        let sa = super::Xl147SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_147_sa_empty() {
        let sa = super::Xl147SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_147_rope_slice() {
        let rope = super::Xl147Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_147_sa_search_start() {
        let sa = super::Xl147SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_147_sparse_set_get() {
        let mut m = super::Xm147MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_147_sparse_row_col() {
        let mut m = super::Xm147MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_147_sparse_transpose() {
        let mut m = super::Xm147MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_147_sparse_multiply_vec() {
        let mut m = super::Xm147MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_147_sparse_nnz_density() {
        let mut m = super::Xm147MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_147_sparse_clear() {
        let mut m = super::Xm147MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_147_sparse_overwrite_zero() {
        let mut m = super::Xm147MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_147_tokenizer_basic() {
        let t = super::Xm147Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_147_tokenizer_count() {
        let t = super::Xm147Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_147_tokenizer_unique() {
        let t = super::Xm147Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_147_tokenizer_frequency() {
        let t = super::Xm147Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_147_tokenizer_delimiter() {
        let t = super::Xm147Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_147_tokenizer_whitespace() {
        let t = super::Xm147Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_147_tokenizer_empty() {
        let t = super::Xm147Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 147 ----

    #[test]
    fn xn_147_fenwick_prefix_sum() {
        let mut ft = super::Xn147Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_147_fenwick_range_sum() {
        let mut ft = super::Xn147Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_147_fenwick_point_query() {
        let mut ft = super::Xn147Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_147_fenwick_len() {
        let ft = super::Xn147Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_147_fenwick_multiple_updates() {
        let mut ft = super::Xn147Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_147_fenwick_single_element() {
        let mut ft = super::Xn147Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_147_fenwick_find_kth() {
        let mut ft = super::Xn147Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_147_fenwick_negative_delta() {
        let mut ft = super::Xn147Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 147 ----

    #[test]
    fn xn_147_avl_insert_get() {
        let mut m = super::Xn147AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_147_avl_remove() {
        let mut m = super::Xn147AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_147_avl_in_order() {
        let mut m = super::Xn147AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_147_avl_min_max() {
        let mut m = super::Xn147AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_147_avl_floor_ceiling() {
        let mut m = super::Xn147AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_147_avl_height_balanced() {
        let mut m = super::Xn147AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_147_avl_overwrite() {
        let mut m = super::Xn147AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_147_avl_empty() {
        let m: super::Xn147AVL<i32, i32> = super::Xn147AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo147RedBlack tests ---

    #[test]
    fn xo_147_rb_insert_and_get() {
        let mut tree = super::Xo147RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_147_rb_len_and_empty() {
        let mut tree = super::Xo147RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_147_rb_min_max() {
        let mut tree = super::Xo147RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_147_rb_contains() {
        let mut tree = super::Xo147RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_147_rb_remove() {
        let mut tree = super::Xo147RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_147_rb_in_order() {
        let mut tree = super::Xo147RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_147_rb_black_height() {
        let mut tree = super::Xo147RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_147_rb_overwrite() {
        let mut tree = super::Xo147RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo147ConsistentHash tests ---

    #[test]
    fn xo_147_ch_add_and_count() {
        let mut ring = super::Xo147ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_147_ch_remove_node() {
        let mut ring = super::Xo147ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_147_ch_get_node() {
        let mut ring = super::Xo147ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_147_ch_empty_ring() {
        let ring = super::Xo147ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_147_ch_distribution() {
        let mut ring = super::Xo147ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_147_ch_rebalance() {
        let mut ring = super::Xo147ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_147_ch_virtual_nodes() {
        let mut ring = super::Xo147ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_147_ch_consistent_lookup() {
        let mut ring = super::Xo147ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
