//! Extension install/update management.
//!
//! RPC bridge between the extension host and the main thread for extension management.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_mgmt";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MgmtMessage {
    GetInstalled,
    GetExtension {
        extension_id: String,
    },
    Install {
        extension_id: String,
    },
    Uninstall {
        extension_id: String,
    },
    Enable {
        extension_id: String,
    },
    Disable {
        extension_id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionKind {
    Ui,
    Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionInfo {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub publisher: String,
    pub kind: ExtensionKind,
    pub is_enabled: bool,
    pub extension_path: Option<String>,
}

// ── Bridge ──

pub struct MgmtBridge {
    extensions: Vec<ExtensionInfo>,
}

impl MgmtBridge {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    pub fn install(&mut self, ext: ExtensionInfo) {
        if !self.extensions.iter().any(|e| e.id == ext.id) {
            self.extensions.push(ext);
        }
    }

    pub fn uninstall(&mut self, id: &str) -> bool {
        let before = self.extensions.len();
        self.extensions.retain(|e| e.id != id);
        self.extensions.len() < before
    }

    pub fn get_extension(&self, id: &str) -> Option<&ExtensionInfo> {
        self.extensions.iter().find(|e| e.id == id)
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            ext.is_enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn list_installed(&self) -> &[ExtensionInfo] {
        &self.extensions
    }

    pub fn handle_message(&mut self, msg: &MgmtMessage) -> serde_json::Value {
        match msg {
            MgmtMessage::GetInstalled => {
                let ids: Vec<&str> = self.extensions.iter().map(|e| e.id.as_str()).collect();
                serde_json::json!({"extensions": ids})
            }
            MgmtMessage::GetExtension { extension_id } => {
                let found = self.get_extension(extension_id).is_some();
                serde_json::json!({"found": found, "id": extension_id})
            }
            MgmtMessage::Install { extension_id } => {
                // In real impl would download/install; here we acknowledge
                serde_json::json!({"installing": extension_id})
            }
            MgmtMessage::Uninstall { extension_id } => {
                let ok = self.uninstall(extension_id);
                serde_json::json!({"uninstalled": ok})
            }
            MgmtMessage::Enable { extension_id } => {
                let ok = self.set_enabled(extension_id, true);
                serde_json::json!({"enabled": ok})
            }
            MgmtMessage::Disable { extension_id } => {
                let ok = self.set_enabled(extension_id, false);
                serde_json::json!({"disabled": ok})
            }
        }
    }
}

impl Default for MgmtBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the mgmt extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ext() -> ExtensionInfo {
        ExtensionInfo {
            id: "publisher.extension".into(),
            display_name: "My Extension".into(),
            version: "1.0.0".into(),
            publisher: "publisher".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: Some("/ext/path".into()),
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = MgmtMessage::Install {
            extension_id: "publisher.ext".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: MgmtMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn extension_info_serialization() {
        let ext = test_ext();
        let json = serde_json::to_string(&ext).unwrap();
        let back: ExtensionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, back);
    }

    #[test]
    fn bridge_install_and_uninstall() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        assert!(bridge.get_extension("publisher.extension").is_some());
        assert!(bridge.uninstall("publisher.extension"));
        assert!(bridge.get_extension("publisher.extension").is_none());
    }

    #[test]
    fn bridge_enable_disable() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.set_enabled("publisher.extension", false);
        assert!(!bridge.get_extension("publisher.extension").unwrap().is_enabled);
        bridge.set_enabled("publisher.extension", true);
        assert!(bridge.get_extension("publisher.extension").unwrap().is_enabled);
    }

    #[test]
    fn bridge_uninstall_unknown() {
        let mut bridge = MgmtBridge::new();
        assert!(!bridge.uninstall("nope"));
    }
}
