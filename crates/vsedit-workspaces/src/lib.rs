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

// ── Setting Scope ──

/// Identifies where a setting value originates from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingScope {
    /// A default value provided by the application.
    Default,
    /// Set at the user/global level.
    User,
    /// Set at the workspace level.
    Workspace,
    /// Set at the workspace-folder level.
    WorkspaceFolder,
}

impl fmt::Display for SettingScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingScope::Default => write!(f, "default"),
            SettingScope::User => write!(f, "user"),
            SettingScope::Workspace => write!(f, "workspace"),
            SettingScope::WorkspaceFolder => write!(f, "workspace-folder"),
        }
    }
}

/// A resolved setting value with its originating scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSetting {
    pub key: String,
    pub value: String,
    pub scope: SettingScope,
}

/// Layered setting resolution across multiple scopes.
///
/// Settings are resolved in order of increasing specificity:
/// default → user → workspace → workspace-folder.
pub struct SettingResolver {
    defaults: HashMap<String, String>,
    user: HashMap<String, String>,
    workspace: HashMap<String, String>,
    folder_overrides: HashMap<String, HashMap<String, String>>,
}

impl SettingResolver {
    pub fn new() -> Self {
        Self {
            defaults: HashMap::new(),
            user: HashMap::new(),
            workspace: HashMap::new(),
            folder_overrides: HashMap::new(),
        }
    }

    /// Register a default value for a setting key.
    pub fn set_default(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.defaults.insert(key.into(), value.into());
    }

    /// Register a user-level value for a setting key.
    pub fn set_user(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.user.insert(key.into(), value.into());
    }

    /// Register a workspace-level value for a setting key.
    pub fn set_workspace(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.workspace.insert(key.into(), value.into());
    }

    /// Register a folder-level override for a setting key.
    pub fn set_folder_override(
        &mut self,
        folder_uri: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.folder_overrides
            .entry(folder_uri.into())
            .or_default()
            .insert(key.into(), value.into());
    }

    /// Resolve a setting key to its most specific value and scope.
    ///
    /// If `folder_uri` is provided, folder-level overrides are considered.
    pub fn resolve(&self, key: &str, folder_uri: Option<&str>) -> Option<ResolvedSetting> {
        // Check folder overrides first (most specific).
        if let Some(uri) = folder_uri {
            if let Some(overrides) = self.folder_overrides.get(uri) {
                if let Some(val) = overrides.get(key) {
                    return Some(ResolvedSetting {
                        key: key.to_string(),
                        value: val.clone(),
                        scope: SettingScope::WorkspaceFolder,
                    });
                }
            }
        }

        if let Some(val) = self.workspace.get(key) {
            return Some(ResolvedSetting {
                key: key.to_string(),
                value: val.clone(),
                scope: SettingScope::Workspace,
            });
        }

        if let Some(val) = self.user.get(key) {
            return Some(ResolvedSetting {
                key: key.to_string(),
                value: val.clone(),
                scope: SettingScope::User,
            });
        }

        if let Some(val) = self.defaults.get(key) {
            return Some(ResolvedSetting {
                key: key.to_string(),
                value: val.clone(),
                scope: SettingScope::Default,
            });
        }

        None
    }

    /// Resolve a setting, returning just the value string or a fallback.
    pub fn resolve_value(&self, key: &str, folder_uri: Option<&str>, fallback: &str) -> String {
        self.resolve(key, folder_uri)
            .map(|r| r.value)
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Return all keys that have at least one value registered at any scope.
    pub fn all_keys(&self) -> Vec<String> {
        let mut keys = std::collections::HashSet::new();
        keys.extend(self.defaults.keys().cloned());
        keys.extend(self.user.keys().cloned());
        keys.extend(self.workspace.keys().cloned());
        for overrides in self.folder_overrides.values() {
            keys.extend(overrides.keys().cloned());
        }
        let mut sorted: Vec<String> = keys.into_iter().collect();
        sorted.sort();
        sorted
    }
}

impl Default for SettingResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ── Workspace Event Log ──

/// The kind of mutation that occurred on a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEventKind {
    FolderAdded { uri: String },
    FolderRemoved { uri: String },
    FolderRenamed { uri: String, old_name: String, new_name: String },
    SettingChanged { key: String },
    NameChanged { old: Option<String>, new: Option<String> },
}

/// A timestamped workspace mutation event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEvent {
    pub kind: WorkspaceEventKind,
    pub timestamp: u64,
}

/// An append-only log of workspace mutation events.
#[derive(Debug, Clone)]
pub struct WorkspaceEventLog {
    events: Vec<WorkspaceEvent>,
}

impl WorkspaceEventLog {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Record a new event.
    pub fn record(&mut self, kind: WorkspaceEventKind, timestamp: u64) {
        self.events.push(WorkspaceEvent { kind, timestamp });
    }

    /// Return all events in chronological order.
    pub fn events(&self) -> &[WorkspaceEvent] {
        &self.events
    }

    /// Return events that occurred at or after the given timestamp.
    pub fn events_since(&self, since: u64) -> Vec<&WorkspaceEvent> {
        self.events.iter().filter(|e| e.timestamp >= since).collect()
    }

    /// Return events that match a specific kind discriminant.
    pub fn folder_events(&self) -> Vec<&WorkspaceEvent> {
        self.events
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    WorkspaceEventKind::FolderAdded { .. }
                        | WorkspaceEventKind::FolderRemoved { .. }
                        | WorkspaceEventKind::FolderRenamed { .. }
                )
            })
            .collect()
    }

    /// Number of recorded events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for WorkspaceEventLog {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// WorkspaceRecommendation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WorkspaceRecommendation {
    pub extension_id: String,
    pub reason: String,
}

impl WorkspaceRecommendation {
    pub fn new(ext_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self { extension_id: ext_id.into(), reason: reason.into() }
    }
}

