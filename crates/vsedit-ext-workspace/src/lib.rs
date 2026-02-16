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

/// Accumulated statistics for ext-workspace operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtWorkspaceStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtWorkspaceStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ExtWorkspaceStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ExtWorkspaceStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtWorkspaceStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtWorkspaceStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-workspace.
#[derive(Debug, Clone)]
pub struct ExtWorkspaceValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtWorkspaceValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ExtWorkspaceValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Folder watcher ──

/// Tracks workspace folder changes.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceFolderWatcher {
    folders: Vec<String>,
}

impl WorkspaceFolderWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_folder(&mut self, uri: impl Into<String>) {
        self.folders.push(uri.into());
    }

    pub fn remove_folder(&mut self, uri: &str) -> bool {
        if let Some(pos) = self.folders.iter().position(|f| f == uri) {
            self.folders.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn has_folder(&self, uri: &str) -> bool {
        self.folders.iter().any(|f| f == uri)
    }

    pub fn folder_count(&self) -> usize {
        self.folders.len()
    }

    pub fn folders(&self) -> &[String] {
        &self.folders
    }

    /// Returns `(added, removed)` compared to another watcher.
    pub fn diff(&self, other: &Self) -> (Vec<String>, Vec<String>) {
        let added: Vec<String> = other
            .folders
            .iter()
            .filter(|f| !self.folders.contains(f))
            .cloned()
            .collect();
        let removed: Vec<String> = self
            .folders
            .iter()
            .filter(|f| !other.folders.contains(f))
            .cloned()
            .collect();
        (added, removed)
    }
}

// ── Workspace symbol ──

/// A symbol found in the workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: String,
    pub uri: String,
    pub line: u32,
    pub col: u32,
}

// ── Symbol index ──

/// Cross-file symbol lookup index.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceSymbolIndex {
    symbols: Vec<WorkspaceSymbol>,
}

impl WorkspaceSymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_symbol(&mut self, sym: WorkspaceSymbol) {
        self.symbols.push(sym);
    }

    /// Case-insensitive substring search on symbol name.
    pub fn search(&self, query: &str) -> Vec<&WorkspaceSymbol> {
        let q = query.to_lowercase();
        self.symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&q))
            .collect()
    }

    pub fn search_by_kind(&self, kind: &str) -> Vec<&WorkspaceSymbol> {
        self.symbols.iter().filter(|s| s.kind == kind).collect()
    }

    pub fn symbols_in_file(&self, uri: &str) -> Vec<&WorkspaceSymbol> {
        self.symbols.iter().filter(|s| s.uri == uri).collect()
    }

    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    pub fn clear(&mut self) {
        self.symbols.clear();
    }
}

// ── Multi-file edit builder ──

/// Builder for batching multi-file edits with validation.
#[derive(Debug, Clone, Default)]
pub struct MultiFileEditBuilder {
    edits: HashMap<String, Vec<TextEditEntry>>,
    creates: Vec<String>,
    deletes: Vec<String>,
    renames: Vec<RenameEntry>,
}

impl MultiFileEditBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace_text(
        &mut self,
        uri: impl Into<String>,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
        new_text: impl Into<String>,
    ) -> &mut Self {
        self.edits.entry(uri.into()).or_default().push(TextEditEntry {
            start_line,
            start_col,
            end_line,
            end_col,
            new_text: new_text.into(),
        });
        self
    }

    pub fn create_file(&mut self, uri: impl Into<String>) -> &mut Self {
        self.creates.push(uri.into());
        self
    }

    pub fn delete_file(&mut self, uri: impl Into<String>) -> &mut Self {
        self.deletes.push(uri.into());
        self
    }

    pub fn rename_file(
        &mut self,
        old: impl Into<String>,
        new: impl Into<String>,
    ) -> &mut Self {
        self.renames.push(RenameEntry {
            old_uri: old.into(),
            new_uri: new.into(),
        });
        self
    }

    pub fn edit_count(&self) -> usize {
        self.edits.values().map(|v| v.len()).sum::<usize>()
            + self.creates.len()
            + self.deletes.len()
            + self.renames.len()
    }

    pub fn file_count(&self) -> usize {
        let mut uris: Vec<&str> = self.edits.keys().map(|s| s.as_str()).collect();
        for r in &self.renames {
            uris.push(&r.old_uri);
            uris.push(&r.new_uri);
        }
        for c in &self.creates {
            uris.push(c);
        }
        for d in &self.deletes {
            uris.push(d);
        }
        uris.sort();
        uris.dedup();
        uris.len()
    }

    pub fn has_edits_for(&self, uri: &str) -> bool {
        self.edits.contains_key(uri)
    }

    /// Consume the builder and produce a [`WorkspaceEdit`].
    pub fn build(self) -> Result<WorkspaceEdit, WorkspaceError> {
        if self.edits.is_empty()
            && self.creates.is_empty()
            && self.deletes.is_empty()
            && self.renames.is_empty()
        {
            return Err(WorkspaceError::EmptyEdit);
        }
        Ok(WorkspaceEdit {
            text_edits: self.edits,
            renames: self.renames,
            creates: self.creates,
            deletes: self.deletes,
        })
    }
}

