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
}
