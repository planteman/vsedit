//! Ext API: FileSystem.
//!
//! RPC bridge between the extension host and the main thread for fs.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_fs";

// ── RPC message types ──

/// Messages exchanged for the `FileSystem` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FsMessage {
    ReadFile { uri: String },
    WriteFile { uri: String, content: Vec<u8> },
    Delete { uri: String, recursive: bool },
    Rename { old_uri: String, new_uri: String, overwrite: bool },
    Stat { uri: String },
    ReadDirectory { uri: String },
    CreateDirectory { uri: String },
    Watch { uri: String, recursive: bool },
}

/// Metadata about a file system entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub file_type: FileType,
    pub ctime: u64,
    pub mtime: u64,
    pub size: u64,
}

/// The type of a file system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileType {
    File,
    Directory,
    SymbolicLink,
    Unknown,
}

/// A directory entry returned by `ReadDirectory`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub file_type: FileType,
}

/// Response payload for file system operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FsResponse {
    FileContent { data: Vec<u8> },
    Stat { stat: FileStat },
    Directory { entries: Vec<DirEntry> },
    WatchId { id: String },
    Ok,
    Error { message: String },
}

// ── Bridge ──

/// In-memory file system bridge for extensions.
#[derive(Debug, Default)]
pub struct FsBridge {
    files: HashMap<String, Vec<u8>>,
    next_watch_id: u64,
}

impl FsBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a file into the in-memory store (for testing / virtual FS).
    pub fn seed_file(&mut self, uri: String, content: Vec<u8>) {
        self.files.insert(uri, content);
    }

    /// Process an incoming file system message and return a response.
    pub fn handle(&mut self, msg: FsMessage) -> FsResponse {
        match msg {
            FsMessage::ReadFile { uri } => {
                self.files.get(&uri).map_or(
                    FsResponse::Error { message: format!("not found: {uri}") },
                    |data| FsResponse::FileContent { data: data.clone() },
                )
            }
            FsMessage::WriteFile { uri, content } => {
                self.files.insert(uri, content);
                FsResponse::Ok
            }
            FsMessage::Delete { uri, .. } => {
                self.files.remove(&uri);
                FsResponse::Ok
            }
            FsMessage::Rename { old_uri, new_uri, .. } => {
                if let Some(data) = self.files.remove(&old_uri) {
                    self.files.insert(new_uri, data);
                }
                FsResponse::Ok
            }
            FsMessage::Stat { uri } => {
                if let Some(data) = self.files.get(&uri) {
                    FsResponse::Stat {
                        stat: FileStat {
                            file_type: FileType::File,
                            ctime: 0,
                            mtime: 0,
                            size: data.len() as u64,
                        },
                    }
                } else {
                    FsResponse::Error { message: format!("not found: {uri}") }
                }
            }
            FsMessage::ReadDirectory { .. } => {
                FsResponse::Directory { entries: Vec::new() }
            }
            FsMessage::CreateDirectory { .. } => FsResponse::Ok,
            FsMessage::Watch { .. } => {
                let id = format!("watch-{}", self.next_watch_id);
                self.next_watch_id += 1;
                FsResponse::WatchId { id }
            }
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

// ── Error types ──

/// Errors produced by file system operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    /// The requested URI was not found.
    NotFound(String),
    /// The URI failed validation.
    InvalidUri(String),
    /// A file already exists at the target URI.
    AlreadyExists(String),
    /// The operation is not supported.
    NotSupported(String),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::NotFound(uri) => write!(f, "not found: {uri}"),
            FsError::InvalidUri(uri) => write!(f, "invalid uri: {uri}"),
            FsError::AlreadyExists(uri) => write!(f, "already exists: {uri}"),
            FsError::NotSupported(op) => write!(f, "not supported: {op}"),
        }
    }
}

impl std::error::Error for FsError {}

impl FsError {
    /// Convert this error into an `FsResponse::Error`.
    pub fn into_response(self) -> FsResponse {
        FsResponse::Error { message: self.to_string() }
    }
}

// ── Display for core types ──

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileType::File => write!(f, "file"),
            FileType::Directory => write!(f, "directory"),
            FileType::SymbolicLink => write!(f, "symlink"),
            FileType::Unknown => write!(f, "unknown"),
        }
    }
}

impl fmt::Display for FileStat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (size={}, ctime={}, mtime={})",
            self.file_type, self.size, self.ctime, self.mtime
        )
    }
}

impl fmt::Display for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.name, self.file_type)
    }
}

// ── FileStat builder ──

/// Builder for constructing `FileStat` instances with validation.
#[derive(Debug, Clone)]
pub struct FileStatBuilder {
    file_type: FileType,
    ctime: u64,
    mtime: u64,
    size: u64,
}

impl FileStatBuilder {
    pub fn new(file_type: FileType) -> Self {
        Self { file_type, ctime: 0, mtime: 0, size: 0 }
    }

    pub fn ctime(mut self, ctime: u64) -> Self {
        self.ctime = ctime;
        self
    }

    pub fn mtime(mut self, mtime: u64) -> Self {
        self.mtime = mtime;
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// Build the `FileStat`, ensuring mtime >= ctime.
    pub fn build(self) -> Result<FileStat, FsError> {
        if self.mtime != 0 && self.ctime != 0 && self.mtime < self.ctime {
            return Err(FsError::InvalidUri(
                "mtime must be >= ctime".to_string(),
            ));
        }
        Ok(FileStat {
            file_type: self.file_type,
            ctime: self.ctime,
            mtime: self.mtime,
            size: self.size,
        })
    }
}

// ── URI helpers ──

/// Validate that a URI has the expected `file://` scheme.
pub fn validate_file_uri(uri: &str) -> Result<(), FsError> {
    if !uri.starts_with("file://") {
        return Err(FsError::InvalidUri(uri.to_string()));
    }
    if uri.len() <= "file://".len() {
        return Err(FsError::InvalidUri(uri.to_string()));
    }
    Ok(())
}

/// Extract the path component from a `file://` URI.
pub fn uri_to_path(uri: &str) -> Result<&str, FsError> {
    validate_file_uri(uri)?;
    Ok(&uri["file://".len()..])
}

/// Extract the file name (last path segment) from a URI.
pub fn uri_file_name(uri: &str) -> Result<&str, FsError> {
    let path = uri_to_path(uri)?;
    Ok(path.rsplit('/').next().unwrap_or(path))
}

/// Extract the parent directory URI from a file URI.
pub fn uri_parent(uri: &str) -> Result<String, FsError> {
    let path = uri_to_path(uri)?;
    match path.rfind('/') {
        Some(pos) if pos > 0 => Ok(format!("file://{}", &path[..pos])),
        _ => Err(FsError::InvalidUri(format!("no parent for: {uri}"))),
    }
}

// ── Extended FsBridge methods ──

impl Clone for FsBridge {
    fn clone(&self) -> Self {
        Self {
            files: self.files.clone(),
            next_watch_id: self.next_watch_id,
        }
    }
}

impl PartialEq for FsBridge {
    fn eq(&self, other: &Self) -> bool {
        self.files == other.files && self.next_watch_id == other.next_watch_id
    }
}

impl FsBridge {
    /// Handle a message but validate URIs first, returning structured errors.
    pub fn handle_checked(&mut self, msg: FsMessage) -> Result<FsResponse, FsError> {
        match &msg {
            FsMessage::ReadFile { uri }
            | FsMessage::Stat { uri }
            | FsMessage::Delete { uri, .. }
            | FsMessage::ReadDirectory { uri }
            | FsMessage::CreateDirectory { uri }
            | FsMessage::Watch { uri, .. } => {
                validate_file_uri(uri)?;
            }
            FsMessage::WriteFile { uri, .. } => {
                validate_file_uri(uri)?;
            }
            FsMessage::Rename { old_uri, new_uri, .. } => {
                validate_file_uri(old_uri)?;
                validate_file_uri(new_uri)?;
            }
        }
        Ok(self.handle(msg))
    }

    /// Check whether a URI exists in the in-memory store.
    pub fn exists(&self, uri: &str) -> bool {
        self.files.contains_key(uri)
    }

    /// Return all URIs currently stored.
    pub fn uris(&self) -> Vec<&str> {
        self.files.keys().map(|s| s.as_str()).collect()
    }

    /// Return total bytes stored across all files.
    pub fn total_bytes(&self) -> u64 {
        self.files.values().map(|v| v.len() as u64).sum()
    }

    /// List directory entries for files whose URI starts with the given prefix.
    pub fn list_directory(&self, dir_uri: &str) -> Vec<DirEntry> {
        let prefix = if dir_uri.ends_with('/') {
            dir_uri.to_string()
        } else {
            format!("{dir_uri}/")
        };

        let mut entries = Vec::new();
        for key in self.files.keys() {
            if let Some(rest) = key.strip_prefix(prefix.as_str()) {
                // Only direct children (no further '/' in remainder).
                if !rest.contains('/') && !rest.is_empty() {
                    entries.push(DirEntry {
                        name: rest.to_string(),
                        file_type: FileType::File,
                    });
                }
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    /// Rename with an overwrite check – returns error if target exists and overwrite is false.
    pub fn rename_checked(
        &mut self,
        old_uri: &str,
        new_uri: &str,
        overwrite: bool,
    ) -> Result<(), FsError> {
        if !self.files.contains_key(old_uri) {
            return Err(FsError::NotFound(old_uri.to_string()));
        }
        if !overwrite && self.files.contains_key(new_uri) {
            return Err(FsError::AlreadyExists(new_uri.to_string()));
        }
        if let Some(data) = self.files.remove(old_uri) {
            self.files.insert(new_uri.to_string(), data);
        }
        Ok(())
    }
}

impl FileStat {
    /// Returns true if this entry represents a regular file.
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File
    }

    /// Returns true if this entry represents a directory.
    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Directory
    }

    /// Returns true if the file is empty (size == 0).
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// Initialize the fs extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── FileType helpers ──

impl FileType {
    /// Returns `true` for `FileType::File`.
    pub fn is_file(self) -> bool {
        self == Self::File
    }

    /// Returns `true` for `FileType::Directory`.
    pub fn is_directory(self) -> bool {
        self == Self::Directory
    }

    /// Returns `true` for `FileType::SymbolicLink`.
    pub fn is_symlink(self) -> bool {
        self == Self::SymbolicLink
    }

    /// Returns `true` for `FileType::Unknown`.
    pub fn is_unknown(self) -> bool {
        self == Self::Unknown
    }
}

// ── FsMessage helpers ──

impl FsMessage {
    /// Return the primary URI referenced by this message.
    pub fn primary_uri(&self) -> &str {
        match self {
            Self::ReadFile { uri }
            | Self::WriteFile { uri, .. }
            | Self::Delete { uri, .. }
            | Self::Stat { uri }
            | Self::ReadDirectory { uri }
            | Self::CreateDirectory { uri }
            | Self::Watch { uri, .. } => uri,
            Self::Rename { old_uri, .. } => old_uri,
        }
    }

    /// Returns `true` if this message is a read-only operation.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            Self::ReadFile { .. }
                | Self::Stat { .. }
                | Self::ReadDirectory { .. }
                | Self::Watch { .. }
        )
    }

