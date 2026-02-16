//! Extension install/update management.
//!
//! RPC bridge between the extension host and the main thread for extension management.

use std::collections::HashMap;

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
    #[serde(default)]
    pub dependencies: Vec<ExtensionDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionDependency {
    pub id: String,
    pub version_range: String,
}

/// Aggregate statistics about installed extensions.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionStats {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub by_kind: HashMap<String, usize>,
    pub by_publisher: HashMap<String, usize>,
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

    pub fn installed_count(&self) -> usize {
        self.extensions.len()
    }

    pub fn get_enabled_extensions(&self) -> Vec<&ExtensionInfo> {
        self.extensions.iter().filter(|e| e.is_enabled).collect()
    }

    pub fn get_disabled_extensions(&self) -> Vec<&ExtensionInfo> {
        self.extensions.iter().filter(|e| !e.is_enabled).collect()
    }

    pub fn get_extensions_by_publisher(&self, publisher: &str) -> Vec<&ExtensionInfo> {
        self.extensions
            .iter()
            .filter(|e| e.publisher == publisher)
            .collect()
    }

    pub fn get_extensions_by_kind(&self, kind: ExtensionKind) -> Vec<&ExtensionInfo> {
        self.extensions
            .iter()
            .filter(|e| e.kind == kind)
            .collect()
    }

    pub fn update_version(&mut self, id: &str, new_version: &str) -> bool {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            ext.version = new_version.to_string();
            true
        } else {
            false
        }
    }

    /// Returns `true` if the given extension declares `dep_id` as a dependency.
    pub fn has_dependency(&self, id: &str, dep_id: &str) -> bool {
        self.extensions
            .iter()
            .find(|e| e.id == id)
            .map_or(false, |e| e.dependencies.iter().any(|d| d.id == dep_id))
    }

    /// Returns every installed extension that lists `id` among its dependencies.
    pub fn get_dependents(&self, id: &str) -> Vec<&ExtensionInfo> {
        self.extensions
            .iter()
            .filter(|e| e.dependencies.iter().any(|d| d.id == id))
            .collect()
    }

    pub fn get_stats(&self) -> ExtensionStats {
        let total = self.extensions.len();
        let enabled = self.extensions.iter().filter(|e| e.is_enabled).count();
        let disabled = total - enabled;

        let mut by_kind: HashMap<String, usize> = HashMap::new();
        let mut by_publisher: HashMap<String, usize> = HashMap::new();

        for ext in &self.extensions {
            let kind_key = format!("{:?}", ext.kind);
            *by_kind.entry(kind_key).or_insert(0) += 1;
            *by_publisher.entry(ext.publisher.clone()).or_insert(0) += 1;
        }

        ExtensionStats {
            total,
            enabled,
            disabled,
            by_kind,
            by_publisher,
        }
    }

    /// Case-insensitive search across extension `id` and `display_name`.
    pub fn search_extensions(&self, query: &str) -> Vec<&ExtensionInfo> {
        let q = query.to_lowercase();
        self.extensions
            .iter()
            .filter(|e| {
                e.id.to_lowercase().contains(&q)
                    || e.display_name.to_lowercase().contains(&q)
            })
            .collect()
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
            dependencies: Vec::new(),
        }
    }

    fn test_ext_ui(id: &str, publisher: &str, enabled: bool) -> ExtensionInfo {
        ExtensionInfo {
            id: id.into(),
            display_name: format!("Ext {id}"),
            version: "0.1.0".into(),
            publisher: publisher.into(),
            kind: ExtensionKind::Ui,
            is_enabled: enabled,
            extension_path: None,
            dependencies: Vec::new(),
        }
    }

    fn test_ext_with_deps(id: &str, deps: Vec<(&str, &str)>) -> ExtensionInfo {
        ExtensionInfo {
            id: id.into(),
            display_name: format!("Ext {id}"),
            version: "1.0.0".into(),
            publisher: "acme".into(),
            kind: ExtensionKind::Workspace,
            is_enabled: true,
            extension_path: None,
            dependencies: deps
                .into_iter()
                .map(|(did, vr)| ExtensionDependency {
                    id: did.into(),
                    version_range: vr.into(),
                })
                .collect(),
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

    #[test]
    fn installed_count() {
        let mut bridge = MgmtBridge::new();
        assert_eq!(bridge.installed_count(), 0);
        bridge.install(test_ext());
        assert_eq!(bridge.installed_count(), 1);
        bridge.install(test_ext_ui("a.b", "a", true));
        assert_eq!(bridge.installed_count(), 2);
    }

    #[test]
    fn get_enabled_and_disabled() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_ui("a.one", "a", true));
        bridge.install(test_ext_ui("a.two", "a", false));
        bridge.install(test_ext_ui("b.one", "b", true));

        let enabled = bridge.get_enabled_extensions();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().all(|e| e.is_enabled));

        let disabled = bridge.get_disabled_extensions();
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].id, "a.two");
    }

    #[test]
    fn get_extensions_by_publisher() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_ui("a.one", "alpha", true));
        bridge.install(test_ext_ui("a.two", "alpha", false));
        bridge.install(test_ext_ui("b.one", "beta", true));

        let alpha = bridge.get_extensions_by_publisher("alpha");
        assert_eq!(alpha.len(), 2);

        let gamma = bridge.get_extensions_by_publisher("gamma");
        assert!(gamma.is_empty());
    }

    #[test]
    fn get_extensions_by_kind() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.install(test_ext_ui("ui.ext", "pub", true));

        let ws = bridge.get_extensions_by_kind(ExtensionKind::Workspace);
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].id, "publisher.extension");

        let ui = bridge.get_extensions_by_kind(ExtensionKind::Ui);
        assert_eq!(ui.len(), 1);
        assert_eq!(ui[0].id, "ui.ext");
    }

    #[test]
    fn update_version() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        assert!(bridge.update_version("publisher.extension", "2.0.0"));
        assert_eq!(
            bridge.get_extension("publisher.extension").unwrap().version,
            "2.0.0"
        );
        assert!(!bridge.update_version("nope", "3.0.0"));
    }

    #[test]
    fn dependency_tracking() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext_with_deps("a.child", vec![("a.parent", "^1.0")]));
        bridge.install(test_ext_with_deps("a.parent", vec![]));

        assert!(bridge.has_dependency("a.child", "a.parent"));
        assert!(!bridge.has_dependency("a.parent", "a.child"));

        let dependents = bridge.get_dependents("a.parent");
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].id, "a.child");

        assert!(bridge.get_dependents("a.child").is_empty());
    }

    #[test]
    fn get_stats() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.install(test_ext_ui("ui.one", "alpha", true));
        bridge.install(test_ext_ui("ui.two", "alpha", false));

        let stats = bridge.get_stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.enabled, 2);
        assert_eq!(stats.disabled, 1);
        assert_eq!(stats.by_kind.get("Workspace"), Some(&1));
        assert_eq!(stats.by_kind.get("Ui"), Some(&2));
        assert_eq!(stats.by_publisher.get("alpha"), Some(&2));
        assert_eq!(stats.by_publisher.get("publisher"), Some(&1));
    }

    #[test]
    fn search_extensions_case_insensitive() {
        let mut bridge = MgmtBridge::new();
        bridge.install(test_ext());
        bridge.install(test_ext_ui("other.tool", "other", true));

        let results = bridge.search_extensions("EXTENSION");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "publisher.extension");

        let results = bridge.search_extensions("ext");
        assert_eq!(results.len(), 2);

        assert!(bridge.search_extensions("zzz").is_empty());
    }

    #[test]
    fn search_extensions_by_display_name() {
        let mut bridge = MgmtBridge::new();
        bridge.install(ExtensionInfo {
            id: "x.y".into(),
            display_name: "Fancy Editor Theme".into(),
            version: "1.0.0".into(),
            publisher: "x".into(),
            kind: ExtensionKind::Ui,
            is_enabled: true,
            extension_path: None,
            dependencies: Vec::new(),
        });

        let results = bridge.search_extensions("fancy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, "Fancy Editor Theme");
    }

    #[test]
    fn extension_dependency_serialization() {
        let ext = test_ext_with_deps("a.child", vec![("a.parent", ">=1.0.0")]);
        let json = serde_json::to_string(&ext).unwrap();
        let back: ExtensionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(ext, back);
        assert_eq!(back.dependencies.len(), 1);
        assert_eq!(back.dependencies[0].id, "a.parent");
    }

    #[test]
    fn extension_info_without_dependencies_field() {
        let json = r#"{
            "id": "x.y",
            "display_name": "Test",
            "version": "1.0.0",
            "publisher": "x",
            "kind": "workspace",
            "is_enabled": true,
            "extension_path": null
        }"#;
        let ext: ExtensionInfo = serde_json::from_str(json).unwrap();
        assert!(ext.dependencies.is_empty());
    }

    #[test]
    fn stats_empty_bridge() {
        let bridge = MgmtBridge::new();
        let stats = bridge.get_stats();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.enabled, 0);
        assert_eq!(stats.disabled, 0);
        assert!(stats.by_kind.is_empty());
        assert!(stats.by_publisher.is_empty());
    }
}
