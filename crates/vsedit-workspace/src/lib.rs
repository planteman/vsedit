//! Workspace folder management.
//!
//! Manages workspace folders and multi-root workspaces, equivalent to
//! VS Code's `vs/platform/workspace/common/workspace.ts`.

use std::collections::HashMap;
use std::fmt;
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

    /// Number of recent workspace entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no recent entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove a specific workspace by path.
    pub fn remove_by_path(&mut self, path: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.path != path);
        if self.entries.len() < before {
            self.persist();
            true
        } else {
            false
        }
    }

    /// Return entries matching a label substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&RecentWorkspace> {
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.label.to_lowercase().contains(&lower))
            .collect()
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

    /// Number of folders in the workspace.
    pub fn folder_count(&self) -> usize {
        self.folders.len()
    }

    /// Whether the workspace has no folders.
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    /// Get a folder by its index.
    pub fn get_folder_by_index(&self, index: usize) -> Option<&WorkspaceFolder> {
        self.folders.get(index)
    }

    /// Get a folder by its display name.
    pub fn get_folder_by_name(&self, name: &str) -> Option<&WorkspaceFolder> {
        self.folders.iter().find(|f| f.name == name)
    }

    /// Return all folder names.
    pub fn folder_names(&self) -> Vec<&str> {
        self.folders.iter().map(|f| f.name.as_str()).collect()
    }

    /// Return all folder URIs.
    pub fn folder_uris(&self) -> Vec<&VsUri> {
        self.folders.iter().map(|f| &f.uri).collect()
    }

    /// Mark the workspace as untrusted.
    pub fn distrust_workspace(&mut self) {
        self.trust = WorkspaceTrust::Untrusted;
    }

    /// Set workspace-level configuration.
    pub fn set_configuration(&mut self, config: serde_json::Value) {
        self.configuration = config;
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
// WorkspaceFolder helpers
// ---------------------------------------------------------------------------

impl WorkspaceFolder {
    /// Create a new workspace folder from components.
    pub fn new(uri: VsUri, name: impl Into<String>, index: usize) -> Self {
        Self {
            uri,
            name: name.into(),
            index,
        }
    }

    /// Whether this folder's URI points to a `file://` scheme.
    pub fn is_local(&self) -> bool {
        self.uri.is_file()
    }

    /// Return the filesystem path of this folder (only meaningful for `file://`).
    pub fn fs_path(&self) -> PathBuf {
        PathBuf::from(self.uri.fs_path())
    }

    /// Check whether a given path is inside this folder (path-prefix match).
    pub fn contains_path(&self, path: &Path) -> bool {
        let folder = ensure_trailing_slash(&self.uri.path);
        let candidate = ensure_trailing_slash(&path.to_string_lossy());
        candidate.starts_with(&folder)
    }

    /// Return the folder name uppercased – handy for display headers.
    pub fn display_name(&self) -> String {
        self.name.clone()
    }
}

// ---------------------------------------------------------------------------
// WorkspaceType helpers
// ---------------------------------------------------------------------------

impl WorkspaceType {
    /// Whether this workspace type has at least one folder.
    pub fn has_folders(&self) -> bool {
        !matches!(self, WorkspaceType::Empty)
    }

    /// Whether this is a multi-root workspace.
    pub fn is_multi_root(&self) -> bool {
        matches!(self, WorkspaceType::MultiRoot(_))
    }

    /// Whether this is a single-folder workspace.
    pub fn is_single_folder(&self) -> bool {
        matches!(self, WorkspaceType::SingleFolder(_))
    }

    /// Return the associated path, if any.
    pub fn path(&self) -> Option<&Path> {
        match self {
            WorkspaceType::Empty => None,
            WorkspaceType::SingleFolder(p) | WorkspaceType::MultiRoot(p) => Some(p),
        }
    }

    /// A short human-readable label for the workspace type.
    pub fn label(&self) -> &'static str {
        match self {
            WorkspaceType::Empty => "Empty Workspace",
            WorkspaceType::SingleFolder(_) => "Folder",
            WorkspaceType::MultiRoot(_) => "Workspace",
        }
    }
}

// ---------------------------------------------------------------------------
// WorkspaceTrust helpers
// ---------------------------------------------------------------------------

impl WorkspaceTrust {
    /// Whether trust has been explicitly decided (trusted or untrusted).
    pub fn is_decided(&self) -> bool {
        *self != WorkspaceTrust::Unknown
    }

    /// Whether the workspace is explicitly untrusted.
    pub fn is_untrusted(&self) -> bool {
        *self == WorkspaceTrust::Untrusted
    }

    /// Merge two trust levels – the more restrictive one wins.
    ///
    /// Order: Untrusted < Unknown < Trusted
    pub fn merge(self, other: WorkspaceTrust) -> WorkspaceTrust {
        match (self, other) {
            (WorkspaceTrust::Untrusted, _) | (_, WorkspaceTrust::Untrusted) => {
                WorkspaceTrust::Untrusted
            }
            (WorkspaceTrust::Unknown, _) | (_, WorkspaceTrust::Unknown) => {
                WorkspaceTrust::Unknown
            }
            _ => WorkspaceTrust::Trusted,
        }
    }
}

// ---------------------------------------------------------------------------
// WorkspaceFile helpers
// ---------------------------------------------------------------------------

impl WorkspaceFile {
    /// Create a minimal workspace file with only folder entries.
    pub fn with_folders(folders: Vec<FolderEntry>) -> Self {
        Self {
            folders,
            settings: serde_json::Value::Null,
            extensions: serde_json::Value::Null,
            launch: serde_json::Value::Null,
        }
    }

    /// Number of folder entries.
    pub fn folder_count(&self) -> usize {
        self.folders.len()
    }

    /// Whether there are any workspace-level settings.
    pub fn has_settings(&self) -> bool {
        !self.settings.is_null()
            && self.settings != serde_json::Value::Object(serde_json::Map::new())
    }

    /// Collect all folder paths as a `Vec<&str>`.
    pub fn folder_paths(&self) -> Vec<&str> {
        self.folders.iter().map(|f| f.path.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// FolderEntry helpers
// ---------------------------------------------------------------------------

impl FolderEntry {
    /// Create a folder entry with just a path.
    pub fn from_path(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: None,
        }
    }

    /// Create a folder entry with a path and display name.
    pub fn with_name(path: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: Some(name.into()),
        }
    }

    /// Return the effective display name (explicit name or last path segment).
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or_else(|| {
            self.path
                .rsplit('/')
                .find(|s| !s.is_empty())
                .unwrap_or(&self.path)
        })
    }
}

// ---------------------------------------------------------------------------
// WorkspaceFileIndex
// ---------------------------------------------------------------------------

/// Fast file lookup index for a workspace.
pub struct WorkspaceFileIndex {
    pub files: Vec<String>,
    pub root: String,
}