// ---------------------------------------------------------------------------
// Configuration management
// ---------------------------------------------------------------------------

/// In-memory workspace configuration store.
#[derive(Debug, Clone)]
pub struct WorkspaceConfigStore {
    sections: HashMap<String, Value>,
}

impl WorkspaceConfigStore {
    /// Create an empty configuration store.
    pub fn new() -> Self {
        Self {
            sections: HashMap::new(),
        }
    }

    /// Set a configuration value for a section.
    pub fn set(&mut self, section: impl Into<String>, value: Value) {
        self.sections.insert(section.into(), value);
    }

    /// Get a configuration value by section name.
    pub fn get(&self, section: &str) -> Option<&Value> {
        self.sections.get(section)
    }

    /// Remove a configuration section. Returns the old value if present.
    pub fn remove(&mut self, section: &str) -> Option<Value> {
        self.sections.remove(section)
    }

    /// List all section names.
    pub fn sections(&self) -> Vec<&str> {
        self.sections.keys().map(|s| s.as_str()).collect()
    }

    /// Number of configuration sections.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Check if a section exists.
    pub fn has_section(&self, section: &str) -> bool {
        self.sections.contains_key(section)
    }

    /// Merge another config store into this one (overwrites on conflict).
    pub fn merge(&mut self, other: &WorkspaceConfigStore) {
        for (k, v) in &other.sections {
            self.sections.insert(k.clone(), v.clone());
        }
    }
}

impl Default for WorkspaceConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Workspace edit analysis
// ---------------------------------------------------------------------------

/// Summary of a workspace edit.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceEditSummary {
    pub files_modified: usize,
    pub total_text_edits: usize,
    pub renames: usize,
    pub creates: usize,
    pub deletes: usize,
}

impl fmt::Display for WorkspaceEditSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} files modified, {} edits, {} renames, {} creates, {} deletes",
            self.files_modified,
            self.total_text_edits,
            self.renames,
            self.creates,
            self.deletes,
        )
    }
}

/// Compute a summary of a workspace edit.
pub fn summarize_workspace_edit(edit: &WorkspaceEdit) -> WorkspaceEditSummary {
    WorkspaceEditSummary {
        files_modified: edit.text_edits.len(),
        total_text_edits: edit.text_edits.values().map(|v| v.len()).sum(),
        renames: edit.renames.len(),
        creates: edit.creates.len(),
        deletes: edit.deletes.len(),
    }
}

// ---------------------------------------------------------------------------
// URI utilities
// ---------------------------------------------------------------------------

/// Extract the file extension from a workspace URI.
pub fn uri_extension(uri: &str) -> Option<&str> {
    let path = uri.strip_prefix("file://")?;
    let last_segment = path.rsplit('/').next()?;
    let dot_pos = last_segment.rfind('.')?;
    Some(&last_segment[dot_pos + 1..])
}

/// Extract the file name (last segment) from a workspace URI.
pub fn uri_filename(uri: &str) -> Option<&str> {
    let path = if let Some(stripped) = uri.strip_prefix("file://") {
        stripped
    } else {
        uri
    };
    path.rsplit('/').next()
}

