//! Settings sync service managing the full sync lifecycle.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::extensions::ExtensionSyncData;
use crate::keybindings::KeybindingEntry;
use crate::merge::{ConflictEntry, MergeResult, merge_settings};
use crate::profile::SyncProfile;
use crate::snippets::SnippetFile;
use crate::state::SyncState;
use crate::{SyncError, SyncResource, SyncStatus};

/// A bundle of all syncable data for export/import.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncBundle {
    pub settings: Option<Value>,
    pub keybindings: Option<Vec<KeybindingEntry>>,
    pub extensions: Option<Vec<ExtensionSyncData>>,
    pub snippets: Option<HashMap<String, SnippetFile>>,
    pub global_state: Option<Value>,
}

impl SyncBundle {
    pub fn empty() -> Self {
        Self {
            settings: None,
            keybindings: None,
            extensions: None,
            snippets: None,
            global_state: None,
        }
    }
}

/// Callback type for sync status change events.
type StatusCallback = Box<dyn Fn(&SyncStatus) + Send + Sync>;

/// The settings sync service coordinating all sync operations.
pub struct SettingsSyncService {
    profile: SyncProfile,
    state: SyncState,
    status: SyncStatus,
    local_data: SyncBundle,
    conflicts: Vec<ConflictEntry>,
    listeners: Vec<StatusCallback>,
}

impl SettingsSyncService {
    pub fn new(profile: SyncProfile) -> Self {
        Self {
            profile,
            state: SyncState::new(),
            status: SyncStatus::Idle,
            local_data: SyncBundle::empty(),
            conflicts: Vec::new(),
            listeners: Vec::new(),
        }
    }

    /// Register a callback invoked whenever the sync status changes.
    pub fn on_did_change_sync_status(
        &mut self,
        callback: impl Fn(&SyncStatus) + Send + Sync + 'static,
    ) {
        self.listeners.push(Box::new(callback));
    }

    fn set_status(&mut self, status: SyncStatus) {
        self.status = status;
        for listener in &self.listeners {
            listener(&self.status);
        }
    }

    pub fn is_syncing(&self) -> bool {
        self.status == SyncStatus::Syncing
    }

    pub fn last_sync_time(&self) -> Option<u64> {
        self.state.last_sync_time
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflicts(&self) -> &[ConflictEntry] {
        &self.conflicts
    }

    pub fn status(&self) -> &SyncStatus {
        &self.status
    }

    pub fn profile(&self) -> &SyncProfile {
        &self.profile
    }

    pub fn state(&self) -> &SyncState {
        &self.state
    }

    /// Set local data that will be used as the local side during sync.
    pub fn set_local_data(&mut self, data: SyncBundle) {
        self.local_data = data;
    }

    /// Trigger a manual sync against the given remote data.
    ///
    /// Returns the merged bundle on clean merge or the first conflict set on
    /// conflict.
    pub fn sync_now(
        &mut self,
        remote: &SyncBundle,
        base: &SyncBundle,
        now: u64,
    ) -> Result<SyncBundle, SyncError> {
        if self.is_syncing() {
            return Err(SyncError::SyncInProgress);
        }
        self.set_status(SyncStatus::Syncing);
        self.conflicts.clear();

        let mut result = SyncBundle::empty();

        // Merge settings.
        if self.profile.settings {
            let local = self
                .local_data
                .settings
                .clone()
                .unwrap_or(Value::Object(Default::default()));
            let remote_val = remote
                .settings
                .clone()
                .unwrap_or(Value::Object(Default::default()));
            let base_val = base
                .settings
                .clone()
                .unwrap_or(Value::Object(Default::default()));
            match merge_settings(&local, &remote_val, &base_val) {
                MergeResult::Clean(v) => result.settings = Some(v),
                MergeResult::Conflict(c) => {
                    self.conflicts.extend(c);
                }
            }
        }

        // Pass through other resources.
        if self.profile.keybindings {
            result.keybindings = self.local_data.keybindings.clone();
        }
        if self.profile.extensions {
            result.extensions = self.local_data.extensions.clone();
        }
        if self.profile.snippets {
            result.snippets = self.local_data.snippets.clone();
        }
        if self.profile.ui_state {
            result.global_state = self.local_data.global_state.clone();
        }

        if self.has_conflicts() {
            self.set_status(SyncStatus::Conflict);
            return Err(SyncError::ConflictDetected);
        }

        self.state.record_sync(now);
        self.set_status(SyncStatus::UpToDate);
        Ok(result)
    }

    /// Export all local sync data as a bundle.
    pub fn export_sync_data(&self) -> SyncBundle {
        self.local_data.clone()
    }

    /// Import a sync bundle, replacing local data.
    pub fn import_sync_data(&mut self, bundle: SyncBundle) {
        self.local_data = bundle;
        self.conflicts.clear();
        self.set_status(SyncStatus::Idle);
    }

    /// Mark a resource dirty in the sync state.
    pub fn mark_dirty(&mut self, resource: SyncResource) {
        if let Some(rs) = self.state.get_resource_mut(&resource) {
            rs.mark_dirty();
        }
    }
}
