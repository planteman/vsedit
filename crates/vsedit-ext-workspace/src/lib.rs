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

// ---------------------------------------------------------------------------
// Workspace folder prioritization
// ---------------------------------------------------------------------------

/// Priority level for workspace folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FolderPriority {
    /// Low priority – auxiliary or reference folders.
    Low = 0,
    /// Normal priority – standard workspace folders.
    Normal = 1,
    /// High priority – primary working folders.
    High = 2,
}

impl Default for FolderPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// A workspace folder with an associated priority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrioritizedFolder {
    pub uri: String,
    pub priority: FolderPriority,
}

/// Manages workspace folders with priority ordering.
#[derive(Debug, Clone, Default)]
pub struct FolderPriorityManager {
    folders: Vec<PrioritizedFolder>,
}

impl FolderPriorityManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a folder with the given priority.
    pub fn add(&mut self, uri: impl Into<String>, priority: FolderPriority) {
        let uri = uri.into();
        if !self.folders.iter().any(|f| f.uri == uri) {
            self.folders.push(PrioritizedFolder { uri, priority });
        }
    }

    /// Update the priority of an existing folder. Returns `false` if not found.
    pub fn set_priority(&mut self, uri: &str, priority: FolderPriority) -> bool {
        if let Some(f) = self.folders.iter_mut().find(|f| f.uri == uri) {
            f.priority = priority;
            true
        } else {
            false
        }
    }

    /// Return folders sorted by descending priority (highest first).
    pub fn sorted(&self) -> Vec<&PrioritizedFolder> {
        let mut sorted: Vec<&PrioritizedFolder> = self.folders.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    /// Return only folders matching the given priority.
    pub fn by_priority(&self, priority: FolderPriority) -> Vec<&PrioritizedFolder> {
        self.folders.iter().filter(|f| f.priority == priority).collect()
    }

    /// Remove a folder by URI. Returns `true` if removed.
    pub fn remove(&mut self, uri: &str) -> bool {
        let before = self.folders.len();
        self.folders.retain(|f| f.uri != uri);
        self.folders.len() < before
    }

    pub fn len(&self) -> usize {
        self.folders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Workspace trust management
// ---------------------------------------------------------------------------

/// Trust level assigned to a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Untrusted,
    Restricted,
    Trusted,
}

/// Manages per-folder trust decisions.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceTrustManager {
    trust: HashMap<String, TrustLevel>,
}

impl WorkspaceTrustManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the trust level for a workspace folder URI.
    pub fn set_trust(&mut self, uri: impl Into<String>, level: TrustLevel) {
        self.trust.insert(uri.into(), level);
    }

    /// Get the trust level for a folder. Returns `Untrusted` if unknown.
    pub fn trust_level(&self, uri: &str) -> TrustLevel {
        self.trust.get(uri).copied().unwrap_or(TrustLevel::Untrusted)
    }

    /// Returns `true` if the folder is fully trusted.
    pub fn is_trusted(&self, uri: &str) -> bool {
        self.trust_level(uri) == TrustLevel::Trusted
    }

    /// Returns `true` if any workspace folder is untrusted.
    pub fn has_untrusted(&self) -> bool {
        self.trust.values().any(|t| *t == TrustLevel::Untrusted)
    }

    /// Return the count of folders at each trust level.
    pub fn summary(&self) -> (usize, usize, usize) {
        let mut untrusted = 0;
        let mut restricted = 0;
        let mut trusted = 0;
        for t in self.trust.values() {
            match t {
                TrustLevel::Untrusted => untrusted += 1,
                TrustLevel::Restricted => restricted += 1,
                TrustLevel::Trusted => trusted += 1,
            }
        }
        (untrusted, restricted, trusted)
    }
}

// ---------------------------------------------------------------------------
// Workspace event coalescing
// ---------------------------------------------------------------------------

/// Kind of file-system event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FsEventKind {
    Created,
    Changed,
    Deleted,
}

/// A coalesced file-system event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoalescedEvent {
    pub uri: String,
    pub kind: FsEventKind,
}

/// Accumulates raw file-system events and coalesces them so that redundant
/// notifications are collapsed (e.g. Created+Changed → Created, Changed+Deleted → Deleted).
#[derive(Debug, Clone, Default)]
pub struct EventCoalescer {
    events: HashMap<String, FsEventKind>,
}

impl EventCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a raw event into the coalescer.
    pub fn push(&mut self, uri: impl Into<String>, kind: FsEventKind) {
        let uri = uri.into();
        let merged = match (self.events.get(&uri), kind) {
            // A create followed by a change is still just a create.
            (Some(FsEventKind::Created), FsEventKind::Changed) => FsEventKind::Created,
            // A create followed by a delete cancels out.
            (Some(FsEventKind::Created), FsEventKind::Deleted) => {
                self.events.remove(&uri);
                return;
            }
            // A change followed by a delete is just a delete.
            (Some(FsEventKind::Changed), FsEventKind::Deleted) => FsEventKind::Deleted,
            // Otherwise the latest event wins.
            (_, k) => k,
        };
        self.events.insert(uri, merged);
    }

    /// Drain the coalesced events and return them.
    pub fn drain(&mut self) -> Vec<CoalescedEvent> {
        let mut out: Vec<CoalescedEvent> = self
            .events
            .drain()
            .map(|(uri, kind)| CoalescedEvent { uri, kind })
            .collect();
        out.sort_by(|a, b| a.uri.cmp(&b.uri));
        out
    }

    /// Number of distinct URIs with pending events.
    pub fn pending_count(&self) -> usize {
        self.events.len()
    }
}

