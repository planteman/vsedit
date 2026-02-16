//! Port forwarding service.

/// Privacy level for a tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelPrivacy {
    Private,
    Public,
}

/// Protocol used by a tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelProtocol {
    Http,
    Https,
    Tcp,
}

/// Describes a tunnel to be created.
#[derive(Debug, Clone)]
pub struct TunnelDescriptor {
    pub remote_address: String,
    pub local_port: u16,
    pub privacy: TunnelPrivacy,
    pub protocol: TunnelProtocol,
    pub label: Option<String>,
}

/// Current state of a tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelState {
    Connecting,
    Connected,
    Closed,
    Error(String),
}

/// A tunnel managed by the service.
#[derive(Debug)]
pub struct ManagedTunnel {
    pub descriptor: TunnelDescriptor,
    pub state: TunnelState,
    pub id: u64,
    pub created_at: u64,
    pub bytes_transferred: u64,
}

/// Aggregate statistics about managed tunnels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelStats {
    pub active_count: usize,
    pub total_created: usize,
    pub total_bytes_transferred: u64,
    pub http_count: usize,
    pub https_count: usize,
    pub tcp_count: usize,
}

/// Trait for external tunnel providers.
pub trait TunnelProvider {
    /// Create the underlying tunnel resource. Returns `true` on success.
    fn create_tunnel(&self, descriptor: &TunnelDescriptor) -> bool {
        let _ = descriptor;
        true
    }

    /// Dispose of the underlying tunnel resource. Returns `true` on success.
    fn dispose_tunnel(&self, id: u64) -> bool {
        let _ = id;
        true
    }
}

/// Service for tunnel workbench functionality.
pub struct TunnelWorkbenchService {
    tunnels: Vec<ManagedTunnel>,
    next_id: u64,
    total_created: usize,
    clock: u64,
}

impl TunnelWorkbenchService {
    pub fn new() -> Self {
        Self {
            tunnels: Vec::new(),
            next_id: 1,
            total_created: 0,
            clock: 1,
        }
    }

    /// Creates a tunnel and returns its id.
    pub fn create_tunnel(&mut self, descriptor: TunnelDescriptor) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let created_at = self.clock;
        self.clock += 1;
        self.total_created += 1;
        self.tunnels.push(ManagedTunnel {
            descriptor,
            state: TunnelState::Connecting,
            id,
            created_at,
            bytes_transferred: 0,
        });
        id
    }

    /// Closes a tunnel by id. Returns `true` if it existed.
    pub fn close_tunnel(&mut self, id: u64) -> bool {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.state = TunnelState::Closed;
            true
        } else {
            false
        }
    }

    /// Sets the state of a tunnel by id.
    pub fn set_state(&mut self, id: u64, state: TunnelState) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.state = state;
        }
    }

    pub fn get_tunnel(&self, id: u64) -> Option<&ManagedTunnel> {
        self.tunnels.iter().find(|t| t.id == id)
    }

    /// Returns tunnels that are not closed.
    pub fn get_active_tunnels(&self) -> Vec<&ManagedTunnel> {
        self.tunnels
            .iter()
            .filter(|t| t.state != TunnelState::Closed)
            .collect()
    }

    pub fn tunnel_count(&self) -> usize {
        self.tunnels.len()
    }

    /// Returns aggregate statistics about all tunnels.
    pub fn get_stats(&self) -> TunnelStats {
        let active_count = self.tunnels.iter().filter(|t| t.state != TunnelState::Closed).count();
        let total_bytes_transferred = self.tunnels.iter().map(|t| t.bytes_transferred).sum();
        let http_count = self
            .tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == TunnelProtocol::Http)
            .count();
        let https_count = self
            .tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == TunnelProtocol::Https)
            .count();
        let tcp_count = self
            .tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == TunnelProtocol::Tcp)
            .count();
        TunnelStats {
            active_count,
            total_created: self.total_created,
            total_bytes_transferred,
            http_count,
            https_count,
            tcp_count,
        }
    }

    /// Returns tunnels matching the given protocol.
    pub fn get_tunnels_by_protocol(&self, protocol: TunnelProtocol) -> Vec<&ManagedTunnel> {
        self.tunnels
            .iter()
            .filter(|t| t.descriptor.protocol == protocol)
            .collect()
    }

    /// Returns tunnels matching the given privacy level.
    pub fn get_tunnels_by_privacy(&self, privacy: TunnelPrivacy) -> Vec<&ManagedTunnel> {
        self.tunnels
            .iter()
            .filter(|t| t.descriptor.privacy == privacy)
            .collect()
    }

    /// Finds a tunnel by its local port.
    pub fn find_by_port(&self, port: u16) -> Option<&ManagedTunnel> {
        self.tunnels.iter().find(|t| t.descriptor.local_port == port)
    }

    /// Records bytes transferred on a tunnel.
    pub fn record_transfer(&mut self, id: u64, bytes: u64) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.bytes_transferred += bytes;
        }
    }

    /// Closes all active tunnels, returning the number closed.
    pub fn close_all(&mut self) -> usize {
        let mut count = 0;
        for t in &mut self.tunnels {
            if t.state != TunnelState::Closed {
                t.state = TunnelState::Closed;
                count += 1;
            }
        }
        count
    }

    /// Removes tunnels in the `Closed` state, returning the number removed.
    pub fn remove_closed(&mut self) -> usize {
        let before = self.tunnels.len();
        self.tunnels.retain(|t| t.state != TunnelState::Closed);
        before - self.tunnels.len()
    }

    /// Renames a tunnel by setting its label.
    pub fn rename_tunnel(&mut self, id: u64, label: &str) {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.id == id) {
            t.descriptor.label = Some(label.to_string());
        }
    }
}

