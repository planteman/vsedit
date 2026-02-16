//! Workspace folders, configuration, and trust management.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during workspace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceError {
    /// A folder with the given URI already exists.
    DuplicateFolder(String),
    /// The requested folder index is out of range.
    FolderIndexOutOfRange(u32),
    /// A required setting key was empty.
    EmptySettingKey,
    /// The workspace name failed validation.
    InvalidName(String),
    /// The folder URI is empty or invalid.
    InvalidUri(String),
    /// A provided index is invalid for the operation.
    InvalidIndex(u32),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::DuplicateFolder(uri) => write!(f, "folder already exists: {}", uri),
            WorkspaceError::FolderIndexOutOfRange(idx) => {
                write!(f, "folder index out of range: {}", idx)
            }
            WorkspaceError::EmptySettingKey => write!(f, "setting key must not be empty"),
            WorkspaceError::InvalidName(reason) => {
                write!(f, "invalid workspace name: {}", reason)
            }
            WorkspaceError::InvalidUri(uri) => write!(f, "invalid folder URI: {}", uri),
            WorkspaceError::InvalidIndex(idx) => write!(f, "invalid index: {}", idx),
        }
    }
}

impl std::error::Error for WorkspaceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
    pub index: u32,
}

impl WorkspaceFolder {
    /// Returns the display name of this folder.
    pub fn display_name(&self) -> &str {
        &self.name
    }
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

impl Clone for WorkspaceConfiguration {
    fn clone(&self) -> Self {
        Self {
            folders: self.folders.clone(),
            settings: self.settings.clone(),
            name: self.name.clone(),
        }
    }
}

impl PartialEq for WorkspaceConfiguration {
    fn eq(&self, other: &Self) -> bool {
        self.folders == other.folders
            && self.settings == other.settings
            && self.name == other.name
    }
}

impl fmt::Debug for WorkspaceConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceConfiguration")
            .field("name", &self.name)
            .field("folders", &self.folders.len())
            .field("settings", &self.settings.len())
            .finish()
    }
}

impl fmt::Display for WorkspaceConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = self.name.as_deref().unwrap_or("<unnamed>");
        write!(
            f,
            "Workspace '{}' ({} folder(s), {} setting(s))",
            label,
            self.folders.len(),
            self.settings.len()
        )
    }
}

impl WorkspaceConfiguration {
    /// Try to add a folder, returning an error if the URI is empty or already present.
    pub fn try_add_folder(
        &mut self,
        uri: String,
        name: String,
    ) -> Result<&WorkspaceFolder, WorkspaceError> {
        if uri.is_empty() {
            return Err(WorkspaceError::InvalidUri(uri));
        }
        if self.contains_uri(&uri) {
            return Err(WorkspaceError::DuplicateFolder(uri));
        }
        self.add_folder(uri, name);
        Ok(self.folders.last().unwrap())
    }

    /// Try to remove a folder by index, returning an error if not found.
    pub fn try_remove_folder(&mut self, index: u32) -> Result<WorkspaceFolder, WorkspaceError> {
        let pos = self
            .folders
            .iter()
            .position(|f| f.index == index)
            .ok_or(WorkspaceError::FolderIndexOutOfRange(index))?;
        let removed = self.folders.remove(pos);
        for (i, folder) in self.folders.iter_mut().enumerate() {
            folder.index = i as u32;
        }
        Ok(removed)
    }

    /// Set a setting, returning an error if the key is empty.
    pub fn try_set_setting(
        &mut self,
        key: String,
        value: String,
    ) -> Result<Option<String>, WorkspaceError> {
        if key.is_empty() {
            return Err(WorkspaceError::EmptySettingKey);
        }
        Ok(self.settings.insert(key, value))
    }

    /// Set the workspace name with validation.
    pub fn set_name(&mut self, name: String) -> Result<(), WorkspaceError> {
        if name.trim().is_empty() {
            return Err(WorkspaceError::InvalidName(
                "name must not be blank".to_string(),
            ));
        }
        if name.len() > 255 {
            return Err(WorkspaceError::InvalidName(
                "name must be 255 characters or fewer".to_string(),
            ));
        }
        self.name = Some(name);
        Ok(())
    }

    /// Return all folder URIs as a vector of string slices.
    pub fn folder_uris(&self) -> Vec<&str> {
        self.folders.iter().map(|f| f.uri.as_str()).collect()
    }

    /// Merge settings from another configuration, overwriting on conflict.
    pub fn merge_settings(&mut self, other: &WorkspaceConfiguration) {
        for (k, v) in &other.settings {
            self.settings.insert(k.clone(), v.clone());
        }
    }

    /// Clear all settings.
    pub fn clear_settings(&mut self) {
        self.settings.clear();
    }

    /// Check whether a setting with the given key exists.
    pub fn has_setting(&self, key: &str) -> bool {
        self.settings.contains_key(key)
    }

    /// Return all setting keys as a sorted vector of string slices.
    pub fn settings_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.settings.keys().map(|k| k.as_str()).collect();
        keys.sort();
        keys
    }

    /// Rename the folder at the given index, returning an error if the index is invalid.
    pub fn rename_folder(
        &mut self,
        index: u32,
        new_name: String,
    ) -> Result<(), WorkspaceError> {
        let folder = self
            .folders
            .iter_mut()
            .find(|f| f.index == index)
            .ok_or(WorkspaceError::InvalidIndex(index))?;
        folder.name = new_name;
        Ok(())
    }

    /// Get a setting value, or return the provided default if the key is absent.
    pub fn get_or_default(&self, key: &str, default: &str) -> String {
        self.settings
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }
}

