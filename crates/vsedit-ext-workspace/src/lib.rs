//! Ext API: Workspace.
//!
//! RPC bridge between the extension host and the main thread for workspace.

use std::collections::HashMap;
use std::fmt;

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

// ── Error types ──

/// Errors that can occur during workspace operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceError {
    /// The requested configuration section was not found.
    ConfigNotFound { section: String },
    /// A glob pattern is invalid.
    InvalidGlobPattern { pattern: String, reason: String },
    /// The workspace edit is empty and cannot be applied.
    EmptyEdit,
    /// The URI is malformed or unsupported.
    InvalidUri { uri: String },
    /// A watcher with the given ID was not found.
    WatcherNotFound { id: String },
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigNotFound { section } => {
                write!(f, "configuration section not found: {section}")
            }
            Self::InvalidGlobPattern { pattern, reason } => {
                write!(f, "invalid glob pattern '{pattern}': {reason}")
            }
            Self::EmptyEdit => write!(f, "workspace edit is empty"),
            Self::InvalidUri { uri } => write!(f, "invalid URI: {uri}"),
            Self::WatcherNotFound { id } => write!(f, "watcher not found: {id}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

// ── Display impls ──

impl fmt::Display for WorkspaceMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GetWorkspaceFolders => write!(f, "GetWorkspaceFolders"),
            Self::GetConfiguration { section } => {
                write!(f, "GetConfiguration({section})")
            }
            Self::UpdateConfiguration { section, .. } => {
                write!(f, "UpdateConfiguration({section})")
            }
            Self::FindFiles { include, .. } => write!(f, "FindFiles({include})"),
            Self::OpenTextDocument { uri } => write!(f, "OpenTextDocument({uri})"),
            Self::ApplyEdit { .. } => write!(f, "ApplyEdit"),
            Self::CreateFileSystemWatcher { .. } => write!(f, "CreateFileSystemWatcher"),
        }
    }
}

impl fmt::Display for WorkspaceResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Folders { uris } => write!(f, "Folders({})", uris.len()),
            Self::Configuration { .. } => write!(f, "Configuration"),
            Self::Files { uris } => write!(f, "Files({})", uris.len()),
            Self::WatcherId { id } => write!(f, "WatcherId({id})"),
            Self::Ok => write!(f, "Ok"),
        }
    }
}

impl fmt::Display for FileSystemWatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut events = Vec::new();
        if self.watch_create {
            events.push("create");
        }
        if self.watch_change {
            events.push("change");
        }
        if self.watch_delete {
            events.push("delete");
        }
        write!(f, "Watcher('{}', [{}])", self.glob_pattern, events.join(", "))
    }
}

// ── Builder for WorkspaceEdit ──

/// Builder for constructing a [`WorkspaceEdit`] incrementally.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceEditBuilder {
    text_edits: HashMap<String, Vec<TextEditEntry>>,
    renames: Vec<RenameEntry>,
    creates: Vec<String>,
    deletes: Vec<String>,
}

impl WorkspaceEditBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a text edit for a given file URI.
    pub fn text_edit(mut self, uri: impl Into<String>, entry: TextEditEntry) -> Self {
        self.text_edits.entry(uri.into()).or_default().push(entry);
        self
    }

    /// Add a file rename operation.
    pub fn rename(mut self, old_uri: impl Into<String>, new_uri: impl Into<String>) -> Self {
        self.renames.push(RenameEntry {
            old_uri: old_uri.into(),
            new_uri: new_uri.into(),
        });
        self
    }

    /// Add a file creation.
    pub fn create(mut self, uri: impl Into<String>) -> Self {
        self.creates.push(uri.into());
        self
    }

    /// Add a file deletion.
    pub fn delete(mut self, uri: impl Into<String>) -> Self {
        self.deletes.push(uri.into());
        self
    }

    /// Build the [`WorkspaceEdit`], returning an error if the edit is empty.
    pub fn build(self) -> Result<WorkspaceEdit, WorkspaceError> {
        if self.text_edits.is_empty()
            && self.renames.is_empty()
            && self.creates.is_empty()
            && self.deletes.is_empty()
        {
            return Err(WorkspaceError::EmptyEdit);
        }
        Ok(WorkspaceEdit {
            text_edits: self.text_edits,
            renames: self.renames,
            creates: self.creates,
            deletes: self.deletes,
        })
    }
}

