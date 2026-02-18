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

}
