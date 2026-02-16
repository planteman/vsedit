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
}