impl WorkspaceFileIndex {
    /// Create a new empty file index rooted at the given path.
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            files: Vec::new(),
            root: root.into(),
        }
    }

    /// Add a file path to the index.
    pub fn add_file(&mut self, path: impl Into<String>) {
        self.files.push(path.into());
    }

    /// Remove a file path from the index. Returns `true` if the file was found.
    pub fn remove_file(&mut self, path: &str) -> bool {
        if let Some(pos) = self.files.iter().position(|f| f == path) {
            self.files.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check whether the index contains a given file path.
    pub fn contains(&self, path: &str) -> bool {
        self.files.iter().any(|f| f == path)
    }

    /// Return the number of indexed files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Return all indexed file paths.
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// Return file paths that end with the given extension (e.g. `"rs"`).
    pub fn files_with_extension(&self, ext: &str) -> Vec<&str> {
        let suffix = format!(".{ext}");
        self.files
            .iter()
            .filter(|f| f.ends_with(&suffix))
            .map(String::as_str)
            .collect()
    }

    /// Remove all files from the index.
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Return files matching a glob pattern.
    pub fn search_glob(&self, pattern: &str) -> Vec<&str> {
        self.files
            .iter()
            .filter(|f| workspace_glob_match(pattern, f))
            .map(String::as_str)
            .collect()
    }

    /// Return files that do NOT match any of the exclusion patterns.
    pub fn exclude(&self, patterns: &[&str]) -> Vec<&str> {
        self.files
            .iter()
            .filter(|f| !workspace_exclude_pattern(patterns, f))
            .map(String::as_str)
            .collect()
    }

    /// Return all unique file extensions present in the index.
    pub fn extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = self
            .files
            .iter()
            .filter_map(|f| {
                f.rsplit('.')
                    .next()
                    .filter(|e| !e.contains('/') && !e.is_empty())
                    .map(String::from)
            })
            .collect();
        exts.sort();
        exts.dedup();
        exts
    }

    /// Return files whose basename (last path segment) contains `query`
    /// (case-insensitive).
    pub fn search_by_name(&self, query: &str) -> Vec<&str> {
        let lower = query.to_lowercase();
        self.files
            .iter()
            .filter(|f| {
                let basename = f.rsplit('/').next().unwrap_or(f);
                basename.to_lowercase().contains(&lower)
            })
            .map(String::as_str)
            .collect()
    }

    /// Group files by their parent directory (first path segment before `/`).
    pub fn group_by_directory(&self) -> std::collections::HashMap<String, Vec<&str>> {
        let mut map: std::collections::HashMap<String, Vec<&str>> =
            std::collections::HashMap::new();
        for file in &self.files {
            let dir = match file.rfind('/') {
                Some(pos) => &file[..pos],
                None => ".",
            };
            map.entry(dir.to_string()).or_default().push(file.as_str());
        }
        map
    }
}

// ---------------------------------------------------------------------------
// Glob helpers
// ---------------------------------------------------------------------------

/// Simple glob matching supporting `*`, `**`, and `?`.
///
/// - `*` matches any sequence of characters within a single path segment
///   (i.e. not `/`).
/// - `**` matches any sequence of characters across segment boundaries.
/// - `?` matches exactly one non-`/` character.
pub fn workspace_glob_match(pattern: &str, path: &str) -> bool {
    glob_match_recursive(pattern.as_bytes(), path.as_bytes())
}

fn glob_match_recursive(pat: &[u8], text: &[u8]) -> bool {
    let mut pi = 0;
    let mut ti = 0;

    // Positions for backtracking on single `*`.
    let mut star_pi: Option<usize> = None;
    let mut star_ti: usize = 0;

    while ti < text.len() {
        if pi < pat.len() && pat[pi] == b'*' {
            // Check for `**`
            if pi + 1 < pat.len() && pat[pi + 1] == b'*' {
                // `**` – try matching the rest of the pattern against every
                // possible suffix of the text (greedy across `/`).
                let rest = pi + 2;
                // Skip an optional `/` after `**`.
                let rest = if rest < pat.len() && pat[rest] == b'/' {
                    rest + 1
                } else {
                    rest
                };
                // If `**` is at the end of the pattern, it matches everything.
                if rest >= pat.len() {
                    return true;
                }
                for start in ti..=text.len() {
                    if glob_match_recursive(&pat[rest..], &text[start..]) {
                        return true;
                    }
                }
                return false;
            }

            // Single `*` – matches anything except `/`.
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
            continue;
        }

        if pi < pat.len() && pat[pi] == b'?' {
            if text[ti] == b'/' {
                // `?` must not cross segments.
                if let Some(sp) = star_pi {
                    pi = sp + 1;
                    star_ti += 1;
                    ti = star_ti;
                    continue;
                }
                return false;
            }
            pi += 1;
            ti += 1;
            continue;
        }

        if pi < pat.len() && pat[pi] == text[ti] {
            pi += 1;
            ti += 1;
            continue;
        }

        // Mismatch – backtrack to last `*` if possible.
        if let Some(sp) = star_pi {
            if text[ti] == b'/' {
                // `*` cannot cross `/`.
                return false;
            }
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
            continue;
        }

        return false;
    }

    // Consume trailing `*` / `**` in pattern.
    while pi < pat.len() && pat[pi] == b'*' {
        pi += 1;
    }

    pi == pat.len()
}

/// Returns `true` if `path` matches any of the given exclusion patterns.
pub fn workspace_exclude_pattern(patterns: &[&str], path: &str) -> bool {
    patterns
        .iter()
        .any(|pat| workspace_glob_match(pat, path))
}

// ---------------------------------------------------------------------------
// WorkspaceSearchIndex
// ---------------------------------------------------------------------------

/// Search index for workspace files by name and content snippets.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceSearchIndex {
    entries: Vec<WorkspaceSearchEntry>,
}

/// An indexed entry for search.
#[derive(Debug, Clone)]
pub struct WorkspaceSearchEntry {
    pub path: String,
    pub name: String,
    pub content_snippet: Option<String>,
}

impl WorkspaceSearchIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file to the search index.
    pub fn add(&mut self, path: &str, content_snippet: Option<&str>) {
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .to_string();
        self.entries.push(WorkspaceSearchEntry {
            path: path.to_string(),
            name,
            content_snippet: content_snippet.map(String::from),
        });
    }

    /// Remove an entry by path.
    pub fn remove(&mut self, path: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.path != path);
        self.entries.len() < before
    }

    /// Search by file name substring (case-insensitive).
    pub fn search_by_name(&self, query: &str) -> Vec<&WorkspaceSearchEntry> {
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.name.to_lowercase().contains(&lower))
            .collect()
    }

    /// Search by content snippet substring (case-insensitive).
    pub fn search_by_content(&self, query: &str) -> Vec<&WorkspaceSearchEntry> {
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.content_snippet
                    .as_ref()
                    .map_or(false, |s| s.to_lowercase().contains(&lower))
            })
            .collect()
    }

    /// Number of indexed entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return all indexed entries.
    pub fn entries(&self) -> &[WorkspaceSearchEntry] {
        &self.entries
    }

    /// Search by both name and content snippet.
    pub fn search(&self, query: &str) -> Vec<&WorkspaceSearchEntry> {
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&lower)
                    || e.content_snippet
                        .as_ref()
                        .map_or(false, |s| s.to_lowercase().contains(&lower))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// WorkspaceLayoutConfig
// ---------------------------------------------------------------------------

/// Configuration for workspace panel layout.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceLayoutConfig {
    pub sidebar_visible: bool,
    pub sidebar_position: SidebarPosition,
    pub panel_visible: bool,
    pub panel_position: PanelPosition,
    pub editor_tab_size: u32,
    pub minimap_enabled: bool,
}

/// Sidebar position in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarPosition {
    Left,
    Right,
}

/// Panel position in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelPosition {
    Bottom,
    Right,
    Left,
}

impl WorkspaceLayoutConfig {
    pub fn new() -> Self {
        Self {
            sidebar_visible: true,
            sidebar_position: SidebarPosition::Left,
            panel_visible: true,
            panel_position: PanelPosition::Bottom,
            editor_tab_size: 4,
            minimap_enabled: true,
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    pub fn toggle_minimap(&mut self) {
        self.minimap_enabled = !self.minimap_enabled;
    }

    pub fn set_tab_size(&mut self, size: u32) {
        if size > 0 && size <= 16 {
            self.editor_tab_size = size;
        }
    }
}

impl Default for WorkspaceLayoutConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WorkspaceSnapshot
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of workspace state for comparison.
#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    pub folder_paths: Vec<String>,
    pub trust: WorkspaceTrust,
    pub file_count: usize,
    pub tags: Vec<String>,
    pub timestamp_ms: u64,
}

impl WorkspaceSnapshot {
    /// Capture a snapshot from a workspace and file index.
    pub fn capture(ws: &Workspace, index: &WorkspaceFileIndex, timestamp_ms: u64) -> Self {
        Self {
            folder_paths: ws.get_folders().iter().map(|f| f.uri.path.clone()).collect(),
            trust: ws.trust(),
            file_count: index.file_count(),
            tags: Vec::new(),
            timestamp_ms,
        }
    }

    /// Compare two snapshots and return a summary of differences.
    pub fn diff(&self, other: &WorkspaceSnapshot) -> WorkspaceSnapshotDiff {
        let folders_added: Vec<String> = other
            .folder_paths
            .iter()
            .filter(|p| !self.folder_paths.contains(p))
            .cloned()
            .collect();
        let folders_removed: Vec<String> = self
            .folder_paths
            .iter()
            .filter(|p| !other.folder_paths.contains(p))
            .cloned()
            .collect();
        WorkspaceSnapshotDiff {
            folders_added,
            folders_removed,
            file_count_delta: other.file_count as i64 - self.file_count as i64,
            trust_changed: self.trust != other.trust,
        }
    }

