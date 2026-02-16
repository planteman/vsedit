//! Workspace folders, configuration, and trust management.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
    pub index: u32,
}

pub struct WorkspaceConfiguration {
    pub folders: Vec<WorkspaceFolder>,
    pub settings: HashMap<String, String>,
    pub name: Option<String>,
}

impl WorkspaceConfiguration {
    pub fn new() -> Self {
        Self {
            folders: Vec::new(),
            settings: HashMap::new(),
            name: None,
        }
    }

    pub fn add_folder(&mut self, uri: String, name: String) {
        let index = self.folders.len() as u32;
        self.folders.push(WorkspaceFolder { uri, name, index });
    }

    pub fn remove_folder(&mut self, index: u32) -> bool {
        let len = self.folders.len();
        self.folders.retain(|f| f.index != index);
        if self.folders.len() < len {
            // Re-index remaining folders
            for (i, folder) in self.folders.iter_mut().enumerate() {
                folder.index = i as u32;
            }
            true
        } else {
            false
        }
    }

    pub fn get_folder(&self, index: u32) -> Option<&WorkspaceFolder> {
        self.folders.iter().find(|f| f.index == index)
    }

    pub fn folder_count(&self) -> usize {
        self.folders.len()
    }

    pub fn set_setting(&mut self, key: String, value: String) {
        self.settings.insert(key, value);
    }

    pub fn get_setting(&self, key: &str) -> Option<&str> {
        self.settings.get(key).map(|s| s.as_str())
    }

    pub fn is_multi_root(&self) -> bool {
        self.folders.len() > 1
    }

    pub fn find_folder_by_uri(&self, uri: &str) -> Option<&WorkspaceFolder> {
        self.folders.iter().find(|f| f.uri == uri)
    }

    pub fn find_folder_by_name(&self, name: &str) -> Option<&WorkspaceFolder> {
        self.folders.iter().find(|f| f.name == name)
    }

    pub fn get_settings_with_prefix(&self, prefix: &str) -> Vec<(&str, &str)> {
        self.settings
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    pub fn remove_setting(&mut self, key: &str) -> bool {
        self.settings.remove(key).is_some()
    }

    pub fn setting_count(&self) -> usize {
        self.settings.len()
    }

    pub fn contains_uri(&self, uri: &str) -> bool {
        self.folders.iter().any(|f| f.uri == uri)
    }
}

impl Default for WorkspaceConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTrust {
    Trusted,
    Untrusted,
    Unknown,
}

impl std::fmt::Display for WorkspaceTrust {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceTrust::Trusted => write!(f, "Trusted"),
            WorkspaceTrust::Untrusted => write!(f, "Untrusted"),
            WorkspaceTrust::Unknown => write!(f, "Unknown"),
        }
    }
}

pub struct WorkspaceTrustService {
    pub trust_state: WorkspaceTrust,
    pub trusted_folders: Vec<String>,
}

impl WorkspaceTrustService {
    pub fn new() -> Self {
        Self {
            trust_state: WorkspaceTrust::Unknown,
            trusted_folders: Vec::new(),
        }
    }

    pub fn set_trust(&mut self, trust: WorkspaceTrust) {
        self.trust_state = trust;
    }

    pub fn is_trusted(&self) -> bool {
        self.trust_state == WorkspaceTrust::Trusted
    }

    pub fn add_trusted_folder(&mut self, uri: String) {
        if !self.trusted_folders.contains(&uri) {
            self.trusted_folders.push(uri);
        }
    }

    pub fn is_folder_trusted(&self, uri: &str) -> bool {
        self.trusted_folders.iter().any(|f| f == uri)
    }
}