/// Builder for creating a [`WorkspaceConfiguration`] step by step.
pub struct WorkspaceConfigurationBuilder {
    name: Option<String>,
    folders: Vec<(String, String)>,
    settings: Vec<(String, String)>,
}

impl WorkspaceConfigurationBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            folders: Vec::new(),
            settings: Vec::new(),
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn folder(mut self, uri: impl Into<String>, name: impl Into<String>) -> Self {
        self.folders.push((uri.into(), name.into()));
        self
    }

    pub fn setting(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.settings.push((key.into(), value.into()));
        self
    }

    pub fn build(self) -> Result<WorkspaceConfiguration, WorkspaceError> {
        let mut config = WorkspaceConfiguration::new();
        if let Some(name) = self.name {
            config.set_name(name)?;
        }
        for (uri, name) in self.folders {
            config.try_add_folder(uri, name)?;
        }
        for (key, value) in self.settings {
            config.try_set_setting(key, value)?;
        }
        Ok(config)
    }
}

impl Default for WorkspaceConfigurationBuilder {
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

impl Clone for WorkspaceTrustService {
    fn clone(&self) -> Self {
        Self {
            trust_state: self.trust_state,
            trusted_folders: self.trusted_folders.clone(),
        }
    }
}

impl PartialEq for WorkspaceTrustService {
    fn eq(&self, other: &Self) -> bool {
        self.trust_state == other.trust_state && self.trusted_folders == other.trusted_folders
    }
}

impl fmt::Debug for WorkspaceTrustService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceTrustService")
            .field("trust_state", &self.trust_state)
            .field("trusted_folders", &self.trusted_folders.len())
            .finish()
    }
}

impl fmt::Display for WorkspaceTrustService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TrustService({}, {} trusted folder(s))",
            self.trust_state,
            self.trusted_folders.len()
        )
    }
}

impl WorkspaceTrustService {
    /// Remove a folder from the trusted list. Returns `true` if it was present.
    pub fn remove_trusted_folder(&mut self, uri: &str) -> bool {
        let len = self.trusted_folders.len();
        self.trusted_folders.retain(|f| f != uri);
        self.trusted_folders.len() < len
    }

    /// Number of explicitly trusted folders.
    pub fn trusted_folder_count(&self) -> usize {
        self.trusted_folders.len()
    }

    /// Check whether a workspace configuration is fully trusted (all folders trusted).
    pub fn is_workspace_fully_trusted(&self, config: &WorkspaceConfiguration) -> bool {
        config.folders.iter().all(|f| self.is_folder_trusted(&f.uri))
    }

    /// Return the list of untrusted folder URIs from a workspace configuration.
    pub fn untrusted_folders<'a>(&self, config: &'a WorkspaceConfiguration) -> Vec<&'a str> {
        config
            .folders
            .iter()
            .filter(|f| !self.is_folder_trusted(&f.uri))
            .map(|f| f.uri.as_str())
            .collect()
    }

    /// Reset to the default unknown state and clear all trusted folders.
    pub fn reset(&mut self) {
        self.trust_state = WorkspaceTrust::Unknown;
        self.trusted_folders.clear();
    }
}

/// Statistics about a workspace configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceStats {
    /// Total number of folders.
    pub folder_count: usize,
    /// Total number of settings.
    pub setting_count: usize,
    /// Number of unique setting prefixes (e.g. "editor" in "editor.tabSize").
    pub setting_prefix_count: usize,
    /// Whether the workspace has a name set.
    pub has_name: bool,
    /// Whether this is a multi-root workspace.
    pub is_multi_root: bool,
}

impl WorkspaceConfiguration {
    /// Compute summary statistics for this workspace.
    pub fn stats(&self) -> WorkspaceStats {
        let mut prefixes = std::collections::HashSet::new();
        for key in self.settings.keys() {
            if let Some(prefix) = key.split('.').next() {
                prefixes.insert(prefix.to_string());
            }
        }
        WorkspaceStats {
            folder_count: self.folders.len(),
            setting_count: self.settings.len(),
            setting_prefix_count: prefixes.len(),
            has_name: self.name.is_some(),
            is_multi_root: self.is_multi_root(),
        }
    }

    /// Search across all folder names and URIs for a substring match.
    pub fn search_folders(&self, query: &str) -> Vec<&WorkspaceFolder> {
        let q = query.to_lowercase();
        self.folders
            .iter()
            .filter(|f| f.name.to_lowercase().contains(&q) || f.uri.to_lowercase().contains(&q))
            .collect()
    }

    /// Serialize the workspace configuration to a simple summary string.
    pub fn serialize_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref name) = self.name {
            parts.push(format!("name={}", name));
        }
        parts.push(format!("folders={}", self.folders.len()));
        parts.push(format!("settings={}", self.settings.len()));
        for folder in &self.folders {
            parts.push(format!("folder[{}]={}", folder.index, folder.uri));
        }
        parts.join(";")
    }

    /// Deserialize a summary string back into basic workspace info.
    /// Returns (name, folder_count, setting_count).
    pub fn parse_summary(summary: &str) -> (Option<String>, usize, usize) {
        let mut name = None;
        let mut folders = 0usize;
        let mut settings = 0usize;
        for part in summary.split(';') {
            if let Some(val) = part.strip_prefix("name=") {
                name = Some(val.to_string());
            } else if let Some(val) = part.strip_prefix("folders=") {
                folders = val.parse().unwrap_or(0);
            } else if let Some(val) = part.strip_prefix("settings=") {
                settings = val.parse().unwrap_or(0);
            }
        }
        (name, folders, settings)
    }

    /// Reorder folders by moving the folder at `from` to position `to`.
    /// Returns an error if either index is out of range.
    pub fn reorder_folder(&mut self, from: usize, to: usize) -> Result<(), WorkspaceError> {
        if from >= self.folders.len() {
            return Err(WorkspaceError::FolderIndexOutOfRange(from as u32));
        }
        if to >= self.folders.len() {
            return Err(WorkspaceError::FolderIndexOutOfRange(to as u32));
        }
        let folder = self.folders.remove(from);
        self.folders.insert(to, folder);
        for (i, f) in self.folders.iter_mut().enumerate() {
            f.index = i as u32;
        }
        Ok(())
    }

    /// Return the folder with the highest index (last added).
    pub fn last_folder(&self) -> Option<&WorkspaceFolder> {
        self.folders.last()
    }

    /// Return folders sorted by name alphabetically.
    pub fn folders_sorted_by_name(&self) -> Vec<&WorkspaceFolder> {
        let mut sorted: Vec<&WorkspaceFolder> = self.folders.iter().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));
        sorted
    }
}