    /// Add a tag to the snapshot.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.push(tag.into());
    }

    /// Check if the snapshot has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Number of folders in this snapshot.
    pub fn folder_count(&self) -> usize {
        self.folder_paths.len()
    }

    /// Whether this snapshot represents an empty workspace.
    pub fn is_empty(&self) -> bool {
        self.folder_paths.is_empty()
    }
}

/// Differences between two workspace snapshots.
#[derive(Debug, Clone)]
pub struct WorkspaceSnapshotDiff {
    pub folders_added: Vec<String>,
    pub folders_removed: Vec<String>,
    pub file_count_delta: i64,
    pub trust_changed: bool,
}

impl WorkspaceSnapshotDiff {
    /// Whether there are any differences.
    pub fn has_changes(&self) -> bool {
        !self.folders_added.is_empty()
            || !self.folders_removed.is_empty()
            || self.file_count_delta != 0
            || self.trust_changed
    }
}

// ---------------------------------------------------------------------------
// Workspace tag/label extension
// ---------------------------------------------------------------------------

/// Tags/labels attached to a workspace for organization.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceTags {
    tags: Vec<String>,
}

impl WorkspaceTags {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tag if not already present.
    pub fn add(&mut self, tag: &str) -> bool {
        if self.tags.iter().any(|t| t == tag) {
            return false;
        }
        self.tags.push(tag.to_string());
        true
    }

    /// Remove a tag. Returns `true` if it was present.
    pub fn remove(&mut self, tag: &str) -> bool {
        let before = self.tags.len();
        self.tags.retain(|t| t != tag);
        self.tags.len() < before
    }