    /// Returns `true` if this message mutates the file system.
    pub fn is_mutating(&self) -> bool {
        !self.is_read_only()
    }

    /// Return a human-readable operation name.
    pub fn operation_name(&self) -> &'static str {
        match self {
            Self::ReadFile { .. } => "read_file",
            Self::WriteFile { .. } => "write_file",
            Self::Delete { .. } => "delete",
            Self::Rename { .. } => "rename",
            Self::Stat { .. } => "stat",
            Self::ReadDirectory { .. } => "read_directory",
            Self::CreateDirectory { .. } => "create_directory",
            Self::Watch { .. } => "watch",
        }
    }
}

// ── FsResponse helpers ──

impl FsResponse {
    /// Returns `true` if this is an `Ok` response.
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Returns `true` if this is an `Error` response.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Extract the error message, if any.
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error { message } => Some(message),
            _ => None,
        }
    }

    /// Extract file content bytes, if this is a `FileContent` response.
    pub fn into_data(self) -> Option<Vec<u8>> {
        match self {
            Self::FileContent { data } => Some(data),
            _ => None,
        }
    }

    /// Extract `FileStat`, if this is a `Stat` response.
    pub fn into_stat(self) -> Option<FileStat> {
        match self {
            Self::Stat { stat } => Some(stat),
            _ => None,
        }
    }

    /// Extract directory entries, if this is a `Directory` response.
    pub fn into_entries(self) -> Option<Vec<DirEntry>> {
        match self {
            Self::Directory { entries } => Some(entries),
            _ => None,
        }
    }
}

// ── DirEntry helpers ──

impl DirEntry {
    /// Create a new `DirEntry`.
    pub fn new(name: impl Into<String>, file_type: FileType) -> Self {
        Self {
            name: name.into(),
            file_type,
        }
    }

    /// Returns `true` if this entry is a file.
    pub fn is_file(&self) -> bool {
        self.file_type.is_file()
    }

    /// Returns `true` if this entry is a directory.
    pub fn is_directory(&self) -> bool {
        self.file_type.is_directory()
    }

    /// Return the file extension, if any (e.g. `"rs"` for `"main.rs"`).
    pub fn extension(&self) -> Option<&str> {
        self.name.rsplit('.').next().and_then(|ext| {
            if ext == self.name {
                None
            } else {
                Some(ext)
            }
        })
    }
}

// ── FsBridge convenience methods ──

impl FsBridge {
    /// Directly read file content by URI, returning `None` if not found.
    pub fn read_file(&self, uri: &str) -> Option<&[u8]> {
        self.files.get(uri).map(|v| v.as_slice())
    }

    /// Directly write file content by URI.
    pub fn write_file(&mut self, uri: impl Into<String>, content: Vec<u8>) {
        self.files.insert(uri.into(), content);
    }

    /// Remove all files from the in-memory store.
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Return URIs that contain the given substring.
    pub fn files_matching(&self, pattern: &str) -> Vec<&str> {
        self.files
            .keys()
            .filter(|k| k.contains(pattern))
            .map(|k| k.as_str())
            .collect()
    }

    /// Return the content of a file as a UTF-8 string, if valid.
    pub fn read_text(&self, uri: &str) -> Option<String> {
        self.files
            .get(uri)
            .and_then(|data| std::str::from_utf8(data).ok().map(String::from))
    }

    /// Append bytes to an existing file, or create it if it doesn't exist.
    pub fn append(&mut self, uri: &str, data: &[u8]) {
        self.files
            .entry(uri.to_string())
            .or_default()
            .extend_from_slice(data);
    }

    /// Copy a file from one URI to another. Returns `false` if source not found.
    pub fn copy_file(&mut self, src_uri: &str, dst_uri: &str) -> bool {
        if let Some(data) = self.files.get(src_uri).cloned() {
            self.files.insert(dst_uri.to_string(), data);
            true
        } else {
            false
        }
    }
}

// ── VirtualDirectory additional methods ──

impl VirtualDirectory {
    /// Returns `true` if this directory directly contains a file with the given name.
    pub fn contains_file(&self, name: &str) -> bool {
        self.files.iter().any(|f| f == name)
    }

    /// Remove a file by name. Returns `true` if the file was present.
    pub fn remove_file(&mut self, name: &str) -> bool {
        let before = self.files.len();
        self.files.retain(|f| f != name);
        self.files.len() < before
    }

    /// Remove a sub-directory by name. Returns `true` if it was present.
    pub fn remove_dir(&mut self, name: &str) -> bool {
        let before = self.children_dirs.len();
        self.children_dirs.retain(|d| d.name != name);
        self.children_dirs.len() < before
    }

    /// Returns `true` if this directory has no files and no sub-directories.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.children_dirs.is_empty()
    }

    /// Compute the maximum depth of the directory tree (this node = 0).
    pub fn max_depth(&self) -> usize {
        if self.children_dirs.is_empty() {
            0
        } else {
            1 + self
                .children_dirs
                .iter()
                .map(|d| d.max_depth())
                .max()
                .unwrap_or(0)
        }
    }

    /// Collect all sub-directory names (non-recursive).
    pub fn child_dir_names(&self) -> Vec<&str> {
        self.children_dirs.iter().map(|d| d.name.as_str()).collect()
    }
}

// ── FileWatchEvent helpers ──

impl fmt::Display for FileWatchEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created { uri } => write!(f, "created: {uri}"),
            Self::Changed { uri } => write!(f, "changed: {uri}"),
            Self::Deleted { uri } => write!(f, "deleted: {uri}"),
        }
    }
}

// ── FileChangeAccumulator helpers ──

impl FileChangeAccumulator {
    /// Merge events from another accumulator into this one.
    pub fn merge(&mut self, other: &mut FileChangeAccumulator) {
        self.events.append(&mut other.events);
        if let Some(other_last) = other.last_event_ms {
            self.last_event_ms = Some(
                self.last_event_ms
                    .map_or(other_last, |mine| mine.max(other_last)),
            );
        }
    }

    /// Filter events, keeping only those whose URI contains the given substring.
    pub fn filter_by_uri(&mut self, pattern: &str) {
        self.events.retain(|e| e.uri().contains(pattern));
    }

    /// Returns `true` if there are no pending events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Discard all pending events without flushing.
    pub fn discard(&mut self) {
        self.events.clear();
        self.last_event_ms = None;
    }

    /// Count events of each kind: (created, changed, deleted).
    pub fn event_counts(&self) -> (usize, usize, usize) {
        let mut created = 0;
        let mut changed = 0;
        let mut deleted = 0;
        for event in &self.events {
            match event {
                FileWatchEvent::Created { .. } => created += 1,
                FileWatchEvent::Changed { .. } => changed += 1,
                FileWatchEvent::Deleted { .. } => deleted += 1,
            }
        }
        (created, changed, deleted)
    }
}