// ---------------------------------------------------------------------------
// WorkspaceFileWatcher – glob-based file watching
// ---------------------------------------------------------------------------

/// A file watcher that matches paths against glob patterns.
pub struct GlobFileWatcher {
    patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    events: Vec<(String, FsEventKind)>,
}

impl GlobFileWatcher {
    /// Create a new watcher with no patterns.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Add an include glob pattern (e.g. `"**/*.rs"`).
    pub fn add_pattern(&mut self, pattern: impl Into<String>) {
        self.patterns.push(pattern.into());
    }

    /// Add an exclude glob pattern (e.g. `"**/node_modules/**"`).
    pub fn add_exclude(&mut self, pattern: impl Into<String>) {
        self.exclude_patterns.push(pattern.into());
    }

    /// Check if a path matches any include pattern using simple glob matching.
    pub fn matches(&self, path: &str) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let included = self.patterns.iter().any(|p| simple_glob_match(p, path));
        let excluded = self.exclude_patterns.iter().any(|p| simple_glob_match(p, path));
        included && !excluded
    }

    /// Record an event for a path, only if it matches the patterns.
    pub fn record_event(&mut self, path: &str, kind: FsEventKind) -> bool {
        if self.matches(path) {
            self.events.push((path.to_string(), kind));
            true
        } else {
            false
        }
    }

    /// Drain all recorded events.
    pub fn drain_events(&mut self) -> Vec<(String, FsEventKind)> {
        std::mem::take(&mut self.events)
    }

    /// Number of include patterns.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

/// Simple glob match: supports `*` for single segment and `**` for recursive.
fn simple_glob_match(pattern: &str, path: &str) -> bool {
    if pattern == "**" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return path.ends_with(suffix) || path.contains(&format!("/{}", suffix));
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix) || path.contains(&format!("{}/", prefix));
    }
    if let Some(ext) = pattern.strip_prefix("*.") {
        return path.ends_with(&format!(".{}", ext));
    }
    pattern == path
}

// ---------------------------------------------------------------------------
// WorkspaceSearchScope – limit search to specific folders/patterns
// ---------------------------------------------------------------------------

/// Defines a search scope within the workspace.
pub struct WorkspaceSearchScope {
    include_folders: Vec<String>,
    include_globs: Vec<String>,
    exclude_globs: Vec<String>,
    max_results: Option<u32>,
}

impl WorkspaceSearchScope {
    /// Create a scope that searches everywhere.
    pub fn all() -> Self {
        Self {
            include_folders: Vec::new(),
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            max_results: None,
        }
    }

    /// Restrict search to specific folders.
    pub fn with_folders(mut self, folders: Vec<String>) -> Self {
        self.include_folders = folders;
        self
    }

    /// Add include globs (e.g. `"*.rs"`).
    pub fn with_include_globs(mut self, globs: Vec<String>) -> Self {
        self.include_globs = globs;
        self
    }

    /// Add exclude globs.
    pub fn with_exclude_globs(mut self, globs: Vec<String>) -> Self {
        self.exclude_globs = globs;
        self
    }

    /// Set a maximum number of results.
    pub fn with_max_results(mut self, max: u32) -> Self {
        self.max_results = Some(max);
        self
    }

    /// Check if a file path is within this scope.
    pub fn includes(&self, path: &str) -> bool {
        let folder_ok = self.include_folders.is_empty()
            || self.include_folders.iter().any(|f| path.starts_with(f));
        let glob_ok = self.include_globs.is_empty()
            || self.include_globs.iter().any(|g| simple_glob_match(g, path));
        let not_excluded = !self.exclude_globs.iter().any(|g| simple_glob_match(g, path));
        folder_ok && glob_ok && not_excluded
    }

    /// Maximum results, if set.
    pub fn max_results(&self) -> Option<u32> {
        self.max_results
    }
}

// ---------------------------------------------------------------------------
// Multi-root workspace folder ordering
// ---------------------------------------------------------------------------

/// Manages ordering of workspace folders in a multi-root workspace.
pub struct WorkspaceFolderOrdering {
    folders: Vec<String>,
}

impl WorkspaceFolderOrdering {
    /// Create from a list of folders in order.
    pub fn new(folders: Vec<String>) -> Self {
        Self { folders }
    }

    /// Move a folder to a specific position (0-indexed).
    pub fn move_to(&mut self, folder_uri: &str, position: usize) -> bool {
        if let Some(idx) = self.folders.iter().position(|f| f == folder_uri) {
            let folder = self.folders.remove(idx);
            let pos = position.min(self.folders.len());
            self.folders.insert(pos, folder);
            true
        } else {
            false
        }
    }

    /// Swap two folders by index.
    pub fn swap(&mut self, a: usize, b: usize) -> bool {
        if a < self.folders.len() && b < self.folders.len() {
            self.folders.swap(a, b);
            true
        } else {
            false
        }
    }

    /// Get the ordered list of folders.
    pub fn ordered(&self) -> &[String] {
        &self.folders
    }

    /// Find the index of a folder.
    pub fn index_of(&self, folder_uri: &str) -> Option<usize> {
        self.folders.iter().position(|f| f == folder_uri)
    }

    /// Number of folders.
    pub fn len(&self) -> usize {
        self.folders.len()
    }

    /// Whether there are no folders.
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }
}

// ── Bulk file operations builder ──