impl Default for WorkspaceTrustService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_folders() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/home/user/project".into(), "project".into());
        ws.add_folder("/home/user/lib".into(), "lib".into());
        assert_eq!(ws.folder_count(), 2);
        assert!(ws.remove_folder(0));
        assert_eq!(ws.folder_count(), 1);
        assert_eq!(ws.folders[0].index, 0);
    }

    #[test]
    fn settings() {
        let mut ws = WorkspaceConfiguration::new();
        ws.set_setting("editor.tabSize".into(), "4".into());
        assert_eq!(ws.get_setting("editor.tabSize"), Some("4"));
        assert_eq!(ws.get_setting("missing"), None);
    }

    #[test]
    fn multi_root_detection() {
        let mut ws = WorkspaceConfiguration::new();
        assert!(!ws.is_multi_root());
        ws.add_folder("/a".into(), "a".into());
        assert!(!ws.is_multi_root());
        ws.add_folder("/b".into(), "b".into());
        assert!(ws.is_multi_root());
    }

    #[test]
    fn get_folder_by_index() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/project".into(), "project".into());
        let folder = ws.get_folder(0).unwrap();
        assert_eq!(folder.uri, "/project");
        assert!(ws.get_folder(5).is_none());
    }

    #[test]
    fn find_folder_by_uri() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/home/user/project".into(), "project".into());
        ws.add_folder("/home/user/lib".into(), "lib".into());
        let folder = ws.find_folder_by_uri("/home/user/lib").unwrap();
        assert_eq!(folder.name, "lib");
        assert!(ws.find_folder_by_uri("/nonexistent").is_none());
    }

    #[test]
    fn find_folder_by_name() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/a".into(), "alpha".into());
        ws.add_folder("/b".into(), "beta".into());
        let folder = ws.find_folder_by_name("beta").unwrap();
        assert_eq!(folder.uri, "/b");
        assert!(ws.find_folder_by_name("gamma").is_none());
    }

    #[test]
    fn settings_with_prefix() {
        let mut ws = WorkspaceConfiguration::new();
        ws.set_setting("editor.tabSize".into(), "4".into());
        ws.set_setting("editor.fontSize".into(), "14".into());
        ws.set_setting("terminal.shell".into(), "/bin/bash".into());
        let editor_settings = ws.get_settings_with_prefix("editor.");
        assert_eq!(editor_settings.len(), 2);
        let terminal_settings = ws.get_settings_with_prefix("terminal.");
        assert_eq!(terminal_settings.len(), 1);
        assert!(ws.get_settings_with_prefix("nonexistent.").is_empty());
    }

    #[test]
    fn remove_setting_and_count() {
        let mut ws = WorkspaceConfiguration::new();
        ws.set_setting("a".into(), "1".into());
        ws.set_setting("b".into(), "2".into());
        assert_eq!(ws.setting_count(), 2);
        assert!(ws.remove_setting("a"));
        assert_eq!(ws.setting_count(), 1);
        assert!(!ws.remove_setting("a"));
    }

    #[test]
    fn contains_uri() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/project".into(), "project".into());
        assert!(ws.contains_uri("/project"));
        assert!(!ws.contains_uri("/other"));
    }

    #[test]
    fn trust_service_basics() {
        let mut svc = WorkspaceTrustService::new();
        assert_eq!(svc.trust_state, WorkspaceTrust::Unknown);
        assert!(!svc.is_trusted());
        svc.set_trust(WorkspaceTrust::Trusted);
        assert!(svc.is_trusted());
        svc.set_trust(WorkspaceTrust::Untrusted);
        assert!(!svc.is_trusted());
    }

    #[test]
    fn trust_service_folders() {
        let mut svc = WorkspaceTrustService::new();
        svc.add_trusted_folder("/safe".into());
        svc.add_trusted_folder("/also-safe".into());
        svc.add_trusted_folder("/safe".into()); // duplicate
        assert!(svc.is_folder_trusted("/safe"));
        assert!(svc.is_folder_trusted("/also-safe"));
        assert!(!svc.is_folder_trusted("/unknown"));
        assert_eq!(svc.trusted_folders.len(), 2);
    }

    #[test]
    fn workspace_trust_display() {
        assert_eq!(format!("{}", WorkspaceTrust::Trusted), "Trusted");
        assert_eq!(format!("{}", WorkspaceTrust::Untrusted), "Untrusted");
        assert_eq!(format!("{}", WorkspaceTrust::Unknown), "Unknown");
    }
}