// ── Validation & helpers ──

impl WorkspaceEdit {
    /// Returns `true` if this edit contains no operations.
    pub fn is_empty(&self) -> bool {
        self.text_edits.is_empty()
            && self.renames.is_empty()
            && self.creates.is_empty()
            && self.deletes.is_empty()
    }

    /// Returns the total number of individual operations in this edit.
    pub fn operation_count(&self) -> usize {
        let text_edit_count: usize = self.text_edits.values().map(|v| v.len()).sum();
        text_edit_count + self.renames.len() + self.creates.len() + self.deletes.len()
    }

    /// Returns the set of all file URIs affected by this edit.
    pub fn affected_uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self.text_edits.keys().cloned().collect();
        for r in &self.renames {
            uris.push(r.old_uri.clone());
            uris.push(r.new_uri.clone());
        }
        uris.extend(self.creates.iter().cloned());
        uris.extend(self.deletes.iter().cloned());
        uris.sort();
        uris.dedup();
        uris
    }
}

impl TextEditEntry {
    /// Create a new text edit spanning a single line.
    pub fn single_line(line: u32, start_col: u32, end_col: u32, new_text: impl Into<String>) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col,
            new_text: new_text.into(),
        }
    }

    /// Returns `true` if this edit is an insertion (zero-width range).
    pub fn is_insert(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    /// Returns the number of lines this edit spans.
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }
}

impl FileSystemWatcher {
    /// Validate the glob pattern (basic validation: must not be empty).
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.glob_pattern.is_empty() {
            return Err(WorkspaceError::InvalidGlobPattern {
                pattern: self.glob_pattern.clone(),
                reason: "glob pattern must not be empty".into(),
            });
        }
        Ok(())
    }

    /// Returns `true` if this watcher is watching any event kind.
    pub fn is_active(&self) -> bool {
        self.watch_create || self.watch_change || self.watch_delete
    }
}

impl WorkspaceBridge {
    /// Get a configuration value, returning an error if not found.
    pub fn get_config(&self, section: &str) -> Result<&Value, WorkspaceError> {
        self.config
            .get(section)
            .ok_or_else(|| WorkspaceError::ConfigNotFound {
                section: section.to_string(),
            })
    }