// ── Workspace History ──

/// Tracks recently opened workspaces with timestamps.
#[derive(Debug, Clone)]
pub struct WorkspaceHistory {
    /// Entries stored as `(path, timestamp)`, most recent last.
    entries: Vec<(String, u64)>,
    /// Maximum number of entries to retain.
    capacity: usize,
}

impl WorkspaceHistory {
    /// Create a new history with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Add (or update) a workspace path with the given timestamp.
    /// If the path already exists, its timestamp is updated and it moves to the end.
    pub fn add(&mut self, path: &str, timestamp: u64) {
        self.entries.retain(|(p, _)| p != path);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((path.to_string(), timestamp));
    }

    /// Return the most recent `n` entries, newest first.
    pub fn recent(&self, n: usize) -> Vec<(&str, u64)> {
        self.entries
            .iter()
            .rev()
            .take(n)
            .map(|(p, t)| (p.as_str(), *t))
            .collect()
    }

    /// Number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check whether a path is already tracked.
    pub fn contains(&self, path: &str) -> bool {
        self.entries.iter().any(|(p, _)| p == path)
    }

    /// Clear all history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Search for `.code-workspace` files recursively under the given directory.
pub fn find_workspace_files(root: &str) -> Vec<String> {
    let mut results = Vec::new();
    fn walk(dir: &std::path::Path, results: &mut Vec<String>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, results);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".code-workspace") {
                    results.push(path.to_string_lossy().to_string());
                }
            }
        }
    }
    walk(std::path::Path::new(root), &mut results);
    results
}

// ---------------------------------------------------------------------------
// Workspace path to URI conversion
// ---------------------------------------------------------------------------

/// Convert a workspace filesystem path to a file URI.
///
/// This performs simple percent-encoding of spaces and follows the `file://`
/// URI scheme convention. On Windows-style paths (starting with a drive letter)
/// a leading slash is prepended.
///
/// # Examples
/// ```
/// # use vsedit_workspaces::workspace_to_uri;
/// assert_eq!(workspace_to_uri("/home/user/project"), "file:///home/user/project");
/// ```
pub fn workspace_to_uri(path: &str) -> String {
    let encoded = path
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23")
        .replace('?', "%3F");

    // Windows-style absolute path: C:\foo  →  file:///C:/foo
    if encoded.len() >= 2 && encoded.as_bytes()[0].is_ascii_alphabetic() && encoded.as_bytes()[1] == b':' {
        let unix_style = encoded.replace('\\', "/");
        format!("file:///{unix_style}")
    } else {
        format!("file://{encoded}")
    }
}

/// Convert a file URI back to a workspace filesystem path.
///
/// Reverses the encoding done by [`workspace_to_uri`]. Returns `None` if the
/// URI does not start with `file://`.
pub fn uri_to_workspace_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = rest
        .replace("%20", " ")
        .replace("%23", "#")
        .replace("%3F", "?")
        .replace("%25", "%");

    // Windows: /C:/foo → C:\foo
    if decoded.len() >= 3
        && decoded.starts_with('/')
        && decoded.as_bytes()[1].is_ascii_alphabetic()
        && decoded.as_bytes()[2] == b':'
    {
        Some(decoded[1..].replace('/', "\\"))
    } else {
        Some(decoded.to_string())
    }
}

/// Resolve a relative path against a workspace folder URI.
pub fn resolve_workspace_path(folder: &WorkspaceFolder, relative: &str) -> String {
    let base = if folder.uri.ends_with('/') {
        folder.uri.clone()
    } else {
        format!("{}/", folder.uri)
    };
    let clean = relative.trim_start_matches('/');
    format!("{base}{clean}")
}

// ── Workspace Snapshot ──

/// A point-in-time snapshot of a workspace configuration that can be restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub name: Option<String>,
    pub folders: Vec<WorkspaceFolder>,
    pub settings: HashMap<String, String>,
    pub timestamp: u64,
    pub label: String,
}

impl WorkspaceSnapshot {
    /// Capture the current state of a workspace configuration.
    pub fn capture(config: &WorkspaceConfiguration, timestamp: u64, label: impl Into<String>) -> Self {
        Self {
            name: config.name.clone(),
            folders: config.folders.clone(),
            settings: config.settings.clone(),
            timestamp,
            label: label.into(),
        }
    }

    /// Restore this snapshot into the given workspace configuration, replacing all state.
    pub fn restore_into(&self, config: &mut WorkspaceConfiguration) {
        config.name = self.name.clone();
        config.folders = self.folders.clone();
        config.settings = self.settings.clone();
    }

    /// Return the number of folders captured in this snapshot.
    pub fn folder_count(&self) -> usize {
        self.folders.len()
    }

    /// Return the number of settings captured in this snapshot.
    pub fn setting_count(&self) -> usize {
        self.settings.len()
    }
}