    /// Check if a tag exists.
    pub fn contains(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Return all tags.
    pub fn all(&self) -> &[String] {
        &self.tags
    }

    /// Number of tags.
    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// Whether there are no tags.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// Return tags matching a prefix (case-insensitive).
    pub fn search(&self, prefix: &str) -> Vec<&str> {
        let lower = prefix.to_lowercase();
        self.tags
            .iter()
            .filter(|t| t.to_lowercase().starts_with(&lower))
            .map(String::as_str)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// === Workspace Settings Merger ===

/// Workspace Settings Merger implementation.
#[derive(Debug, Clone)]
pub struct WorkspaceSettingsMerger {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: WorkspaceSettingsMergerStats,
}

/// Statistics for WorkspaceSettingsMerger.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceSettingsMergerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl WorkspaceSettingsMergerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl WorkspaceSettingsMerger {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: WorkspaceSettingsMergerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &WorkspaceSettingsMergerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for WorkspaceSettingsMerger {
    fn default() -> Self {
        Self::new()
    }
}

// === Workspace Folder Watcher ===

/// Priority level for WorkspaceFolderWatcher items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkspaceFolderWatcherPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl WorkspaceFolderWatcherPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for WorkspaceFolderWatcherPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Workspace Folder Watcher implementation.
#[derive(Debug, Clone)]
pub struct WorkspaceFolderWatcher {
    items: Vec<WorkspaceFolderWatcherItem>,
    max_items: usize,
    default_priority: WorkspaceFolderWatcherPriority,
}

/// A single item in WorkspaceFolderWatcher.
#[derive(Debug, Clone)]
pub struct WorkspaceFolderWatcherItem {
    pub id: String,
    pub label: String,
    pub priority: WorkspaceFolderWatcherPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl WorkspaceFolderWatcherItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: WorkspaceFolderWatcherPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: WorkspaceFolderWatcherPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl WorkspaceFolderWatcher {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: WorkspaceFolderWatcherPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: WorkspaceFolderWatcherItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<WorkspaceFolderWatcherItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&WorkspaceFolderWatcherItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: WorkspaceFolderWatcherPriority) -> Vec<&WorkspaceFolderWatcherItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&WorkspaceFolderWatcherItem> {
        let mut sorted: Vec<&WorkspaceFolderWatcherItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&WorkspaceFolderWatcherItem> {
        let mut sorted: Vec<&WorkspaceFolderWatcherItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&WorkspaceFolderWatcherItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: WorkspaceFolderWatcherPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> WorkspaceFolderWatcherPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceFolderWatcherItem> {
        self.items.iter()
    }
}

impl Default for WorkspaceFolderWatcher {
    fn default() -> Self {
        Self::new()
    }
}


/// Workspace configuration manager.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    entries: Vec<WorkspaceEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workspace entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WorkspaceEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WorkspaceConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WorkspaceEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WorkspaceEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WorkspaceEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WorkspaceEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WorkspaceEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WorkspaceEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WorkspaceEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for workspace
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWorkspaceRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWorkspaceRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaWorkspaceCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWorkspaceCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaWorkspaceCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 240
// ---------------------------------------------------------------------------

/// Generic object pool `Xc240Pool<T>`.
pub struct Xc240Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc240Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc240PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc240Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc240PoolStats {
        Xc240PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc240Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc240Scheduler`.
pub struct Xc240Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc240Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc240Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_240 hash for the given byte slice.
pub fn xc_240_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_240 convention.
pub fn xc_240_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_47 deepening: state machine + event bus ---

/// States for the Xd47 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd47State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd47State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd47Transition {
    pub from: Xd47State,
    pub to: Xd47State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd47StateMachine {
    current: Xd47State,
    history: Vec<Xd47Transition>,
    step_counter: usize,
}

impl Xd47StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd47State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd47State {
        self.current
    }

    pub fn history(&self) -> &[Xd47Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd47State) -> Result<Xd47State, String> {
        let allowed = match (self.current, target) {
            (Xd47State::Idle, Xd47State::Running) => true,
            (Xd47State::Running, Xd47State::Paused) => true,
            (Xd47State::Running, Xd47State::Done) => true,
            (Xd47State::Paused, Xd47State::Running) => true,
            (Xd47State::Paused, Xd47State::Done) => true,
            (Xd47State::Done, Xd47State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_47: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd47Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd47SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd47State> {
        let prefix = "Xd47SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd47State::Idle),
            "Running" => Some(Xd47State::Running),
            "Paused" => Some(Xd47State::Paused),
            "Done" => Some(Xd47State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd47State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd47 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd47Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd47Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd47HandlerFn = Box<dyn Fn(&Xd47Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd47EventBus {
    handlers: Vec<(usize, Option<String>, Xd47HandlerFn)>,
    next_id: usize,
    published: Vec<Xd47Event>,
}

impl Xd47EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd47Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd47Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd47Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd47Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #45
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf45Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf45TrieNode {
    children: std::collections::HashMap<char, Xf45TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf45Trie {
    root: Xf45TrieNode,
    count: usize,
}

impl Xf45Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf45TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf45TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf45TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf45BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf45BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 239).
pub struct Xh239SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh239SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 281 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 239).
pub struct Xh239BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh239BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 239).
pub struct Xi239Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi239Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi239Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi239Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 239).
pub struct Xi239IntervalTree {
    xi_intervals: Vec<Xi239Interval>,
}

impl Xi239IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi239Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi239Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi239Interval) -> Vec<&Xi239Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi239Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi239Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi239Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi239Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi239Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi239Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 239) ---

/// Disjoint set / union-find for crate 239.
pub struct Xj239UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj239UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ239_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 239.
pub struct Xj239BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj239BTreeNode<K, V>>>,
    len: usize,
}

struct Xj239BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj239BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj239BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ239_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ239_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj239BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj239BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj239BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj239BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_239 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk239SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk239SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk239DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk239DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_239).
#[derive(Debug, Clone)]
pub struct Xl239Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl239Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_239).
#[derive(Debug, Clone)]
pub struct Xl239SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl239SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm239MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm239MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm239Tokenizer {
    text: String,
}

impl Xm239Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 239.
pub struct Xn239Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn239Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 239 -----

#[derive(Debug, Clone)]
struct Xn239AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn239AvlNode<K, V>>>,
    right: Option<Box<Xn239AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 239.
#[derive(Debug, Clone)]
pub struct Xn239AVL<K, V> {
    root: Option<Box<Xn239AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn239AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn239AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn239AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn239AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn239AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn239AvlNode<K, V>>) -> Box<Xn239AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn239AvlNode<K, V>>) -> Box<Xn239AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn239AvlNode<K, V>>) -> Box<Xn239AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn239AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn239AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn239AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn239AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn239AvlNode<K, V>>) -> &Xn239AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn239AvlNode<K, V>>) -> (Box<Xn239AvlNode<K, V>>, Option<Box<Xn239AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn239AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn239AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn239AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn239AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn239AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn239AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn239AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo239RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo239Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo239RBNode<K, V> {
    key: K,
    value: V,
    color: Xo239Color,
    left: Option<Box<Xo239RBNode<K, V>>>,
    right: Option<Box<Xo239RBNode<K, V>>>,
}

/// A red-black tree map for crate 239.
#[derive(Debug, Clone)]
pub struct Xo239RedBlack<K, V> {
    root: Option<Box<Xo239RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo239RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo239Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo239RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo239RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo239RBNode {
                    key, value, color: Xo239Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo239RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo239Color::Red)
    }

    fn xo_balance(mut h: Box<Xo239RBNode<K, V>>) -> Box<Xo239RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo239Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo239RBNode<K, V>>) -> Box<Xo239RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo239Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo239RBNode<K, V>>) -> Box<Xo239RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo239Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo239RBNode<K, V>>) {
        h.color = Xo239Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo239Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo239Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo239Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo239RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo239RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo239RBNode<K, V>) -> (K, V, Option<Box<Xo239RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo239RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo239Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo239RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo239ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 239.
#[derive(Debug, Clone)]
pub struct Xo239ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo239ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo239#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo239#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}

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

    // -- File index and glob tests --

    #[test]
    fn test_file_index_add_contains() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("src/main.rs");
        idx.add_file("src/lib.rs");
        assert!(idx.contains("src/main.rs"));
        assert!(idx.contains("src/lib.rs"));
        assert!(!idx.contains("src/other.rs"));
        assert_eq!(idx.file_count(), 2);
    }

    #[test]
    fn test_file_index_remove() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("a.rs");
        idx.add_file("b.rs");
        assert!(idx.remove_file("a.rs"));
        assert!(!idx.contains("a.rs"));
        assert!(idx.contains("b.rs"));
        assert!(!idx.remove_file("nonexistent.rs"));
        assert_eq!(idx.file_count(), 1);
    }

    #[test]
    fn test_file_index_files_with_extension() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("main.rs");
        idx.add_file("lib.rs");
        idx.add_file("readme.md");
        idx.add_file("config.toml");
        let rs_files = idx.files_with_extension("rs");
        assert_eq!(rs_files.len(), 2);
        assert!(rs_files.contains(&"main.rs"));
        assert!(rs_files.contains(&"lib.rs"));
        assert!(idx.files_with_extension("py").is_empty());
    }

    #[test]
    fn test_file_index_clear() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("a.rs");
        idx.add_file("b.rs");
        idx.clear();
        assert_eq!(idx.file_count(), 0);
        assert!(idx.files().is_empty());
    }

    #[test]
    fn test_glob_match_star() {
        assert!(workspace_glob_match("*.rs", "main.rs"));
        assert!(workspace_glob_match("src/*.rs", "src/main.rs"));
        assert!(!workspace_glob_match("*.rs", "src/main.rs"));
        assert!(!workspace_glob_match("*.rs", "main.txt"));
    }

    #[test]
    fn test_glob_match_double_star() {
        assert!(workspace_glob_match("**/*.rs", "src/main.rs"));
        assert!(workspace_glob_match("**/*.rs", "a/b/c/main.rs"));
        assert!(workspace_glob_match("src/**", "src/a/b/c.rs"));
        assert!(workspace_glob_match("**", "anything/at/all"));
    }

    #[test]
    fn test_glob_match_question_mark() {
        assert!(workspace_glob_match("?.rs", "a.rs"));
        assert!(!workspace_glob_match("?.rs", "ab.rs"));
        assert!(!workspace_glob_match("?", "/"));
    }

    #[test]
    fn test_glob_match_no_match() {
        assert!(!workspace_glob_match("*.rs", "main.txt"));
        assert!(!workspace_glob_match("src/*.rs", "lib/main.rs"));
        assert!(!workspace_glob_match("foo", "bar"));
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(workspace_glob_match("main.rs", "main.rs"));
        assert!(workspace_glob_match("src/lib.rs", "src/lib.rs"));
        assert!(!workspace_glob_match("main.rs", "other.rs"));
    }

    #[test]
    fn test_exclude_pattern_single() {
        assert!(workspace_exclude_pattern(&["*.log"], "debug.log"));
        assert!(!workspace_exclude_pattern(&["*.log"], "main.rs"));
    }

    #[test]
    fn test_exclude_pattern_multiple() {
        let patterns = &["*.log", ".git/**", "node_modules/**"];
        assert!(workspace_exclude_pattern(patterns, "app.log"));
        assert!(workspace_exclude_pattern(patterns, ".git/config"));
        assert!(workspace_exclude_pattern(patterns, "node_modules/foo/index.js"));
        assert!(!workspace_exclude_pattern(patterns, "src/main.rs"));
    }

    #[test]
    fn test_file_index_search_glob() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("src/main.rs");
        idx.add_file("src/lib.rs");
        idx.add_file("tests/test.rs");
        idx.add_file("README.md");
        let results = idx.search_glob("src/*.rs");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"src/main.rs"));
        assert!(results.contains(&"src/lib.rs"));
    }

    #[test]
    fn test_file_index_exclude() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("src/main.rs");
        idx.add_file("build.log");
        idx.add_file(".git/config");
        idx.add_file("node_modules/foo/index.js");
        let kept = idx.exclude(&["*.log", ".git/**", "node_modules/**"]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0], "src/main.rs");
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

    #[test]
    fn workspace_search_index_by_name() {
        let mut idx = WorkspaceSearchIndex::new();
        idx.add("/src/main.rs", Some("fn main() {}"));
        idx.add("/src/lib.rs", Some("pub mod utils;"));
        idx.add("/README.md", None);

        let results = idx.search_by_name("main");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "/src/main.rs");

        let results = idx.search_by_name(".rs");
        assert_eq!(results.len(), 2);
        assert_eq!(idx.len(), 3);
    }

    #[test]
    fn workspace_search_index_by_content() {
        let mut idx = WorkspaceSearchIndex::new();
        idx.add("/a.rs", Some("fn hello() {}"));
        idx.add("/b.rs", Some("fn world() {}"));
        idx.add("/c.rs", None);

        let results = idx.search_by_content("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "/a.rs");
    }

    #[test]
    fn workspace_layout_config_toggles() {
        let mut cfg = WorkspaceLayoutConfig::new();
        assert!(cfg.sidebar_visible);
        cfg.toggle_sidebar();
        assert!(!cfg.sidebar_visible);
        cfg.toggle_sidebar();
        assert!(cfg.sidebar_visible);

        cfg.toggle_minimap();
        assert!(!cfg.minimap_enabled);

        cfg.set_tab_size(2);
        assert_eq!(cfg.editor_tab_size, 2);
        cfg.set_tab_size(0); // invalid, should not change
        assert_eq!(cfg.editor_tab_size, 2);
        cfg.set_tab_size(20); // too large, should not change
        assert_eq!(cfg.editor_tab_size, 2);
    }

    #[test]
    fn workspace_snapshot_diff() {
        let snap1 = WorkspaceSnapshot {
            folder_paths: vec!["/a".into(), "/b".into()],
            trust: WorkspaceTrust::Trusted,
            file_count: 10,
            tags: vec![],
            timestamp_ms: 1000,
        };
        let snap2 = WorkspaceSnapshot {
            folder_paths: vec!["/a".into(), "/c".into()],
            trust: WorkspaceTrust::Untrusted,
            file_count: 15,
            tags: vec![],
            timestamp_ms: 2000,
        };
        let diff = snap1.diff(&snap2);
        assert!(diff.has_changes());
        assert_eq!(diff.folders_added, vec!["/c".to_string()]);
        assert_eq!(diff.folders_removed, vec!["/b".to_string()]);
        assert_eq!(diff.file_count_delta, 5);
        assert!(diff.trust_changed);
    }

    #[test]
    fn workspace_snapshot_no_diff() {
        let snap = WorkspaceSnapshot {
            folder_paths: vec!["/a".into()],
            trust: WorkspaceTrust::Trusted,
            file_count: 5,
            tags: vec![],
            timestamp_ms: 1000,
        };
        let diff = snap.diff(&snap);
        assert!(!diff.has_changes());
    }

    #[test]
    fn workspace_tags_operations() {
        let mut tags = WorkspaceTags::new();
        assert!(tags.is_empty());
        assert!(tags.add("rust"));
        assert!(tags.add("editor"));
        assert!(!tags.add("rust")); // duplicate
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("rust"));

        let search = tags.search("ru");
        assert_eq!(search.len(), 1);
        assert_eq!(search[0], "rust");

        assert!(tags.remove("rust"));
        assert!(!tags.contains("rust"));
        assert_eq!(tags.len(), 1);
    }

    // -- New impl-block tests --

    #[test]
    fn workspace_folder_is_local() {
        let f = WorkspaceFolder::new(VsUri::file("/home/user/project"), "project", 0);
        assert!(f.is_local());

        let f2 = WorkspaceFolder::new(
            VsUri::from_components("untitled", "", "/tmp/file", "", ""),
            "tmp",
            0,
        );
        assert!(!f2.is_local());
    }

    #[test]
    fn workspace_folder_contains_path() {
        let f = WorkspaceFolder::new(VsUri::file("/home/user/project"), "project", 0);
        assert!(f.contains_path(Path::new("/home/user/project/src/main.rs")));
        assert!(!f.contains_path(Path::new("/home/other/file.rs")));
    }

    #[test]
    fn workspace_folder_fs_path() {
        let f = WorkspaceFolder::new(VsUri::file("/home/user/project"), "project", 0);
        assert_eq!(f.fs_path(), PathBuf::from("/home/user/project"));
    }

    #[test]
    fn workspace_folder_display_name() {
        let f = WorkspaceFolder::new(VsUri::file("/home/user/project"), "My Project", 0);
        assert_eq!(f.display_name(), "My Project");
    }

    #[test]
    fn workspace_type_predicates() {
        assert!(!WorkspaceType::Empty.has_folders());
        assert!(WorkspaceType::SingleFolder(PathBuf::from("/tmp")).has_folders());
        assert!(WorkspaceType::MultiRoot(PathBuf::from("/tmp")).has_folders());

        assert!(!WorkspaceType::Empty.is_multi_root());
        assert!(!WorkspaceType::SingleFolder(PathBuf::from("/tmp")).is_multi_root());
        assert!(WorkspaceType::MultiRoot(PathBuf::from("/tmp")).is_multi_root());

        assert!(WorkspaceType::SingleFolder(PathBuf::from("/tmp")).is_single_folder());
        assert!(!WorkspaceType::MultiRoot(PathBuf::from("/tmp")).is_single_folder());
    }

    #[test]
    fn workspace_type_path() {
        assert!(WorkspaceType::Empty.path().is_none());
        assert_eq!(
            WorkspaceType::SingleFolder(PathBuf::from("/a")).path(),
            Some(Path::new("/a"))
        );
        assert_eq!(
            WorkspaceType::MultiRoot(PathBuf::from("/b")).path(),
            Some(Path::new("/b"))
        );
    }

    #[test]
    fn workspace_type_label() {
        assert_eq!(WorkspaceType::Empty.label(), "Empty Workspace");
        assert_eq!(
            WorkspaceType::SingleFolder(PathBuf::from("/a")).label(),
            "Folder"
        );
        assert_eq!(
            WorkspaceType::MultiRoot(PathBuf::from("/a")).label(),
            "Workspace"
        );
    }

    #[test]
    fn workspace_trust_is_decided() {
        assert!(!WorkspaceTrust::Unknown.is_decided());
        assert!(WorkspaceTrust::Trusted.is_decided());
        assert!(WorkspaceTrust::Untrusted.is_decided());
    }

    #[test]
    fn workspace_trust_is_untrusted() {
        assert!(WorkspaceTrust::Untrusted.is_untrusted());
        assert!(!WorkspaceTrust::Trusted.is_untrusted());
        assert!(!WorkspaceTrust::Unknown.is_untrusted());
    }

    #[test]
    fn workspace_trust_merge() {
        // Untrusted always wins
        assert_eq!(
            WorkspaceTrust::Trusted.merge(WorkspaceTrust::Untrusted),
            WorkspaceTrust::Untrusted
        );
        assert_eq!(
            WorkspaceTrust::Untrusted.merge(WorkspaceTrust::Trusted),
            WorkspaceTrust::Untrusted
        );
        // Unknown beats Trusted
        assert_eq!(
            WorkspaceTrust::Trusted.merge(WorkspaceTrust::Unknown),
            WorkspaceTrust::Unknown
        );
        // Both trusted
        assert_eq!(
            WorkspaceTrust::Trusted.merge(WorkspaceTrust::Trusted),
            WorkspaceTrust::Trusted
        );
    }

    #[test]
    fn workspace_file_with_folders() {
        let wf = WorkspaceFile::with_folders(vec![
            FolderEntry::from_path("/a"),
            FolderEntry::from_path("/b"),
        ]);
        assert_eq!(wf.folder_count(), 2);
        assert!(!wf.has_settings());
        assert_eq!(wf.folder_paths(), vec!["/a", "/b"]);
    }

    #[test]
    fn workspace_file_has_settings() {
        let mut wf = WorkspaceFile::with_folders(vec![]);
        assert!(!wf.has_settings());
        wf.settings = serde_json::json!({});
        assert!(!wf.has_settings()); // empty object
        wf.settings = serde_json::json!({"a": 1});
        assert!(wf.has_settings());
    }

    #[test]
    fn folder_entry_from_path() {
        let e = FolderEntry::from_path("/home/user/src");
        assert_eq!(e.path, "/home/user/src");
        assert!(e.name.is_none());
    }

    #[test]
    fn folder_entry_with_name() {
        let e = FolderEntry::with_name("/home/user/src", "Source");
        assert_eq!(e.path, "/home/user/src");
        assert_eq!(e.name.as_deref(), Some("Source"));
    }

    #[test]
    fn folder_entry_display_name() {
        let e1 = FolderEntry::from_path("/home/user/src");
        assert_eq!(e1.display_name(), "src");

        let e2 = FolderEntry::with_name("/home/user/src", "Source");
        assert_eq!(e2.display_name(), "Source");
    }

    #[test]
    fn workspace_folder_count_and_is_empty() {
        let ws = Workspace::empty();
        assert!(ws.is_empty());
        assert_eq!(ws.folder_count(), 0);

        let ws2 = Workspace::single_folder(VsUri::file("/a"));
        assert!(!ws2.is_empty());
        assert_eq!(ws2.folder_count(), 1);
    }

    #[test]
    fn workspace_get_folder_by_index() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/a"), Some("Alpha".into()));
        ws.add_folder(VsUri::file("/b"), Some("Beta".into()));

        assert_eq!(ws.get_folder_by_index(0).unwrap().name, "Alpha");
        assert_eq!(ws.get_folder_by_index(1).unwrap().name, "Beta");
        assert!(ws.get_folder_by_index(2).is_none());
    }

    #[test]
    fn workspace_get_folder_by_name() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/a"), Some("Alpha".into()));
        ws.add_folder(VsUri::file("/b"), Some("Beta".into()));

        assert!(ws.get_folder_by_name("Alpha").is_some());
        assert!(ws.get_folder_by_name("Gamma").is_none());
    }

    #[test]
    fn workspace_folder_names_and_uris() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/a"), Some("A".into()));
        ws.add_folder(VsUri::file("/b"), Some("B".into()));

        assert_eq!(ws.folder_names(), vec!["A", "B"]);
        assert_eq!(ws.folder_uris().len(), 2);
    }

    #[test]
    fn workspace_distrust() {
        let mut ws = Workspace::empty();
        ws.distrust_workspace();
        assert_eq!(ws.trust(), WorkspaceTrust::Untrusted);
        assert!(ws.trust().is_untrusted());
    }

    #[test]
    fn workspace_set_configuration() {
        let mut ws = Workspace::empty();
        ws.set_configuration(serde_json::json!({"editor.fontSize": 16}));
        assert_eq!(ws.configuration()["editor.fontSize"], 16);
    }

    #[test]
    fn file_index_extensions() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("main.rs");
        idx.add_file("lib.rs");
        idx.add_file("README.md");
        idx.add_file("Cargo.toml");
        let exts = idx.extensions();
        assert_eq!(exts, vec!["md", "rs", "toml"]);
    }

    #[test]
    fn file_index_search_by_name() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("src/main.rs");
        idx.add_file("src/lib.rs");
        idx.add_file("tests/main_test.rs");
        let results = idx.search_by_name("main");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"src/main.rs"));
        assert!(results.contains(&"tests/main_test.rs"));
    }

    #[test]
    fn file_index_group_by_directory() {
        let mut idx = WorkspaceFileIndex::new("/project");
        idx.add_file("src/main.rs");
        idx.add_file("src/lib.rs");
        idx.add_file("tests/test1.rs");
        idx.add_file("README.md");
        let groups = idx.group_by_directory();
        assert_eq!(groups.get("src").unwrap().len(), 2);
        assert_eq!(groups.get("tests").unwrap().len(), 1);
        assert_eq!(groups.get(".").unwrap().len(), 1);
    }

    #[test]
    fn search_index_combined_search() {
        let mut idx = WorkspaceSearchIndex::new();
        idx.add("/src/app.rs", Some("fn run_app() {}"));
        idx.add("/src/config.rs", Some("fn load_config() {}"));
        idx.add("/README.md", Some("This is an app readme"));

        // Matches name "app"
        let results = idx.search("app");
        assert_eq!(results.len(), 2); // app.rs by name, README by content
        assert!(results.iter().any(|e| e.path == "/src/app.rs"));
        assert!(results.iter().any(|e| e.path == "/README.md"));
    }

    #[test]
    fn search_index_entries() {
        let mut idx = WorkspaceSearchIndex::new();
        idx.add("/a.rs", None);
        idx.add("/b.rs", None);
        assert_eq!(idx.entries().len(), 2);
        assert_eq!(idx.entries()[0].path, "/a.rs");
    }

    #[test]
    fn workspace_snapshot_tags() {
        let mut snap = WorkspaceSnapshot {
            folder_paths: vec!["/a".into()],
            trust: WorkspaceTrust::Trusted,
            file_count: 5,
            tags: vec![],
            timestamp_ms: 1000,
        };
        assert!(!snap.has_tag("release"));
        snap.add_tag("release");
        assert!(snap.has_tag("release"));
        assert_eq!(snap.folder_count(), 1);
        assert!(!snap.is_empty());
    }

    #[test]
    fn workspace_snapshot_empty() {
        let snap = WorkspaceSnapshot {
            folder_paths: vec![],
            trust: WorkspaceTrust::Unknown,
            file_count: 0,
            tags: vec![],
            timestamp_ms: 0,
        };
        assert!(snap.is_empty());
        assert_eq!(snap.folder_count(), 0);
    }

    #[test]
    fn recent_workspaces_remove_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("recents.json");
        let mut store = RecentWorkspaces::new(state_path);
        store.add_recent(RecentWorkspace {
            path: "/a".into(),
            label: "A".into(),
            last_opened: 100,
        });
        store.add_recent(RecentWorkspace {
            path: "/b".into(),
            label: "B".into(),
            last_opened: 200,
        });
        assert!(store.remove_by_path("/a"));
        assert_eq!(store.len(), 1);
        assert!(!store.remove_by_path("/nonexistent"));
    }

    #[test]
    fn recent_workspaces_search() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("recents.json");
        let mut store = RecentWorkspaces::new(state_path);
        store.add_recent(RecentWorkspace {
            path: "/a".into(),
            label: "My Project".into(),
            last_opened: 100,
        });
        store.add_recent(RecentWorkspace {
            path: "/b".into(),
            label: "Other Work".into(),
            last_opened: 200,
        });
        let results = store.search("project");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "My Project");
    }

    #[test]
    fn recent_workspaces_len_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("recents.json");
        let store = RecentWorkspaces::new(state_path);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn workspaceSettingsMerger_new() {
        let s = WorkspaceSettingsMerger::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn workspaceSettingsMerger_add_contains() {
        let mut s = WorkspaceSettingsMerger::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn workspaceSettingsMerger_add_duplicate() {
        let mut s = WorkspaceSettingsMerger::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn workspaceSettingsMerger_remove() {
        let mut s = WorkspaceSettingsMerger::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn workspaceSettingsMerger_capacity() {
        let s = WorkspaceSettingsMerger::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn workspaceSettingsMerger_search() {
        let mut s = WorkspaceSettingsMerger::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn workspaceSettingsMerger_stats() {
        let mut s = WorkspaceSettingsMerger::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn workspaceFolderWatcher_new() {
        let m = WorkspaceFolderWatcher::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn workspaceFolderWatcher_add_find() {
        let mut m = WorkspaceFolderWatcher::new();
        m.add(WorkspaceFolderWatcherItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn workspaceFolderWatcher_priority_filter() {
        let mut m = WorkspaceFolderWatcher::new();
        m.add(WorkspaceFolderWatcherItem::new("a", "A").with_priority(WorkspaceFolderWatcherPriority::High));
        m.add(WorkspaceFolderWatcherItem::new("b", "B").with_priority(WorkspaceFolderWatcherPriority::Low));
        m.add(WorkspaceFolderWatcherItem::new("c", "C").with_priority(WorkspaceFolderWatcherPriority::High));
        assert_eq!(m.by_priority(WorkspaceFolderWatcherPriority::High).len(), 2);
    }

    #[test]
    fn workspaceFolderWatcher_remove() {
        let mut m = WorkspaceFolderWatcher::new();
        m.add(WorkspaceFolderWatcherItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn workspaceFolderWatcher_search() {
        let mut m = WorkspaceFolderWatcher::new();
        m.add(WorkspaceFolderWatcherItem::new("id1", "Hello World"));
        m.add(WorkspaceFolderWatcherItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn workspaceFolderWatcher_total_weight() {
        let mut m = WorkspaceFolderWatcher::new();
        m.add(WorkspaceFolderWatcherItem::new("a", "A").with_priority(WorkspaceFolderWatcherPriority::Critical));
        m.add(WorkspaceFolderWatcherItem::new("b", "B").with_priority(WorkspaceFolderWatcherPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn workspaceFolderWatcher_capacity_limit() {
        let mut m = WorkspaceFolderWatcher::new().with_max_items(2);
        m.add(WorkspaceFolderWatcherItem::new("1", "one"));
        m.add(WorkspaceFolderWatcherItem::new("2", "two"));
        assert!(!m.add(WorkspaceFolderWatcherItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn workspaceFolderWatcher_sorted_by_priority() {
        let mut m = WorkspaceFolderWatcher::new();
        m.add(WorkspaceFolderWatcherItem::new("lo", "Low").with_priority(WorkspaceFolderWatcherPriority::Low));
        m.add(WorkspaceFolderWatcherItem::new("hi", "High").with_priority(WorkspaceFolderWatcherPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn workspaceFolderWatcher_item_metadata() {
        let mut item = WorkspaceFolderWatcherItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn workspaceSettingsMerger_enabled_toggle() {
        let mut s = WorkspaceSettingsMerger::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn workspaceFolderWatcher_priority_display() {
        assert_eq!(format!("{}", WorkspaceFolderWatcherPriority::High), "high");
        assert_eq!(format!("{}", WorkspaceFolderWatcherPriority::Low), "low");
    }


    #[test]
    fn workspace_entry_creation() {
        let e = WorkspaceEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn workspace_entry_with_priority() {
        let e = WorkspaceEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn workspace_entry_metadata() {
        let e = WorkspaceEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn workspace_entry_remove_meta() {
        let mut e = WorkspaceEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn workspace_entry_activate_deactivate() {
        let mut e = WorkspaceEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn workspace_config_add_sorted() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("lo", "Lo").with_priority(1));
        c.add(WorkspaceEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn workspace_config_capacity() {
        let mut c = WorkspaceConfig::new(1);
        assert!(c.add(WorkspaceEntry::new("a", "A")));
        assert!(!c.add(WorkspaceEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn workspace_config_remove() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn workspace_config_get() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn workspace_config_active_entries() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "A"));
        c.add(WorkspaceEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn workspace_config_enable_disable() {
        let mut c = WorkspaceConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn workspace_config_clear() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn workspace_config_find_by_label() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn workspace_config_top_n() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "A").with_priority(1));
        c.add(WorkspaceEntry::new("b", "B").with_priority(2));
        c.add(WorkspaceEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn workspace_config_deactivate_activate_all() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "A"));
        c.add(WorkspaceEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn workspace_config_highest_priority() {
        let mut c = WorkspaceConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WorkspaceEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn workspace_config_contains() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn workspace_config_labels() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "Alpha"));
        c.add(WorkspaceEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn workspace_config_drain_inactive() {
        let mut c = WorkspaceConfig::new(10);
        c.add(WorkspaceEntry::new("a", "A"));
        c.add(WorkspaceEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for workspace
    #[test]
    fn xa_workspace_ring_new() {
        let rb = super::XaWorkspaceRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_workspace_ring_push_len() {
        let mut rb = super::XaWorkspaceRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_workspace_ring_wrap() {
        let mut rb = super::XaWorkspaceRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_workspace_ring_mean_empty() {
        let rb = super::XaWorkspaceRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_workspace_ring_mean_values() {
        let mut rb = super::XaWorkspaceRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_workspace_ring_min_max() {
        let mut rb = super::XaWorkspaceRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_workspace_ring_iter() {
        let mut rb = super::XaWorkspaceRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_workspace_counter_new() {
        let c = super::XaWorkspaceCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_workspace_counter_inc() {
        let mut c = super::XaWorkspaceCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_workspace_counter_inc_by() {
        let mut c = super::XaWorkspaceCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_workspace_counter_reset() {
        let mut c = super::XaWorkspaceCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_workspace_counter_clear() {
        let mut c = super::XaWorkspaceCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_workspace_counter_default() {
        let c = super::XaWorkspaceCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 240 ----

    #[test]
    fn xc_240_pool_new_empty() {
        let pool: super::Xc240Pool<i32> = super::Xc240Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_240_pool_release_acquire() {
        let mut pool = super::Xc240Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_240_pool_acquire_empty() {
        let mut pool: super::Xc240Pool<i32> = super::Xc240Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_240_pool_full() {
        let mut pool = super::Xc240Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_240_pool_drain() {
        let mut pool = super::Xc240Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_240_pool_stats() {
        let mut pool = super::Xc240Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_240_pool_clear() {
        let mut pool = super::Xc240Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_240_pool_shrink() {
        let mut pool = super::Xc240Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_240_pool_default() {
        let pool: super::Xc240Pool<String> = super::Xc240Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_240_pool_extend() {
        let mut pool = super::Xc240Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_240_pool_retain() {
        let mut pool = super::Xc240Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_240_scheduler_round_robin() {
        let mut sched = super::Xc240Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_240_scheduler_empty() {
        let mut sched = super::Xc240Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_240_scheduler_reset() {
        let mut sched = super::Xc240Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_240_scheduler_add_remove() {
        let mut sched = super::Xc240Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_240_scheduler_targets() {
        let sched = super::Xc240Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_240_hash_empty() {
        assert_eq!(super::xc_240_hash(b""), 5381);
    }

    #[test]
    fn xc_240_hash_data() {
        let h = super::xc_240_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_240_hash(b"hello"), h);
    }

    #[test]
    fn xc_240_reverse_str() {
        assert_eq!(super::xc_240_reverse("abc"), "cba");
        assert_eq!(super::xc_240_reverse(""), "");
    }


    // --- xd_47 deepening tests ---

    #[test]
    fn xd_47_sm_initial_state() {
        let sm = Xd47StateMachine::new();
        assert_eq!(sm.current_state(), Xd47State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_47_sm_valid_idle_to_running() {
        let mut sm = Xd47StateMachine::new();
        assert!(sm.transition(Xd47State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd47State::Running);
    }

    #[test]
    fn xd_47_sm_valid_running_to_paused() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        assert!(sm.transition(Xd47State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd47State::Paused);
    }

    #[test]
    fn xd_47_sm_valid_running_to_done() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        assert!(sm.transition(Xd47State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd47State::Done);
    }

    #[test]
    fn xd_47_sm_valid_paused_to_running() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        sm.transition(Xd47State::Paused).unwrap();
        assert!(sm.transition(Xd47State::Running).is_ok());
    }

    #[test]
    fn xd_47_sm_valid_done_to_idle() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        sm.transition(Xd47State::Done).unwrap();
        assert!(sm.transition(Xd47State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd47State::Idle);
    }

    #[test]
    fn xd_47_sm_invalid_idle_to_done() {
        let mut sm = Xd47StateMachine::new();
        assert!(sm.transition(Xd47State::Done).is_err());
    }

    #[test]
    fn xd_47_sm_invalid_idle_to_paused() {
        let mut sm = Xd47StateMachine::new();
        assert!(sm.transition(Xd47State::Paused).is_err());
    }

    #[test]
    fn xd_47_sm_history_tracking() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        sm.transition(Xd47State::Paused).unwrap();
        sm.transition(Xd47State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd47State::Idle);
        assert_eq!(sm.history()[0].to, Xd47State::Running);
        assert_eq!(sm.history()[1].from, Xd47State::Running);
        assert_eq!(sm.history()[2].to, Xd47State::Done);
    }

    #[test]
    fn xd_47_sm_serialize_deserialize() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd47StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd47State::Running));
    }

    #[test]
    fn xd_47_sm_deserialize_invalid() {
        assert_eq!(Xd47StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_47_sm_reset() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd47State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_47_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd47EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd47Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_47_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd47EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd47Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd47Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_47_bus_unsubscribe() {
        let mut bus = Xd47EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_47_event_kind_and_payload() {
        let e = Xd47Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd47Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_47_bus_clear_history() {
        let mut bus = Xd47EventBus::new();
        bus.publish(Xd47Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_47_sm_step_counter_increments() {
        let mut sm = Xd47StateMachine::new();
        sm.transition(Xd47State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd47State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #45 --

    #[test]
    fn xf45_trie_insert_search() {
        let mut t = Xf45Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf45_trie_starts_with() {
        let mut t = Xf45Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf45_trie_remove() {
        let mut t = Xf45Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf45_trie_word_count() {
        let mut t = Xf45Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf45_trie_longest_prefix() {
        let mut t = Xf45Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf45_trie_all_words() {
        let mut t = Xf45Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf45_trie_autocomplete() {
        let mut t = Xf45Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf45_trie_empty_search() {
        let t = Xf45Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf45_bloom_add_contains() {
        let mut bf = Xf45BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf45_bloom_probably_absent() {
        let bf = Xf45BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf45_bloom_false_positive_rate() {
        let mut bf = Xf45BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf45_bloom_clear() {
        let mut bf = Xf45BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf45_bloom_union() {
        let mut a = Xf45BloomFilter::xf_new(512, 2);
        let mut b = Xf45BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf45_bloom_intersection_estimate() {
        let mut a = Xf45BloomFilter::xf_new(512, 2);
        let mut b = Xf45BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf45_bloom_union_size_mismatch() {
        let a = Xf45BloomFilter::xf_new(256, 2);
        let b = Xf45BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh239_skip_insert_contains() {
        let mut sl = super::Xh239SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh239_skip_remove() {
        let mut sl = super::Xh239SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh239_skip_len() {
        let mut sl = super::Xh239SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh239_skip_range_query() {
        let mut sl = super::Xh239SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh239_skip_floor_ceiling() {
        let mut sl = super::Xh239SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh239_skip_rank() {
        let mut sl = super::Xh239SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh239_skip_empty() {
        let sl = super::Xh239SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh239_skip_duplicates() {
        let mut sl = super::Xh239SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh239_bitset_set_test() {
        let mut bs = super::Xh239BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh239_bitset_clear_count() {
        let mut bs = super::Xh239BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh239_bitset_and_or_xor() {
        let mut a = super::Xh239BitSet::xh_new(128);
        let mut b = super::Xh239BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh239_bitset_iter_ones() {
        let mut bs = super::Xh239BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh239_bitset_first_last() {
        let mut bs = super::Xh239BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh239_bitset_empty() {
        let bs = super::Xh239BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi239_deque_push_pop_back() {
        let mut dq = super::Xi239Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi239_deque_push_pop_front() {
        let mut dq = super::Xi239Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi239_deque_mixed_ops() {
        let mut dq = super::Xi239Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi239_deque_get_and_split() {
        let mut dq = super::Xi239Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi239_deque_rotate_left() {
        let mut dq = super::Xi239Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi239_deque_rotate_right() {
        let mut dq = super::Xi239Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi239_deque_grow() {
        let mut dq = super::Xi239Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi239_deque_empty() {
        let dq = super::Xi239Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi239_interval_tree_insert_query() {
        let mut tree = super::Xi239IntervalTree::xi_new();
        tree.xi_insert(super::Xi239Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi239Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi239Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi239_interval_tree_overlap() {
        let mut tree = super::Xi239IntervalTree::xi_new();
        tree.xi_insert(super::Xi239Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi239Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi239Interval::xi_new(12, 20));
        let q = super::Xi239Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi239_interval_tree_remove() {
        let mut tree = super::Xi239IntervalTree::xi_new();
        tree.xi_insert(super::Xi239Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi239Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi239_interval_tree_gaps() {
        let mut tree = super::Xi239IntervalTree::xi_new();
        tree.xi_insert(super::Xi239Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi239Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi239Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi239Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi239Interval::xi_new(8, 10));
    }

    #[test]
    fn xi239_interval_tree_merge() {
        let mut tree = super::Xi239IntervalTree::xi_new();
        tree.xi_insert(super::Xi239Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi239Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi239Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi239Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi239Interval::xi_new(10, 15));
    }

    #[test]
    fn xi239_interval_tree_all() {
        let mut tree = super::Xi239IntervalTree::xi_new();
        tree.xi_insert(super::Xi239Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi239Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi239_interval_tree_empty() {
        let tree = super::Xi239IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi239_interval_tree_contains_point() {
        let iv = super::Xi239Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 239) ---

    #[test]
    fn xj_239_uf_make_and_find() {
        let mut uf = super::Xj239UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_239_uf_union_connected() {
        let mut uf = super::Xj239UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_239_uf_component_count() {
        let mut uf = super::Xj239UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_239_uf_component_size() {
        let mut uf = super::Xj239UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_239_uf_largest_component() {
        let mut uf = super::Xj239UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_239_uf_many_elements() {
        let mut uf = super::Xj239UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_239_uf_separate_components() {
        let mut uf = super::Xj239UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_239_uf_path_compression() {
        let mut uf = super::Xj239UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_239_bt_insert_get() {
        let mut bt = super::Xj239BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_239_bt_contains_len() {
        let mut bt = super::Xj239BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_239_bt_replace() {
        let mut bt = super::Xj239BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_239_bt_remove() {
        let mut bt = super::Xj239BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_239_bt_keys_values() {
        let mut bt = super::Xj239BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_239_bt_range() {
        let mut bt = super::Xj239BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_239_bt_min_max() {
        let mut bt = super::Xj239BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_239_bt_many_inserts() {
        let mut bt = super::Xj239BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_239 segment tree tests ---

    #[test]
    fn xk_239_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk239SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_239_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk239SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_239_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk239SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_239_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk239SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_239_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk239SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_239_st_single_element() {
        let data = vec![42];
        let st = super::Xk239SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_239_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk239SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_239_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk239SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_239 disjoint intervals tests ---

    #[test]
    fn xk_239_di_add_and_count() {
        let mut di = super::Xk239DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_239_di_merge_overlap() {
        let mut di = super::Xk239DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_239_di_contains() {
        let mut di = super::Xk239DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_239_di_remove() {
        let mut di = super::Xk239DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_239_di_covered_length() {
        let mut di = super::Xk239DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_239_di_gaps() {
        let mut di = super::Xk239DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_239_di_merge_adjacent() {
        let mut di = super::Xk239DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_239_di_empty() {
        let di = super::Xk239DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_239_rope_new_empty() {
        let rope = super::Xl239Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_239_rope_from_str() {
        let rope = super::Xl239Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_239_rope_insert_at() {
        let mut rope = super::Xl239Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_239_rope_delete_range() {
        let mut rope = super::Xl239Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_239_rope_char_at() {
        let rope = super::Xl239Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_239_rope_split_concat() {
        let rope = super::Xl239Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_239_rope_line_count() {
        let rope = super::Xl239Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_239_rope_line_at() {
        let rope = super::Xl239Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_239_sa_build_and_search() {
        let sa = super::Xl239SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_239_sa_count() {
        let sa = super::Xl239SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_239_sa_longest_repeated() {
        let sa = super::Xl239SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_239_sa_all_positions() {
        let sa = super::Xl239SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_239_sa_len() {
        let sa = super::Xl239SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_239_sa_empty() {
        let sa = super::Xl239SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_239_rope_slice() {
        let rope = super::Xl239Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_239_sa_search_start() {
        let sa = super::Xl239SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_239_sparse_set_get() {
        let mut m = super::Xm239MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_239_sparse_row_col() {
        let mut m = super::Xm239MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_239_sparse_transpose() {
        let mut m = super::Xm239MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_239_sparse_multiply_vec() {
        let mut m = super::Xm239MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_239_sparse_nnz_density() {
        let mut m = super::Xm239MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_239_sparse_clear() {
        let mut m = super::Xm239MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_239_sparse_overwrite_zero() {
        let mut m = super::Xm239MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_239_tokenizer_basic() {
        let t = super::Xm239Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_239_tokenizer_count() {
        let t = super::Xm239Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_239_tokenizer_unique() {
        let t = super::Xm239Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_239_tokenizer_frequency() {
        let t = super::Xm239Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_239_tokenizer_delimiter() {
        let t = super::Xm239Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_239_tokenizer_whitespace() {
        let t = super::Xm239Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_239_tokenizer_empty() {
        let t = super::Xm239Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 239 ----

    #[test]
    fn xn_239_fenwick_prefix_sum() {
        let mut ft = super::Xn239Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_239_fenwick_range_sum() {
        let mut ft = super::Xn239Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_239_fenwick_point_query() {
        let mut ft = super::Xn239Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_239_fenwick_len() {
        let ft = super::Xn239Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_239_fenwick_multiple_updates() {
        let mut ft = super::Xn239Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_239_fenwick_single_element() {
        let mut ft = super::Xn239Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_239_fenwick_find_kth() {
        let mut ft = super::Xn239Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_239_fenwick_negative_delta() {
        let mut ft = super::Xn239Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 239 ----

    #[test]
    fn xn_239_avl_insert_get() {
        let mut m = super::Xn239AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_239_avl_remove() {
        let mut m = super::Xn239AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_239_avl_in_order() {
        let mut m = super::Xn239AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_239_avl_min_max() {
        let mut m = super::Xn239AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_239_avl_floor_ceiling() {
        let mut m = super::Xn239AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_239_avl_height_balanced() {
        let mut m = super::Xn239AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_239_avl_overwrite() {
        let mut m = super::Xn239AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_239_avl_empty() {
        let m: super::Xn239AVL<i32, i32> = super::Xn239AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo239RedBlack tests ---

    #[test]
    fn xo_239_rb_insert_and_get() {
        let mut tree = super::Xo239RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_239_rb_len_and_empty() {
        let mut tree = super::Xo239RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_239_rb_min_max() {
        let mut tree = super::Xo239RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_239_rb_contains() {
        let mut tree = super::Xo239RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_239_rb_remove() {
        let mut tree = super::Xo239RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_239_rb_in_order() {
        let mut tree = super::Xo239RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_239_rb_black_height() {
        let mut tree = super::Xo239RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_239_rb_overwrite() {
        let mut tree = super::Xo239RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo239ConsistentHash tests ---

    #[test]
    fn xo_239_ch_add_and_count() {
        let mut ring = super::Xo239ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_239_ch_remove_node() {
        let mut ring = super::Xo239ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_239_ch_get_node() {
        let mut ring = super::Xo239ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_239_ch_empty_ring() {
        let ring = super::Xo239ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_239_ch_distribution() {
        let mut ring = super::Xo239ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_239_ch_rebalance() {
        let mut ring = super::Xo239ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_239_ch_virtual_nodes() {
        let mut ring = super::Xo239ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_239_ch_consistent_lookup() {
        let mut ring = super::Xo239ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
