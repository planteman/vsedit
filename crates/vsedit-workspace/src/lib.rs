//! Workspace folder management.
//!
//! Manages workspace folders and multi-root workspaces, equivalent to
//! VS Code's `vs/platform/workspace/common/workspace.ts`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vsedit_events::{Emitter, Event};
use vsedit_uri::VsUri;

// ---------------------------------------------------------------------------
// WorkspaceFolder
// ---------------------------------------------------------------------------

/// A folder in the workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceFolder {
    pub uri: VsUri,
    pub name: String,
    pub index: usize,
}

// ---------------------------------------------------------------------------
// WorkspaceType
// ---------------------------------------------------------------------------

/// The type of the current workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceType {
    Empty,
    SingleFolder(PathBuf),
    MultiRoot(PathBuf),
}

// ---------------------------------------------------------------------------
// WorkspaceTrust
// ---------------------------------------------------------------------------

/// Trust level for a workspace, mirroring VS Code's workspace trust model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceTrust {
    Trusted,
    Untrusted,
    Unknown,
}

// ---------------------------------------------------------------------------
// WorkspaceFoldersChangeEvent
// ---------------------------------------------------------------------------

/// Fired when workspace folders are added or removed.
#[derive(Debug, Clone)]
pub struct WorkspaceFoldersChangeEvent {
    pub added: Vec<WorkspaceFolder>,
    pub removed: Vec<WorkspaceFolder>,
}

// ---------------------------------------------------------------------------
// WorkspaceOpenEvent
// ---------------------------------------------------------------------------

/// Fired when a workspace is opened.
#[derive(Debug, Clone)]
pub struct WorkspaceOpenEvent {
    pub workspace_type: WorkspaceType,
}

// ---------------------------------------------------------------------------
// .code-workspace file model
// ---------------------------------------------------------------------------

/// A single folder entry in a `.code-workspace` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderEntry {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Parsed representation of a `.code-workspace` JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub folders: Vec<FolderEntry>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub settings: serde_json::Value,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub extensions: serde_json::Value,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub launch: serde_json::Value,
}

/// Parse a `.code-workspace` JSON file from disk.
pub fn parse_workspace_file(path: &Path) -> Result<WorkspaceFile, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read workspace file: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("invalid workspace file: {e}"))
}