impl fmt::Display for WorkspaceSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Snapshot '{}' at {} ({} folder(s), {} setting(s))",
            self.label,
            self.timestamp,
            self.folders.len(),
            self.settings.len()
        )
    }
}

// ── Workspace Diff ──

/// Describes a single difference between two workspace configurations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceDiffEntry {
    FolderAdded(String),
    FolderRemoved(String),
    SettingAdded(String),
    SettingRemoved(String),
    SettingChanged { key: String, old: String, new: String },
    NameChanged { old: Option<String>, new: Option<String> },
}

/// The result of comparing two workspace configurations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDiff {
    pub entries: Vec<WorkspaceDiffEntry>,
}

impl WorkspaceDiff {
    /// Compare two workspace configurations and produce a diff.
    pub fn compare(before: &WorkspaceConfiguration, after: &WorkspaceConfiguration) -> Self {
        let mut entries = Vec::new();

        if before.name != after.name {
            entries.push(WorkspaceDiffEntry::NameChanged {
                old: before.name.clone(),
                new: after.name.clone(),
            });
        }

        let before_uris: std::collections::HashSet<&str> =
            before.folders.iter().map(|f| f.uri.as_str()).collect();
        let after_uris: std::collections::HashSet<&str> =
            after.folders.iter().map(|f| f.uri.as_str()).collect();

        for uri in &after_uris {
            if !before_uris.contains(uri) {
                entries.push(WorkspaceDiffEntry::FolderAdded(uri.to_string()));
            }
        }
        for uri in &before_uris {
            if !after_uris.contains(uri) {
                entries.push(WorkspaceDiffEntry::FolderRemoved(uri.to_string()));
            }
        }

        for (key, new_val) in &after.settings {
            match before.settings.get(key) {
                None => entries.push(WorkspaceDiffEntry::SettingAdded(key.clone())),
                Some(old_val) if old_val != new_val => {
                    entries.push(WorkspaceDiffEntry::SettingChanged {
                        key: key.clone(),
                        old: old_val.clone(),
                        new: new_val.clone(),
                    });
                }
                _ => {}
            }
        }
        for key in before.settings.keys() {
            if !after.settings.contains_key(key) {
                entries.push(WorkspaceDiffEntry::SettingRemoved(key.clone()));
            }
        }

        Self { entries }
    }

    /// Returns `true` if the two configurations are identical.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of differences found.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns only the folder-related changes.
    pub fn folder_changes(&self) -> Vec<&WorkspaceDiffEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e, WorkspaceDiffEntry::FolderAdded(_) | WorkspaceDiffEntry::FolderRemoved(_)))
            .collect()
    }

    /// Returns only the setting-related changes.
    pub fn setting_changes(&self) -> Vec<&WorkspaceDiffEntry> {
        self.entries
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    WorkspaceDiffEntry::SettingAdded(_)
                        | WorkspaceDiffEntry::SettingRemoved(_)
                        | WorkspaceDiffEntry::SettingChanged { .. }
                )
            })
            .collect()
    }
}

// ── Workspace Template ──

/// A reusable template for creating workspace configurations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTemplate {
    pub template_name: String,
    pub description: String,
    pub default_settings: Vec<(String, String)>,
    pub folder_patterns: Vec<String>,
}

impl WorkspaceTemplate {
    /// Create a new template with a name and description.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            template_name: name.into(),
            description: description.into(),
            default_settings: Vec::new(),
            folder_patterns: Vec::new(),
        }
    }

    /// Add a default setting to the template.
    pub fn add_setting(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.default_settings.push((key.into(), value.into()));
    }

    /// Add a folder pattern (e.g. "src", "tests") to the template.
    pub fn add_folder_pattern(&mut self, pattern: impl Into<String>) {
        self.folder_patterns.push(pattern.into());
    }

    /// Instantiate a workspace configuration from this template using a root path.
    pub fn instantiate(&self, root: &str) -> Result<WorkspaceConfiguration, WorkspaceError> {
        let mut config = WorkspaceConfiguration::new();
        let root_trimmed = root.trim_end_matches('/');

        for pattern in &self.folder_patterns {
            let uri = format!("{}/{}", root_trimmed, pattern);
            let name = pattern.clone();
            config.try_add_folder(uri, name)?;
        }

        for (key, value) in &self.default_settings {
            config.try_set_setting(key.clone(), value.clone())?;
        }

        Ok(config)
    }

    /// Number of default settings in the template.
    pub fn setting_count(&self) -> usize {
        self.default_settings.len()
    }

    /// Number of folder patterns in the template.
    pub fn folder_pattern_count(&self) -> usize {
        self.folder_patterns.len()
    }
}

impl fmt::Display for WorkspaceTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Template '{}': {} ({} folder(s), {} setting(s))",
            self.template_name,
            self.description,
            self.folder_patterns.len(),
            self.default_settings.len()
        )
    }
}

// ── Workspace Health Check ──

/// The severity of a health check finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthSeverity {
    Info,
    Warning,
    Error,
}

impl fmt::Display for HealthSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthSeverity::Info => write!(f, "INFO"),
            HealthSeverity::Warning => write!(f, "WARN"),
            HealthSeverity::Error => write!(f, "ERROR"),
        }
    }
}

/// A single finding from a workspace health check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthFinding {
    pub severity: HealthSeverity,
    pub message: String,
}

/// Results of running a health check on a workspace configuration.
#[derive(Debug, Clone)]
pub struct WorkspaceHealthCheck {
    pub findings: Vec<HealthFinding>,
}

