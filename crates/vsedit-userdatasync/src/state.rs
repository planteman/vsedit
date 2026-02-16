//! Local sync state tracking.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::SyncResource;

/// Per-resource sync state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSyncState {
    pub version: u64,
    pub content_hash: String,
    pub last_modified: u64,
    pub is_dirty: bool,
}

impl ResourceSyncState {
    pub fn new(version: u64, content_hash: String, last_modified: u64) -> Self {
        Self {
            version,
            content_hash,
            last_modified,
            is_dirty: false,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }
}

/// Overall sync state across all resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub last_sync_time: Option<u64>,
    pub resources: HashMap<SyncResource, ResourceSyncState>,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            last_sync_time: None,
            resources: HashMap::new(),
        }
    }

    pub fn set_resource(&mut self, resource: SyncResource, state: ResourceSyncState) {
        self.resources.insert(resource, state);
    }

    pub fn get_resource(&self, resource: &SyncResource) -> Option<&ResourceSyncState> {
        self.resources.get(resource)
    }

    pub fn get_resource_mut(
        &mut self,
        resource: &SyncResource,
    ) -> Option<&mut ResourceSyncState> {
        self.resources.get_mut(resource)
    }

    /// Returns resources that have been modified since last sync.
    pub fn dirty_resources(&self) -> Vec<&SyncResource> {
        self.resources
            .iter()
            .filter(|(_, s)| s.is_dirty)
            .map(|(r, _)| r)
            .collect()
    }

    pub fn record_sync(&mut self, time: u64) {
        self.last_sync_time = Some(time);
        for state in self.resources.values_mut() {
            state.mark_clean();
        }
    }
}

impl Default for SyncState {
    fn default() -> Self {
        Self::new()
    }
}