    /// Remove a watcher by its ID string (e.g. "watcher-0").
    pub fn remove_watcher(&mut self, id: &str) -> Result<FileSystemWatcher, WorkspaceError> {
        let idx: usize = id
            .strip_prefix("watcher-")
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| WorkspaceError::WatcherNotFound { id: id.to_string() })?;
        if idx >= self.watchers.len() {
            return Err(WorkspaceError::WatcherNotFound { id: id.to_string() });
        }
        Ok(self.watchers.remove(idx))
    }

    /// Returns all currently registered watcher glob patterns.
    pub fn watcher_patterns(&self) -> Vec<&str> {
        self.watchers.iter().map(|w| w.glob_pattern.as_str()).collect()
    }

    /// Returns a snapshot of the current folder list.
    pub fn folders(&self) -> &[String] {
        &self.folders
    }

    /// Validate a URI string (must start with a known scheme).
    pub fn validate_uri(uri: &str) -> Result<(), WorkspaceError> {
        const SCHEMES: &[&str] = &["file://", "untitled:", "vscode-remote://"];
        if SCHEMES.iter().any(|s| uri.starts_with(s)) {
            Ok(())
        } else {
            Err(WorkspaceError::InvalidUri { uri: uri.to_string() })
        }
    }
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

    #[test]
    fn workspace_edit_builder_success() {
        let edit = WorkspaceEditBuilder::new()
            .create("file:///new.rs")
            .delete("file:///old.rs")
            .rename("file:///a.rs", "file:///b.rs")
            .text_edit(
                "file:///c.rs",
                TextEditEntry::single_line(10, 0, 5, "hello"),
            )
            .build()
            .unwrap();
        assert_eq!(edit.operation_count(), 4);
        assert!(!edit.is_empty());
    }

    #[test]
    fn workspace_edit_builder_empty_fails() {
        let result = WorkspaceEditBuilder::new().build();
        assert_eq!(result, Err(WorkspaceError::EmptyEdit));
    }

    #[test]
    fn workspace_edit_affected_uris() {
        let edit = WorkspaceEditBuilder::new()
            .create("file:///x.rs")
            .delete("file:///y.rs")
            .rename("file:///a.rs", "file:///b.rs")
            .build()
            .unwrap();
        let uris = edit.affected_uris();
        assert_eq!(uris, vec![
            "file:///a.rs",
            "file:///b.rs",
            "file:///x.rs",
            "file:///y.rs",
        ]);
    }

    #[test]
    fn text_edit_single_line_insert() {
        let entry = TextEditEntry::single_line(5, 3, 3, "inserted");
        assert!(entry.is_insert());
        assert_eq!(entry.line_span(), 1);
    }

    #[test]
    fn text_edit_multi_line_span() {
        let entry = TextEditEntry {
            start_line: 2,
            start_col: 0,
            end_line: 7,
            end_col: 10,
            new_text: "replacement".into(),
        };
        assert!(!entry.is_insert());
        assert_eq!(entry.line_span(), 6);
    }

    #[test]
    fn watcher_validate_empty_pattern() {
        let w = FileSystemWatcher {
            glob_pattern: "".into(),
            watch_create: true,
            watch_change: false,
            watch_delete: false,
        };
        assert!(w.validate().is_err());
    }

    #[test]
    fn watcher_is_active() {
        let inactive = FileSystemWatcher {
            glob_pattern: "*.rs".into(),
            watch_create: false,
            watch_change: false,
            watch_delete: false,
        };
        assert!(!inactive.is_active());

        let active = FileSystemWatcher {
            glob_pattern: "*.rs".into(),
            watch_create: false,
            watch_change: true,
            watch_delete: false,
        };
        assert!(active.is_active());
    }

    #[test]
    fn bridge_get_config_not_found() {
        let bridge = WorkspaceBridge::new();
        let err = bridge.get_config("nonexistent").unwrap_err();
        assert_eq!(
            err,
            WorkspaceError::ConfigNotFound { section: "nonexistent".into() }
        );
    }

    #[test]
    fn bridge_remove_watcher() {
        let mut bridge = WorkspaceBridge::new();
        bridge.handle(WorkspaceMessage::CreateFileSystemWatcher {
            watcher: FileSystemWatcher {
                glob_pattern: "**/*.ts".into(),
                watch_create: true,
                watch_change: false,
                watch_delete: false,
            },
        });
        assert_eq!(bridge.watcher_count(), 1);
        let removed = bridge.remove_watcher("watcher-0").unwrap();
        assert_eq!(removed.glob_pattern, "**/*.ts");
        assert_eq!(bridge.watcher_count(), 0);
    }

    #[test]
    fn bridge_remove_watcher_not_found() {
        let mut bridge = WorkspaceBridge::new();
        assert!(bridge.remove_watcher("watcher-99").is_err());
        assert!(bridge.remove_watcher("bad-id").is_err());
    }

    #[test]
    fn validate_uri() {
        assert!(WorkspaceBridge::validate_uri("file:///home/user/project").is_ok());
        assert!(WorkspaceBridge::validate_uri("untitled:Untitled-1").is_ok());
        assert!(WorkspaceBridge::validate_uri("https://example.com").is_err());
    }

    #[test]
    fn display_impls() {
        let msg = WorkspaceMessage::GetWorkspaceFolders;
        assert_eq!(msg.to_string(), "GetWorkspaceFolders");

        let resp = WorkspaceResponse::Folders { uris: vec!["a".into(), "b".into()] };
        assert_eq!(resp.to_string(), "Folders(2)");

        let w = FileSystemWatcher {
            glob_pattern: "*.rs".into(),
            watch_create: true,
            watch_change: false,
            watch_delete: true,
        };
        assert_eq!(w.to_string(), "Watcher('*.rs', [create, delete])");

        let err = WorkspaceError::EmptyEdit;
        assert_eq!(err.to_string(), "workspace edit is empty");
    }
}
