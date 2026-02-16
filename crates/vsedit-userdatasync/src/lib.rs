//! Settings sync across devices.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum SyncResource {
    Settings,
    Keybindings,
    Snippets,
    Extensions,
    UIState,
}

impl fmt::Display for SyncResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncResource::Settings => write!(f, "Settings"),
            SyncResource::Keybindings => write!(f, "Keybindings"),
            SyncResource::Snippets => write!(f, "Snippets"),
            SyncResource::Extensions => write!(f, "Extensions"),
            SyncResource::UIState => write!(f, "UI State"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Conflict,
    Error(String),
    UpToDate,
}

impl fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncStatus::Idle => write!(f, "Idle"),
            SyncStatus::Syncing => write!(f, "Syncing"),
            SyncStatus::Conflict => write!(f, "Conflict"),
            SyncStatus::Error(msg) => write!(f, "Error: {msg}"),
            SyncStatus::UpToDate => write!(f, "Up to Date"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncError {
    ResourceNotFound(String),
    AlreadyExists(String),
    SyncInProgress,
    NotEnabled,
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::ResourceNotFound(name) => write!(f, "resource not found: {name}"),
            SyncError::AlreadyExists(name) => write!(f, "resource already exists: {name}"),
            SyncError::SyncInProgress => write!(f, "sync already in progress"),
            SyncError::NotEnabled => write!(f, "sync is not enabled"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyncEntry {
    pub resource: SyncResource,
    pub local_version: u64,
    pub remote_version: Option<u64>,
    pub status: SyncStatus,
}

impl SyncEntry {
    /// Returns true if local and remote versions match.
    pub fn is_in_sync(&self) -> bool {
        self.remote_version == Some(self.local_version)
    }
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

    pub fn remove_resource(&mut self, resource: &SyncResource) -> Result<(), SyncError> {
        let idx = self
            .entries
            .iter()
            .position(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        self.entries.remove(idx);
        Ok(())
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

    /// Begin syncing a resource. Fails if sync is disabled, the resource is
    /// not found, or the resource is already syncing.
    pub fn try_sync(&mut self, resource: &SyncResource) -> Result<(), SyncError> {
        if !self.enabled {
            return Err(SyncError::NotEnabled);
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        if entry.status == SyncStatus::Syncing {
            return Err(SyncError::SyncInProgress);
        }
        entry.status = SyncStatus::Syncing;
        Ok(())
    }

    /// Resolve a conflict by setting the resource status to UpToDate.
    pub fn resolve_conflict(&mut self, resource: &SyncResource) -> Result<(), SyncError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        entry.status = SyncStatus::UpToDate;
        Ok(())
    }

    pub fn get_all_entries(&self) -> &[SyncEntry] {
        &self.entries
    }

    /// Update the local and/or remote version of a resource.
    pub fn update_version(
        &mut self,
        resource: &SyncResource,
        local: Option<u64>,
        remote: Option<u64>,
    ) -> Result<(), SyncError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| &e.resource == resource)
            .ok_or_else(|| SyncError::ResourceNotFound(resource.to_string()))?;
        if let Some(v) = local {
            entry.local_version = v;
        }
        if let Some(v) = remote {
            entry.remote_version = Some(v);
        }
        Ok(())
    }

    /// Returns true if any resource has a local version greater than its
    /// remote version.
    pub fn needs_sync(&self) -> bool {
        self.entries.iter().any(|e| match e.remote_version {
            Some(rv) => e.local_version > rv,
            None => true,
        })
    }

    pub fn resource_count(&self) -> usize {
        self.entries.len()
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

    #[test]
    fn sync_error_display() {
        assert_eq!(
            SyncError::ResourceNotFound("Settings".into()).to_string(),
            "resource not found: Settings"
        );
        assert_eq!(
            SyncError::AlreadyExists("Settings".into()).to_string(),
            "resource already exists: Settings"
        );
        assert_eq!(
            SyncError::SyncInProgress.to_string(),
            "sync already in progress"
        );
        assert_eq!(SyncError::NotEnabled.to_string(), "sync is not enabled");
    }

    #[test]
    fn resource_and_status_display() {
        assert_eq!(SyncResource::Settings.to_string(), "Settings");
        assert_eq!(SyncResource::UIState.to_string(), "UI State");
        assert_eq!(SyncStatus::Idle.to_string(), "Idle");
        assert_eq!(SyncStatus::UpToDate.to_string(), "Up to Date");
        assert_eq!(
            SyncStatus::Error("oops".into()).to_string(),
            "Error: oops"
        );
    }

    #[test]
    fn remove_resource_success_and_not_found() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Extensions);
        assert_eq!(svc.resource_count(), 1);
        assert!(svc.remove_resource(&SyncResource::Extensions).is_ok());
        assert_eq!(svc.resource_count(), 0);
        assert_eq!(
            svc.remove_resource(&SyncResource::Extensions),
            Err(SyncError::ResourceNotFound("Extensions".into()))
        );
    }

    #[test]
    fn try_sync_requires_enabled() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Settings);
        assert_eq!(
            svc.try_sync(&SyncResource::Settings),
            Err(SyncError::NotEnabled)
        );
        svc.enable();
        assert!(svc.try_sync(&SyncResource::Settings).is_ok());
        assert_eq!(
            svc.get_status(&SyncResource::Settings),
            Some(&SyncStatus::Syncing)
        );
    }

    #[test]
    fn try_sync_prevents_double_sync() {
        let mut svc = SyncService::new();
        svc.enable();
        svc.add_resource(SyncResource::Snippets);
        svc.try_sync(&SyncResource::Snippets).unwrap();
        assert_eq!(
            svc.try_sync(&SyncResource::Snippets),
            Err(SyncError::SyncInProgress)
        );
    }

    #[test]
    fn try_sync_resource_not_found() {
        let mut svc = SyncService::new();
        svc.enable();
        assert_eq!(
            svc.try_sync(&SyncResource::UIState),
            Err(SyncError::ResourceNotFound("UI State".into()))
        );
    }

    #[test]
    fn resolve_conflict() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Keybindings);
        svc.set_status(&SyncResource::Keybindings, SyncStatus::Conflict);
        assert!(svc.has_conflicts());
        svc.resolve_conflict(&SyncResource::Keybindings).unwrap();
        assert!(!svc.has_conflicts());
        assert_eq!(
            svc.get_status(&SyncResource::Keybindings),
            Some(&SyncStatus::UpToDate)
        );
    }

    #[test]
    fn get_all_entries_and_resource_count() {
        let mut svc = SyncService::new();
        assert_eq!(svc.get_all_entries().len(), 0);
        svc.add_resource(SyncResource::Settings);
        svc.add_resource(SyncResource::Extensions);
        svc.add_resource(SyncResource::Snippets);
        assert_eq!(svc.resource_count(), 3);
        assert_eq!(svc.get_all_entries().len(), 3);
    }

    #[test]
    fn update_version_and_needs_sync() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::Settings);
        // No remote version yet — needs sync
        assert!(svc.needs_sync());
        svc.update_version(&SyncResource::Settings, Some(1), Some(1))
            .unwrap();
        assert!(!svc.needs_sync());
        // Bump local ahead of remote
        svc.update_version(&SyncResource::Settings, Some(2), None)
            .unwrap();
        assert!(svc.needs_sync());
    }

    #[test]
    fn entry_is_in_sync() {
        let mut svc = SyncService::new();
        svc.add_resource(SyncResource::UIState);
        let entry = &svc.get_all_entries()[0];
        assert!(!entry.is_in_sync()); // remote is None
        svc.update_version(&SyncResource::UIState, Some(5), Some(5))
            .unwrap();
        assert!(svc.get_all_entries()[0].is_in_sync());
        svc.update_version(&SyncResource::UIState, Some(6), None)
            .unwrap();
        assert!(!svc.get_all_entries()[0].is_in_sync());
    }

    #[test]
    fn default_trait() {
        let svc = SyncService::default();
        assert!(!svc.is_enabled());
        assert_eq!(svc.resource_count(), 0);
    }
}
