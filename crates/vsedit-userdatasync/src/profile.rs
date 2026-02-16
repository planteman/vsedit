//! Sync profile model.

use serde::{Deserialize, Serialize};

/// A sync profile controlling which resources participate in sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncProfile {
    pub id: String,
    pub name: String,
    pub settings: bool,
    pub keybindings: bool,
    pub extensions: bool,
    pub ui_state: bool,
    pub snippets: bool,
}

impl SyncProfile {
    /// Create a new profile that syncs everything.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            settings: true,
            keybindings: true,
            extensions: true,
            ui_state: true,
            snippets: true,
        }
    }

    /// Create a profile that syncs nothing.
    pub fn empty(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            settings: false,
            keybindings: false,
            extensions: false,
            ui_state: false,
            snippets: false,
        }
    }
}