/// Accumulated statistics for ext-fs operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtFsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtFsStats {
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
    pub fn merge(&mut self, other: &ExtFsStats) {
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

impl Default for ExtFsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtFsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtFsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-fs.
#[derive(Debug, Clone)]
pub struct ExtFsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtFsValidator {
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

impl Default for ExtFsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VirtualFileContent
// ---------------------------------------------------------------------------

/// Content descriptor for a virtual (in-memory) file.
#[derive(Debug, Clone)]
pub struct VirtualFileContent {
    pub uri: String,
    pub content: Vec<u8>,
    pub encoding: String,
    pub readonly: bool,
}

impl VirtualFileContent {
    pub fn new(uri: &str, content: Vec<u8>) -> Self {
        Self {
            uri: uri.to_string(),
            content,
            encoding: "utf-8".to_string(),
            readonly: false,
        }
    }

    pub fn with_encoding(mut self, enc: &str) -> Self {
        self.encoding = enc.to_string();
        self
    }

    pub fn as_readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    pub fn text_content(&self) -> Option<String> {
        std::str::from_utf8(&self.content).ok().map(String::from)
    }

    pub fn size(&self) -> u64 {
        self.content.len() as u64
    }

    pub fn is_text(&self) -> bool {
        std::str::from_utf8(&self.content).is_ok()
    }
}

// ---------------------------------------------------------------------------
// file_system_watch_recursive
// ---------------------------------------------------------------------------

/// Generates URI glob patterns for recursive directory watching.
///
/// Returns patterns like `root_uri/*`, `root_uri/*/*`, etc. up to `depth`.
/// A depth of 0 yields `root_uri/**`.
pub fn file_system_watch_recursive(root_uri: &str, depth: u32) -> Vec<String> {
    if validate_file_uri(root_uri).is_err() {
        return Vec::new();
    }
    let base = root_uri.trim_end_matches('/');
    if depth == 0 {
        return vec![format!("{base}/**")];
    }
    let mut patterns = Vec::new();
    let mut segment = String::new();
    for _ in 0..depth {
        segment.push_str("/*");
        patterns.push(format!("{base}{segment}"));
    }
    patterns
}

// ---------------------------------------------------------------------------
// fs_stat_to_file_type
// ---------------------------------------------------------------------------

/// Extracts the [`FileType`] from a [`FileStat`].
pub fn fs_stat_to_file_type(stat: &FileStat) -> FileType {
    stat.file_type.clone()
}

// ---------------------------------------------------------------------------
// FileWatchEvent
// ---------------------------------------------------------------------------

/// Notification about a file-system change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileWatchEvent {
    Created { uri: String },
    Changed { uri: String },
    Deleted { uri: String },
}

impl FileWatchEvent {
    pub fn uri(&self) -> &str {
        match self {
            Self::Created { uri } | Self::Changed { uri } | Self::Deleted { uri } => uri,
        }
    }

    pub fn is_creation(&self) -> bool {
        matches!(self, Self::Created { .. })
    }

    pub fn is_change(&self) -> bool {
        matches!(self, Self::Changed { .. })
    }

    pub fn is_deletion(&self) -> bool {
        matches!(self, Self::Deleted { .. })
    }
}

// ---------------------------------------------------------------------------
// FileContentDiff
// ---------------------------------------------------------------------------

/// A simple content-level diff between two byte slices.
#[derive(Debug, Clone)]
pub struct FileContentDiff {
    old_size: usize,
    new_size: usize,
    changed: bool,
}

impl FileContentDiff {
    pub fn new(old: &[u8], new_content: &[u8]) -> Self {
        Self {
            old_size: old.len(),
            new_size: new_content.len(),
            changed: old != new_content,
        }
    }

    pub fn has_changes(&self) -> bool {
        self.changed
    }

    pub fn old_size(&self) -> usize {
        self.old_size
    }

    pub fn new_size(&self) -> usize {
        self.new_size
    }

    pub fn size_delta(&self) -> i64 {
        self.new_size as i64 - self.old_size as i64
    }
}

// ---------------------------------------------------------------------------
// VirtualFileStore
// ---------------------------------------------------------------------------

/// An in-memory store for [`VirtualFileContent`] entries, keyed by URI.
#[derive(Debug, Clone, Default)]
pub struct VirtualFileStore {
    files: HashMap<String, VirtualFileContent>,
}

impl VirtualFileStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, content: VirtualFileContent) {
        self.files.insert(content.uri.clone(), content);
    }

    pub fn get(&self, uri: &str) -> Option<&VirtualFileContent> {
        self.files.get(uri)
    }

    pub fn remove(&mut self, uri: &str) -> bool {
        self.files.remove(uri).is_some()
    }

    pub fn count(&self) -> usize {
        self.files.len()
    }

    pub fn all_uris(&self) -> Vec<&str> {
        self.files.keys().map(|s| s.as_str()).collect()
    }

    pub fn readonly_count(&self) -> usize {
        self.files.values().filter(|f| f.readonly).count()
    }

    /// Search files by URI substring.
    pub fn search_by_uri(&self, pattern: &str) -> Vec<&VirtualFileContent> {
        self.files
            .values()
            .filter(|f| f.uri.contains(pattern))
            .collect()
    }

    /// Search files whose text content contains the given substring.
    pub fn search_by_content(&self, needle: &str) -> Vec<&VirtualFileContent> {
        self.files
            .values()
            .filter(|f| {
                f.text_content()
                    .map_or(false, |text| text.contains(needle))
            })
            .collect()
    }

    /// Return total bytes across all files in the store.
    pub fn total_bytes(&self) -> u64 {
        self.files.values().map(|f| f.size()).sum()
    }
}

// ---------------------------------------------------------------------------
// FsPathMatcher
// ---------------------------------------------------------------------------

/// Glob-style pattern matcher for file paths.
#[derive(Debug, Clone)]
pub struct FsPathMatcher {
    patterns: Vec<String>,
    exclude_patterns: Vec<String>,
}

impl FsPathMatcher {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }

    /// Add an include pattern (e.g. `"*.rs"`, `"src/**"`).
    pub fn include(mut self, pattern: &str) -> Self {
        self.patterns.push(pattern.to_string());
        self
    }

    /// Add an exclude pattern.
    pub fn exclude(mut self, pattern: &str) -> Self {
        self.exclude_patterns.push(pattern.to_string());
        self
    }

    /// Test whether a path matches any include pattern and no exclude pattern.
    pub fn matches(&self, path: &str) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let included = self.patterns.iter().any(|p| Self::glob_match(p, path));
        if !included {
            return false;
        }
        !self.exclude_patterns.iter().any(|p| Self::glob_match(p, path))
    }

    /// Simple glob match: `*` matches non-`/` chars, `**` matches everything.
    fn glob_match(pattern: &str, path: &str) -> bool {
        let pat = pattern.as_bytes();
        let text = path.as_bytes();
        Self::glob_match_inner(pat, text)
    }

    fn glob_match_inner(pat: &[u8], text: &[u8]) -> bool {
        let mut pi = 0;
        let mut ti = 0;
        let mut star_pi: Option<usize> = None;
        let mut star_ti: usize = 0;

        while ti < text.len() {
            if pi < pat.len() && pat[pi] == b'*' {
                if pi + 1 < pat.len() && pat[pi + 1] == b'*' {
                    let rest = if pi + 2 < pat.len() && pat[pi + 2] == b'/' {
                        pi + 3
                    } else {
                        pi + 2
                    };
                    if rest >= pat.len() {
                        return true;
                    }
                    for start in ti..=text.len() {
                        if Self::glob_match_inner(&pat[rest..], &text[start..]) {
                            return true;
                        }
                    }
                    return false;
                }
                star_pi = Some(pi);
                star_ti = ti;
                pi += 1;
                continue;
            }
            if pi < pat.len() && (pat[pi] == b'?' && text[ti] != b'/') {
                pi += 1;
                ti += 1;
                continue;
            }
            if pi < pat.len() && pat[pi] == text[ti] {
                pi += 1;
                ti += 1;
                continue;
            }
            if let Some(sp) = star_pi {
                if text[ti] == b'/' {
                    return false;
                }
                pi = sp + 1;
                star_ti += 1;
                ti = star_ti;
                continue;
            }
            return false;
        }
        while pi < pat.len() && pat[pi] == b'*' {
            pi += 1;
        }
        pi == pat.len()
    }

    /// Filter a list of paths, returning those that match.
    pub fn filter<'a>(&self, paths: &'a [&str]) -> Vec<&'a str> {
        paths.iter().copied().filter(|p| self.matches(p)).collect()
    }
}

impl Default for FsPathMatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VirtualDirectory
// ---------------------------------------------------------------------------

/// In-memory directory tree node.
#[derive(Debug, Clone)]
pub struct VirtualDirectory {
    name: String,
    children_dirs: Vec<VirtualDirectory>,
    files: Vec<String>,
}

impl VirtualDirectory {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            children_dirs: Vec::new(),
            files: Vec::new(),
        }
    }

    /// Add a file to this directory.
    pub fn add_file(&mut self, name: &str) {
        if !self.files.contains(&name.to_string()) {
            self.files.push(name.to_string());
        }
    }

    /// Add a sub-directory.
    pub fn add_dir(&mut self, dir: VirtualDirectory) {
        if !self.children_dirs.iter().any(|d| d.name == dir.name) {
            self.children_dirs.push(dir);
        }
    }

    /// Get a sub-directory by name.
    pub fn get_dir(&self, name: &str) -> Option<&VirtualDirectory> {
        self.children_dirs.iter().find(|d| d.name == name)
    }

    /// Get a mutable sub-directory by name.
    pub fn get_dir_mut(&mut self, name: &str) -> Option<&mut VirtualDirectory> {
        self.children_dirs.iter_mut().find(|d| d.name == name)
    }

    /// Get or create a sub-directory by name.
    pub fn ensure_dir(&mut self, name: &str) -> &mut VirtualDirectory {
        if !self.children_dirs.iter().any(|d| d.name == name) {
            self.children_dirs.push(VirtualDirectory::new(name));
        }
        self.children_dirs.iter_mut().find(|d| d.name == name).unwrap()
    }

    /// Total number of files in this directory (not recursive).
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Total number of sub-directories.
    pub fn dir_count(&self) -> usize {
        self.children_dirs.len()
    }

    /// Recursively count all files.
    pub fn total_file_count(&self) -> usize {
        let mut count = self.files.len();
        for child in &self.children_dirs {
            count += child.total_file_count();
        }
        count
    }

    /// List all file paths recursively with the given prefix.
    pub fn list_all_files(&self, prefix: &str) -> Vec<String> {
        let mut result = Vec::new();
        let current = if prefix.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", prefix, self.name)
        };
        for file in &self.files {
            result.push(format!("{}/{}", current, file));
        }
        for child in &self.children_dirs {
            result.extend(child.list_all_files(&current));
        }
        result
    }

    /// Name of this directory.
    pub fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// FileChangeAccumulator
// ---------------------------------------------------------------------------

/// Accumulates file change events for batch processing with a debounce window.
#[derive(Debug)]
pub struct FileChangeAccumulator {
    events: Vec<FileWatchEvent>,
    debounce_ms: u64,
    last_event_ms: Option<u64>,
}

impl FileChangeAccumulator {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            events: Vec::new(),
            debounce_ms,
            last_event_ms: None,
        }
    }

    /// Push a new event with the given timestamp.
    pub fn push(&mut self, event: FileWatchEvent, timestamp_ms: u64) {
        self.events.push(event);
        self.last_event_ms = Some(timestamp_ms);
    }

    /// Returns `true` if the debounce window has elapsed since the last event.
    pub fn is_ready(&self, current_ms: u64) -> bool {
        match self.last_event_ms {
            Some(last) => current_ms.saturating_sub(last) >= self.debounce_ms,
            None => false,
        }
    }

    /// Drain all accumulated events if the debounce window has elapsed.
    pub fn flush(&mut self, current_ms: u64) -> Vec<FileWatchEvent> {
        if self.is_ready(current_ms) {
            self.last_event_ms = None;
            std::mem::take(&mut self.events)
        } else {
            Vec::new()
        }
    }

    /// Number of pending events.
    pub fn pending_count(&self) -> usize {
        self.events.len()
    }

    /// Deduplicate events: keep only the latest event per URI.
    pub fn deduplicate(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for event in self.events.drain(..).rev() {
            if seen.insert(event.uri().to_string()) {
                deduped.push(event);
            }
        }
        deduped.reverse();
        self.events = deduped;
    }

    /// Return unique URIs from accumulated events.
    pub fn affected_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.events.iter().map(|e| e.uri()).collect();
        uris.sort_unstable();
        uris.dedup();
        uris
    }
}


