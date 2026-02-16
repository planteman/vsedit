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
}

/// Service for tunnel workbench functionality.
pub struct TunnelWorkbenchService {
    tunnels: Vec<ManagedTunnel>,
    next_id: u64,
}

impl TunnelWorkbenchService {
    pub fn new() -> Self {
        Self {
            tunnels: Vec::new(),
            next_id: 1,
        }
    }

    /// Creates a tunnel and returns its id.
    pub fn create_tunnel(&mut self, descriptor: TunnelDescriptor) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.tunnels.push(ManagedTunnel {
            descriptor,
            state: TunnelState::Connecting,
            id,
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
}