/// Represents a single bulk file operation.
#[derive(Debug, Clone, PartialEq)]
pub enum BulkFileOp {
    CreateFile { path: String, content: String },
    DeleteFile { path: String },
    RenameFile { old: String, new: String },
    TextEdit { path: String, line: usize, old_text: String, new_text: String },
}

impl fmt::Display for BulkFileOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateFile { path, .. } => write!(f, "Create({path})"),
            Self::DeleteFile { path } => write!(f, "Delete({path})"),
            Self::RenameFile { old, new } => write!(f, "Rename({old} -> {new})"),
            Self::TextEdit { path, line, .. } => write!(f, "TextEdit({path}:{line})"),
        }
    }
}

/// Builds a list of bulk file operations for batch application.
#[derive(Debug, Clone, Default)]
pub struct BulkFileEditBuilder {
    ops: Vec<BulkFileOp>,
}

impl BulkFileEditBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a file creation.
    pub fn create_file(&mut self, path: &str, content: &str) {
        self.ops.push(BulkFileOp::CreateFile {
            path: path.to_string(),
            content: content.to_string(),
        });
    }

    /// Queue a file deletion.
    pub fn delete_file(&mut self, path: &str) {
        self.ops.push(BulkFileOp::DeleteFile {
            path: path.to_string(),
        });
    }

    /// Queue a file rename.
    pub fn rename_file(&mut self, old_path: &str, new_path: &str) {
        self.ops.push(BulkFileOp::RenameFile {
            old: old_path.to_string(),
            new: new_path.to_string(),
        });
    }

    /// Queue a text replacement at a specific line.
    pub fn replace_text(&mut self, path: &str, line: usize, old_text: &str, new_text: &str) {
        self.ops.push(BulkFileOp::TextEdit {
            path: path.to_string(),
            line,
            old_text: old_text.to_string(),
            new_text: new_text.to_string(),
        });
    }

    /// Return all queued edits.
    pub fn edits(&self) -> &[BulkFileOp] {
        &self.ops
    }

    /// Number of queued edits.
    pub fn edit_count(&self) -> usize {
        self.ops.len()
    }

    /// Whether any edits have been queued.
    pub fn has_edits(&self) -> bool {
        !self.ops.is_empty()
    }
}

impl fmt::Display for BulkFileEditBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BulkFileEditBuilder({} ops)", self.ops.len())
    }
}

// ── Workspace config editor ──

/// Editor for workspace configuration as a key-value map of JSON values.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceConfigEditor {
    entries: HashMap<String, serde_json::Value>,
}

impl WorkspaceConfigEditor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a configuration value.
    pub fn set(&mut self, key: &str, value: serde_json::Value) {
        self.entries.insert(key.to_string(), value);
    }

    /// Get a configuration value by key.
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.entries.get(key)
    }

    /// Remove a configuration value, returning it if it existed.
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.entries.remove(key)
    }

    /// Serialize all entries to a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.entries).unwrap_or_else(|_| "{}".to_string())
    }

    /// List all configuration keys.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|k| k.as_str()).collect()
    }

    /// Number of configuration entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the configuration is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for WorkspaceConfigEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkspaceConfigEditor({} entries)", self.entries.len())
    }
}

// ── Workspace symbol registry ──

/// A registered symbol provider with its target language.
#[derive(Debug, Clone, PartialEq)]
struct SymbolProvider {
    name: String,
    language: String,
}

/// Registry for workspace symbol providers, indexed by language.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceSymbolRegistry {
    providers: Vec<SymbolProvider>,
}

impl WorkspaceSymbolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a symbol provider for a language.
    pub fn register_provider(&mut self, name: &str, language: &str) {
        self.providers.push(SymbolProvider {
            name: name.to_string(),
            language: language.to_string(),
        });
    }

    /// Return all provider names registered for the given language.
    pub fn providers_for_language(&self, language: &str) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|p| p.language == language)
            .map(|p| p.name.as_str())
            .collect()
    }

    /// Return all (name, language) pairs.
    pub fn all_providers(&self) -> Vec<(&str, &str)> {
        self.providers
            .iter()
            .map(|p| (p.name.as_str(), p.language.as_str()))
            .collect()
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Whether a provider with the given name is registered.
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.iter().any(|p| p.name == name)
    }
}

impl fmt::Display for WorkspaceSymbolRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WorkspaceSymbolRegistry({} providers)", self.providers.len())
    }
}

// ── Workspace diagnostics summary ──

/// Severity level for a diagnostic entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for DiagSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

/// A single diagnostic entry tied to a file.
#[derive(Debug, Clone, PartialEq)]
struct DiagEntry {
    file: String,
    severity: DiagSeverity,
    message: String,
}

/// Accumulates diagnostics across a workspace and provides summary queries.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceDiagnosticsSummary {
    entries: Vec<DiagEntry>,
}