/// Check if a URI matches a simple glob pattern (supports `*` wildcard only).
pub fn uri_matches_glob(uri: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(ext) = pattern.strip_prefix("*.") {
        return uri.ends_with(&format!(".{ext}"));
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return uri.starts_with(prefix);
    }
    uri == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_watcher_add_remove() {
        let mut w = WorkspaceFolderWatcher::new();
        w.add_folder("file:///a");
        w.add_folder("file:///b");
        assert_eq!(w.folder_count(), 2);
        assert!(w.remove_folder("file:///a"));
        assert_eq!(w.folder_count(), 1);
        assert!(!w.remove_folder("file:///missing"));
    }

    #[test]
    fn test_folder_watcher_has_folder() {
        let mut w = WorkspaceFolderWatcher::new();
        w.add_folder("file:///x");
        assert!(w.has_folder("file:///x"));
        assert!(!w.has_folder("file:///y"));
    }

    #[test]
    fn test_folder_watcher_diff() {
        let mut a = WorkspaceFolderWatcher::new();
        a.add_folder("file:///1");
        a.add_folder("file:///2");
        let mut b = WorkspaceFolderWatcher::new();
        b.add_folder("file:///2");
        b.add_folder("file:///3");
        let (added, removed) = a.diff(&b);
        assert_eq!(added, vec!["file:///3".to_string()]);
        assert_eq!(removed, vec!["file:///1".to_string()]);
    }

    #[test]
    fn test_symbol_index_search() {
        let mut idx = WorkspaceSymbolIndex::new();
        idx.add_symbol(WorkspaceSymbol {
            name: "MyFunction".into(),
            kind: "function".into(),
            uri: "file:///a.rs".into(),
            line: 1,
            col: 0,
        });
        idx.add_symbol(WorkspaceSymbol {
            name: "OtherStruct".into(),
            kind: "struct".into(),
            uri: "file:///b.rs".into(),
            line: 10,
            col: 0,
        });
        let results = idx.search("myfunc");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyFunction");
    }

    #[test]
    fn test_symbol_index_search_by_kind() {
        let mut idx = WorkspaceSymbolIndex::new();
        idx.add_symbol(WorkspaceSymbol {
            name: "Foo".into(),
            kind: "struct".into(),
            uri: "file:///a.rs".into(),
            line: 1,
            col: 0,
        });
        idx.add_symbol(WorkspaceSymbol {
            name: "bar".into(),
            kind: "function".into(),
            uri: "file:///a.rs".into(),
            line: 5,
            col: 0,
        });
        assert_eq!(idx.search_by_kind("struct").len(), 1);
        assert_eq!(idx.search_by_kind("function").len(), 1);
        assert_eq!(idx.search_by_kind("enum").len(), 0);
    }

    #[test]
    fn test_symbol_index_symbols_in_file() {
        let mut idx = WorkspaceSymbolIndex::new();
        idx.add_symbol(WorkspaceSymbol {
            name: "A".into(),
            kind: "struct".into(),
            uri: "file:///a.rs".into(),
            line: 1,
            col: 0,
        });
        idx.add_symbol(WorkspaceSymbol {
            name: "B".into(),
            kind: "struct".into(),
            uri: "file:///b.rs".into(),
            line: 1,
            col: 0,
        });
        assert_eq!(idx.symbols_in_file("file:///a.rs").len(), 1);
        assert_eq!(idx.symbols_in_file("file:///c.rs").len(), 0);
    }

    #[test]
    fn test_symbol_index_clear() {
        let mut idx = WorkspaceSymbolIndex::new();
        idx.add_symbol(WorkspaceSymbol {
            name: "X".into(),
            kind: "function".into(),
            uri: "file:///a.rs".into(),
            line: 1,
            col: 0,
        });
        assert_eq!(idx.symbol_count(), 1);
        idx.clear();
        assert_eq!(idx.symbol_count(), 0);
    }

    #[test]
    fn test_multi_file_edit_builder_basic() {
        let mut b = MultiFileEditBuilder::new();
        b.replace_text("file:///a.rs", 0, 0, 0, 5, "hello");
        let edit = b.build().unwrap();
        assert_eq!(edit.text_edits.len(), 1);
        assert_eq!(edit.text_edits["file:///a.rs"][0].new_text, "hello");
    }

    #[test]
    fn test_multi_file_edit_builder_empty_error() {
        let b = MultiFileEditBuilder::new();
        assert_eq!(b.build().unwrap_err(), WorkspaceError::EmptyEdit);
    }

    #[test]
    fn test_multi_file_edit_builder_has_edits() {
        let mut b = MultiFileEditBuilder::new();
        b.replace_text("file:///a.rs", 0, 0, 0, 5, "x");
        assert!(b.has_edits_for("file:///a.rs"));
        assert!(!b.has_edits_for("file:///b.rs"));
    }

    #[test]
    fn test_multi_file_edit_builder_file_ops() {
        let mut b = MultiFileEditBuilder::new();
        b.create_file("file:///new.rs")
            .delete_file("file:///old.rs")
            .rename_file("file:///src.rs", "file:///dst.rs");
        assert_eq!(b.edit_count(), 3);
        let edit = b.build().unwrap();
        assert_eq!(edit.creates, vec!["file:///new.rs".to_string()]);
        assert_eq!(edit.deletes, vec!["file:///old.rs".to_string()]);
        assert_eq!(edit.renames.len(), 1);
    }

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

    #[test]
    fn ext_workspace_stats_new_defaults() {
        let stats = ExtWorkspaceStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_workspace_stats_record_success() {
        let mut stats = ExtWorkspaceStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_workspace_stats_record_failure() {
        let mut stats = ExtWorkspaceStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_workspace_stats_reset() {
        let mut stats = ExtWorkspaceStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_workspace_stats_merge() {
        let mut a = ExtWorkspaceStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtWorkspaceStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn ext_workspace_stats_display() {
        let mut stats = ExtWorkspaceStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_workspace_stats_default() {
        let stats = ExtWorkspaceStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_workspace_validator_accepts_valid_name() {
        let v = ExtWorkspaceValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_workspace_validator_rejects_empty() {
        let v = ExtWorkspaceValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_workspace_validator_rejects_too_long() {
        let v = ExtWorkspaceValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_workspace_validator_forbidden_prefix() {
        let v = ExtWorkspaceValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_workspace_validator_allowed_chars() {
        let v = ExtWorkspaceValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_workspace_validator_range() {
        let v = ExtWorkspaceValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_workspace_sanitize_removes_control() {
        let result = ExtWorkspaceValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_workspace_truncate_short_string() {
        assert_eq!(ExtWorkspaceValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_workspace_truncate_long_string() {
        let result = ExtWorkspaceValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_workspace_is_ascii_printable() {
        assert!(ExtWorkspaceValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtWorkspaceValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- new tests --

    #[test]
    fn config_store_set_and_get() {
        let mut store = WorkspaceConfigStore::new();
        store.set("editor.fontSize", serde_json::json!(14));
        assert_eq!(store.get("editor.fontSize"), Some(&serde_json::json!(14)));
        assert!(store.has_section("editor.fontSize"));
        assert_eq!(store.section_count(), 1);
    }

    #[test]
    fn config_store_remove() {
        let mut store = WorkspaceConfigStore::new();
        store.set("a", serde_json::json!("value"));
        assert!(store.remove("a").is_some());
        assert!(store.get("a").is_none());
        assert!(store.remove("nonexistent").is_none());
    }

    #[test]
    fn config_store_merge() {
        let mut s1 = WorkspaceConfigStore::new();
        s1.set("a", serde_json::json!(1));
        let mut s2 = WorkspaceConfigStore::new();
        s2.set("b", serde_json::json!(2));
        s2.set("a", serde_json::json!(99));
        s1.merge(&s2);
        assert_eq!(s1.section_count(), 2);
        assert_eq!(s1.get("a"), Some(&serde_json::json!(99)));
    }

    #[test]
    fn summarize_workspace_edit_counts() {
        let edit = WorkspaceEditBuilder::new()
            .text_edit("file:///a.rs", TextEditEntry::single_line(1, 1, 5, "hello"))
            .create("file:///new.rs")
            .delete("file:///old.rs")
            .rename("file:///a.rs", "file:///b.rs")
            .build()
            .unwrap();
        let summary = summarize_workspace_edit(&edit);
        assert_eq!(summary.files_modified, 1);
        assert_eq!(summary.total_text_edits, 1);
        assert_eq!(summary.creates, 1);
        assert_eq!(summary.deletes, 1);
        assert_eq!(summary.renames, 1);
    }

    #[test]
    fn workspace_edit_summary_display() {
        let s = WorkspaceEditSummary {
            files_modified: 2,
            total_text_edits: 5,
            renames: 1,
            creates: 0,
            deletes: 0,
        };
        let text = format!("{s}");
        assert!(text.contains("2 files modified"));
        assert!(text.contains("5 edits"));
    }

    #[test]
    fn uri_extension_extracts() {
        assert_eq!(uri_extension("file:///src/main.rs"), Some("rs"));
        assert_eq!(uri_extension("file:///README"), None);
    }

    #[test]
    fn uri_filename_extracts() {
        assert_eq!(uri_filename("file:///src/main.rs"), Some("main.rs"));
        assert_eq!(uri_filename("/a/b/c.txt"), Some("c.txt"));
    }

    #[test]
    fn uri_matches_glob_wildcard_ext() {
        assert!(uri_matches_glob("file:///src/main.rs", "*.rs"));
        assert!(!uri_matches_glob("file:///src/main.ts", "*.rs"));
    }

    #[test]
    fn uri_matches_glob_star() {
        assert!(uri_matches_glob("anything", "*"));
    }

    #[test]
    fn uri_matches_glob_prefix() {
        assert!(uri_matches_glob("file:///src/lib.rs", "file:///src/*"));
        assert!(!uri_matches_glob("file:///other/lib.rs", "file:///src/*"));
    }
}