/// Save a [`WorkspaceFile`] to disk as JSON.
pub fn save_workspace_file(workspace: &WorkspaceFile, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(workspace)
        .map_err(|e| format!("failed to serialize workspace file: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("failed to write workspace file: {e}"))
}

// ---------------------------------------------------------------------------
// RecentWorkspace
// ---------------------------------------------------------------------------

/// An entry in the recent workspaces list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentWorkspace {
    pub path: String,
    pub label: String,
    pub last_opened: u64,
}

/// Persistent recent-workspaces storage backed by a JSON file.
#[derive(Debug)]
pub struct RecentWorkspaces {
    state_path: PathBuf,
    entries: Vec<RecentWorkspace>,
}

#[derive(Serialize, Deserialize, Default)]
struct RecentState {
    recents: Vec<RecentWorkspace>,
}

impl RecentWorkspaces {
    /// Create a new store, loading existing entries from `state_path`.
    pub fn new(state_path: PathBuf) -> Self {
        let entries = std::fs::read_to_string(&state_path)
            .ok()
            .and_then(|s| serde_json::from_str::<RecentState>(&s).ok())
            .map(|s| s.recents)
            .unwrap_or_default();
        Self {
            state_path,
            entries,
        }
    }

    /// Add a workspace to the recent list (moves to front if already present).
    pub fn add_recent(&mut self, workspace: RecentWorkspace) {
        self.entries.retain(|e| e.path != workspace.path);
        self.entries.insert(0, workspace);
        self.persist();
    }

    /// Return the recent workspaces list (most recent first).
    pub fn get_recents(&self) -> &[RecentWorkspace] {
        &self.entries
    }

    /// Clear all recent workspace entries.
    pub fn clear_recents(&mut self) {
        self.entries.clear();
        self.persist();
    }

    fn persist(&self) {
        let state = RecentState {
            recents: self.entries.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = std::fs::write(&self.state_path, json);
        }
    }
}

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------

/// The current workspace.
pub struct Workspace {
    folders: Vec<WorkspaceFolder>,
    workspace_file: Option<VsUri>,
    configuration: serde_json::Value,
    is_untitled: bool,
    trust: WorkspaceTrust,
    on_did_change_folders: Emitter<WorkspaceFoldersChangeEvent>,
    on_did_open_workspace: Emitter<WorkspaceOpenEvent>,
    on_will_delete_folder: Emitter<WorkspaceFolder>,
}

impl Workspace {
    /// Create an empty workspace with no folders.
    pub fn empty() -> Self {
        Self {
            folders: Vec::new(),
            workspace_file: None,
            configuration: serde_json::Value::Null,
            is_untitled: true,
            trust: WorkspaceTrust::Unknown,
            on_did_change_folders: Emitter::new(),
            on_did_open_workspace: Emitter::new(),
            on_will_delete_folder: Emitter::new(),
        }
    }

    /// Create a single-folder workspace (equivalent to `code /path/to/folder`).
    pub fn single_folder(uri: VsUri) -> Self {
        let name = folder_name_from_uri(&uri);
        Self {
            folders: vec![WorkspaceFolder {
                uri,
                name,
                index: 0,
            }],
            workspace_file: None,
            configuration: serde_json::Value::Null,
            is_untitled: false,
            trust: WorkspaceTrust::Unknown,
            on_did_change_folders: Emitter::new(),
            on_did_open_workspace: Emitter::new(),
            on_will_delete_folder: Emitter::new(),
        }
    }

    /// Open a single folder as a workspace.
    pub fn open_folder(path: &Path) -> Self {
        let uri = VsUri::file(&path.to_string_lossy());
        let mut ws = Self::single_folder(uri);
        ws.on_did_open_workspace.fire(&WorkspaceOpenEvent {
            workspace_type: ws.get_workspace_type(),
        });
        ws
    }

    /// Create a workspace from a `.code-workspace` file URI and its JSON content.
    pub fn from_workspace_file(file_uri: VsUri, content: &str) -> Result<Self, String> {
        let data: WorkspaceFile =
            serde_json::from_str(content).map_err(|e| format!("invalid workspace file: {e}"))?;

        let folders = data
            .folders
            .iter()
            .enumerate()
            .map(|(index, f)| {
                let uri = VsUri::file(&f.path);
                let name = f
                    .name
                    .clone()
                    .unwrap_or_else(|| folder_name_from_uri(&uri));
                WorkspaceFolder { uri, name, index }
            })
            .collect();

        Ok(Self {
            folders,
            workspace_file: Some(file_uri),
            configuration: data.settings.clone(),
            is_untitled: false,
            trust: WorkspaceTrust::Unknown,
            on_did_change_folders: Emitter::new(),
            on_did_open_workspace: Emitter::new(),
            on_will_delete_folder: Emitter::new(),
        })
    }

    /// Returns the workspace type.
    pub fn get_workspace_type(&self) -> WorkspaceType {
        match self.folders.len() {
            0 => WorkspaceType::Empty,
            _ if self.workspace_file.is_some() => {
                let path = self.workspace_file.as_ref().unwrap().fs_path();
                WorkspaceType::MultiRoot(PathBuf::from(path))
            }
            _ => {
                let path = self.folders[0].uri.fs_path();
                WorkspaceType::SingleFolder(PathBuf::from(path))
            }
        }
    }

    /// Whether this workspace is untitled (not yet saved).
    pub fn is_untitled(&self) -> bool {
        self.is_untitled
    }

    /// Returns workspace-level configuration/settings.
    pub fn configuration(&self) -> &serde_json::Value {
        &self.configuration
    }

    /// Returns the workspace folders.
    pub fn get_folders(&self) -> &[WorkspaceFolder] {
        &self.folders
    }

    /// Find a workspace folder whose URI exactly matches `uri`.
    pub fn get_folder_by_uri(&self, uri: &VsUri) -> Option<&WorkspaceFolder> {
        self.folders.iter().find(|f| &f.uri == uri)
    }

    /// Find the workspace folder that contains the given resource URI.
    pub fn get_workspace_folder(&self, uri: &VsUri) -> Option<&WorkspaceFolder> {
        self.get_workspace_folder_of_resource(uri)
    }

    /// Find the workspace folder that contains the given resource URI.
    ///
    /// Resolution uses path-prefix matching: the folder whose path is the
    /// longest prefix of the resource path wins.
    pub fn get_workspace_folder_of_resource(&self, uri: &VsUri) -> Option<&WorkspaceFolder> {
        let resource_path = ensure_trailing_slash(&uri.path);
        let mut best: Option<&WorkspaceFolder> = None;
        let mut best_len: usize = 0;

        for folder in &self.folders {
            if folder.uri.scheme != uri.scheme {
                continue;
            }
            let folder_prefix = ensure_trailing_slash(&folder.uri.path);
            if resource_path.starts_with(&folder_prefix) && folder_prefix.len() > best_len {
                best = Some(folder);
                best_len = folder_prefix.len();
            }
        }

        best
    }

    /// Get a path relative to the workspace folder that contains `uri`.
    pub fn relative_path(&self, uri: &VsUri) -> Option<String> {
        let folder = self.get_workspace_folder_of_resource(uri)?;
        let folder_path = ensure_trailing_slash(&folder.uri.path);
        if uri.path.starts_with(&folder_path) {
            Some(uri.path[folder_path.len()..].to_string())
        } else {
            None
        }
    }

    /// Add a folder to the workspace.
    pub fn add_folder(&mut self, uri: VsUri, name: Option<String>) {
        if self.folders.iter().any(|f| f.uri == uri) {
            return;
        }

        let name = name.unwrap_or_else(|| folder_name_from_uri(&uri));
        let index = self.folders.len();
        let folder = WorkspaceFolder { uri, name, index };

        self.folders.push(folder.clone());
        self.on_did_change_folders.fire(&WorkspaceFoldersChangeEvent {
            added: vec![folder],
            removed: vec![],
        });
    }

    /// Remove a folder from the workspace by index.
    pub fn remove_folder_by_index(&mut self, idx: usize) -> Option<WorkspaceFolder> {
        if idx >= self.folders.len() {
            return None;
        }
        self.on_will_delete_folder
            .fire(&self.folders[idx].clone());
        let removed = self.folders.remove(idx);
        for (i, f) in self.folders.iter_mut().enumerate() {
            f.index = i;
        }
        self.on_did_change_folders.fire(&WorkspaceFoldersChangeEvent {
            added: vec![],
            removed: vec![removed.clone()],
        });
        Some(removed)
    }

    /// Remove a folder from the workspace by URI.
    pub fn remove_folder(&mut self, uri: &VsUri) {
        if let Some(pos) = self.folders.iter().position(|f| &f.uri == uri) {
            self.remove_folder_by_index(pos);
        }
    }

    /// Save the current workspace as a `.code-workspace` file.
    pub fn save_workspace_as(&self, path: &Path) -> Result<(), String> {
        let wf = WorkspaceFile {
            folders: self
                .folders
                .iter()
                .map(|f| FolderEntry {
                    path: f.uri.fs_path(),
                    name: Some(f.name.clone()),
                })
                .collect(),
            settings: self.configuration.clone(),
            extensions: serde_json::Value::Null,
            launch: serde_json::Value::Null,
        };
        save_workspace_file(&wf, path)
    }

    // -- Workspace trust ---------------------------------------------------

    /// Get the current trust level.
    pub fn trust(&self) -> WorkspaceTrust {
        self.trust
    }

    /// Mark the workspace as trusted.
    pub fn trust_workspace(&mut self) {
        self.trust = WorkspaceTrust::Trusted;
    }

    /// Whether the workspace is trusted.
    pub fn is_trusted(&self) -> bool {
        self.trust == WorkspaceTrust::Trusted
    }

    // -- Path resolution ---------------------------------------------------

    /// Resolve a relative path against the first workspace folder root.
    pub fn resolve_path(&self, relative: &str) -> Option<PathBuf> {
        self.folders
            .first()
            .map(|f| PathBuf::from(f.uri.fs_path()).join(relative))
    }

    /// Check whether `path` is inside any workspace folder.
    pub fn is_inside_workspace(&self, path: &Path) -> bool {
        let p = path.to_string_lossy();
        let uri = VsUri::file(&p);
        self.get_workspace_folder_of_resource(&uri).is_some()
    }

    /// Get the root path of the first workspace folder.
    pub fn get_workspace_root(&self) -> Option<PathBuf> {
        self.folders.first().map(|f| PathBuf::from(f.uri.fs_path()))
    }

    // -- Events ------------------------------------------------------------

    /// Subscribe to folder change events.
    pub fn on_did_change_folders(&self) -> Event<WorkspaceFoldersChangeEvent> {
        self.on_did_change_folders.event()
    }

    /// Subscribe to workspace open events.
    pub fn on_did_open_workspace(&self) -> Event<WorkspaceOpenEvent> {
        self.on_did_open_workspace.event()
    }

    /// Subscribe to folder deletion events (fired before removal).
    pub fn on_will_delete_workspace_folder(&self) -> Event<WorkspaceFolder> {
        self.on_will_delete_folder.event()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a human-readable name from a URI (last path segment).
fn folder_name_from_uri(uri: &VsUri) -> String {
    uri.path
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(&uri.path)
        .to_string()
}

/// Ensure a path ends with `/` for prefix matching.
fn ensure_trailing_slash(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn empty_workspace() {
        let ws = Workspace::empty();
        assert_eq!(ws.get_workspace_type(), WorkspaceType::Empty);
        assert!(ws.get_folders().is_empty());
        assert!(ws.is_untitled());
    }

    #[test]
    fn single_folder_workspace() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        assert!(matches!(ws.get_workspace_type(), WorkspaceType::SingleFolder(_)));
        assert_eq!(ws.get_folders().len(), 1);
        assert_eq!(ws.get_folders()[0].name, "project");
        assert_eq!(ws.get_folders()[0].index, 0);
        assert!(!ws.is_untitled());
    }

    #[test]
    fn multi_root_workspace_from_file() {
        let content = r#"{
            "folders": [
                { "path": "/home/user/project1" },
                { "path": "/home/user/project2", "name": "Custom Name" }
            ],
            "settings": {}
        }"#;
        let ws = Workspace::from_workspace_file(
            VsUri::file("/home/user/test.code-workspace"),
            content,
        )
        .unwrap();

        assert!(matches!(ws.get_workspace_type(), WorkspaceType::MultiRoot(_)));
        assert_eq!(ws.get_folders().len(), 2);
        assert_eq!(ws.get_folders()[0].name, "project1");
        assert_eq!(ws.get_folders()[0].index, 0);
        assert_eq!(ws.get_folders()[1].name, "Custom Name");
        assert_eq!(ws.get_folders()[1].index, 1);
    }

    #[test]
    fn workspace_file_parsing_error() {
        let result = Workspace::from_workspace_file(
            VsUri::file("/home/user/test.code-workspace"),
            "not json",
        );
        assert!(result.is_err());
    }

    #[test]
    fn get_folder_by_uri() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let found = ws.get_folder_by_uri(&VsUri::file("/home/user/project"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "project");

        let not_found = ws.get_folder_by_uri(&VsUri::file("/home/user/other"));
        assert!(not_found.is_none());
    }

    #[test]
    fn resource_resolution_single() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let resource = VsUri::file("/home/user/project/src/main.rs");
        let folder = ws.get_workspace_folder_of_resource(&resource);
        assert!(folder.is_some());
        assert_eq!(folder.unwrap().name, "project");
    }

    #[test]
    fn resource_resolution_no_match() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let resource = VsUri::file("/home/other/file.rs");
        assert!(ws.get_workspace_folder_of_resource(&resource).is_none());
    }

    #[test]
    fn resource_resolution_best_match() {
        let content = r#"{
            "folders": [
                { "path": "/home/user" },
                { "path": "/home/user/project" }
            ]
        }"#;
        let ws = Workspace::from_workspace_file(
            VsUri::file("/home/user/ws.code-workspace"),
            content,
        )
        .unwrap();

        let resource = VsUri::file("/home/user/project/src/main.rs");
        let folder = ws.get_workspace_folder_of_resource(&resource).unwrap();
        assert_eq!(folder.uri.path, "/home/user/project");
    }

    #[test]
    fn resource_resolution_scheme_mismatch() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let resource = VsUri::from_components("untitled", "", "/home/user/project/file", "", "");
        assert!(ws.get_workspace_folder_of_resource(&resource).is_none());
    }

    #[test]
    fn add_folder() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/project1"), None);
        assert_eq!(ws.get_folders().len(), 1);
        assert_eq!(ws.get_folders()[0].name, "project1");

        ws.add_folder(
            VsUri::file("/home/user/project2"),
            Some("My Project".into()),
        );
        assert_eq!(ws.get_folders().len(), 2);
        assert_eq!(ws.get_folders()[1].name, "My Project");
        assert_eq!(ws.get_folders()[1].index, 1);
    }

    #[test]
    fn add_folder_duplicate_ignored() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/project"), None);
        ws.add_folder(VsUri::file("/home/user/project"), None);
        assert_eq!(ws.get_folders().len(), 1);
    }

    #[test]
    fn remove_folder() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/a"), None);
        ws.add_folder(VsUri::file("/home/user/b"), None);
        ws.add_folder(VsUri::file("/home/user/c"), None);

        ws.remove_folder(&VsUri::file("/home/user/b"));
        assert_eq!(ws.get_folders().len(), 2);
        assert_eq!(ws.get_folders()[0].name, "a");
        assert_eq!(ws.get_folders()[0].index, 0);
        assert_eq!(ws.get_folders()[1].name, "c");
        assert_eq!(ws.get_folders()[1].index, 1);
    }

    #[test]
    fn remove_folder_nonexistent_is_noop() {
        let mut ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        ws.remove_folder(&VsUri::file("/home/user/other"));
        assert_eq!(ws.get_folders().len(), 1);
    }

    #[test]
    fn change_event_on_add() {
        let mut ws = Workspace::empty();
        let events = Arc::new(Mutex::new(Vec::new()));
        let e = events.clone();
        let _h = ws.on_did_change_folders().on(move |ev| {
            e.lock().unwrap().push((ev.added.len(), ev.removed.len()));
        });

        ws.add_folder(VsUri::file("/home/user/project"), None);
        let evts = events.lock().unwrap();
        assert_eq!(evts.len(), 1);
        assert_eq!(evts[0], (1, 0));
    }

    #[test]
    fn change_event_on_remove() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/project"), None);

        let events = Arc::new(Mutex::new(Vec::new()));
        let e = events.clone();
        let _h = ws.on_did_change_folders().on(move |ev| {
            e.lock().unwrap().push((ev.added.len(), ev.removed.len()));
        });

        ws.remove_folder(&VsUri::file("/home/user/project"));
        let evts = events.lock().unwrap();
        assert_eq!(evts.len(), 1);
        assert_eq!(evts[0], (0, 1));
    }

    #[test]
    fn folder_name_derived_from_path() {
        let uri = VsUri::file("/home/user/my-project");
        assert_eq!(folder_name_from_uri(&uri), "my-project");
    }

    #[test]
    fn workspace_type_single_with_file_is_multiroot() {
        let content = r#"{ "folders": [{ "path": "/home/user/project" }] }"#;
        let ws = Workspace::from_workspace_file(
            VsUri::file("/home/user/ws.code-workspace"),
            content,
        )
        .unwrap();
        assert!(matches!(ws.get_workspace_type(), WorkspaceType::MultiRoot(_)));
    }

    #[test]
    fn eq_workspacetype_same() {
        assert_eq!(WorkspaceType::Empty, WorkspaceType::Empty);
    }

    #[test]
    fn ne_workspacetype_diff() {
        assert_ne!(
            WorkspaceType::Empty,
            WorkspaceType::SingleFolder(PathBuf::from("/tmp"))
        );
    }

    // -- New tests for requested features --

    #[test]
    fn open_folder_creates_single_folder_workspace() {
        let ws = Workspace::open_folder(Path::new("/home/user/myapp"));
        assert!(matches!(ws.get_workspace_type(), WorkspaceType::SingleFolder(_)));
        assert_eq!(ws.get_folders().len(), 1);
        assert_eq!(ws.get_folders()[0].name, "myapp");
    }

    #[test]
    fn remove_folder_by_index() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/a"), None);
        ws.add_folder(VsUri::file("/home/user/b"), None);
        ws.add_folder(VsUri::file("/home/user/c"), None);

        let removed = ws.remove_folder_by_index(1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "b");
        assert_eq!(ws.get_folders().len(), 2);
        assert_eq!(ws.get_folders()[1].index, 1);
    }

    #[test]
    fn remove_folder_by_index_out_of_bounds() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/a"), None);
        assert!(ws.remove_folder_by_index(5).is_none());
        assert_eq!(ws.get_folders().len(), 1);
    }

    #[test]
    fn get_workspace_folder_alias() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let resource = VsUri::file("/home/user/project/src/main.rs");
        assert!(ws.get_workspace_folder(&resource).is_some());
    }

    #[test]
    fn relative_path_resolution() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let resource = VsUri::file("/home/user/project/src/main.rs");
        let rel = ws.relative_path(&resource);
        assert_eq!(rel.as_deref(), Some("src/main.rs"));
    }

    #[test]
    fn relative_path_no_match() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let resource = VsUri::file("/home/other/file.rs");
        assert!(ws.relative_path(&resource).is_none());
    }

    #[test]
    fn resolve_path_from_workspace_root() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        let resolved = ws.resolve_path("src/lib.rs").unwrap();
        assert_eq!(resolved, PathBuf::from("/home/user/project/src/lib.rs"));
    }

    #[test]
    fn resolve_path_empty_workspace() {
        let ws = Workspace::empty();
        assert!(ws.resolve_path("src/lib.rs").is_none());
    }

    #[test]
    fn is_inside_workspace_true() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        assert!(ws.is_inside_workspace(Path::new("/home/user/project/src/main.rs")));
    }

    #[test]
    fn is_inside_workspace_false() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        assert!(!ws.is_inside_workspace(Path::new("/home/other/file.rs")));
    }

    #[test]
    fn get_workspace_root() {
        let ws = Workspace::single_folder(VsUri::file("/home/user/project"));
        assert_eq!(
            ws.get_workspace_root(),
            Some(PathBuf::from("/home/user/project"))
        );
    }

    #[test]
    fn workspace_trust_default_unknown() {
        let ws = Workspace::empty();
        assert_eq!(ws.trust(), WorkspaceTrust::Unknown);
        assert!(!ws.is_trusted());
    }

    #[test]
    fn trust_workspace() {
        let mut ws = Workspace::empty();
        ws.trust_workspace();
        assert_eq!(ws.trust(), WorkspaceTrust::Trusted);
        assert!(ws.is_trusted());
    }

    #[test]
    fn workspace_configuration_from_file() {
        let content = r#"{
            "folders": [{ "path": "/home/user/project" }],
            "settings": { "editor.fontSize": 14 }
        }"#;
        let ws = Workspace::from_workspace_file(
            VsUri::file("/home/user/ws.code-workspace"),
            content,
        )
        .unwrap();
        assert_eq!(ws.configuration()["editor.fontSize"], 14);
    }

    #[test]
    fn save_and_parse_workspace_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.code-workspace");

        let wf = WorkspaceFile {
            folders: vec![
                FolderEntry {
                    path: "/home/user/a".into(),
                    name: Some("Alpha".into()),
                },
                FolderEntry {
                    path: "/home/user/b".into(),
                    name: None,
                },
            ],
            settings: serde_json::json!({ "editor.tabSize": 2 }),
            extensions: serde_json::Value::Null,
            launch: serde_json::Value::Null,
        };
        save_workspace_file(&wf, &path).unwrap();

        let parsed = parse_workspace_file(&path).unwrap();
        assert_eq!(parsed.folders.len(), 2);
        assert_eq!(parsed.folders[0].path, "/home/user/a");
        assert_eq!(parsed.folders[0].name.as_deref(), Some("Alpha"));
        assert_eq!(parsed.folders[1].name, None);
        assert_eq!(parsed.settings["editor.tabSize"], 2);
    }

    #[test]
    fn parse_workspace_file_not_found() {
        let result = parse_workspace_file(Path::new("/nonexistent/file.code-workspace"));
        assert!(result.is_err());
    }

    #[test]
    fn save_workspace_as() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saved.code-workspace");

        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/alpha"), Some("Alpha".into()));
        ws.add_folder(VsUri::file("/home/user/beta"), None);
        ws.save_workspace_as(&path).unwrap();

        let parsed = parse_workspace_file(&path).unwrap();
        assert_eq!(parsed.folders.len(), 2);
        assert_eq!(parsed.folders[0].name.as_deref(), Some("Alpha"));
    }

    #[test]
    fn recent_workspaces_add_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("recents.json");

        let mut store = RecentWorkspaces::new(state_path.clone());
        assert!(store.get_recents().is_empty());

        store.add_recent(RecentWorkspace {
            path: "/home/user/a".into(),
            label: "A".into(),
            last_opened: 1000,
        });
        store.add_recent(RecentWorkspace {
            path: "/home/user/b".into(),
            label: "B".into(),
            last_opened: 2000,
        });
        assert_eq!(store.get_recents().len(), 2);
        assert_eq!(store.get_recents()[0].path, "/home/user/b");

        // Reload from disk.
        let store2 = RecentWorkspaces::new(state_path);
        assert_eq!(store2.get_recents().len(), 2);
    }

    #[test]
    fn recent_workspaces_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("recents.json");

        let mut store = RecentWorkspaces::new(state_path);
        store.add_recent(RecentWorkspace {
            path: "/home/user/a".into(),
            label: "A".into(),
            last_opened: 1000,
        });
        store.add_recent(RecentWorkspace {
            path: "/home/user/a".into(),
            label: "A (updated)".into(),
            last_opened: 2000,
        });
        assert_eq!(store.get_recents().len(), 1);
        assert_eq!(store.get_recents()[0].label, "A (updated)");
    }

    #[test]
    fn recent_workspaces_clear() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("recents.json");

        let mut store = RecentWorkspaces::new(state_path);
        store.add_recent(RecentWorkspace {
            path: "/home/user/a".into(),
            label: "A".into(),
            last_opened: 1000,
        });
        store.clear_recents();
        assert!(store.get_recents().is_empty());
    }

    #[test]
    fn on_did_open_workspace_event() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let e = events.clone();

        // Build manually so we can subscribe before firing.
        let mut ws = Workspace::empty();
        let _h = ws.on_did_open_workspace().on(move |ev| {
            e.lock().unwrap().push(ev.workspace_type.clone());
        });
        // Simulate opening.
        ws.add_folder(VsUri::file("/home/user/project"), None);
        ws.on_did_open_workspace.fire(&WorkspaceOpenEvent {
            workspace_type: ws.get_workspace_type(),
        });

        let evts = events.lock().unwrap();
        assert_eq!(evts.len(), 1);
    }

    #[test]
    fn on_will_delete_workspace_folder_event() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/home/user/a"), None);
        ws.add_folder(VsUri::file("/home/user/b"), None);

        let deleted = Arc::new(Mutex::new(Vec::new()));
        let d = deleted.clone();
        let _h = ws.on_will_delete_workspace_folder().on(move |folder| {
            d.lock().unwrap().push(folder.name.clone());
        });

        ws.remove_folder_by_index(0);
        let names = deleted.lock().unwrap();
        assert_eq!(&*names, &["a"]);
    }

    #[test]
    fn workspace_file_roundtrip_with_extensions_and_launch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("full.code-workspace");

        let wf = WorkspaceFile {
            folders: vec![FolderEntry {
                path: "/src".into(),
                name: None,
            }],
            settings: serde_json::json!({}),
            extensions: serde_json::json!({ "recommendations": ["rust-lang.rust-analyzer"] }),
            launch: serde_json::json!({ "version": "0.2.0" }),
        };
        save_workspace_file(&wf, &path).unwrap();
        let parsed = parse_workspace_file(&path).unwrap();
        assert_eq!(
            parsed.extensions["recommendations"][0],
            "rust-lang.rust-analyzer"
        );
        assert_eq!(parsed.launch["version"], "0.2.0");
    }

    #[test]
    fn folder_entry_serde() {
        let entry = FolderEntry {
            path: "/home/user/src".into(),
            name: Some("Source".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: FolderEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "/home/user/src");
        assert_eq!(parsed.name.as_deref(), Some("Source"));
    }

    #[test]
    fn workspace_trust_serde() {
        let t = WorkspaceTrust::Trusted;
        let json = serde_json::to_string(&t).unwrap();
        let parsed: WorkspaceTrust = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorkspaceTrust::Trusted);
    }
}