impl WorkspaceHealthCheck {
    /// Run a health check against the given workspace configuration.
    pub fn check(config: &WorkspaceConfiguration) -> Self {
        let mut findings = Vec::new();

        if config.name.is_none() {
            findings.push(HealthFinding {
                severity: HealthSeverity::Warning,
                message: "Workspace has no name set".to_string(),
            });
        }

        if config.folders.is_empty() {
            findings.push(HealthFinding {
                severity: HealthSeverity::Error,
                message: "Workspace has no folders".to_string(),
            });
        }

        // Check for folders with identical names (confusing for users).
        let mut seen_names = std::collections::HashSet::new();
        for folder in &config.folders {
            if !seen_names.insert(&folder.name) {
                findings.push(HealthFinding {
                    severity: HealthSeverity::Warning,
                    message: format!("Duplicate folder name: {}", folder.name),
                });
            }
        }

        // Check for settings with empty values.
        for (key, value) in &config.settings {
            if value.is_empty() {
                findings.push(HealthFinding {
                    severity: HealthSeverity::Info,
                    message: format!("Setting '{}' has an empty value", key),
                });
            }
        }

        Self { findings }
    }

    /// Returns `true` if no findings were produced.
    pub fn is_healthy(&self) -> bool {
        self.findings.is_empty()
    }

