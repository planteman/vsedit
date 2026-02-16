//! Ext API: Workspace.
//!
//! RPC bridge between the extension host and the main thread for workspace.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_workspace";

// ── RPC message types ──

/// Messages exchanged for the `workspace` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorkspaceMessage {
    GetWorkspaceFolders,
    GetConfiguration { section: String },
    UpdateConfiguration { section: String, value: Value },
    FindFiles { include: String, exclude: Option<String>, max_results: Option<u32> },
    OpenTextDocument { uri: String },
    ApplyEdit { edit: WorkspaceEdit },
    CreateFileSystemWatcher { watcher: FileSystemWatcher },
}

/// A workspace edit containing text edits, renames, creates, and deletes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdit {
    #[serde(default)]
    pub text_edits: HashMap<String, Vec<TextEditEntry>>,
    #[serde(default)]
    pub renames: Vec<RenameEntry>,
    #[serde(default)]
    pub creates: Vec<String>,
    #[serde(default)]
    pub deletes: Vec<String>,
}

/// A single text edit entry within a workspace edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEditEntry {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
}

/// A file rename operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameEntry {
    pub old_uri: String,
    pub new_uri: String,
}

/// A file system watcher registration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemWatcher {
    pub glob_pattern: String,
    #[serde(default)]
    pub watch_create: bool,
    #[serde(default)]
    pub watch_change: bool,
    #[serde(default)]
    pub watch_delete: bool,
}

/// Response payload for workspace operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorkspaceResponse {
    Folders { uris: Vec<String> },
    Configuration { value: Value },
    Files { uris: Vec<String> },
    WatcherId { id: String },
    Ok,
}

// ── Bridge ──

/// Processes workspace messages from the extension host.
#[derive(Debug, Default)]
pub struct WorkspaceBridge {
    folders: Vec<String>,
    config: HashMap<String, Value>,
    watchers: Vec<FileSystemWatcher>,
}

impl WorkspaceBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the workspace folders visible to extensions.
    pub fn set_folders(&mut self, folders: Vec<String>) {
        self.folders = folders;
    }

    /// Process an incoming workspace message and return a response.
    pub fn handle(&mut self, msg: WorkspaceMessage) -> WorkspaceResponse {
        match msg {
            WorkspaceMessage::GetWorkspaceFolders => {
                WorkspaceResponse::Folders { uris: self.folders.clone() }
            }
            WorkspaceMessage::GetConfiguration { section } => {
                let value = self.config.get(&section).cloned().unwrap_or(Value::Null);
                WorkspaceResponse::Configuration { value }
            }
            WorkspaceMessage::UpdateConfiguration { section, value } => {
                self.config.insert(section, value);
                WorkspaceResponse::Ok
            }
            WorkspaceMessage::FindFiles { .. } => {
                // Actual file search is delegated to the file service.
                WorkspaceResponse::Files { uris: Vec::new() }
            }
            WorkspaceMessage::OpenTextDocument { .. } => WorkspaceResponse::Ok,
            WorkspaceMessage::ApplyEdit { .. } => WorkspaceResponse::Ok,
            WorkspaceMessage::CreateFileSystemWatcher { watcher } => {
                let id = format!("watcher-{}", self.watchers.len());
                self.watchers.push(watcher);
                WorkspaceResponse::WatcherId { id }
            }
        }
    }

    pub fn watcher_count(&self) -> usize {
        self.watchers.len()
    }
}

/// Initialize the workspace extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn get_workspace_folders() {
        let mut bridge = WorkspaceBridge::new();
        bridge.set_folders(vec!["file:///project".into()]);
        let resp = bridge.handle(WorkspaceMessage::GetWorkspaceFolders);
        assert_eq!(resp, WorkspaceResponse::Folders { uris: vec!["file:///project".into()] });
    }

    #[test]
    fn configuration_round_trip() {
        let mut bridge = WorkspaceBridge::new();
        bridge.handle(WorkspaceMessage::UpdateConfiguration {
            section: "editor.tabSize".into(),
            value: Value::Number(4.into()),
        });
        let resp = bridge.handle(WorkspaceMessage::GetConfiguration {
            section: "editor.tabSize".into(),
        });
        assert_eq!(resp, WorkspaceResponse::Configuration { value: Value::Number(4.into()) });
    }

    #[test]
    fn create_watcher() {
        let mut bridge = WorkspaceBridge::new();
        let resp = bridge.handle(WorkspaceMessage::CreateFileSystemWatcher {
            watcher: FileSystemWatcher {
                glob_pattern: "**/*.rs".into(),
                watch_create: true,
                watch_change: true,
                watch_delete: false,
            },
        });
        assert_eq!(resp, WorkspaceResponse::WatcherId { id: "watcher-0".into() });
        assert_eq!(bridge.watcher_count(), 1);
    }

    #[test]
    fn find_files_returns_empty() {
        let mut bridge = WorkspaceBridge::new();
        let resp = bridge.handle(WorkspaceMessage::FindFiles {
            include: "**/*.rs".into(),
            exclude: None,
            max_results: Some(100),
        });
        assert_eq!(resp, WorkspaceResponse::Files { uris: Vec::new() });
    }

    #[test]
    fn serde_round_trip() {
        let msg = WorkspaceMessage::ApplyEdit {
            edit: WorkspaceEdit {
                text_edits: HashMap::new(),
                renames: vec![RenameEntry {
                    old_uri: "file:///a.rs".into(),
                    new_uri: "file:///b.rs".into(),
                }],
                creates: Vec::new(),
                deletes: vec!["file:///c.rs".into()],
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: WorkspaceMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }
}