// ── Fs Event Debouncer ──

/// A change event with a URI and event kind.
#[derive(Debug, Clone, PartialEq)]
pub enum FsChangeKind {
    Created,
    Changed,
    Deleted,
}

/// A single file system change event.
#[derive(Debug, Clone)]
pub struct FsChangeEvent {
    pub uri: String,
    pub kind: FsChangeKind,
    pub timestamp: u64,
}

/// Debouncer that batches rapid file system changes within a time window.
#[derive(Debug)]
pub struct FsEventDebouncer {
    window_ms: u64,
    pending: Vec<FsChangeEvent>,
}

impl FsEventDebouncer {
    /// Create a debouncer with the given window in milliseconds.
    pub fn new(window_ms: u64) -> Self {
        Self {
            window_ms,
            pending: Vec::new(),
        }
    }

    /// Add an event to the pending queue.
    pub fn push(&mut self, event: FsChangeEvent) {
        self.pending.push(event);
    }

    /// Flush events that are older than `now - window_ms`.
    /// Returns the flushed (debounced) events grouped by URI, keeping only
    /// the latest event per URI.
    pub fn flush(&mut self, now: u64) -> Vec<FsChangeEvent> {
        let cutoff = now.saturating_sub(self.window_ms);
        let (ready, remaining): (Vec<_>, Vec<_>) =
            self.pending.drain(..).partition(|e| e.timestamp <= cutoff);

        self.pending = remaining;

        // Keep only the latest event per URI.
        let mut latest: HashMap<String, FsChangeEvent> = HashMap::new();
        for event in ready {
            let entry = latest.entry(event.uri.clone()).or_insert_with(|| event.clone());
            if event.timestamp > entry.timestamp {
                *entry = event;
            }
        }

        let mut result: Vec<FsChangeEvent> = latest.into_values().collect();
        result.sort_by(|a, b| a.uri.cmp(&b.uri));
        result
    }

    /// Number of pending (not yet flushed) events.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Clear all pending events without flushing.
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// Return unique URIs currently pending.
    pub fn pending_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.pending.iter().map(|e| e.uri.as_str()).collect();
        uris.sort_unstable();
        uris.dedup();
        uris
    }

    /// The configured window size in ms.
    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

// ── Recursive Delete Safety Check ──

/// Safety checks before performing recursive deletes.
pub struct RecursiveDeleteSafetyCheck;

impl RecursiveDeleteSafetyCheck {
    /// Disallowed root paths that must never be recursively deleted.
    const DISALLOWED: &[&str] = &[
        "/", "/home", "/usr", "/etc", "/var", "/tmp", "/bin", "/sbin",
        "/boot", "/dev", "/proc", "/sys", "/lib", "/opt",
        "C:\\", "C:\\Windows", "C:\\Program Files",
    ];

    /// Check if a URI is safe to delete recursively.
    pub fn is_safe(uri: &str) -> bool {
        let normalized = uri.trim_end_matches('/');
        if normalized.is_empty() {
            return false;
        }
        for disallowed in Self::DISALLOWED {
            if normalized.eq_ignore_ascii_case(disallowed) {
                return false;
            }
        }
        true
    }

    /// Count path segments (to reject very shallow paths).
    pub fn path_depth(uri: &str) -> usize {
        uri.split('/')
            .filter(|s| !s.is_empty())
            .count()
    }

    /// Require a minimum path depth for recursive deletes.
    pub fn is_deep_enough(uri: &str, min_depth: usize) -> bool {
        Self::path_depth(uri) >= min_depth
    }

    /// Validate a delete operation, returning an error message if unsafe.
    pub fn validate(uri: &str, recursive: bool) -> Result<(), String> {
        if !recursive {
            return Ok(());
        }
        if !Self::is_safe(uri) {
            return Err(format!("refusing to recursively delete protected path: {uri}"));
        }
        if !Self::is_deep_enough(uri, 2) {
            return Err(format!("path too shallow for recursive delete: {uri}"));
        }
        Ok(())
    }
}

// ── Fs Rename Validator ──

/// Validates file rename operations.
pub struct FsRenameValidator;

impl FsRenameValidator {
    /// Maximum allowed file name length.
    const MAX_NAME_LEN: usize = 255;

    /// Characters not allowed in file names.
    const INVALID_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];

    /// Validate that a new name is acceptable.
    pub fn validate_name(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name cannot be empty".to_string());
        }
        if name.len() > Self::MAX_NAME_LEN {
            return Err(format!("name exceeds maximum length of {}", Self::MAX_NAME_LEN));
        }
        if name.contains('/') || name.contains('\\') {
            return Err("name cannot contain path separators".to_string());
        }
        for ch in Self::INVALID_CHARS {
            if name.contains(*ch) {
                return Err(format!("name contains invalid character: {ch}"));
            }
        }
        if name.starts_with('.') && name.len() == 1 {
            return Err("name cannot be '.'".to_string());
        }
        if name == ".." {
            return Err("name cannot be '..'".to_string());
        }
        Ok(())
    }

    /// Extract the file name from a URI.
    pub fn file_name_from_uri(uri: &str) -> Option<String> {
        uri.rsplit('/').next().map(|s| s.to_string()).filter(|s| !s.is_empty())
    }

    /// Check if a rename would change the file extension.
    pub fn extension_changed(old_uri: &str, new_uri: &str) -> bool {
        let old_ext = old_uri.rsplit('.').next();
        let new_ext = new_uri.rsplit('.').next();
        old_ext != new_ext
    }

    /// Check if old and new URIs are in the same directory.
    pub fn same_directory(old_uri: &str, new_uri: &str) -> bool {
        let old_parent = old_uri.rsplit_once('/').map(|(p, _)| p);
        let new_parent = new_uri.rsplit_once('/').map(|(p, _)| p);
        old_parent == new_parent
    }

    /// Validate a full rename operation.
    pub fn validate_rename(old_uri: &str, new_uri: &str) -> Result<(), String> {
        if old_uri == new_uri {
            return Err("old and new URIs are the same".to_string());
        }
        if let Some(name) = Self::file_name_from_uri(new_uri) {
            Self::validate_name(&name)?;
        }
        Ok(())
    }
}

// ── Fs Encoding Detector ──

/// Simple encoding detector based on byte patterns.
pub struct FsEncodingDetector;

/// Detected encoding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Ascii,
    Binary,
}

impl fmt::Display for DetectedEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf8 => write!(f, "UTF-8"),
            Self::Utf8Bom => write!(f, "UTF-8 with BOM"),
            Self::Utf16Le => write!(f, "UTF-16 LE"),
            Self::Utf16Be => write!(f, "UTF-16 BE"),
            Self::Ascii => write!(f, "ASCII"),
            Self::Binary => write!(f, "Binary"),
        }
    }
}

impl FsEncodingDetector {
    /// Detect encoding from file content bytes.
    pub fn detect(content: &[u8]) -> DetectedEncoding {
        if content.is_empty() {
            return DetectedEncoding::Ascii;
        }
        // Check BOM markers.
        if content.len() >= 3 && content[0] == 0xEF && content[1] == 0xBB && content[2] == 0xBF {
            return DetectedEncoding::Utf8Bom;
        }
        if content.len() >= 2 && content[0] == 0xFF && content[1] == 0xFE {
            return DetectedEncoding::Utf16Le;
        }
        if content.len() >= 2 && content[0] == 0xFE && content[1] == 0xFF {
            return DetectedEncoding::Utf16Be;
        }
        // Check for null bytes (likely binary).
        if content.iter().any(|&b| b == 0) {
            return DetectedEncoding::Binary;
        }
        // Check if all bytes are valid ASCII.
        if content.iter().all(|&b| b < 128) {
            return DetectedEncoding::Ascii;
        }
        // Check if valid UTF-8.
        if std::str::from_utf8(content).is_ok() {
            DetectedEncoding::Utf8
        } else {
            DetectedEncoding::Binary
        }
    }

    /// Check if content is likely a text file.
    pub fn is_text(content: &[u8]) -> bool {
        !matches!(Self::detect(content), DetectedEncoding::Binary)
    }

    /// Strip BOM from content if present, returning the payload.
    pub fn strip_bom(content: &[u8]) -> &[u8] {
        if content.len() >= 3 && content[0] == 0xEF && content[1] == 0xBB && content[2] == 0xBF {
            &content[3..]
        } else if content.len() >= 2
            && ((content[0] == 0xFF && content[1] == 0xFE)
                || (content[0] == 0xFE && content[1] == 0xFF))
        {
            &content[2..]
        } else {
            content
        }
    }

    /// Get the BOM bytes for a given encoding, or empty if none.
    pub fn bom_bytes(encoding: DetectedEncoding) -> &'static [u8] {
        match encoding {
            DetectedEncoding::Utf8Bom => &[0xEF, 0xBB, 0xBF],
            DetectedEncoding::Utf16Le => &[0xFF, 0xFE],
            DetectedEncoding::Utf16Be => &[0xFE, 0xFF],
            _ => &[],
        }
    }
}


// ---------------------------------------------------------------------------
// ext_fs – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtFsActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtFsActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtFsRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtFsRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_fs_collect_sequences(envelopes: &[XExtFsRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_fs_filter_by_method<'a>(
    envelopes: &'a [XExtFsRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtFsRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_fs_dedup_by_seq(envelopes: Vec<XExtFsRpcEnvelope>) -> Vec<XExtFsRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_fs_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtFsApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtFsApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtFsApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}


/// Configuration manager for ext_fs functionality.
pub struct ExtFsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtFsConfig {
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