    /// Returns `true` if any findings have Error severity.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == HealthSeverity::Error)
    }

    /// Returns `true` if any findings have Warning severity.
    pub fn has_warnings(&self) -> bool {
        self.findings.iter().any(|f| f.severity == HealthSeverity::Warning)
    }

    /// Returns the total number of findings.
    pub fn finding_count(&self) -> usize {
        self.findings.len()
    }

    /// Filter findings by severity.
    pub fn findings_by_severity(&self, severity: HealthSeverity) -> Vec<&HealthFinding> {
        self.findings.iter().filter(|f| f.severity == severity).collect()
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

    #[test]
    fn workspace_error_display() {
        let err = WorkspaceError::DuplicateFolder("/a".into());
        assert_eq!(format!("{}", err), "folder already exists: /a");
        let err = WorkspaceError::FolderIndexOutOfRange(5);
        assert_eq!(format!("{}", err), "folder index out of range: 5");
        let err = WorkspaceError::EmptySettingKey;
        assert_eq!(format!("{}", err), "setting key must not be empty");
    }

    #[test]
    fn try_add_folder_rejects_duplicates() {
        let mut ws = WorkspaceConfiguration::new();
        ws.try_add_folder("/a".into(), "a".into()).unwrap();
        let err = ws.try_add_folder("/a".into(), "a2".into()).unwrap_err();
        assert_eq!(err, WorkspaceError::DuplicateFolder("/a".into()));
    }

    #[test]
    fn try_add_folder_rejects_empty_uri() {
        let mut ws = WorkspaceConfiguration::new();
        let err = ws.try_add_folder("".into(), "x".into()).unwrap_err();
        assert_eq!(err, WorkspaceError::InvalidUri("".into()));
    }

    #[test]
    fn try_remove_folder_error_on_missing() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/a".into(), "a".into());
        let err = ws.try_remove_folder(99).unwrap_err();
        assert_eq!(err, WorkspaceError::FolderIndexOutOfRange(99));
        let removed = ws.try_remove_folder(0).unwrap();
        assert_eq!(removed.uri, "/a");
        assert_eq!(ws.folder_count(), 0);
    }

    #[test]
    fn try_set_setting_rejects_empty_key() {
        let mut ws = WorkspaceConfiguration::new();
        let err = ws.try_set_setting("".into(), "val".into()).unwrap_err();
        assert_eq!(err, WorkspaceError::EmptySettingKey);
    }

    #[test]
    fn set_name_validation() {
        let mut ws = WorkspaceConfiguration::new();
        assert!(ws.set_name("  ".into()).is_err());
        assert!(ws.set_name("x".repeat(256)).is_err());
        assert!(ws.set_name("My Project".into()).is_ok());
        assert_eq!(ws.name.as_deref(), Some("My Project"));
    }

    #[test]
    fn builder_creates_valid_config() {
        let config = WorkspaceConfigurationBuilder::new()
            .name("test-ws")
            .folder("/src", "source")
            .folder("/lib", "library")
            .setting("editor.tabSize", "2")
            .build()
            .unwrap();
        assert_eq!(config.name.as_deref(), Some("test-ws"));
        assert_eq!(config.folder_count(), 2);
        assert_eq!(config.get_setting("editor.tabSize"), Some("2"));
    }

    #[test]
    fn builder_rejects_duplicate_folders() {
        let result = WorkspaceConfigurationBuilder::new()
            .folder("/a", "a")
            .folder("/a", "a-dup")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn workspace_display_and_debug() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/p".into(), "p".into());
        ws.set_setting("k".into(), "v".into());
        ws.name = Some("demo".into());
        let display = format!("{}", ws);
        assert!(display.contains("demo"));
        assert!(display.contains("1 folder(s)"));
        let debug = format!("{:?}", ws);
        assert!(debug.contains("WorkspaceConfiguration"));
    }

    #[test]
    fn folder_uris_and_merge_settings() {
        let mut ws1 = WorkspaceConfiguration::new();
        ws1.add_folder("/a".into(), "a".into());
        ws1.add_folder("/b".into(), "b".into());
        assert_eq!(ws1.folder_uris(), vec!["/a", "/b"]);

        let mut ws2 = WorkspaceConfiguration::new();
        ws2.set_setting("x".into(), "1".into());
        ws2.set_setting("y".into(), "2".into());
        ws1.merge_settings(&ws2);
        assert_eq!(ws1.get_setting("x"), Some("1"));
        assert_eq!(ws1.get_setting("y"), Some("2"));

        ws1.clear_settings();
        assert_eq!(ws1.setting_count(), 0);
    }

    #[test]
    fn trust_service_remove_and_reset() {
        let mut svc = WorkspaceTrustService::new();
        svc.set_trust(WorkspaceTrust::Trusted);
        svc.add_trusted_folder("/a".into());
        svc.add_trusted_folder("/b".into());
        assert!(svc.remove_trusted_folder("/a"));
        assert!(!svc.remove_trusted_folder("/a"));
        assert_eq!(svc.trusted_folder_count(), 1);
        svc.reset();
        assert_eq!(svc.trust_state, WorkspaceTrust::Unknown);
        assert_eq!(svc.trusted_folder_count(), 0);
    }

    #[test]
    fn trust_service_workspace_integration() {
        let mut svc = WorkspaceTrustService::new();
        svc.add_trusted_folder("/safe".into());

        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/safe".into(), "safe".into());
        assert!(svc.is_workspace_fully_trusted(&ws));

        ws.add_folder("/unsafe".into(), "unsafe".into());
        assert!(!svc.is_workspace_fully_trusted(&ws));
        assert_eq!(svc.untrusted_folders(&ws), vec!["/unsafe"]);
    }

    #[test]
    fn trust_service_display_and_debug() {
        let mut svc = WorkspaceTrustService::new();
        svc.add_trusted_folder("/x".into());
        let display = format!("{}", svc);
        assert!(display.contains("1 trusted folder(s)"));
        let debug = format!("{:?}", svc);
        assert!(debug.contains("WorkspaceTrustService"));
    }

    #[test]
    fn workspace_config_clone_and_eq() {
        let config = WorkspaceConfigurationBuilder::new()
            .name("ws")
            .folder("/a", "a")
            .setting("k", "v")
            .build()
            .unwrap();
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn workspace_stats_computation() {
        let config = WorkspaceConfigurationBuilder::new()
            .name("test")
            .folder("/a", "alpha")
            .folder("/b", "beta")
            .setting("editor.tabSize", "4")
            .setting("editor.fontSize", "14")
            .setting("terminal.shell", "/bin/bash")
            .build()
            .unwrap();
        let stats = config.stats();
        assert_eq!(stats.folder_count, 2);
        assert_eq!(stats.setting_count, 3);
        assert_eq!(stats.setting_prefix_count, 2);
        assert!(stats.has_name);
        assert!(stats.is_multi_root);
    }

    #[test]
    fn search_folders_by_name_and_uri() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/home/user/project".into(), "my-project".into());
        ws.add_folder("/home/user/lib".into(), "library".into());
        ws.add_folder("/opt/tools".into(), "tools".into());
        let results = ws.search_folders("project");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "my-project");
        let results_uri = ws.search_folders("/home");
        assert_eq!(results_uri.len(), 2);
        assert!(ws.search_folders("nonexistent").is_empty());
    }

    #[test]
    fn serialize_and_parse_summary() {
        let config = WorkspaceConfigurationBuilder::new()
            .name("demo")
            .folder("/src", "source")
            .setting("k", "v")
            .build()
            .unwrap();
        let summary = config.serialize_summary();
        assert!(summary.contains("name=demo"));
        assert!(summary.contains("folders=1"));
        let (name, folders, settings) = WorkspaceConfiguration::parse_summary(&summary);
        assert_eq!(name.as_deref(), Some("demo"));
        assert_eq!(folders, 1);
        assert_eq!(settings, 1);
    }

    #[test]
    fn reorder_folder_moves_correctly() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/a".into(), "alpha".into());
        ws.add_folder("/b".into(), "beta".into());
        ws.add_folder("/c".into(), "gamma".into());
        ws.reorder_folder(2, 0).unwrap();
        assert_eq!(ws.folders[0].uri, "/c");
        assert_eq!(ws.folders[1].uri, "/a");
        assert_eq!(ws.folders[0].index, 0);
        assert_eq!(ws.folders[1].index, 1);
        assert!(ws.reorder_folder(10, 0).is_err());
    }

    #[test]
    fn last_folder_and_sorted_by_name() {
        let mut ws = WorkspaceConfiguration::new();
        assert!(ws.last_folder().is_none());
        ws.add_folder("/z".into(), "zulu".into());
        ws.add_folder("/a".into(), "alpha".into());
        ws.add_folder("/m".into(), "mike".into());
        assert_eq!(ws.last_folder().unwrap().name, "mike");
        let sorted = ws.folders_sorted_by_name();
        assert_eq!(sorted[0].name, "alpha");
        assert_eq!(sorted[1].name, "mike");
        assert_eq!(sorted[2].name, "zulu");
    }

    #[test]
    fn stats_empty_workspace() {
        let ws = WorkspaceConfiguration::new();
        let stats = ws.stats();
        assert_eq!(stats.folder_count, 0);
        assert_eq!(stats.setting_count, 0);
        assert_eq!(stats.setting_prefix_count, 0);
        assert!(!stats.has_name);
        assert!(!stats.is_multi_root);
    }

    // ── WorkspaceHistory tests ──

    #[test]
    fn workspace_history_add_and_recent() {
        let mut history = WorkspaceHistory::new(5);
        history.add("/home/user/project-a", 1000);
        history.add("/home/user/project-b", 2000);
        history.add("/home/user/project-c", 3000);
        let recent = history.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].0, "/home/user/project-c");
        assert_eq!(recent[1].0, "/home/user/project-b");
    }

    #[test]
    fn workspace_history_deduplicates() {
        let mut history = WorkspaceHistory::new(5);
        history.add("/project", 1000);
        history.add("/project", 2000);
        assert_eq!(history.len(), 1);
        let recent = history.recent(5);
        assert_eq!(recent[0].1, 2000);
    }

    #[test]
    fn workspace_history_capacity() {
        let mut history = WorkspaceHistory::new(2);
        history.add("/a", 100);
        history.add("/b", 200);
        history.add("/c", 300);
        assert_eq!(history.len(), 2);
        assert!(!history.contains("/a"));
        assert!(history.contains("/c"));
    }

    #[test]
    fn workspace_history_clear() {
        let mut history = WorkspaceHistory::new(5);
        history.add("/project", 100);
        history.clear();
        assert!(history.is_empty());
    }

    #[test]
    fn find_workspace_files_in_dir() {
        // Use a temp directory to test file discovery
        let dir = std::env::temp_dir().join("vsedit_ws_test");
        let _ = std::fs::create_dir_all(&dir);
        let ws_file = dir.join("test.code-workspace");
        std::fs::write(&ws_file, "{}").unwrap();

        let files = find_workspace_files(dir.to_str().unwrap());
        assert!(files.iter().any(|p| p.ends_with("test.code-workspace")));

        let _ = std::fs::remove_file(&ws_file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn find_workspace_files_nonexistent_dir() {
        let files = find_workspace_files("/nonexistent_path_12345");
        assert!(files.is_empty());
    }

    #[test]
    fn workspace_to_uri_unix() {
        assert_eq!(workspace_to_uri("/home/user/project"), "file:///home/user/project");
    }

    #[test]
    fn workspace_to_uri_with_spaces() {
        assert_eq!(workspace_to_uri("/home/user/my project"), "file:///home/user/my%20project");
    }

    #[test]
    fn workspace_to_uri_windows() {
        let uri = workspace_to_uri("C:\\Users\\me\\code");
        assert!(uri.starts_with("file:///C:"));
        assert!(uri.contains("/Users/me/code"));
    }

    #[test]
    fn uri_to_workspace_path_unix() {
        let path = uri_to_workspace_path("file:///home/user/project").unwrap();
        assert_eq!(path, "/home/user/project");
    }

    #[test]
    fn uri_to_workspace_path_invalid() {
        assert!(uri_to_workspace_path("http://example.com").is_none());
    }

    #[test]
    fn uri_roundtrip() {
        let original = "/home/user/my project";
        let uri = workspace_to_uri(original);
        let back = uri_to_workspace_path(&uri).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn resolve_workspace_path_basic() {
        let folder = WorkspaceFolder { uri: "/workspace".to_string(), name: "ws".to_string(), index: 0 };
        let resolved = resolve_workspace_path(&folder, "src/main.rs");
        assert_eq!(resolved, "/workspace/src/main.rs");
    }

    #[test]
    fn display_name_returns_folder_name() {
        let folder = WorkspaceFolder {
            uri: "/home/user/project".to_string(),
            name: "my-project".to_string(),
            index: 0,
        };
        assert_eq!(folder.display_name(), "my-project");
    }

    #[test]
    fn has_setting_checks_presence() {
        let mut ws = WorkspaceConfiguration::new();
        assert!(!ws.has_setting("editor.tabSize"));
        ws.set_setting("editor.tabSize".into(), "4".into());
        assert!(ws.has_setting("editor.tabSize"));
        assert!(!ws.has_setting("editor.fontSize"));
    }

    #[test]
    fn settings_keys_returns_sorted() {
        let mut ws = WorkspaceConfiguration::new();
        ws.set_setting("zebra".into(), "1".into());
        ws.set_setting("alpha".into(), "2".into());
        ws.set_setting("middle".into(), "3".into());
        let keys = ws.settings_keys();
        assert_eq!(keys, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn rename_folder_success_and_error() {
        let mut ws = WorkspaceConfiguration::new();
        ws.add_folder("/src".into(), "source".into());
        ws.add_folder("/lib".into(), "library".into());
        ws.rename_folder(0, "renamed-source".into()).unwrap();
        assert_eq!(ws.get_folder(0).unwrap().name, "renamed-source");
        assert_eq!(ws.get_folder(0).unwrap().display_name(), "renamed-source");
        let err = ws.rename_folder(99, "nope".into()).unwrap_err();
        assert_eq!(err, WorkspaceError::InvalidIndex(99));
    }

    #[test]
    fn get_or_default_returns_value_or_fallback() {
        let mut ws = WorkspaceConfiguration::new();
        ws.set_setting("theme".into(), "dark".into());
        assert_eq!(ws.get_or_default("theme", "light"), "dark");
        assert_eq!(ws.get_or_default("missing", "fallback"), "fallback");
    }

    #[test]
    fn invalid_index_error_display() {
        let err = WorkspaceError::InvalidIndex(42);
        assert_eq!(format!("{err}"), "invalid index: 42");
    }

    #[test]
    fn merge_settings_overwrites_on_conflict() {
        let mut ws1 = WorkspaceConfiguration::new();
        ws1.set_setting("key".into(), "old".into());
        let mut ws2 = WorkspaceConfiguration::new();
        ws2.set_setting("key".into(), "new".into());
        ws2.set_setting("extra".into(), "val".into());
        ws1.merge_settings(&ws2);
        assert_eq!(ws1.get_setting("key"), Some("new"));
        assert_eq!(ws1.get_setting("extra"), Some("val"));
    }

    // ── Snapshot tests ──

    #[test]
    fn snapshot_capture_and_restore() {
        let mut ws = WorkspaceConfiguration::new();
        ws.set_name("original".into()).unwrap();
        ws.add_folder("/src".into(), "source".into());
        ws.set_setting("editor.tabSize".into(), "4".into());

        let snap = WorkspaceSnapshot::capture(&ws, 1000, "before refactor");
        assert_eq!(snap.label, "before refactor");
        assert_eq!(snap.timestamp, 1000);
        assert_eq!(snap.folder_count(), 1);
        assert_eq!(snap.setting_count(), 1);

        // Mutate the workspace
        ws.add_folder("/lib".into(), "library".into());
        ws.set_setting("editor.tabSize".into(), "2".into());
        ws.set_name("modified".into()).unwrap();
        assert_eq!(ws.folder_count(), 2);

        // Restore the snapshot
        snap.restore_into(&mut ws);
        assert_eq!(ws.folder_count(), 1);
        assert_eq!(ws.name.as_deref(), Some("original"));
        assert_eq!(ws.get_setting("editor.tabSize"), Some("4"));
    }

    #[test]
    fn snapshot_display() {
        let ws = WorkspaceConfiguration::new();
        let snap = WorkspaceSnapshot::capture(&ws, 42, "empty");
        let display = format!("{}", snap);
        assert!(display.contains("empty"));
        assert!(display.contains("42"));
    }

    // ── Diff tests ──

    #[test]
    fn diff_detects_all_change_types() {
        let mut before = WorkspaceConfiguration::new();
        before.set_name("old-name".into()).unwrap();
        before.add_folder("/a".into(), "alpha".into());
        before.add_folder("/removed".into(), "removed".into());
        before.set_setting("keep".into(), "same".into());
        before.set_setting("change".into(), "old-val".into());
        before.set_setting("delete-me".into(), "x".into());

        let mut after = WorkspaceConfiguration::new();
        after.set_name("new-name".into()).unwrap();
        after.add_folder("/a".into(), "alpha".into());
        after.add_folder("/added".into(), "added".into());
        after.set_setting("keep".into(), "same".into());
        after.set_setting("change".into(), "new-val".into());
        after.set_setting("new-setting".into(), "y".into());

        let diff = WorkspaceDiff::compare(&before, &after);
        assert!(!diff.is_empty());

        // Name changed
        assert!(diff.entries.contains(&WorkspaceDiffEntry::NameChanged {
            old: Some("old-name".into()),
            new: Some("new-name".into()),
        }));
        // Folder added/removed
        assert!(diff.entries.contains(&WorkspaceDiffEntry::FolderAdded("/added".into())));
        assert!(diff.entries.contains(&WorkspaceDiffEntry::FolderRemoved("/removed".into())));
        // Setting changed/added/removed
        assert!(diff.entries.contains(&WorkspaceDiffEntry::SettingChanged {
            key: "change".into(),
            old: "old-val".into(),
            new: "new-val".into(),
        }));
        assert!(diff.entries.contains(&WorkspaceDiffEntry::SettingAdded("new-setting".into())));
        assert!(diff.entries.contains(&WorkspaceDiffEntry::SettingRemoved("delete-me".into())));

        assert!(!diff.folder_changes().is_empty());
        assert!(!diff.setting_changes().is_empty());
    }

    #[test]
    fn diff_identical_configs_is_empty() {
        let config = WorkspaceConfigurationBuilder::new()
            .name("same")
            .folder("/a", "a")
            .setting("k", "v")
            .build()
            .unwrap();
        let diff = WorkspaceDiff::compare(&config, &config);
        assert!(diff.is_empty());
        assert_eq!(diff.len(), 0);
    }

    // ── Template tests ──

    #[test]
    fn template_instantiate_creates_config() {
        let mut tmpl = WorkspaceTemplate::new("rust-project", "Standard Rust project layout");
        tmpl.add_folder_pattern("src");
        tmpl.add_folder_pattern("tests");
        tmpl.add_setting("editor.tabSize", "4");
        tmpl.add_setting("editor.formatOnSave", "true");

        assert_eq!(tmpl.folder_pattern_count(), 2);
        assert_eq!(tmpl.setting_count(), 2);

        let config = tmpl.instantiate("/home/user/myproject").unwrap();
        assert_eq!(config.folder_count(), 2);
        assert_eq!(config.setting_count(), 2);
        assert!(config.contains_uri("/home/user/myproject/src"));
        assert!(config.contains_uri("/home/user/myproject/tests"));
        assert_eq!(config.get_setting("editor.tabSize"), Some("4"));

        let display = format!("{}", tmpl);
        assert!(display.contains("rust-project"));
    }

    // ── Health check tests ──

    #[test]
    fn health_check_reports_issues() {
        let mut ws = WorkspaceConfiguration::new();
        // No name, no folders → should flag both
        let hc = WorkspaceHealthCheck::check(&ws);
        assert!(hc.has_errors()); // no folders
        assert!(hc.has_warnings()); // no name
        assert!(!hc.is_healthy());

        // Add a folder and a name, add empty-value setting
        ws.add_folder("/src".into(), "source".into());
        ws.set_name("project".into()).unwrap();
        ws.set_setting("placeholder".into(), "".into());

        let hc2 = WorkspaceHealthCheck::check(&ws);
        assert!(!hc2.has_errors());
        assert!(!hc2.has_warnings());
        // Info about empty setting value
        let infos = hc2.findings_by_severity(HealthSeverity::Info);
        assert_eq!(infos.len(), 1);
        assert!(infos[0].message.contains("placeholder"));
    }

    #[test]
    fn health_check_duplicate_folder_names() {
        let mut ws = WorkspaceConfiguration::new();
        ws.set_name("test".into()).unwrap();
        ws.add_folder("/a".into(), "same-name".into());
        ws.add_folder("/b".into(), "same-name".into());

        let hc = WorkspaceHealthCheck::check(&ws);
        assert!(hc.has_warnings());
        let warnings = hc.findings_by_severity(HealthSeverity::Warning);
        assert!(warnings.iter().any(|f| f.message.contains("Duplicate folder name")));
    }
}