impl Default for TunnelWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_descriptor() -> TunnelDescriptor {
        TunnelDescriptor {
            remote_address: "tunnel.example.com:8080".into(),
            local_port: 3000,
            privacy: TunnelPrivacy::Private,
            protocol: TunnelProtocol::Http,
            label: None,
        }
    }

    #[test]
    fn create_and_lookup() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        assert_eq!(svc.tunnel_count(), 1);
        let t = svc.get_tunnel(id).unwrap();
        assert_eq!(t.state, TunnelState::Connecting);
        assert_eq!(t.descriptor.local_port, 3000);
    }

    #[test]
    fn close_tunnel_filters_active() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        let id2 = svc.create_tunnel(TunnelDescriptor {
            label: Some("web".into()),
            ..sample_descriptor()
        });
        svc.set_state(id1, TunnelState::Connected);
        svc.set_state(id2, TunnelState::Connected);
        assert_eq!(svc.get_active_tunnels().len(), 2);

        assert!(svc.close_tunnel(id1));
        assert_eq!(svc.get_active_tunnels().len(), 1);
        assert_eq!(svc.tunnel_count(), 2);
    }

    #[test]
    fn close_nonexistent_returns_false() {
        let mut svc = TunnelWorkbenchService::new();
        assert!(!svc.close_tunnel(999));
    }

    #[test]
    fn set_error_state() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        svc.set_state(id, TunnelState::Error("timeout".into()));
        let t = svc.get_tunnel(id).unwrap();
        assert_eq!(t.state, TunnelState::Error("timeout".into()));
    }

    #[test]
    fn get_stats_empty() {
        let svc = TunnelWorkbenchService::new();
        let stats = svc.get_stats();
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.total_created, 0);
        assert_eq!(stats.total_bytes_transferred, 0);
        assert_eq!(stats.http_count, 0);
        assert_eq!(stats.https_count, 0);
        assert_eq!(stats.tcp_count, 0);
    }

    #[test]
    fn get_stats_mixed_tunnels() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor()); // Http
        let _id2 = svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Https,
            local_port: 3001,
            ..sample_descriptor()
        });
        let _id3 = svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Tcp,
            local_port: 3002,
            ..sample_descriptor()
        });
        svc.record_transfer(id1, 1024);
        svc.close_tunnel(id1);

        let stats = svc.get_stats();
        assert_eq!(stats.active_count, 2);
        assert_eq!(stats.total_created, 3);
        assert_eq!(stats.total_bytes_transferred, 1024);
        assert_eq!(stats.http_count, 1);
        assert_eq!(stats.https_count, 1);
        assert_eq!(stats.tcp_count, 1);
    }

    #[test]
    fn filter_by_protocol() {
        let mut svc = TunnelWorkbenchService::new();
        svc.create_tunnel(sample_descriptor());
        svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Tcp,
            local_port: 4000,
            ..sample_descriptor()
        });
        svc.create_tunnel(TunnelDescriptor {
            protocol: TunnelProtocol::Tcp,
            local_port: 4001,
            ..sample_descriptor()
        });
        assert_eq!(svc.get_tunnels_by_protocol(TunnelProtocol::Tcp).len(), 2);
        assert_eq!(svc.get_tunnels_by_protocol(TunnelProtocol::Http).len(), 1);
        assert_eq!(svc.get_tunnels_by_protocol(TunnelProtocol::Https).len(), 0);
    }

    #[test]
    fn filter_by_privacy() {
        let mut svc = TunnelWorkbenchService::new();
        svc.create_tunnel(sample_descriptor()); // Private
        svc.create_tunnel(TunnelDescriptor {
            privacy: TunnelPrivacy::Public,
            local_port: 5000,
            ..sample_descriptor()
        });
        assert_eq!(svc.get_tunnels_by_privacy(TunnelPrivacy::Private).len(), 1);
        assert_eq!(svc.get_tunnels_by_privacy(TunnelPrivacy::Public).len(), 1);
    }

    #[test]
    fn find_by_port_hit_and_miss() {
        let mut svc = TunnelWorkbenchService::new();
        svc.create_tunnel(sample_descriptor());
        assert!(svc.find_by_port(3000).is_some());
        assert!(svc.find_by_port(9999).is_none());
    }

    #[test]
    fn record_transfer_accumulates() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        svc.record_transfer(id, 100);
        svc.record_transfer(id, 200);
        assert_eq!(svc.get_tunnel(id).unwrap().bytes_transferred, 300);
    }

    #[test]
    fn record_transfer_nonexistent_is_noop() {
        let mut svc = TunnelWorkbenchService::new();
        svc.record_transfer(999, 100); // should not panic
        assert_eq!(svc.tunnel_count(), 0);
    }

    #[test]
    fn close_all_returns_count() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        svc.create_tunnel(TunnelDescriptor {
            local_port: 3001,
            ..sample_descriptor()
        });
        svc.close_tunnel(id1);
        // id1 already closed, so close_all should close only the second one
        assert_eq!(svc.close_all(), 1);
        assert_eq!(svc.get_active_tunnels().len(), 0);
    }

    #[test]
    fn remove_closed_garbage_collects() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        svc.create_tunnel(TunnelDescriptor {
            local_port: 3001,
            ..sample_descriptor()
        });
        svc.close_tunnel(id1);
        assert_eq!(svc.remove_closed(), 1);
        assert_eq!(svc.tunnel_count(), 1);
        // removing again should return 0
        assert_eq!(svc.remove_closed(), 0);
    }

    #[test]
    fn rename_tunnel_sets_label() {
        let mut svc = TunnelWorkbenchService::new();
        let id = svc.create_tunnel(sample_descriptor());
        assert!(svc.get_tunnel(id).unwrap().descriptor.label.is_none());
        svc.rename_tunnel(id, "my-tunnel");
        assert_eq!(
            svc.get_tunnel(id).unwrap().descriptor.label.as_deref(),
            Some("my-tunnel")
        );
    }

    #[test]
    fn rename_nonexistent_is_noop() {
        let mut svc = TunnelWorkbenchService::new();
        svc.rename_tunnel(999, "ghost"); // should not panic
    }

    #[test]
    fn created_at_increments() {
        let mut svc = TunnelWorkbenchService::new();
        let id1 = svc.create_tunnel(sample_descriptor());
        let id2 = svc.create_tunnel(TunnelDescriptor {
            local_port: 3001,
            ..sample_descriptor()
        });
        let t1 = svc.get_tunnel(id1).unwrap();
        let t2 = svc.get_tunnel(id2).unwrap();
        assert!(t2.created_at > t1.created_at);
    }

    struct NoopProvider;
    impl TunnelProvider for NoopProvider {}

    #[test]
    fn tunnel_provider_defaults() {
        let provider = NoopProvider;
        assert!(provider.create_tunnel(&sample_descriptor()));
        assert!(provider.dispose_tunnel(1));
    }
}