impl WorkspaceDiagnosticsSummary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a diagnostic for a file.
    pub fn add_diagnostic(&mut self, file: &str, severity: DiagSeverity, message: &str) {
        self.entries.push(DiagEntry {
            file: file.to_string(),
            severity,
            message: message.to_string(),
        });
    }

    /// Count of diagnostics with `Error` severity.
    pub fn error_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity == DiagSeverity::Error).count()
    }

    /// Count of diagnostics with `Warning` severity.
    pub fn warning_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity == DiagSeverity::Warning).count()
    }

    /// Count of diagnostics with `Info` severity.
    pub fn info_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity == DiagSeverity::Info).count()
    }

    /// Unique files that have at least one `Error` diagnostic.
    pub fn files_with_errors(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self
            .entries
            .iter()
            .filter(|e| e.severity == DiagSeverity::Error)
            .map(|e| e.file.as_str())
            .collect();
        files.sort();
        files.dedup();
        files
    }

    /// Total number of diagnostics.
    pub fn total(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are zero diagnostics.
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for WorkspaceDiagnosticsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Diagnostics(errors={}, warnings={}, info={}, total={})",
            self.error_count(),
            self.warning_count(),
            self.info_count(),
            self.total(),
        )
    }
}


// ─── ExtWsC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for workspace files.
#[derive(Debug)]
pub struct ExtWsCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> ExtWsCLruCache<V> {
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

impl<V: Clone + fmt::Display> fmt::Display for ExtWsCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtWsCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── ExtWsB Builder & Validator ─────────────────────────────

/// Builder for constructing extension workspace configurations.
#[derive(Debug, Clone)]
pub struct ExtWsBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl ExtWsBBuilder {
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