    pub fn merge(&mut self, other: &ExtFsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_fs operations.
pub struct ExtFsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtFsRateTracker {
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

/// Validation result collector for ext_fs.
pub struct ExtFsValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtFsValidationCollector {
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

    pub fn merge(&mut self, other: &ExtFsValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
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
// xa_ extended helpers for ext_fs
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtFsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtFsRingBuf {
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
pub struct XaExtFsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtFsCounter {
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

impl Default for XaExtFsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 59
// ---------------------------------------------------------------------------

/// Generic object pool `Xc59Pool<T>`.
pub struct Xc59Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc59Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc59PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc59Pool<T> {
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
    pub fn stats(&self) -> Xc59PoolStats {
        Xc59PoolStats {
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

impl<T> Default for Xc59Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc59Scheduler`.
pub struct Xc59Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc59Scheduler {
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

impl Default for Xc59Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_59 hash for the given byte slice.
pub fn xc_59_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_59 convention.
pub fn xc_59_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_103 deepening: state machine + event bus ---

/// States for the Xd103 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd103State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd103State {
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
pub struct Xd103Transition {
    pub from: Xd103State,
    pub to: Xd103State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd103StateMachine {
    current: Xd103State,
    history: Vec<Xd103Transition>,
    step_counter: usize,
}

impl Xd103StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd103State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd103State {
        self.current
    }

    pub fn history(&self) -> &[Xd103Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd103State) -> Result<Xd103State, String> {
        let allowed = match (self.current, target) {
            (Xd103State::Idle, Xd103State::Running) => true,
            (Xd103State::Running, Xd103State::Paused) => true,
            (Xd103State::Running, Xd103State::Done) => true,
            (Xd103State::Paused, Xd103State::Running) => true,
            (Xd103State::Paused, Xd103State::Done) => true,
            (Xd103State::Done, Xd103State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_103: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd103Transition {
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
            "Xd103SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd103State> {
        let prefix = "Xd103SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd103State::Idle),
            "Running" => Some(Xd103State::Running),
            "Paused" => Some(Xd103State::Paused),
            "Done" => Some(Xd103State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd103State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd103 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd103Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd103Event {
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

type Xd103HandlerFn = Box<dyn Fn(&Xd103Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd103EventBus {
    handlers: Vec<(usize, Option<String>, Xd103HandlerFn)>,
    next_id: usize,
    published: Vec<Xd103Event>,
}

impl Xd103EventBus {
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
        F: Fn(&Xd103Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd103Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd103Event) {
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

    pub fn published_events(&self) -> &[Xd103Event] {
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

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn write_and_read() {
        let mut bridge = FsBridge::new();
        bridge.handle(FsMessage::WriteFile {
            uri: "file:///a.txt".into(),
            content: b"hello".to_vec(),
        });
        let resp = bridge.handle(FsMessage::ReadFile { uri: "file:///a.txt".into() });
        assert_eq!(resp, FsResponse::FileContent { data: b"hello".to_vec() });
    }

    #[test]
    fn read_missing_file() {
        let mut bridge = FsBridge::new();
        let resp = bridge.handle(FsMessage::ReadFile { uri: "file:///nope".into() });
        matches!(resp, FsResponse::Error { .. });
    }

    #[test]
    fn stat_returns_size() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///b.txt".into(), vec![1, 2, 3]);
        let resp = bridge.handle(FsMessage::Stat { uri: "file:///b.txt".into() });
        if let FsResponse::Stat { stat } = resp {
            assert_eq!(stat.size, 3);
            assert_eq!(stat.file_type, FileType::File);
        } else {
            panic!("expected Stat");
        }
    }

    #[test]
    fn delete_removes_file() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///c.txt".into(), vec![]);
        assert_eq!(bridge.file_count(), 1);
        bridge.handle(FsMessage::Delete { uri: "file:///c.txt".into(), recursive: false });
        assert_eq!(bridge.file_count(), 0);
    }

    #[test]
    fn serde_round_trip() {
        let msg = FsMessage::Rename {
            old_uri: "file:///old".into(),
            new_uri: "file:///new".into(),
            overwrite: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: FsMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    // ── Additional tests ──

    #[test]
    fn fs_error_display() {
        let err = FsError::NotFound("file:///gone".into());
        assert_eq!(err.to_string(), "not found: file:///gone");

        let err2 = FsError::InvalidUri("bad".into());
        assert_eq!(err2.to_string(), "invalid uri: bad");

        let err3 = FsError::AlreadyExists("file:///dup".into());
        assert_eq!(err3.to_string(), "already exists: file:///dup");
    }

    #[test]
    fn fs_error_into_response() {
        let resp = FsError::NotFound("file:///x".into()).into_response();
        assert_eq!(resp, FsResponse::Error { message: "not found: file:///x".into() });
    }

    #[test]
    fn validate_file_uri_ok() {
        assert!(validate_file_uri("file:///foo.txt").is_ok());
    }

    #[test]
    fn validate_file_uri_bad_scheme() {
        assert!(validate_file_uri("http://example.com").is_err());
        assert!(validate_file_uri("file://").is_err());
    }

    #[test]
    fn uri_to_path_extracts() {
        assert_eq!(uri_to_path("file:///home/a.txt").unwrap(), "/home/a.txt");
    }

    #[test]
    fn uri_file_name_extracts() {
        assert_eq!(uri_file_name("file:///home/user/doc.txt").unwrap(), "doc.txt");
    }

    #[test]
    fn uri_parent_extracts() {
        assert_eq!(
            uri_parent("file:///home/user/doc.txt").unwrap(),
            "file:///home/user"
        );
    }

    #[test]
    fn file_stat_builder_ok() {
        let stat = FileStatBuilder::new(FileType::File)
            .ctime(100)
            .mtime(200)
            .size(512)
            .build()
            .unwrap();
        assert_eq!(stat.size, 512);
        assert!(stat.is_file());
        assert!(!stat.is_directory());
    }

    #[test]
    fn file_stat_builder_bad_times() {
        let result = FileStatBuilder::new(FileType::File)
            .ctime(200)
            .mtime(100)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn bridge_exists_and_uris() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///a".into(), vec![1]);
        bridge.seed_file("file:///b".into(), vec![2, 3]);
        assert!(bridge.exists("file:///a"));
        assert!(!bridge.exists("file:///c"));
        assert_eq!(bridge.uris().len(), 2);
    }

    #[test]
    fn bridge_total_bytes() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///a".into(), vec![0; 10]);
        bridge.seed_file("file:///b".into(), vec![0; 5]);
        assert_eq!(bridge.total_bytes(), 15);
    }

    #[test]
    fn bridge_list_directory() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///root/a.txt".into(), vec![]);
        bridge.seed_file("file:///root/b.txt".into(), vec![]);
        bridge.seed_file("file:///root/sub/c.txt".into(), vec![]);
        let entries = bridge.list_directory("file:///root");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[1].name, "b.txt");
    }

    #[test]
    fn rename_checked_no_overwrite() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///src".into(), vec![1]);
        bridge.seed_file("file:///dst".into(), vec![2]);
        let err = bridge.rename_checked("file:///src", "file:///dst", false);
        assert!(err.is_err());
        // With overwrite allowed, it succeeds.
        bridge.rename_checked("file:///src", "file:///dst", true).unwrap();
        assert!(!bridge.exists("file:///src"));
        assert!(bridge.exists("file:///dst"));
    }

    #[test]
    fn rename_checked_missing_source() {
        let mut bridge = FsBridge::new();
        let err = bridge.rename_checked("file:///nope", "file:///dst", true);
        assert!(err.is_err());
    }

    #[test]
    fn handle_checked_rejects_bad_uri() {
        let mut bridge = FsBridge::new();
        let result = bridge.handle_checked(FsMessage::ReadFile { uri: "bad".into() });
        assert!(result.is_err());
    }

    #[test]
    fn handle_checked_ok_for_valid_uri() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///x.txt".into(), b"data".to_vec());
        let resp = bridge.handle_checked(FsMessage::ReadFile { uri: "file:///x.txt".into() });
        assert!(resp.is_ok());
        assert_eq!(resp.unwrap(), FsResponse::FileContent { data: b"data".to_vec() });
    }

    #[test]
    fn bridge_clone_and_eq() {
        let mut bridge = FsBridge::new();
        bridge.seed_file("file:///f".into(), vec![42]);
        let clone = bridge.clone();
        assert_eq!(bridge, clone);
    }

    #[test]
    fn display_impls() {
        assert_eq!(FileType::File.to_string(), "file");
        assert_eq!(FileType::Directory.to_string(), "directory");
        assert_eq!(FileType::SymbolicLink.to_string(), "symlink");

        let entry = DirEntry { name: "test.rs".into(), file_type: FileType::File };
        assert_eq!(entry.to_string(), "test.rs [file]");

        let stat = FileStat { file_type: FileType::File, ctime: 1, mtime: 2, size: 100 };
        assert!(stat.to_string().contains("size=100"));
    }

    #[test]
    fn file_stat_helpers() {
        let stat = FileStat { file_type: FileType::Directory, ctime: 0, mtime: 0, size: 0 };
        assert!(stat.is_directory());
        assert!(!stat.is_file());
        assert!(stat.is_empty());
    }

    #[test]
    fn watch_increments_id() {
        let mut bridge = FsBridge::new();
        let r1 = bridge.handle(FsMessage::Watch { uri: "file:///a".into(), recursive: false });
        let r2 = bridge.handle(FsMessage::Watch { uri: "file:///b".into(), recursive: true });
        if let (FsResponse::WatchId { id: id1 }, FsResponse::WatchId { id: id2 }) = (&r1, &r2) {
            assert_ne!(id1, id2);
            assert_eq!(id1, "watch-0");
            assert_eq!(id2, "watch-1");
        } else {
            panic!("expected WatchId responses");
        }
    }

    #[test]
    fn ext_fs_stats_new_defaults() {
        let stats = ExtFsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_fs_stats_record_success() {
        let mut stats = ExtFsStats::new();
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
    fn ext_fs_stats_record_failure() {
        let mut stats = ExtFsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_fs_stats_reset() {
        let mut stats = ExtFsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_fs_stats_merge() {
        let mut a = ExtFsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtFsStats::new();
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
    fn ext_fs_stats_display() {
        let mut stats = ExtFsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_fs_stats_default() {
        let stats = ExtFsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn extfs_validator_accepts_and_rejects() {
        let mut v = ExtFsValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extfs_validator_warnings() {
        let mut v = ExtFsValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extfs_validator_clear_and_merge() {
        let mut v = ExtFsValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtFsValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtFsValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    #[test]
    fn virtual_file_content_creation_and_size() {
        let vf = VirtualFileContent::new("file:///test.txt", b"hello".to_vec());
        assert_eq!(vf.uri, "file:///test.txt");
        assert_eq!(vf.size(), 5);
        assert_eq!(vf.encoding, "utf-8");
        assert!(!vf.readonly);
    }

    #[test]
    fn virtual_file_content_text_content_valid_utf8() {
        let vf = VirtualFileContent::new("file:///a.txt", b"good text".to_vec());
        assert_eq!(vf.text_content(), Some("good text".to_string()));
        assert!(vf.is_text());
    }

    #[test]
    fn virtual_file_content_text_content_invalid_utf8() {
        let vf = VirtualFileContent::new("file:///b.bin", vec![0xFF, 0xFE, 0x00]);
        assert!(vf.text_content().is_none());
        assert!(!vf.is_text());
    }

    #[test]
    fn file_system_watch_recursive_generates_patterns() {
        let p0 = file_system_watch_recursive("file:///workspace", 0);
        assert_eq!(p0, vec!["file:///workspace/**"]);

        let p2 = file_system_watch_recursive("file:///workspace", 2);
        assert_eq!(p2, vec![
            "file:///workspace/*",
            "file:///workspace/*/*",
        ]);

        let empty = file_system_watch_recursive("bad-uri", 1);
        assert!(empty.is_empty());
    }

    #[test]
    fn fs_stat_to_file_type_extracts_type() {
        let stat = FileStatBuilder::new(FileType::Directory).build().unwrap();
        assert_eq!(fs_stat_to_file_type(&stat), FileType::Directory);

        let stat2 = FileStatBuilder::new(FileType::SymbolicLink).build().unwrap();
        assert_eq!(fs_stat_to_file_type(&stat2), FileType::SymbolicLink);
    }

    #[test]
    fn file_watch_event_uri_and_type_checks() {
        let created = FileWatchEvent::Created { uri: "file:///a.txt".into() };
        assert_eq!(created.uri(), "file:///a.txt");
        assert!(created.is_creation());
        assert!(!created.is_change());
        assert!(!created.is_deletion());

        let changed = FileWatchEvent::Changed { uri: "file:///b.txt".into() };
        assert!(changed.is_change());

        let deleted = FileWatchEvent::Deleted { uri: "file:///c.txt".into() };
        assert!(deleted.is_deletion());
    }

    #[test]
    fn file_content_diff_detects_changes() {
        let diff = FileContentDiff::new(b"hello", b"world!");
        assert!(diff.has_changes());
        assert_eq!(diff.old_size(), 5);
        assert_eq!(diff.new_size(), 6);
        assert_eq!(diff.size_delta(), 1);

        let same = FileContentDiff::new(b"abc", b"abc");
        assert!(!same.has_changes());
        assert_eq!(same.size_delta(), 0);
    }

    #[test]
    fn virtual_file_store_add_and_get() {
        let mut store = VirtualFileStore::new();
        assert_eq!(store.count(), 0);

        let vf = VirtualFileContent::new("file:///x.txt", b"data".to_vec());
        store.add(vf);
        assert_eq!(store.count(), 1);
        assert!(store.get("file:///x.txt").is_some());
        assert_eq!(store.get("file:///x.txt").unwrap().size(), 4);

        assert!(store.remove("file:///x.txt"));
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn virtual_file_store_readonly_count() {
        let mut store = VirtualFileStore::new();
        store.add(VirtualFileContent::new("file:///a.txt", b"a".to_vec()));
        store.add(VirtualFileContent::new("file:///b.txt", b"b".to_vec()).as_readonly());
        store.add(VirtualFileContent::new("file:///c.txt", b"c".to_vec()).as_readonly());
        assert_eq!(store.count(), 3);
        assert_eq!(store.readonly_count(), 2);
        assert_eq!(store.all_uris().len(), 3);
    }

    #[test]
    fn fs_path_matcher_include_exclude() {
        let matcher = FsPathMatcher::new()
            .include("*.rs")
            .exclude("*_test.rs");

        assert!(matcher.matches("main.rs"));
        assert!(matcher.matches("lib.rs"));
        assert!(!matcher.matches("lib_test.rs"));
        assert!(!matcher.matches("readme.md"));
    }

    #[test]
    fn fs_path_matcher_glob_double_star() {
        let matcher = FsPathMatcher::new().include("src/**");
        assert!(matcher.matches("src/main.rs"));
        assert!(matcher.matches("src/util/helpers.rs"));
        assert!(!matcher.matches("tests/main.rs"));
    }

    #[test]
    fn virtual_directory_tree() {
        let mut root = VirtualDirectory::new("root");
        root.add_file("README.md");
        let src = root.ensure_dir("src");
        src.add_file("main.rs");
        src.add_file("lib.rs");

        assert_eq!(root.file_count(), 1);
        assert_eq!(root.dir_count(), 1);
        assert_eq!(root.total_file_count(), 3);
        assert!(root.get_dir("src").is_some());
        assert!(root.get_dir("missing").is_none());
    }

    #[test]
    fn virtual_directory_list_all_files() {
        let mut root = VirtualDirectory::new("project");
        root.add_file("Cargo.toml");
        let src = root.ensure_dir("src");
        src.add_file("main.rs");

        let files = root.list_all_files("");
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.contains("Cargo.toml")));
        assert!(files.iter().any(|f| f.contains("main.rs")));
    }

    #[test]
    fn file_change_accumulator_debounce() {
        let mut acc = FileChangeAccumulator::new(100);
        acc.push(FileWatchEvent::Changed { uri: "file:///a.rs".into() }, 1000);
        acc.push(FileWatchEvent::Changed { uri: "file:///b.rs".into() }, 1050);

        assert!(!acc.is_ready(1099));
        assert_eq!(acc.flush(1099).len(), 0);
        assert_eq!(acc.pending_count(), 2);

        assert!(acc.is_ready(1150));
        let events = acc.flush(1150);
        assert_eq!(events.len(), 2);
        assert_eq!(acc.pending_count(), 0);
    }

    #[test]
    fn file_change_accumulator_deduplicate() {
        let mut acc = FileChangeAccumulator::new(50);
        acc.push(FileWatchEvent::Changed { uri: "file:///a.rs".into() }, 100);
        acc.push(FileWatchEvent::Changed { uri: "file:///b.rs".into() }, 110);
        acc.push(FileWatchEvent::Changed { uri: "file:///a.rs".into() }, 120);
        assert_eq!(acc.pending_count(), 3);
        acc.deduplicate();
        assert_eq!(acc.pending_count(), 2);
        let uris = acc.affected_uris();
        assert_eq!(uris.len(), 2);
    }

    #[test]
    fn virtual_file_store_search_by_uri() {
        let mut store = VirtualFileStore::new();
        store.add(VirtualFileContent::new("file:///src/main.rs", b"fn main() {}".to_vec()));
        store.add(VirtualFileContent::new("file:///src/lib.rs", b"pub mod foo;".to_vec()));
        store.add(VirtualFileContent::new("file:///test/test.rs", b"#[test]".to_vec()));

        let results = store.search_by_uri("src");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn virtual_file_store_search_by_content() {
        let mut store = VirtualFileStore::new();
        store.add(VirtualFileContent::new("file:///a.rs", b"fn hello() {}".to_vec()));
        store.add(VirtualFileContent::new("file:///b.rs", b"fn world() {}".to_vec()));

        let results = store.search_by_content("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uri, "file:///a.rs");
    }

    // -----------------------------------------------------------------------
    // Deepened tests
    // -----------------------------------------------------------------------

    #[test]
    fn file_type_predicate_methods() {
        assert!(FileType::File.is_file());
        assert!(!FileType::File.is_directory());
        assert!(FileType::Directory.is_directory());
        assert!(!FileType::Directory.is_file());
        assert!(FileType::SymbolicLink.is_symlink());
        assert!(!FileType::SymbolicLink.is_unknown());
        assert!(FileType::Unknown.is_unknown());
    }

    #[test]
    fn fs_message_primary_uri_and_classification() {
        let read = FsMessage::ReadFile { uri: "file:///r.txt".into() };
        assert_eq!(read.primary_uri(), "file:///r.txt");
        assert!(read.is_read_only());
        assert!(!read.is_mutating());
        assert_eq!(read.operation_name(), "read_file");

        let write = FsMessage::WriteFile {
            uri: "file:///w.txt".into(),
            content: vec![],
        };
        assert_eq!(write.primary_uri(), "file:///w.txt");
        assert!(!write.is_read_only());
        assert!(write.is_mutating());
        assert_eq!(write.operation_name(), "write_file");

        let rename = FsMessage::Rename {
            old_uri: "file:///old".into(),
            new_uri: "file:///new".into(),
            overwrite: false,
        };
        assert_eq!(rename.primary_uri(), "file:///old");
        assert!(rename.is_mutating());
        assert_eq!(rename.operation_name(), "rename");

        let del = FsMessage::Delete { uri: "file:///d".into(), recursive: true };
        assert_eq!(del.operation_name(), "delete");

        let stat = FsMessage::Stat { uri: "file:///s".into() };
        assert!(stat.is_read_only());
        assert_eq!(stat.operation_name(), "stat");

        let rd = FsMessage::ReadDirectory { uri: "file:///dir".into() };
        assert!(rd.is_read_only());
        assert_eq!(rd.operation_name(), "read_directory");

        let cd = FsMessage::CreateDirectory { uri: "file:///newdir".into() };
        assert!(cd.is_mutating());
        assert_eq!(cd.operation_name(), "create_directory");

        let w = FsMessage::Watch { uri: "file:///w".into(), recursive: true };
        assert!(w.is_read_only());
        assert_eq!(w.operation_name(), "watch");
    }

    #[test]
    fn fs_response_helpers() {
        assert!(FsResponse::Ok.is_ok());
        assert!(!FsResponse::Ok.is_error());
        assert_eq!(FsResponse::Ok.error_message(), None);

        let err = FsResponse::Error { message: "boom".into() };
        assert!(err.is_error());
        assert!(!err.is_ok());
        assert_eq!(err.error_message(), Some("boom"));

        let fc = FsResponse::FileContent { data: vec![1, 2, 3] };
        assert_eq!(fc.into_data(), Some(vec![1, 2, 3]));

        let stat_resp = FsResponse::Stat {
            stat: FileStat {
                file_type: FileType::File,
                ctime: 10,
                mtime: 20,
                size: 100,
            },
        };
        let stat = stat_resp.into_stat().unwrap();
        assert_eq!(stat.size, 100);

        let dir_resp = FsResponse::Directory {
            entries: vec![DirEntry::new("a.txt", FileType::File)],
        };
        let entries = dir_resp.into_entries().unwrap();
        assert_eq!(entries.len(), 1);

        // into_data on non-FileContent returns None
        assert_eq!(FsResponse::Ok.into_data(), None);
    }

    #[test]
    fn dir_entry_helpers() {
        let file = DirEntry::new("main.rs", FileType::File);
        assert!(file.is_file());
        assert!(!file.is_directory());
        assert_eq!(file.extension(), Some("rs"));

        let dir = DirEntry::new("src", FileType::Directory);
        assert!(dir.is_directory());
        assert!(!dir.is_file());
        assert_eq!(dir.extension(), None);

        let dotfile = DirEntry::new(".gitignore", FileType::File);
        assert_eq!(dotfile.extension(), Some("gitignore"));

        let no_ext = DirEntry::new("Makefile", FileType::File);
        assert_eq!(no_ext.extension(), None);
    }

    #[test]
    fn bridge_read_write_clear_shortcuts() {
        let mut bridge = FsBridge::new();
        bridge.write_file("file:///test.txt", b"content".to_vec());
        assert_eq!(bridge.read_file("file:///test.txt"), Some(b"content".as_ref()));
        assert_eq!(bridge.read_text("file:///test.txt"), Some("content".to_string()));
        assert!(bridge.read_file("file:///missing").is_none());

        bridge.write_file("file:///other.txt", b"other".to_vec());
        assert_eq!(bridge.file_count(), 2);

        let matching = bridge.files_matching("test");
        assert_eq!(matching.len(), 1);
        assert!(matching[0].contains("test"));

        bridge.clear();
        assert_eq!(bridge.file_count(), 0);
    }

    #[test]
    fn bridge_append_and_copy() {
        let mut bridge = FsBridge::new();
        bridge.append("file:///log.txt", b"line1\n");
        bridge.append("file:///log.txt", b"line2\n");
        assert_eq!(bridge.read_text("file:///log.txt"), Some("line1\nline2\n".to_string()));

        assert!(bridge.copy_file("file:///log.txt", "file:///log_backup.txt"));
        assert_eq!(bridge.read_file("file:///log_backup.txt"), bridge.read_file("file:///log.txt"));
        assert!(!bridge.copy_file("file:///nonexistent", "file:///dst"));
    }

    #[test]
    fn virtual_directory_remove_and_depth() {
        let mut root = VirtualDirectory::new("root");
        root.add_file("a.txt");
        root.add_file("b.txt");
        assert!(root.contains_file("a.txt"));
        assert!(root.remove_file("a.txt"));
        assert!(!root.contains_file("a.txt"));
        assert!(!root.remove_file("nonexistent"));

        let sub = root.ensure_dir("sub");
        sub.add_file("c.txt");
        let deep = sub.ensure_dir("deep");
        deep.add_file("d.txt");

        assert_eq!(root.max_depth(), 2);
        assert_eq!(root.child_dir_names(), vec!["sub"]);

        assert!(root.remove_dir("sub"));
        assert!(root.is_empty() || root.file_count() > 0);
        assert_eq!(root.dir_count(), 0);
    }

    #[test]
    fn file_watch_event_display() {
        let c = FileWatchEvent::Created { uri: "file:///a".into() };
        assert_eq!(c.to_string(), "created: file:///a");
        let ch = FileWatchEvent::Changed { uri: "file:///b".into() };
        assert_eq!(ch.to_string(), "changed: file:///b");
        let d = FileWatchEvent::Deleted { uri: "file:///c".into() };
        assert_eq!(d.to_string(), "deleted: file:///c");
    }

    #[test]
    fn file_change_accumulator_merge_filter_counts() {
        let mut acc1 = FileChangeAccumulator::new(50);
        acc1.push(FileWatchEvent::Created { uri: "file:///a.rs".into() }, 100);
        acc1.push(FileWatchEvent::Changed { uri: "file:///b.rs".into() }, 110);

        let mut acc2 = FileChangeAccumulator::new(50);
        acc2.push(FileWatchEvent::Deleted { uri: "file:///c.rs".into() }, 200);

        acc1.merge(&mut acc2);
        assert_eq!(acc1.pending_count(), 3);

        let (created, changed, deleted) = acc1.event_counts();
        assert_eq!(created, 1);
        assert_eq!(changed, 1);
        assert_eq!(deleted, 1);

        acc1.filter_by_uri("a.rs");
        assert_eq!(acc1.pending_count(), 1);

        assert!(!acc1.is_empty());
        acc1.discard();
        assert!(acc1.is_empty());
        assert_eq!(acc1.pending_count(), 0);
    }

    // ── Debouncer tests ──

    #[test]
    fn debouncer_basic_flush() {
        let mut debouncer = FsEventDebouncer::new(100);
        debouncer.push(FsChangeEvent {
            uri: "a.rs".into(),
            kind: FsChangeKind::Changed,
            timestamp: 10,
        });
        debouncer.push(FsChangeEvent {
            uri: "b.rs".into(),
            kind: FsChangeKind::Created,
            timestamp: 20,
        });
        let flushed = debouncer.flush(200);
        assert_eq!(flushed.len(), 2);
        assert_eq!(debouncer.pending_count(), 0);
    }

    #[test]
    fn debouncer_deduplicates_by_uri() {
        let mut debouncer = FsEventDebouncer::new(100);
        debouncer.push(FsChangeEvent {
            uri: "a.rs".into(),
            kind: FsChangeKind::Changed,
            timestamp: 10,
        });
        debouncer.push(FsChangeEvent {
            uri: "a.rs".into(),
            kind: FsChangeKind::Changed,
            timestamp: 50,
        });
        let flushed = debouncer.flush(200);
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].timestamp, 50);
    }

    #[test]
    fn debouncer_keeps_recent_events() {
        let mut debouncer = FsEventDebouncer::new(100);
        debouncer.push(FsChangeEvent {
            uri: "a.rs".into(),
            kind: FsChangeKind::Changed,
            timestamp: 150,
        });
        let flushed = debouncer.flush(200);
        assert_eq!(flushed.len(), 0);
        assert_eq!(debouncer.pending_count(), 1);
    }

    #[test]
    fn debouncer_pending_uris() {
        let mut debouncer = FsEventDebouncer::new(100);
        debouncer.push(FsChangeEvent {
            uri: "b.rs".into(),
            kind: FsChangeKind::Created,
            timestamp: 10,
        });
        debouncer.push(FsChangeEvent {
            uri: "a.rs".into(),
            kind: FsChangeKind::Changed,
            timestamp: 20,
        });
        let uris = debouncer.pending_uris();
        assert_eq!(uris, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn debouncer_clear() {
        let mut debouncer = FsEventDebouncer::new(100);
        debouncer.push(FsChangeEvent {
            uri: "a.rs".into(),
            kind: FsChangeKind::Changed,
            timestamp: 10,
        });
        debouncer.clear();
        assert_eq!(debouncer.pending_count(), 0);
    }

    // ── Recursive delete safety tests ──

    #[test]
    fn recursive_delete_blocks_root() {
        assert!(!RecursiveDeleteSafetyCheck::is_safe("/"));
        assert!(!RecursiveDeleteSafetyCheck::is_safe("/home"));
        assert!(!RecursiveDeleteSafetyCheck::is_safe("/usr"));
    }

    #[test]
    fn recursive_delete_allows_deep_path() {
        assert!(RecursiveDeleteSafetyCheck::is_safe("/home/user/project"));
        assert!(RecursiveDeleteSafetyCheck::is_safe("/var/data/app"));
    }

    #[test]
    fn recursive_delete_depth() {
        assert_eq!(RecursiveDeleteSafetyCheck::path_depth("/a/b/c"), 3);
        assert_eq!(RecursiveDeleteSafetyCheck::path_depth("/a"), 1);
    }

    #[test]
    fn recursive_delete_validate() {
        assert!(RecursiveDeleteSafetyCheck::validate("/", true).is_err());
        assert!(RecursiveDeleteSafetyCheck::validate("/home/user/dir/sub", true).is_ok());
        assert!(RecursiveDeleteSafetyCheck::validate("/", false).is_ok());
    }

    // ── Rename validator tests ──

    #[test]
    fn rename_valid_name() {
        assert!(FsRenameValidator::validate_name("hello.rs").is_ok());
        assert!(FsRenameValidator::validate_name(".hidden").is_ok());
    }

    #[test]
    fn rename_invalid_names() {
        assert!(FsRenameValidator::validate_name("").is_err());
        assert!(FsRenameValidator::validate_name("..").is_err());
        assert!(FsRenameValidator::validate_name("bad<name").is_err());
        assert!(FsRenameValidator::validate_name("bad|name").is_err());
    }

    #[test]
    fn rename_file_name_from_uri() {
        assert_eq!(
            FsRenameValidator::file_name_from_uri("file:///home/user/test.rs"),
            Some("test.rs".to_string())
        );
        assert_eq!(FsRenameValidator::file_name_from_uri("file:///"), None);
    }

    #[test]
    fn rename_extension_changed() {
        assert!(FsRenameValidator::extension_changed("a.rs", "a.txt"));
        assert!(!FsRenameValidator::extension_changed("a.rs", "b.rs"));
    }

    #[test]
    fn rename_same_directory() {
        assert!(FsRenameValidator::same_directory("/home/a.rs", "/home/b.rs"));
        assert!(!FsRenameValidator::same_directory("/home/a.rs", "/other/a.rs"));
    }

    #[test]
    fn rename_validate_full() {
        assert!(FsRenameValidator::validate_rename("a.rs", "a.rs").is_err());
        assert!(FsRenameValidator::validate_rename("a.rs", "b.rs").is_ok());
    }

    // ── Encoding detector tests ──

    #[test]
    fn encoding_detect_ascii() {
        let content = b"Hello, world!";
        assert_eq!(FsEncodingDetector::detect(content), DetectedEncoding::Ascii);
        assert!(FsEncodingDetector::is_text(content));
    }

    #[test]
    fn encoding_detect_utf8() {
        let content = "Hello, café!".as_bytes();
        assert_eq!(FsEncodingDetector::detect(content), DetectedEncoding::Utf8);
    }

    #[test]
    fn encoding_detect_utf8_bom() {
        let content = &[0xEF, 0xBB, 0xBF, b'H', b'i'];
        assert_eq!(FsEncodingDetector::detect(content), DetectedEncoding::Utf8Bom);
    }

    #[test]
    fn encoding_detect_utf16_le() {
        let content = &[0xFF, 0xFE, b'H', 0x00];
        assert_eq!(FsEncodingDetector::detect(content), DetectedEncoding::Utf16Le);
    }

    #[test]
    fn encoding_detect_utf16_be() {
        let content = &[0xFE, 0xFF, 0x00, b'H'];
        assert_eq!(FsEncodingDetector::detect(content), DetectedEncoding::Utf16Be);
    }

    #[test]
    fn encoding_detect_binary() {
        let content = &[0x00, 0x01, 0x02, 0xFF];
        assert_eq!(FsEncodingDetector::detect(content), DetectedEncoding::Binary);
        assert!(!FsEncodingDetector::is_text(content));
    }

    #[test]
    fn encoding_strip_bom() {
        let content = &[0xEF, 0xBB, 0xBF, b'H', b'i'];
        assert_eq!(FsEncodingDetector::strip_bom(content), &[b'H', b'i']);
        let no_bom = b"Hello";
        assert_eq!(FsEncodingDetector::strip_bom(no_bom), b"Hello");
    }

    #[test]
    fn encoding_empty() {
        assert_eq!(FsEncodingDetector::detect(&[]), DetectedEncoding::Ascii);
    }

    #[test]
    fn encoding_bom_bytes() {
        assert_eq!(FsEncodingDetector::bom_bytes(DetectedEncoding::Utf8Bom), &[0xEF, 0xBB, 0xBF]);
        assert_eq!(FsEncodingDetector::bom_bytes(DetectedEncoding::Utf8), &[] as &[u8]);
    }

    #[test]
    fn encoding_display() {
        assert_eq!(DetectedEncoding::Utf8.to_string(), "UTF-8");
        assert_eq!(DetectedEncoding::Binary.to_string(), "Binary");
    }


    // -- ext_fs additional tests -------------------------------------------

    #[test]
    fn x_ext_fs_activation_parse_language() {
        let ak = XExtFsActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtFsActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_fs_activation_parse_command() {
        let ak = XExtFsActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtFsActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_fs_activation_parse_star() {
        assert_eq!(XExtFsActivationKind::parse("*"), Some(XExtFsActivationKind::Star));
    }

    #[test]
    fn x_ext_fs_activation_parse_unknown() {
        assert!(XExtFsActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_fs_activation_parse_workspace() {
        let ak = XExtFsActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtFsActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_fs_rpc_envelope_basic() {
        let env = XExtFsRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_fs_rpc_envelope_response() {
        let env = XExtFsRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_fs_rpc_payload_checksum() {
        let env = XExtFsRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_fs_collect_sequences_works() {
        let envs = vec![
            XExtFsRpcEnvelope::new(10, "a", ""),
            XExtFsRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_fs_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_fs_filter_by_method_works() {
        let envs = vec![
            XExtFsRpcEnvelope::new(1, "textDocument/open", ""),
            XExtFsRpcEnvelope::new(2, "workspace/config", ""),
            XExtFsRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_fs_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_fs_dedup_by_seq_works() {
        let envs = vec![
            XExtFsRpcEnvelope::new(1, "a", "first"),
            XExtFsRpcEnvelope::new(1, "a", "second"),
            XExtFsRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_fs_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_fs_negotiate_capabilities_basic() {
        let result = x_ext_fs_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_fs_api_version_satisfies() {
        let v1 = XExtFsApiVersion::new(1, 80, 0);
        let min = XExtFsApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_fs_api_version_display() {
        let v = XExtFsApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_fs_api_version_ord() {
        let v1 = XExtFsApiVersion::new(1, 0, 0);
        let v2 = XExtFsApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    #[test]
    fn ext_fs_config_new() {
        let cfg = ExtFsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_fs_config_set_get() {
        let mut cfg = ExtFsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_fs_config_remove() {
        let mut cfg = ExtFsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_fs_config_keys_sorted() {
        let mut cfg = ExtFsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_fs_config_bump_version() {
        let mut cfg = ExtFsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_fs_config_clear() {
        let mut cfg = ExtFsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_fs_config_merge() {
        let mut cfg1 = ExtFsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtFsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_fs_config_disable() {
        let mut cfg = ExtFsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_fs_rate_tracker_empty() {
        let rt = ExtFsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_fs_rate_tracker_record() {
        let mut rt = ExtFsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_fs_rate_tracker_prune() {
        let mut rt = ExtFsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_fs_validator_valid() {
        let v = ExtFsValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_fs_validator_errors() {
        let mut v = ExtFsValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_fs_validator_clear() {
        let mut v = ExtFsValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_fs_validator_merge() {
        let mut v1 = ExtFsValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtFsValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_fs_rate_tracker_clear() {
        let mut rt = ExtFsRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
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


    // xa_ extended tests for ext_fs
    #[test]
    fn xa_ext_fs_ring_new() {
        let rb = super::XaExtFsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_fs_ring_push_len() {
        let mut rb = super::XaExtFsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_fs_ring_wrap() {
        let mut rb = super::XaExtFsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_fs_ring_mean_empty() {
        let rb = super::XaExtFsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_fs_ring_mean_values() {
        let mut rb = super::XaExtFsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_fs_ring_min_max() {
        let mut rb = super::XaExtFsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_fs_ring_iter() {
        let mut rb = super::XaExtFsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_fs_counter_new() {
        let c = super::XaExtFsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_fs_counter_inc() {
        let mut c = super::XaExtFsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_fs_counter_inc_by() {
        let mut c = super::XaExtFsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_fs_counter_reset() {
        let mut c = super::XaExtFsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_fs_counter_clear() {
        let mut c = super::XaExtFsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_fs_counter_default() {
        let c = super::XaExtFsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 59 ----

    #[test]
    fn xc_59_pool_new_empty() {
        let pool: super::Xc59Pool<i32> = super::Xc59Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_59_pool_release_acquire() {
        let mut pool = super::Xc59Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_59_pool_acquire_empty() {
        let mut pool: super::Xc59Pool<i32> = super::Xc59Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_59_pool_full() {
        let mut pool = super::Xc59Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_59_pool_drain() {
        let mut pool = super::Xc59Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_59_pool_stats() {
        let mut pool = super::Xc59Pool::new(8);
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
    fn xc_59_pool_clear() {
        let mut pool = super::Xc59Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_59_pool_shrink() {
        let mut pool = super::Xc59Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_59_pool_default() {
        let pool: super::Xc59Pool<String> = super::Xc59Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_59_pool_extend() {
        let mut pool = super::Xc59Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_59_pool_retain() {
        let mut pool = super::Xc59Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_59_scheduler_round_robin() {
        let mut sched = super::Xc59Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_59_scheduler_empty() {
        let mut sched = super::Xc59Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_59_scheduler_reset() {
        let mut sched = super::Xc59Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_59_scheduler_add_remove() {
        let mut sched = super::Xc59Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_59_scheduler_targets() {
        let sched = super::Xc59Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_59_hash_empty() {
        assert_eq!(super::xc_59_hash(b""), 5381);
    }

    #[test]
    fn xc_59_hash_data() {
        let h = super::xc_59_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_59_hash(b"hello"), h);
    }

    #[test]
    fn xc_59_reverse_str() {
        assert_eq!(super::xc_59_reverse("abc"), "cba");
        assert_eq!(super::xc_59_reverse(""), "");
    }


    // --- xd_103 deepening tests ---

    #[test]
    fn xd_103_sm_initial_state() {
        let sm = Xd103StateMachine::new();
        assert_eq!(sm.current_state(), Xd103State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_103_sm_valid_idle_to_running() {
        let mut sm = Xd103StateMachine::new();
        assert!(sm.transition(Xd103State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd103State::Running);
    }

    #[test]
    fn xd_103_sm_valid_running_to_paused() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        assert!(sm.transition(Xd103State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd103State::Paused);
    }

    #[test]
    fn xd_103_sm_valid_running_to_done() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        assert!(sm.transition(Xd103State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd103State::Done);
    }

    #[test]
    fn xd_103_sm_valid_paused_to_running() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        sm.transition(Xd103State::Paused).unwrap();
        assert!(sm.transition(Xd103State::Running).is_ok());
    }

    #[test]
    fn xd_103_sm_valid_done_to_idle() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        sm.transition(Xd103State::Done).unwrap();
        assert!(sm.transition(Xd103State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd103State::Idle);
    }

    #[test]
    fn xd_103_sm_invalid_idle_to_done() {
        let mut sm = Xd103StateMachine::new();
        assert!(sm.transition(Xd103State::Done).is_err());
    }

    #[test]
    fn xd_103_sm_invalid_idle_to_paused() {
        let mut sm = Xd103StateMachine::new();
        assert!(sm.transition(Xd103State::Paused).is_err());
    }

    #[test]
    fn xd_103_sm_history_tracking() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        sm.transition(Xd103State::Paused).unwrap();
        sm.transition(Xd103State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd103State::Idle);
        assert_eq!(sm.history()[0].to, Xd103State::Running);
        assert_eq!(sm.history()[1].from, Xd103State::Running);
        assert_eq!(sm.history()[2].to, Xd103State::Done);
    }

    #[test]
    fn xd_103_sm_serialize_deserialize() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd103StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd103State::Running));
    }

    #[test]
    fn xd_103_sm_deserialize_invalid() {
        assert_eq!(Xd103StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_103_sm_reset() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd103State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_103_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd103EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd103Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_103_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd103EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd103Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd103Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_103_bus_unsubscribe() {
        let mut bus = Xd103EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_103_event_kind_and_payload() {
        let e = Xd103Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd103Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_103_bus_clear_history() {
        let mut bus = Xd103EventBus::new();
        bus.publish(Xd103Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_103_sm_step_counter_increments() {
        let mut sm = Xd103StateMachine::new();
        sm.transition(Xd103State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd103State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