impl fmt::Display for WorkspaceRecommendation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.extension_id, self.reason)
    }
}

pub struct WorkspaceRecommendations {
    recommendations: Vec<WorkspaceRecommendation>,
}

impl WorkspaceRecommendations {
    pub fn new() -> Self { Self { recommendations: Vec::new() } }

    pub fn add(&mut self, rec: WorkspaceRecommendation) { self.recommendations.push(rec); }

    pub fn recommend_for_files(&mut self, files: &[&str]) {
        for file in files {
            if file.ends_with(".rs") || file.ends_with(".toml") {
                self.add(WorkspaceRecommendation::new("rust-analyzer", "Rust files detected"));
            }
            if file.ends_with(".py") {
                self.add(WorkspaceRecommendation::new("ms-python.python", "Python files detected"));
            }
            if file.ends_with(".ts") || file.ends_with(".js") {
                self.add(WorkspaceRecommendation::new("esbenp.prettier-vscode", "JS/TS files detected"));
            }
        }
        self.recommendations.dedup_by(|a, b| a.extension_id == b.extension_id);
    }

    pub fn list(&self) -> &[WorkspaceRecommendation] { &self.recommendations }
    pub fn len(&self) -> usize { self.recommendations.len() }
    pub fn is_empty(&self) -> bool { self.recommendations.is_empty() }
}

impl Default for WorkspaceRecommendations { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// WorkspaceTaskRunnerConfig
// ---------------------------------------------------------------------------

pub struct WorkspaceTaskRunnerConfig {
    tasks: Vec<(String, String)>,
}

impl WorkspaceTaskRunnerConfig {
    pub fn new() -> Self { Self { tasks: Vec::new() } }

    pub fn add_task(&mut self, label: impl Into<String>, command: impl Into<String>) {
        self.tasks.push((label.into(), command.into()));
    }

    pub fn get_command(&self, label: &str) -> Option<&str> {
        self.tasks.iter().find(|(l, _)| l == label).map(|(_, c)| c.as_str())
    }

    pub fn task_labels(&self) -> Vec<&str> { self.tasks.iter().map(|(l, _)| l.as_str()).collect() }
    pub fn len(&self) -> usize { self.tasks.len() }
    pub fn is_empty(&self) -> bool { self.tasks.is_empty() }

    pub fn remove_task(&mut self, label: &str) -> bool {
        if let Some(i) = self.tasks.iter().position(|(l, _)| l == label) { self.tasks.remove(i); true } else { false }
    }
}

impl Default for WorkspaceTaskRunnerConfig { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// WorkspaceSearchScope
// ---------------------------------------------------------------------------

pub struct WorkspaceSearchScope {
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
}

impl WorkspaceSearchScope {
    pub fn new() -> Self { Self { include_patterns: Vec::new(), exclude_patterns: Vec::new() } }

    pub fn include(&mut self, pattern: impl Into<String>) { self.include_patterns.push(pattern.into()); }
    pub fn exclude(&mut self, pattern: impl Into<String>) { self.exclude_patterns.push(pattern.into()); }

    pub fn matches(&self, path: &str) -> bool {
        let included = self.include_patterns.is_empty() ||
            self.include_patterns.iter().any(|p| path.contains(p));
        let excluded = self.exclude_patterns.iter().any(|p| path.contains(p));
        included && !excluded
    }

    pub fn include_count(&self) -> usize { self.include_patterns.len() }
    pub fn exclude_count(&self) -> usize { self.exclude_patterns.len() }
}

impl Default for WorkspaceSearchScope { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// WorkspaceRecentList
// ---------------------------------------------------------------------------

pub struct WorkspaceRecentList {
    entries: Vec<(String, u64)>,
    max_entries: usize,
}

impl WorkspaceRecentList {
    pub fn new(max_entries: usize) -> Self { Self { entries: Vec::new(), max_entries } }

    pub fn add(&mut self, path: impl Into<String>, timestamp: u64) {
        let path = path.into();
        self.entries.retain(|(p, _)| p != &path);
        self.entries.insert(0, (path, timestamp));
        if self.entries.len() > self.max_entries { self.entries.truncate(self.max_entries); }
    }

    pub fn recent(&self) -> Vec<&str> { self.entries.iter().map(|(p, _)| p.as_str()).collect() }

    pub fn sorted_by_time(&self) -> Vec<String> {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().map(|(p, _)| p).collect()
    }

    pub fn sorted_by_name(&self) -> Vec<String> {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        sorted.into_iter().map(|(p, _)| p).collect()
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn remove(&mut self, path: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(p, _)| p != path);
        self.entries.len() < before
    }
}

impl Default for WorkspaceRecentList { fn default() -> Self { Self::new(20) } }


// === Workspace Recommendation Engine ===

/// Workspace Recommendation Engine implementation.
#[derive(Debug, Clone)]
pub struct WorkspaceRecommendationEngine {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: WorkspaceRecommendationEngineStats,
}

/// Statistics for WorkspaceRecommendationEngine.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceRecommendationEngineStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl WorkspaceRecommendationEngineStats {
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

impl WorkspaceRecommendationEngine {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: WorkspaceRecommendationEngineStats::default(),
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

