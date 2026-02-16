//! Settings sync across devices.

#[derive(Debug, Clone, PartialEq)]
pub enum SyncResource {
    Settings,
    Keybindings,
    Snippets,
    Extensions,
    UIState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Conflict,
    Error(String),
    UpToDate,
}

#[derive(Debug, Clone)]
pub struct SyncEntry {
    pub resource: SyncResource,
    pub local_version: u64,
    pub remote_version: Option<u64>,
    pub status: SyncStatus,
}

pub struct SyncService {
    entries: Vec<SyncEntry>,
    enabled: bool,
}

impl SyncService {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            enabled: false,
        }
    }

    pub fn add_resource(&mut self, resource: SyncResource) {
        if !self.entries.iter().any(|e| e.resource == resource) {
            self.entries.push(SyncEntry {
                resource,
                local_version: 0,
                remote_version: None,
                status: SyncStatus::Idle,
            });
        }
    }

    pub fn set_status(&mut self, resource: &SyncResource, status: SyncStatus) {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.resource == resource) {
            entry.status = status;
        }
    }

    pub fn get_status(&self, resource: &SyncResource) -> Option<&SyncStatus> {
        self.entries
            .iter()
            .find(|e| &e.resource == resource)
            .map(|e| &e.status)
    }

    pub fn has_conflicts(&self) -> bool {
        self.entries.iter().any(|e| e.status == SyncStatus::Conflict)
    }

    pub fn is_syncing(&self) -> bool {
        self.entries.iter().any(|e| e.status == SyncStatus::Syncing)
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for SyncService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_status() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Settings);
        assert_eq!(
            svc.get_status(&SyncResource::Settings),
            Some(&SyncStatus::Idle)
        );
        svc.set_status(&SyncResource::Settings, SyncStatus::UpToDate);
        assert_eq!(
            svc.get_status(&SyncResource::Settings),
            Some(&SyncStatus::UpToDate)
        );
    }

    #[test]
    fn conflicts_and_syncing() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Keybindings);
        svc.add_resource(SyncResource::Snippets);
        assert!(!svc.has_conflicts());
        svc.set_status(&SyncResource::Keybindings, SyncStatus::Conflict);
        assert!(svc.has_conflicts());
        svc.set_status(&SyncResource::Snippets, SyncStatus::Syncing);
        assert!(svc.is_syncing());
    }

    #[test]
    fn enable_disable() {
        let mut svc = SyncService::new();
        assert!(!svc.is_enabled());
        svc.enable();
        assert!(svc.is_enabled());
        svc.disable();
        assert!(!svc.is_enabled());
    }
}