    pub fn build(self) -> Result<ExtWsBCfg, ExtWsBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(ExtWsBBuildErr { errors }); }
        Ok(ExtWsBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated extension workspace configuration.
#[derive(Debug, Clone)]
pub struct ExtWsBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl ExtWsBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &ExtWsBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for ExtWsBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtWsBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct ExtWsBBuildErr { pub errors: Vec<String> }

impl fmt::Display for ExtWsBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtWsBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for ExtWsBBuildErr {}


/// Configuration manager for ext_workspace functionality.
pub struct ExtWorkspaceConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtWorkspaceConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &ExtWorkspaceConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_workspace operations.
pub struct ExtWorkspaceRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtWorkspaceRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for ext_workspace.
pub struct ExtWorkspaceValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtWorkspaceValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &ExtWorkspaceValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Workspace folder and configuration — extended utilities (yg)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_ws operations.
#[derive(Debug, Clone)]
pub struct YgMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YgMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for ext_ws.
#[derive(Debug, Clone)]
pub struct YgRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YgRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for ext_ws lookups.
#[derive(Debug, Clone)]
pub struct YgLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YgLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for ext_workspace
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtWorkspaceRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtWorkspaceRingBuf {
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
pub struct XaExtWorkspaceCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtWorkspaceCounter {
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

impl Default for XaExtWorkspaceCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 79
// ---------------------------------------------------------------------------

/// Generic object pool `Xc79Pool<T>`.
pub struct Xc79Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc79Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc79PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc79Pool<T> {
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
    pub fn stats(&self) -> Xc79PoolStats {
        Xc79PoolStats {
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

impl<T> Default for Xc79Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc79Scheduler`.
pub struct Xc79Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc79Scheduler {
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

impl Default for Xc79Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_79 hash for the given byte slice.
pub fn xc_79_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_79 convention.
pub fn xc_79_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_83 deepening: state machine + event bus ---

/// States for the Xd83 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd83State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd83State {
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
pub struct Xd83Transition {
    pub from: Xd83State,
    pub to: Xd83State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd83StateMachine {
    current: Xd83State,
    history: Vec<Xd83Transition>,
    step_counter: usize,
}

impl Xd83StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd83State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd83State {
        self.current
    }

    pub fn history(&self) -> &[Xd83Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd83State) -> Result<Xd83State, String> {
        let allowed = match (self.current, target) {
            (Xd83State::Idle, Xd83State::Running) => true,
            (Xd83State::Running, Xd83State::Paused) => true,
            (Xd83State::Running, Xd83State::Done) => true,
            (Xd83State::Paused, Xd83State::Running) => true,
            (Xd83State::Paused, Xd83State::Done) => true,
            (Xd83State::Done, Xd83State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_83: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd83Transition {
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
            "Xd83SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd83State> {
        let prefix = "Xd83SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd83State::Idle),
            "Running" => Some(Xd83State::Running),
            "Paused" => Some(Xd83State::Paused),
            "Done" => Some(Xd83State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd83State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd83 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd83Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd83Event {
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

type Xd83HandlerFn = Box<dyn Fn(&Xd83Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd83EventBus {
    handlers: Vec<(usize, Option<String>, Xd83HandlerFn)>,
    next_id: usize,
    published: Vec<Xd83Event>,
}

impl Xd83EventBus {
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
        F: Fn(&Xd83Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd83Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd83Event) {
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

    pub fn published_events(&self) -> &[Xd83Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #104
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf104Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf104TrieNode {
    children: std::collections::HashMap<char, Xf104TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf104Trie {
    root: Xf104TrieNode,
    count: usize,
}

impl Xf104Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf104TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf104TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf104TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf104BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf104BloomFilter {
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
    fn extworkspace_validator_accepts_and_rejects() {
        let mut v = ExtWorkspaceValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extworkspace_validator_warnings() {
        let mut v = ExtWorkspaceValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extworkspace_validator_clear_and_merge() {
        let mut v = ExtWorkspaceValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtWorkspaceValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtWorkspaceValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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

    // -- folder prioritization tests --

    #[test]
    fn folder_priority_manager_sorted_order() {
        let mut mgr = FolderPriorityManager::new();
        mgr.add("file:///low", FolderPriority::Low);
        mgr.add("file:///high", FolderPriority::High);
        mgr.add("file:///normal", FolderPriority::Normal);
        let sorted = mgr.sorted();
        assert_eq!(sorted[0].uri, "file:///high");
        assert_eq!(sorted[1].uri, "file:///normal");
        assert_eq!(sorted[2].uri, "file:///low");
    }

    #[test]
    fn folder_priority_manager_set_priority_and_filter() {
        let mut mgr = FolderPriorityManager::new();
        mgr.add("file:///a", FolderPriority::Normal);
        assert!(mgr.set_priority("file:///a", FolderPriority::High));
        assert!(!mgr.set_priority("file:///missing", FolderPriority::Low));
        assert_eq!(mgr.by_priority(FolderPriority::High).len(), 1);
        assert_eq!(mgr.by_priority(FolderPriority::Normal).len(), 0);
    }

    #[test]
    fn folder_priority_manager_no_duplicates() {
        let mut mgr = FolderPriorityManager::new();
        mgr.add("file:///a", FolderPriority::Normal);
        mgr.add("file:///a", FolderPriority::High);
        assert_eq!(mgr.len(), 1);
        // Priority should remain Normal (first add wins).
        assert_eq!(mgr.sorted()[0].priority, FolderPriority::Normal);
    }

    // -- trust management tests --

    #[test]
    fn trust_manager_defaults_to_untrusted() {
        let mgr = WorkspaceTrustManager::new();
        assert_eq!(mgr.trust_level("file:///unknown"), TrustLevel::Untrusted);
        assert!(!mgr.is_trusted("file:///unknown"));
    }

    #[test]
    fn trust_manager_set_and_query() {
        let mut mgr = WorkspaceTrustManager::new();
        mgr.set_trust("file:///proj", TrustLevel::Trusted);
        mgr.set_trust("file:///vendor", TrustLevel::Restricted);
        assert!(mgr.is_trusted("file:///proj"));
        assert!(!mgr.is_trusted("file:///vendor"));
        assert!(!mgr.has_untrusted());
        let (u, r, t) = mgr.summary();
        assert_eq!((u, r, t), (0, 1, 1));
    }

    // -- event coalescing tests --

    #[test]
    fn event_coalescer_create_then_change_stays_created() {
        let mut c = EventCoalescer::new();
        c.push("file:///a.rs", FsEventKind::Created);
        c.push("file:///a.rs", FsEventKind::Changed);
        let events = c.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, FsEventKind::Created);
    }

    #[test]
    fn event_coalescer_create_then_delete_cancels() {
        let mut c = EventCoalescer::new();
        c.push("file:///tmp.rs", FsEventKind::Created);
        c.push("file:///tmp.rs", FsEventKind::Deleted);
        assert_eq!(c.pending_count(), 0);
        assert!(c.drain().is_empty());
    }

    #[test]
    fn event_coalescer_change_then_delete_becomes_deleted() {
        let mut c = EventCoalescer::new();
        c.push("file:///x.rs", FsEventKind::Changed);
        c.push("file:///x.rs", FsEventKind::Deleted);
        let events = c.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, FsEventKind::Deleted);
    }

    // -- GlobFileWatcher tests --

    #[test]
    fn file_watcher_glob_match() {
        let mut w = GlobFileWatcher::new();
        w.add_pattern("*.rs");
        assert!(w.matches("main.rs"));
        assert!(!w.matches("main.py"));
    }

    #[test]
    fn file_watcher_exclude() {
        let mut w = GlobFileWatcher::new();
        w.add_pattern("*.rs");
        w.add_exclude("*.tmp");
        assert!(w.matches("src/main.rs"));
        assert!(!w.matches("cache.tmp"));
    }

    #[test]
    fn file_watcher_record_event() {
        let mut w = GlobFileWatcher::new();
        w.add_pattern("*.rs");
        assert!(w.record_event("lib.rs", FsEventKind::Changed));
        assert!(!w.record_event("lib.py", FsEventKind::Changed));
        assert_eq!(w.drain_events().len(), 1);
    }

    // -- WorkspaceTrustManager tests (existing type) --

    #[test]
    fn trust_manager_new_defaults_untrusted() {
        let tm = WorkspaceTrustManager::new();
        assert_eq!(tm.trust_level("file:///unknown"), TrustLevel::Untrusted);
        assert!(!tm.is_trusted("file:///unknown"));
    }

    #[test]
    fn trust_manager_set_and_check() {
        let mut tm = WorkspaceTrustManager::new();
        tm.set_trust("file:///proj", TrustLevel::Trusted);
        assert!(tm.is_trusted("file:///proj"));
        assert!(!tm.is_trusted("file:///other"));
    }

    #[test]
    fn trust_manager_summary_counts() {
        let mut tm = WorkspaceTrustManager::new();
        tm.set_trust("file:///a", TrustLevel::Trusted);
        tm.set_trust("file:///b", TrustLevel::Restricted);
        let (u, r, t) = tm.summary();
        assert_eq!((u, r, t), (0, 1, 1));
    }

    // -- WorkspaceSearchScope tests --

    #[test]
    fn search_scope_all() {
        let scope = WorkspaceSearchScope::all();
        assert!(scope.includes("anything/here.rs"));
        assert_eq!(scope.max_results(), None);
    }

    #[test]
    fn search_scope_with_folders() {
        let scope = WorkspaceSearchScope::all()
            .with_folders(vec!["src/".into()])
            .with_max_results(100);
        assert!(scope.includes("src/main.rs"));
        assert!(!scope.includes("tests/test.rs"));
        assert_eq!(scope.max_results(), Some(100));
    }

    // -- WorkspaceFolderOrdering tests --

    #[test]
    fn folder_ordering_move_to() {
        let mut o = WorkspaceFolderOrdering::new(vec!["a".into(), "b".into(), "c".into()]);
        assert!(o.move_to("c", 0));
        assert_eq!(o.ordered(), &["c", "a", "b"]);
    }

    #[test]
    fn folder_ordering_swap() {
        let mut o = WorkspaceFolderOrdering::new(vec!["x".into(), "y".into()]);
        assert!(o.swap(0, 1));
        assert_eq!(o.ordered(), &["y", "x"]);
        assert!(!o.swap(0, 5));
    }

    #[test]
    fn folder_ordering_index_of() {
        let o = WorkspaceFolderOrdering::new(vec!["a".into(), "b".into()]);
        assert_eq!(o.index_of("b"), Some(1));
        assert_eq!(o.index_of("z"), None);
        assert_eq!(o.len(), 2);
    }

    // -- BulkFileEditBuilder tests --

    #[test]
    fn bulk_edit_create_and_delete() {
        let mut b = BulkFileEditBuilder::new();
        assert!(!b.has_edits());
        b.create_file("src/new.rs", "fn main() {}");
        b.delete_file("src/old.rs");
        assert_eq!(b.edit_count(), 2);
        assert!(b.has_edits());
        assert_eq!(
            b.edits()[0],
            BulkFileOp::CreateFile {
                path: "src/new.rs".into(),
                content: "fn main() {}".into(),
            }
        );
    }

    #[test]
    fn bulk_edit_rename_and_replace() {
        let mut b = BulkFileEditBuilder::new();
        b.rename_file("a.rs", "b.rs");
        b.replace_text("b.rs", 10, "old", "new");
        assert_eq!(b.edit_count(), 2);
        assert_eq!(
            b.edits()[1],
            BulkFileOp::TextEdit {
                path: "b.rs".into(),
                line: 10,
                old_text: "old".into(),
                new_text: "new".into(),
            }
        );
    }

    #[test]
    fn bulk_edit_display() {
        let mut b = BulkFileEditBuilder::new();
        b.create_file("x.rs", "");
        assert_eq!(format!("{b}"), "BulkFileEditBuilder(1 ops)");
        assert_eq!(format!("{}", b.edits()[0]), "Create(x.rs)");
    }

    // -- WorkspaceConfigEditor tests --

    #[test]
    fn config_editor_set_get_remove() {
        let mut cfg = WorkspaceConfigEditor::new();
        cfg.set("theme", serde_json::json!("dark"));
        assert_eq!(cfg.get("theme"), Some(&serde_json::json!("dark")));
        assert_eq!(cfg.len(), 1);
        let removed = cfg.remove("theme");
        assert_eq!(removed, Some(serde_json::json!("dark")));
        assert!(cfg.is_empty());
    }

    #[test]
    fn config_editor_to_json() {
        let mut cfg = WorkspaceConfigEditor::new();
        cfg.set("fontSize", serde_json::json!(14));
        let json = cfg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["fontSize"], serde_json::json!(14));
    }

    #[test]
    fn config_editor_keys_and_display() {
        let mut cfg = WorkspaceConfigEditor::new();
        cfg.set("a", serde_json::json!(1));
        cfg.set("b", serde_json::json!(2));
        assert_eq!(cfg.keys().len(), 2);
        assert_eq!(format!("{cfg}"), "WorkspaceConfigEditor(2 entries)");
    }

    // -- WorkspaceSymbolRegistry tests --

    #[test]
    fn symbol_registry_register_and_query() {
        let mut reg = WorkspaceSymbolRegistry::new();
        reg.register_provider("rust-analyzer", "rust");
        reg.register_provider("gopls", "go");
        reg.register_provider("rust-tags", "rust");
        assert_eq!(reg.providers_for_language("rust"), vec!["rust-analyzer", "rust-tags"]);
        assert_eq!(reg.providers_for_language("go"), vec!["gopls"]);
        assert!(reg.providers_for_language("python").is_empty());
    }

    #[test]
    fn symbol_registry_all_and_has() {
        let mut reg = WorkspaceSymbolRegistry::new();
        reg.register_provider("tsserver", "typescript");
        assert_eq!(reg.provider_count(), 1);
        assert!(reg.has_provider("tsserver"));
        assert!(!reg.has_provider("missing"));
        assert_eq!(reg.all_providers(), vec![("tsserver", "typescript")]);
        assert_eq!(format!("{reg}"), "WorkspaceSymbolRegistry(1 providers)");
    }

    // -- WorkspaceDiagnosticsSummary tests --

    #[test]
    fn diagnostics_summary_counts() {
        let mut diag = WorkspaceDiagnosticsSummary::new();
        assert!(diag.is_clean());
        diag.add_diagnostic("main.rs", DiagSeverity::Error, "missing semicolon");
        diag.add_diagnostic("main.rs", DiagSeverity::Warning, "unused variable");
        diag.add_diagnostic("lib.rs", DiagSeverity::Error, "type mismatch");
        diag.add_diagnostic("lib.rs", DiagSeverity::Info, "consider refactoring");
        assert_eq!(diag.error_count(), 2);
        assert_eq!(diag.warning_count(), 1);
        assert_eq!(diag.info_count(), 1);
        assert_eq!(diag.total(), 4);
        assert!(!diag.is_clean());
    }

    #[test]
    fn diagnostics_files_with_errors() {
        let mut diag = WorkspaceDiagnosticsSummary::new();
        diag.add_diagnostic("b.rs", DiagSeverity::Error, "err1");
        diag.add_diagnostic("a.rs", DiagSeverity::Error, "err2");
        diag.add_diagnostic("a.rs", DiagSeverity::Warning, "warn");
        let files = diag.files_with_errors();
        assert_eq!(files, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn diagnostics_display() {
        let mut diag = WorkspaceDiagnosticsSummary::new();
        diag.add_diagnostic("x.rs", DiagSeverity::Error, "e");
        diag.add_diagnostic("x.rs", DiagSeverity::Hint, "h");
        assert_eq!(
            format!("{diag}"),
            "Diagnostics(errors=1, warnings=0, info=0, total=2)"
        );
    }

    #[test]
    fn diag_severity_display() {
        assert_eq!(format!("{}", DiagSeverity::Error), "error");
        assert_eq!(format!("{}", DiagSeverity::Warning), "warning");
        assert_eq!(format!("{}", DiagSeverity::Info), "info");
        assert_eq!(format!("{}", DiagSeverity::Hint), "hint");
    }

    #[test]
    fn extwsc_lru_insert_get() {
        let mut c = ExtWsCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn extwsc_lru_eviction() {
        let mut c = ExtWsCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn extwsc_lru_hit_ratio() {
        let mut c = ExtWsCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn extwsc_lru_clear() {
        let mut c = ExtWsCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn extwsc_lru_remove() {
        let mut c = ExtWsCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn extwsc_lru_peek() {
        let mut c = ExtWsCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn extwsb_builder_valid() {
        let cfg = ExtWsBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn extwsb_builder_empty_name() {
        let r = ExtWsBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn extwsb_builder_bad_priority() {
        assert!(ExtWsBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn extwsb_builder_zero_max() {
        assert!(ExtWsBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn extwsb_cfg_merge() {
        let mut a = ExtWsBBuilder::new("a").property("x", "1").build().unwrap();
        let b = ExtWsBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn extwsb_cfg_display() {
        let cfg = ExtWsBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    #[test]
    fn ext_workspace_config_new() {
        let cfg = ExtWorkspaceConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_workspace_config_set_get() {
        let mut cfg = ExtWorkspaceConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_workspace_config_remove() {
        let mut cfg = ExtWorkspaceConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_workspace_config_keys_sorted() {
        let mut cfg = ExtWorkspaceConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_workspace_config_bump_version() {
        let mut cfg = ExtWorkspaceConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_workspace_config_clear() {
        let mut cfg = ExtWorkspaceConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_workspace_config_merge() {
        let mut cfg1 = ExtWorkspaceConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtWorkspaceConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_workspace_config_disable() {
        let mut cfg = ExtWorkspaceConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_workspace_rate_tracker_empty() {
        let rt = ExtWorkspaceRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_workspace_rate_tracker_record() {
        let mut rt = ExtWorkspaceRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_workspace_rate_tracker_prune() {
        let mut rt = ExtWorkspaceRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_workspace_validator_valid() {
        let v = ExtWorkspaceValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_workspace_validator_errors() {
        let mut v = ExtWorkspaceValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_workspace_validator_clear() {
        let mut v = ExtWorkspaceValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_workspace_validator_merge() {
        let mut v1 = ExtWorkspaceValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtWorkspaceValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_workspace_rate_tracker_clear() {
        let mut rt = ExtWorkspaceRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yg_metrics_empty() {
        let m = YgMetrics::new("ext_ws");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yg_metrics_record_and_mean() {
        let mut m = YgMetrics::new("ext_ws");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yg_metrics_min_max() {
        let mut m = YgMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yg_metrics_variance_and_std() {
        let mut m = YgMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yg_metrics_percentile() {
        let mut m = YgMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yg_metrics_merge() {
        let mut a = YgMetrics::new("a");
        a.record(1.0);
        let mut b = YgMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yg_metrics_reset() {
        let mut m = YgMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yg_rate_window_empty() {
        let rw = YgRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yg_rate_window_tick_and_rate() {
        let mut rw = YgRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yg_lru_cache_basic() {
        let mut c = YgLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yg_lru_cache_contains_and_keys() {
        let mut c = YgLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yg_lru_cache_remove() {
        let mut c = YgLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yg_metrics_sum() {
        let mut m = YgMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yg_metrics_label() {
        let m = YgMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yg_lru_cache_clear() {
        let mut c = YgLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_workspace
    #[test]
    fn xa_ext_workspace_ring_new() {
        let rb = super::XaExtWorkspaceRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_workspace_ring_push_len() {
        let mut rb = super::XaExtWorkspaceRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_workspace_ring_wrap() {
        let mut rb = super::XaExtWorkspaceRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_workspace_ring_mean_empty() {
        let rb = super::XaExtWorkspaceRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_workspace_ring_mean_values() {
        let mut rb = super::XaExtWorkspaceRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_workspace_ring_min_max() {
        let mut rb = super::XaExtWorkspaceRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_workspace_ring_iter() {
        let mut rb = super::XaExtWorkspaceRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_workspace_counter_new() {
        let c = super::XaExtWorkspaceCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_workspace_counter_inc() {
        let mut c = super::XaExtWorkspaceCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_workspace_counter_inc_by() {
        let mut c = super::XaExtWorkspaceCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_workspace_counter_reset() {
        let mut c = super::XaExtWorkspaceCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_workspace_counter_clear() {
        let mut c = super::XaExtWorkspaceCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_workspace_counter_default() {
        let c = super::XaExtWorkspaceCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 79 ----

    #[test]
    fn xc_79_pool_new_empty() {
        let pool: super::Xc79Pool<i32> = super::Xc79Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_79_pool_release_acquire() {
        let mut pool = super::Xc79Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_79_pool_acquire_empty() {
        let mut pool: super::Xc79Pool<i32> = super::Xc79Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_79_pool_full() {
        let mut pool = super::Xc79Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_79_pool_drain() {
        let mut pool = super::Xc79Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_79_pool_stats() {
        let mut pool = super::Xc79Pool::new(8);
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
    fn xc_79_pool_clear() {
        let mut pool = super::Xc79Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_79_pool_shrink() {
        let mut pool = super::Xc79Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_79_pool_default() {
        let pool: super::Xc79Pool<String> = super::Xc79Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_79_pool_extend() {
        let mut pool = super::Xc79Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_79_pool_retain() {
        let mut pool = super::Xc79Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_79_scheduler_round_robin() {
        let mut sched = super::Xc79Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_79_scheduler_empty() {
        let mut sched = super::Xc79Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_79_scheduler_reset() {
        let mut sched = super::Xc79Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_79_scheduler_add_remove() {
        let mut sched = super::Xc79Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_79_scheduler_targets() {
        let sched = super::Xc79Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_79_hash_empty() {
        assert_eq!(super::xc_79_hash(b""), 5381);
    }

    #[test]
    fn xc_79_hash_data() {
        let h = super::xc_79_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_79_hash(b"hello"), h);
    }

    #[test]
    fn xc_79_reverse_str() {
        assert_eq!(super::xc_79_reverse("abc"), "cba");
        assert_eq!(super::xc_79_reverse(""), "");
    }


    // --- xd_83 deepening tests ---

    #[test]
    fn xd_83_sm_initial_state() {
        let sm = Xd83StateMachine::new();
        assert_eq!(sm.current_state(), Xd83State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_83_sm_valid_idle_to_running() {
        let mut sm = Xd83StateMachine::new();
        assert!(sm.transition(Xd83State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd83State::Running);
    }

    #[test]
    fn xd_83_sm_valid_running_to_paused() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        assert!(sm.transition(Xd83State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd83State::Paused);
    }

    #[test]
    fn xd_83_sm_valid_running_to_done() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        assert!(sm.transition(Xd83State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd83State::Done);
    }

    #[test]
    fn xd_83_sm_valid_paused_to_running() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        sm.transition(Xd83State::Paused).unwrap();
        assert!(sm.transition(Xd83State::Running).is_ok());
    }

    #[test]
    fn xd_83_sm_valid_done_to_idle() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        sm.transition(Xd83State::Done).unwrap();
        assert!(sm.transition(Xd83State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd83State::Idle);
    }

    #[test]
    fn xd_83_sm_invalid_idle_to_done() {
        let mut sm = Xd83StateMachine::new();
        assert!(sm.transition(Xd83State::Done).is_err());
    }

    #[test]
    fn xd_83_sm_invalid_idle_to_paused() {
        let mut sm = Xd83StateMachine::new();
        assert!(sm.transition(Xd83State::Paused).is_err());
    }

    #[test]
    fn xd_83_sm_history_tracking() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        sm.transition(Xd83State::Paused).unwrap();
        sm.transition(Xd83State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd83State::Idle);
        assert_eq!(sm.history()[0].to, Xd83State::Running);
        assert_eq!(sm.history()[1].from, Xd83State::Running);
        assert_eq!(sm.history()[2].to, Xd83State::Done);
    }

    #[test]
    fn xd_83_sm_serialize_deserialize() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd83StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd83State::Running));
    }

    #[test]
    fn xd_83_sm_deserialize_invalid() {
        assert_eq!(Xd83StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_83_sm_reset() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd83State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_83_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd83EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd83Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_83_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd83EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd83Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd83Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_83_bus_unsubscribe() {
        let mut bus = Xd83EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_83_event_kind_and_payload() {
        let e = Xd83Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd83Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_83_bus_clear_history() {
        let mut bus = Xd83EventBus::new();
        bus.publish(Xd83Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_83_sm_step_counter_increments() {
        let mut sm = Xd83StateMachine::new();
        sm.transition(Xd83State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd83State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #104 --

    #[test]
    fn xf104_trie_insert_search() {
        let mut t = Xf104Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf104_trie_starts_with() {
        let mut t = Xf104Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf104_trie_remove() {
        let mut t = Xf104Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf104_trie_word_count() {
        let mut t = Xf104Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf104_trie_longest_prefix() {
        let mut t = Xf104Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf104_trie_all_words() {
        let mut t = Xf104Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf104_trie_autocomplete() {
        let mut t = Xf104Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf104_trie_empty_search() {
        let t = Xf104Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf104_bloom_add_contains() {
        let mut bf = Xf104BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf104_bloom_probably_absent() {
        let bf = Xf104BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf104_bloom_false_positive_rate() {
        let mut bf = Xf104BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf104_bloom_clear() {
        let mut bf = Xf104BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf104_bloom_union() {
        let mut a = Xf104BloomFilter::xf_new(512, 2);
        let mut b = Xf104BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf104_bloom_intersection_estimate() {
        let mut a = Xf104BloomFilter::xf_new(512, 2);
        let mut b = Xf104BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf104_bloom_union_size_mismatch() {
        let a = Xf104BloomFilter::xf_new(256, 2);
        let b = Xf104BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}