    pub fn stats(&self) -> &WorkspaceRecommendationEngineStats {
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

impl Default for WorkspaceRecommendationEngine {
    fn default() -> Self {
        Self::new()
    }
}

// === Workspace Template Loader ===

/// Priority level for WorkspaceTemplateLoader items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkspaceTemplateLoaderPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl WorkspaceTemplateLoaderPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for WorkspaceTemplateLoaderPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Workspace Template Loader implementation.
#[derive(Debug, Clone)]
pub struct WorkspaceTemplateLoader {
    items: Vec<WorkspaceTemplateLoaderItem>,
    max_items: usize,
    default_priority: WorkspaceTemplateLoaderPriority,
}

/// A single item in WorkspaceTemplateLoader.
#[derive(Debug, Clone)]
pub struct WorkspaceTemplateLoaderItem {
    pub id: String,
    pub label: String,
    pub priority: WorkspaceTemplateLoaderPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl WorkspaceTemplateLoaderItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: WorkspaceTemplateLoaderPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: WorkspaceTemplateLoaderPriority) -> Self {
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

impl WorkspaceTemplateLoader {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: WorkspaceTemplateLoaderPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: WorkspaceTemplateLoaderItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<WorkspaceTemplateLoaderItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&WorkspaceTemplateLoaderItem> {
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

    pub fn by_priority(&self, priority: WorkspaceTemplateLoaderPriority) -> Vec<&WorkspaceTemplateLoaderItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&WorkspaceTemplateLoaderItem> {
        let mut sorted: Vec<&WorkspaceTemplateLoaderItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&WorkspaceTemplateLoaderItem> {
        let mut sorted: Vec<&WorkspaceTemplateLoaderItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&WorkspaceTemplateLoaderItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: WorkspaceTemplateLoaderPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> WorkspaceTemplateLoaderPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceTemplateLoaderItem> {
        self.items.iter()
    }
}

impl Default for WorkspaceTemplateLoader {
    fn default() -> Self {
        Self::new()
    }
}


// ─── WsC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for workspace meta.
#[derive(Debug)]
pub struct WsCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> WsCLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for WsCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WsCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── WsB Builder & Validator ─────────────────────────────

/// Builder for constructing workspace configurations.
#[derive(Debug, Clone)]
pub struct WsBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl WsBBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<WsBCfg, WsBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(WsBBuildErr { errors }); }
        Ok(WsBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated workspace configuration.
#[derive(Debug, Clone)]
pub struct WsBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl WsBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &WsBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for WsBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WsBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct WsBBuildErr { pub errors: Vec<String> }

impl fmt::Display for WsBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WsBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for WsBBuildErr {}



// ---------------------------------------------------------------------------
// workspaces – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workspace management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWorkspacesWorkspaceTrust {
    Trusted,
    Untrusted,
    Restricted,
    Unknown,
}

impl YWorkspacesWorkspaceTrust {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Trusted => 0,
            Self::Untrusted => 1,
            Self::Restricted => 2,
            Self::Unknown => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Trusted => "Trusted",
            Self::Untrusted => "Untrusted",
            Self::Restricted => "Restricted",
            Self::Unknown => "Unknown",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWorkspacesWorkspaceTrust] {
        &[
            YWorkspacesWorkspaceTrust::Trusted,
            YWorkspacesWorkspaceTrust::Untrusted,
            YWorkspacesWorkspaceTrust::Restricted,
            YWorkspacesWorkspaceTrust::Unknown,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWorkspacesWorkspaceTrust {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks workspace folder data.
#[derive(Debug, Clone)]
pub struct YWorkspacesWorkspaceFolder {
    pub uri: String,
    pub name: String,
    pub index: usize,
}

impl YWorkspacesWorkspaceFolder {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            uri: String::new(),
            name: String::new(),
            index: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWorkspacesWorkspaceFolder({}: {:?})", "uri", self.uri)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_workspaces_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_workspaces_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_workspaces_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_workspaces_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_workspaces_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_workspaces_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_workspaces_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_workspaces_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// workspaces – Extended workspace recent helpers
// ---------------------------------------------------------------------------

/// Priority levels for workspace recent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWorkspacesPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWorkspacesPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZWorkspacesPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWorkspacesPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks workspace recent data.
#[derive(Debug, Clone)]
pub struct ZWorkspacesWorkspaceRecent {
    pub entries: Vec<(String, u64)>,
    pub max_entries: usize,
    pub pinned: Vec<String>,
}

impl ZWorkspacesWorkspaceRecent {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 0,
            pinned: Vec::new(),
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWorkspacesWorkspaceRecent[max_entries={:?}, pinned={:?}]", self.max_entries, self.pinned)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for workspace recent.
pub fn z_workspaces_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_workspaces_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_workspaces_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_workspaces_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_workspaces_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_workspaces_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_workspaces_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 241
// ---------------------------------------------------------------------------

/// Generic object pool `Xc241Pool<T>`.
pub struct Xc241Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc241Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc241PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc241Pool<T> {
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
    pub fn stats(&self) -> Xc241PoolStats {
        Xc241PoolStats {
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

impl<T> Default for Xc241Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc241Scheduler`.
pub struct Xc241Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc241Scheduler {
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

impl Default for Xc241Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_241 hash for the given byte slice.
pub fn xc_241_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_241 convention.
pub fn xc_241_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_23 deepening: state machine + event bus ---

/// States for the Xd23 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd23State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd23State {
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
pub struct Xd23Transition {
    pub from: Xd23State,
    pub to: Xd23State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd23StateMachine {
    current: Xd23State,
    history: Vec<Xd23Transition>,
    step_counter: usize,
}

impl Xd23StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd23State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd23State {
        self.current
    }

    pub fn history(&self) -> &[Xd23Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd23State) -> Result<Xd23State, String> {
        let allowed = match (self.current, target) {
            (Xd23State::Idle, Xd23State::Running) => true,
            (Xd23State::Running, Xd23State::Paused) => true,
            (Xd23State::Running, Xd23State::Done) => true,
            (Xd23State::Paused, Xd23State::Running) => true,
            (Xd23State::Paused, Xd23State::Done) => true,
            (Xd23State::Done, Xd23State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_23: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd23Transition {
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
            "Xd23SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd23State> {
        let prefix = "Xd23SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd23State::Idle),
            "Running" => Some(Xd23State::Running),
            "Paused" => Some(Xd23State::Paused),
            "Done" => Some(Xd23State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd23State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd23 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd23Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd23Event {
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

type Xd23HandlerFn = Box<dyn Fn(&Xd23Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd23EventBus {
    handlers: Vec<(usize, Option<String>, Xd23HandlerFn)>,
    next_id: usize,
    published: Vec<Xd23Event>,
}

impl Xd23EventBus {
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
        F: Fn(&Xd23Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd23Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd23Event) {
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

    pub fn published_events(&self) -> &[Xd23Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #21
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf21Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf21TrieNode {
    children: std::collections::HashMap<char, Xf21TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf21Trie {
    root: Xf21TrieNode,
    count: usize,
}

impl Xf21Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf21TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf21TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf21TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf21BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf21BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 240).
pub struct Xh240SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh240SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 282 as u64,
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

/// A compact bit set supporting boolean operations (variant 240).
pub struct Xh240BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh240BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 240).
pub struct Xi240Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi240Deque<T> {
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
pub struct Xi240Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi240Interval {
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

/// A simple interval tree (variant 240).
pub struct Xi240IntervalTree {
    xi_intervals: Vec<Xi240Interval>,
}

impl Xi240IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi240Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi240Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi240Interval) -> Vec<&Xi240Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi240Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi240Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi240Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi240Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi240Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi240Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 240) ---

/// Disjoint set / union-find for crate 240.
pub struct Xj240UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj240UnionFind {
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

const XJ240_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 240.
pub struct Xj240BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj240BTreeNode<K, V>>>,
    len: usize,
}

struct Xj240BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj240BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj240BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ240_BTREE_ORDER - 1
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
        let mid = XJ240_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj240BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj240BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj240BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj240BTreeNode::xj_new_leaf();
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


// --- xk_240 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk240SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk240SegmentTree {
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
pub struct Xk240DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk240DisjointIntervals {
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

    // ── SettingResolver tests ──

    #[test]
    fn setting_resolver_layered_precedence() {
        let mut resolver = SettingResolver::new();
        resolver.set_default("editor.tabSize", "4");
        resolver.set_user("editor.tabSize", "2");
        resolver.set_workspace("editor.tabSize", "8");
        resolver.set_folder_override("/src", "editor.tabSize", "3");

        // Without folder context, workspace wins.
        let resolved = resolver.resolve("editor.tabSize", None).unwrap();
        assert_eq!(resolved.value, "8");
        assert_eq!(resolved.scope, SettingScope::Workspace);

        // With folder context, folder override wins.
        let resolved = resolver.resolve("editor.tabSize", Some("/src")).unwrap();
        assert_eq!(resolved.value, "3");
        assert_eq!(resolved.scope, SettingScope::WorkspaceFolder);

        // Different folder falls back to workspace.
        let resolved = resolver.resolve("editor.tabSize", Some("/lib")).unwrap();
        assert_eq!(resolved.value, "8");
        assert_eq!(resolved.scope, SettingScope::Workspace);
    }

    #[test]
    fn setting_resolver_fallback_through_scopes() {
        let mut resolver = SettingResolver::new();
        resolver.set_default("theme", "light");

        // Only default is set.
        let resolved = resolver.resolve("theme", None).unwrap();
        assert_eq!(resolved.value, "light");
        assert_eq!(resolved.scope, SettingScope::Default);

        // User overrides default.
        resolver.set_user("theme", "dark");
        let resolved = resolver.resolve("theme", None).unwrap();
        assert_eq!(resolved.value, "dark");
        assert_eq!(resolved.scope, SettingScope::User);

        // Unknown key returns None.
        assert!(resolver.resolve("unknown.key", None).is_none());
    }

    #[test]
    fn setting_resolver_resolve_value_with_fallback() {
        let resolver = SettingResolver::new();
        assert_eq!(resolver.resolve_value("missing", None, "default"), "default");

        let mut resolver = SettingResolver::new();
        resolver.set_default("font", "monospace");
        assert_eq!(resolver.resolve_value("font", None, "serif"), "monospace");
    }

    #[test]
    fn setting_resolver_all_keys() {
        let mut resolver = SettingResolver::new();
        resolver.set_default("a.one", "1");
        resolver.set_user("b.two", "2");
        resolver.set_workspace("a.one", "override");
        resolver.set_folder_override("/x", "c.three", "3");

        let keys = resolver.all_keys();
        assert_eq!(keys, vec!["a.one", "b.two", "c.three"]);
    }

    #[test]
    fn setting_scope_display() {
        assert_eq!(format!("{}", SettingScope::Default), "default");
        assert_eq!(format!("{}", SettingScope::User), "user");
        assert_eq!(format!("{}", SettingScope::Workspace), "workspace");
        assert_eq!(format!("{}", SettingScope::WorkspaceFolder), "workspace-folder");
    }

    // ── WorkspaceEventLog tests ──

    #[test]
    fn event_log_record_and_query() {
        let mut log = WorkspaceEventLog::new();
        assert!(log.is_empty());

        log.record(WorkspaceEventKind::FolderAdded { uri: "/src".into() }, 100);
        log.record(WorkspaceEventKind::SettingChanged { key: "editor.tabSize".into() }, 200);
        log.record(WorkspaceEventKind::FolderRemoved { uri: "/old".into() }, 300);
        log.record(
            WorkspaceEventKind::FolderRenamed {
                uri: "/src".into(),
                old_name: "source".into(),
                new_name: "src-code".into(),
            },
            400,
        );
        log.record(
            WorkspaceEventKind::NameChanged {
                old: Some("old-ws".into()),
                new: Some("new-ws".into()),
            },
            500,
        );

        assert_eq!(log.len(), 5);
        assert!(!log.is_empty());

        // Filter folder events only.
        let folder_events = log.folder_events();
        assert_eq!(folder_events.len(), 3);

        // Filter by timestamp.
        let since_300 = log.events_since(300);
        assert_eq!(since_300.len(), 3);
        assert_eq!(since_300[0].timestamp, 300);
    }


    #[test]
    fn recommendations_basic() {
        let mut recs = WorkspaceRecommendations::new();
        recs.recommend_for_files(&["main.rs", "Cargo.toml"]);
        assert!(!recs.is_empty());
        assert!(recs.list().iter().any(|r| r.extension_id == "rust-analyzer"));
    }

    #[test]
    fn recommendations_dedup() {
        let mut recs = WorkspaceRecommendations::new();
        recs.recommend_for_files(&["a.rs", "b.rs"]);
        assert_eq!(recs.list().iter().filter(|r| r.extension_id == "rust-analyzer").count(), 1);
    }

    #[test]
    fn task_runner_basic() {
        let mut tr = WorkspaceTaskRunnerConfig::new();
        tr.add_task("build", "cargo build");
        tr.add_task("test", "cargo test");
        assert_eq!(tr.get_command("build"), Some("cargo build"));
        assert_eq!(tr.len(), 2);
    }

    #[test]
    fn task_runner_remove() {
        let mut tr = WorkspaceTaskRunnerConfig::new();
        tr.add_task("build", "cargo build");
        assert!(tr.remove_task("build"));
        assert!(tr.is_empty());
    }

    #[test]
    fn search_scope_basic() {
        let mut scope = WorkspaceSearchScope::new();
        scope.include("src");
        scope.exclude("target");
        assert!(scope.matches("src/main.rs"));
        assert!(!scope.matches("target/debug"));
    }

    #[test]
    fn search_scope_all() {
        let scope = WorkspaceSearchScope::new();
        assert!(scope.matches("anything"));
    }

    #[test]
    fn recent_list_basic() {
        let mut rl = WorkspaceRecentList::new(3);
        rl.add("/project/a", 100);
        rl.add("/project/b", 200);
        rl.add("/project/c", 300);
        assert_eq!(rl.len(), 3);
        assert_eq!(rl.recent()[0], "/project/c");
    }

    #[test]
    fn recent_list_max() {
        let mut rl = WorkspaceRecentList::new(2);
        rl.add("a", 1);
        rl.add("b", 2);
        rl.add("c", 3);
        assert_eq!(rl.len(), 2);
    }

    #[test]
    fn recent_list_dedup() {
        let mut rl = WorkspaceRecentList::new(10);
        rl.add("a", 1);
        rl.add("b", 2);
        rl.add("a", 3);
        assert_eq!(rl.len(), 2);
        assert_eq!(rl.recent()[0], "a");
    }

    #[test]
    fn recent_list_sorted_by_name() {
        let mut rl = WorkspaceRecentList::new(10);
        rl.add("b", 1);
        rl.add("a", 2);
        assert_eq!(rl.sorted_by_name()[0], "a");
    }

    #[test]
    fn recent_list_remove() {
        let mut rl = WorkspaceRecentList::new(10);
        rl.add("a", 1);
        assert!(rl.remove("a"));
        assert!(rl.is_empty());
    }

    #[test]
    fn recommendation_display() {
        let r = WorkspaceRecommendation::new("ext", "reason");
        assert!(format!("{r}").contains("ext"));
    }


    #[test]
    fn workspaceRecommendationEngine_new() {
        let s = WorkspaceRecommendationEngine::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn workspaceRecommendationEngine_add_contains() {
        let mut s = WorkspaceRecommendationEngine::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn workspaceRecommendationEngine_add_duplicate() {
        let mut s = WorkspaceRecommendationEngine::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn workspaceRecommendationEngine_remove() {
        let mut s = WorkspaceRecommendationEngine::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn workspaceRecommendationEngine_capacity() {
        let s = WorkspaceRecommendationEngine::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn workspaceRecommendationEngine_search() {
        let mut s = WorkspaceRecommendationEngine::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn workspaceRecommendationEngine_stats() {
        let mut s = WorkspaceRecommendationEngine::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn workspaceTemplateLoader_new() {
        let m = WorkspaceTemplateLoader::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn workspaceTemplateLoader_add_find() {
        let mut m = WorkspaceTemplateLoader::new();
        m.add(WorkspaceTemplateLoaderItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn workspaceTemplateLoader_priority_filter() {
        let mut m = WorkspaceTemplateLoader::new();
        m.add(WorkspaceTemplateLoaderItem::new("a", "A").with_priority(WorkspaceTemplateLoaderPriority::High));
        m.add(WorkspaceTemplateLoaderItem::new("b", "B").with_priority(WorkspaceTemplateLoaderPriority::Low));
        m.add(WorkspaceTemplateLoaderItem::new("c", "C").with_priority(WorkspaceTemplateLoaderPriority::High));
        assert_eq!(m.by_priority(WorkspaceTemplateLoaderPriority::High).len(), 2);
    }

    #[test]
    fn workspaceTemplateLoader_remove() {
        let mut m = WorkspaceTemplateLoader::new();
        m.add(WorkspaceTemplateLoaderItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn workspaceTemplateLoader_search() {
        let mut m = WorkspaceTemplateLoader::new();
        m.add(WorkspaceTemplateLoaderItem::new("id1", "Hello World"));
        m.add(WorkspaceTemplateLoaderItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn workspaceTemplateLoader_total_weight() {
        let mut m = WorkspaceTemplateLoader::new();
        m.add(WorkspaceTemplateLoaderItem::new("a", "A").with_priority(WorkspaceTemplateLoaderPriority::Critical));
        m.add(WorkspaceTemplateLoaderItem::new("b", "B").with_priority(WorkspaceTemplateLoaderPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn workspaceTemplateLoader_capacity_limit() {
        let mut m = WorkspaceTemplateLoader::new().with_max_items(2);
        m.add(WorkspaceTemplateLoaderItem::new("1", "one"));
        m.add(WorkspaceTemplateLoaderItem::new("2", "two"));
        assert!(!m.add(WorkspaceTemplateLoaderItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn workspaceTemplateLoader_sorted_by_priority() {
        let mut m = WorkspaceTemplateLoader::new();
        m.add(WorkspaceTemplateLoaderItem::new("lo", "Low").with_priority(WorkspaceTemplateLoaderPriority::Low));
        m.add(WorkspaceTemplateLoaderItem::new("hi", "High").with_priority(WorkspaceTemplateLoaderPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn workspaceTemplateLoader_item_metadata() {
        let mut item = WorkspaceTemplateLoaderItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn workspaceRecommendationEngine_enabled_toggle() {
        let mut s = WorkspaceRecommendationEngine::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn workspaceTemplateLoader_priority_display() {
        assert_eq!(format!("{}", WorkspaceTemplateLoaderPriority::High), "high");
        assert_eq!(format!("{}", WorkspaceTemplateLoaderPriority::Low), "low");
    }


    #[test]
    fn wsc_lru_insert_get() {
        let mut c = WsCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn wsc_lru_eviction() {
        let mut c = WsCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn wsc_lru_hit_ratio() {
        let mut c = WsCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn wsc_lru_clear() {
        let mut c = WsCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn wsc_lru_remove() {
        let mut c = WsCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn wsc_lru_peek() {
        let mut c = WsCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn wsb_builder_valid() {
        let cfg = WsBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn wsb_builder_empty_name() {
        let r = WsBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn wsb_builder_bad_priority() {
        assert!(WsBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn wsb_builder_zero_max() {
        assert!(WsBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn wsb_cfg_merge() {
        let mut a = WsBBuilder::new("a").property("x", "1").build().unwrap();
        let b = WsBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn wsb_cfg_display() {
        let cfg = WsBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- workspaces extended domain tests ----------------------------------------

    #[test]
    fn y_workspaces_enum_index() {
        assert_eq!(YWorkspacesWorkspaceTrust::Trusted.index(), 0);
        assert_eq!(YWorkspacesWorkspaceTrust::Untrusted.index(), 1);
        assert_eq!(YWorkspacesWorkspaceTrust::Restricted.index(), 2);
        assert_eq!(YWorkspacesWorkspaceTrust::Unknown.index(), 3);
    }

    #[test]
    fn y_workspaces_enum_label() {
        assert_eq!(YWorkspacesWorkspaceTrust::Trusted.label(), "Trusted");
        assert_eq!(YWorkspacesWorkspaceTrust::Untrusted.label(), "Untrusted");
        assert_eq!(YWorkspacesWorkspaceTrust::Restricted.label(), "Restricted");
        assert_eq!(YWorkspacesWorkspaceTrust::Unknown.label(), "Unknown");
    }

    #[test]
    fn y_workspaces_enum_all() {
        let all = YWorkspacesWorkspaceTrust::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_workspaces_enum_is_default() {
        assert!(YWorkspacesWorkspaceTrust::Trusted.is_default());
        assert!(!YWorkspacesWorkspaceTrust::Unknown.is_default());
    }

    #[test]
    fn y_workspaces_enum_display() {
        assert_eq!(format!("{}", YWorkspacesWorkspaceTrust::Trusted), "Trusted");
    }

    #[test]
    fn y_workspaces_struct_new() {
        let s = YWorkspacesWorkspaceFolder::new();
        let _ = s.summary();
    }

    #[test]
    fn y_workspaces_fingerprint_deterministic() {
        let h1 = y_workspaces_fingerprint("hello");
        let h2 = y_workspaces_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_workspaces_fingerprint("a"), y_workspaces_fingerprint("b"));
    }

    #[test]
    fn y_workspaces_truncate_short() {
        assert_eq!(y_workspaces_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_workspaces_truncate_long() {
        let r = y_workspaces_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_workspaces_normalize_key_basic() {
        assert_eq!(y_workspaces_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_workspaces_split_path_basic() {
        let parts = y_workspaces_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_workspaces_count_occurrences_basic() {
        assert_eq!(y_workspaces_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_workspaces_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_workspaces_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_workspaces_in_range_basic() {
        assert!(y_workspaces_in_range(5, 1, 10));
        assert!(y_workspaces_in_range(1, 1, 10));
        assert!(y_workspaces_in_range(10, 1, 10));
        assert!(!y_workspaces_in_range(0, 1, 10));
        assert!(!y_workspaces_in_range(11, 1, 10));
    }

    #[test]
    fn y_workspaces_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_workspaces_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_workspaces_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_workspaces_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- workspaces Z-extended tests -----------------------------------------------

    #[test]
    fn z_workspaces_priority_weight() {
        assert_eq!(ZWorkspacesPriority::Idle.weight(), 0);
        assert_eq!(ZWorkspacesPriority::Normal.weight(), 2);
        assert_eq!(ZWorkspacesPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_workspaces_priority_label() {
        assert_eq!(ZWorkspacesPriority::Low.label(), "low");
        assert_eq!(ZWorkspacesPriority::High.label(), "high");
    }

    #[test]
    fn z_workspaces_priority_is_elevated() {
        assert!(!ZWorkspacesPriority::Normal.is_elevated());
        assert!(ZWorkspacesPriority::High.is_elevated());
        assert!(ZWorkspacesPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_workspaces_priority_display() {
        assert_eq!(format!("{}", ZWorkspacesPriority::Idle), "idle");
    }

    #[test]
    fn z_workspaces_priority_all_asc() {
        let all = ZWorkspacesPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWorkspacesPriority::Idle);
        assert_eq!(all[4], ZWorkspacesPriority::Realtime);
    }

    #[test]
    fn z_workspaces_struct_new() {
        let s = ZWorkspacesWorkspaceRecent::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_workspaces_struct_toggled_clone() {
        let s = ZWorkspacesWorkspaceRecent::new();
        let t = s.toggled_clone();
        let _ = t.pinned;
    }

    #[test]
    fn z_workspaces_rolling_hash_deterministic() {
        let h1 = z_workspaces_rolling_hash(b"test");
        let h2 = z_workspaces_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_workspaces_rolling_hash(b"a"), z_workspaces_rolling_hash(b"b"));
    }

    #[test]
    fn z_workspaces_pad_to_basic() {
        assert_eq!(z_workspaces_pad_to("hi", 5), "hi   ");
        assert_eq!(z_workspaces_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_workspaces_is_identifier_basic() {
        assert!(z_workspaces_is_identifier("foo_bar"));
        assert!(z_workspaces_is_identifier("abc123"));
        assert!(!z_workspaces_is_identifier(""));
        assert!(!z_workspaces_is_identifier("has space"));
    }

    #[test]
    fn z_workspaces_levenshtein_basic() {
        assert_eq!(z_workspaces_levenshtein("", ""), 0);
        assert_eq!(z_workspaces_levenshtein("abc", "abc"), 0);
        assert_eq!(z_workspaces_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_workspaces_unique_words_basic() {
        let w = z_workspaces_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_workspaces_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_workspaces_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_workspaces_common_prefix_basic() {
        assert_eq!(z_workspaces_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_workspaces_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_workspaces_struct_clear() {
        let mut s = ZWorkspacesWorkspaceRecent::new();
        s.entries.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_workspaces_rolling_hash_empty() {
        let h = z_workspaces_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 241 ----

    #[test]
    fn xc_241_pool_new_empty() {
        let pool: super::Xc241Pool<i32> = super::Xc241Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_241_pool_release_acquire() {
        let mut pool = super::Xc241Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_241_pool_acquire_empty() {
        let mut pool: super::Xc241Pool<i32> = super::Xc241Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_241_pool_full() {
        let mut pool = super::Xc241Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_241_pool_drain() {
        let mut pool = super::Xc241Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_241_pool_stats() {
        let mut pool = super::Xc241Pool::new(8);
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
    fn xc_241_pool_clear() {
        let mut pool = super::Xc241Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_241_pool_shrink() {
        let mut pool = super::Xc241Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_241_pool_default() {
        let pool: super::Xc241Pool<String> = super::Xc241Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_241_pool_extend() {
        let mut pool = super::Xc241Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_241_pool_retain() {
        let mut pool = super::Xc241Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_241_scheduler_round_robin() {
        let mut sched = super::Xc241Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_241_scheduler_empty() {
        let mut sched = super::Xc241Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_241_scheduler_reset() {
        let mut sched = super::Xc241Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_241_scheduler_add_remove() {
        let mut sched = super::Xc241Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_241_scheduler_targets() {
        let sched = super::Xc241Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_241_hash_empty() {
        assert_eq!(super::xc_241_hash(b""), 5381);
    }

    #[test]
    fn xc_241_hash_data() {
        let h = super::xc_241_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_241_hash(b"hello"), h);
    }

    #[test]
    fn xc_241_reverse_str() {
        assert_eq!(super::xc_241_reverse("abc"), "cba");
        assert_eq!(super::xc_241_reverse(""), "");
    }


    // --- xd_23 deepening tests ---

    #[test]
    fn xd_23_sm_initial_state() {
        let sm = Xd23StateMachine::new();
        assert_eq!(sm.current_state(), Xd23State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_23_sm_valid_idle_to_running() {
        let mut sm = Xd23StateMachine::new();
        assert!(sm.transition(Xd23State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd23State::Running);
    }

    #[test]
    fn xd_23_sm_valid_running_to_paused() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        assert!(sm.transition(Xd23State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd23State::Paused);
    }

    #[test]
    fn xd_23_sm_valid_running_to_done() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        assert!(sm.transition(Xd23State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd23State::Done);
    }

    #[test]
    fn xd_23_sm_valid_paused_to_running() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        sm.transition(Xd23State::Paused).unwrap();
        assert!(sm.transition(Xd23State::Running).is_ok());
    }

    #[test]
    fn xd_23_sm_valid_done_to_idle() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        sm.transition(Xd23State::Done).unwrap();
        assert!(sm.transition(Xd23State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd23State::Idle);
    }

    #[test]
    fn xd_23_sm_invalid_idle_to_done() {
        let mut sm = Xd23StateMachine::new();
        assert!(sm.transition(Xd23State::Done).is_err());
    }

    #[test]
    fn xd_23_sm_invalid_idle_to_paused() {
        let mut sm = Xd23StateMachine::new();
        assert!(sm.transition(Xd23State::Paused).is_err());
    }

    #[test]
    fn xd_23_sm_history_tracking() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        sm.transition(Xd23State::Paused).unwrap();
        sm.transition(Xd23State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd23State::Idle);
        assert_eq!(sm.history()[0].to, Xd23State::Running);
        assert_eq!(sm.history()[1].from, Xd23State::Running);
        assert_eq!(sm.history()[2].to, Xd23State::Done);
    }

    #[test]
    fn xd_23_sm_serialize_deserialize() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd23StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd23State::Running));
    }

    #[test]
    fn xd_23_sm_deserialize_invalid() {
        assert_eq!(Xd23StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_23_sm_reset() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd23State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_23_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd23EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd23Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_23_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd23EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd23Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd23Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_23_bus_unsubscribe() {
        let mut bus = Xd23EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_23_event_kind_and_payload() {
        let e = Xd23Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd23Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_23_bus_clear_history() {
        let mut bus = Xd23EventBus::new();
        bus.publish(Xd23Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_23_sm_step_counter_increments() {
        let mut sm = Xd23StateMachine::new();
        sm.transition(Xd23State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd23State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #21 --

    #[test]
    fn xf21_trie_insert_search() {
        let mut t = Xf21Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf21_trie_starts_with() {
        let mut t = Xf21Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf21_trie_remove() {
        let mut t = Xf21Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf21_trie_word_count() {
        let mut t = Xf21Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf21_trie_longest_prefix() {
        let mut t = Xf21Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf21_trie_all_words() {
        let mut t = Xf21Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf21_trie_autocomplete() {
        let mut t = Xf21Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf21_trie_empty_search() {
        let t = Xf21Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf21_bloom_add_contains() {
        let mut bf = Xf21BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf21_bloom_probably_absent() {
        let bf = Xf21BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf21_bloom_false_positive_rate() {
        let mut bf = Xf21BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf21_bloom_clear() {
        let mut bf = Xf21BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf21_bloom_union() {
        let mut a = Xf21BloomFilter::xf_new(512, 2);
        let mut b = Xf21BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf21_bloom_intersection_estimate() {
        let mut a = Xf21BloomFilter::xf_new(512, 2);
        let mut b = Xf21BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf21_bloom_union_size_mismatch() {
        let a = Xf21BloomFilter::xf_new(256, 2);
        let b = Xf21BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh240_skip_insert_contains() {
        let mut sl = super::Xh240SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh240_skip_remove() {
        let mut sl = super::Xh240SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh240_skip_len() {
        let mut sl = super::Xh240SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh240_skip_range_query() {
        let mut sl = super::Xh240SkipList::xh_new(4);
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
    fn xh240_skip_floor_ceiling() {
        let mut sl = super::Xh240SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh240_skip_rank() {
        let mut sl = super::Xh240SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh240_skip_empty() {
        let sl = super::Xh240SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh240_skip_duplicates() {
        let mut sl = super::Xh240SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh240_bitset_set_test() {
        let mut bs = super::Xh240BitSet::xh_new(256);
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
    fn xh240_bitset_clear_count() {
        let mut bs = super::Xh240BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh240_bitset_and_or_xor() {
        let mut a = super::Xh240BitSet::xh_new(128);
        let mut b = super::Xh240BitSet::xh_new(128);
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
    fn xh240_bitset_iter_ones() {
        let mut bs = super::Xh240BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh240_bitset_first_last() {
        let mut bs = super::Xh240BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh240_bitset_empty() {
        let bs = super::Xh240BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi240_deque_push_pop_back() {
        let mut dq = super::Xi240Deque::xi_new(4);
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
    fn xi240_deque_push_pop_front() {
        let mut dq = super::Xi240Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi240_deque_mixed_ops() {
        let mut dq = super::Xi240Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi240_deque_get_and_split() {
        let mut dq = super::Xi240Deque::xi_new(8);
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
    fn xi240_deque_rotate_left() {
        let mut dq = super::Xi240Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi240_deque_rotate_right() {
        let mut dq = super::Xi240Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi240_deque_grow() {
        let mut dq = super::Xi240Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi240_deque_empty() {
        let dq = super::Xi240Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi240_interval_tree_insert_query() {
        let mut tree = super::Xi240IntervalTree::xi_new();
        tree.xi_insert(super::Xi240Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi240Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi240Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi240_interval_tree_overlap() {
        let mut tree = super::Xi240IntervalTree::xi_new();
        tree.xi_insert(super::Xi240Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi240Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi240Interval::xi_new(12, 20));
        let q = super::Xi240Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi240_interval_tree_remove() {
        let mut tree = super::Xi240IntervalTree::xi_new();
        tree.xi_insert(super::Xi240Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi240Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi240_interval_tree_gaps() {
        let mut tree = super::Xi240IntervalTree::xi_new();
        tree.xi_insert(super::Xi240Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi240Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi240Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi240Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi240Interval::xi_new(8, 10));
    }

    #[test]
    fn xi240_interval_tree_merge() {
        let mut tree = super::Xi240IntervalTree::xi_new();
        tree.xi_insert(super::Xi240Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi240Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi240Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi240Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi240Interval::xi_new(10, 15));
    }

    #[test]
    fn xi240_interval_tree_all() {
        let mut tree = super::Xi240IntervalTree::xi_new();
        tree.xi_insert(super::Xi240Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi240Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi240_interval_tree_empty() {
        let tree = super::Xi240IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi240_interval_tree_contains_point() {
        let iv = super::Xi240Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 240) ---

    #[test]
    fn xj_240_uf_make_and_find() {
        let mut uf = super::Xj240UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_240_uf_union_connected() {
        let mut uf = super::Xj240UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_240_uf_component_count() {
        let mut uf = super::Xj240UnionFind::xj_new();
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
    fn xj_240_uf_component_size() {
        let mut uf = super::Xj240UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_240_uf_largest_component() {
        let mut uf = super::Xj240UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_240_uf_many_elements() {
        let mut uf = super::Xj240UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_240_uf_separate_components() {
        let mut uf = super::Xj240UnionFind::xj_new();
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
    fn xj_240_uf_path_compression() {
        let mut uf = super::Xj240UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_240_bt_insert_get() {
        let mut bt = super::Xj240BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_240_bt_contains_len() {
        let mut bt = super::Xj240BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_240_bt_replace() {
        let mut bt = super::Xj240BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_240_bt_remove() {
        let mut bt = super::Xj240BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_240_bt_keys_values() {
        let mut bt = super::Xj240BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_240_bt_range() {
        let mut bt = super::Xj240BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_240_bt_min_max() {
        let mut bt = super::Xj240BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_240_bt_many_inserts() {
        let mut bt = super::Xj240BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_240 segment tree tests ---

    #[test]
    fn xk_240_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk240SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_240_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk240SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_240_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk240SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_240_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk240SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_240_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk240SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_240_st_single_element() {
        let data = vec![42];
        let st = super::Xk240SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_240_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk240SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_240_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk240SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_240 disjoint intervals tests ---

    #[test]
    fn xk_240_di_add_and_count() {
        let mut di = super::Xk240DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_240_di_merge_overlap() {
        let mut di = super::Xk240DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_240_di_contains() {
        let mut di = super::Xk240DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_240_di_remove() {
        let mut di = super::Xk240DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_240_di_covered_length() {
        let mut di = super::Xk240DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_240_di_gaps() {
        let mut di = super::Xk240DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_240_di_merge_adjacent() {
        let mut di = super::Xk240DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_240_di_empty() {
        let di = super::Xk240DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}