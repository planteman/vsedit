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
    fn ext_fs_validator_accepts_valid_name() {
        let v = ExtFsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_fs_validator_rejects_empty() {
        let v = ExtFsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_fs_validator_rejects_too_long() {
        let v = ExtFsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_fs_validator_forbidden_prefix() {
        let v = ExtFsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_fs_validator_allowed_chars() {
        let v = ExtFsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_fs_validator_range() {
        let v = ExtFsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_fs_sanitize_removes_control() {
        let result = ExtFsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_fs_truncate_short_string() {
        assert_eq!(ExtFsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_fs_truncate_long_string() {
        let result = ExtFsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_fs_is_ascii_printable() {
        assert!(ExtFsValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtFsValidator::is_ascii_printable("Hello\x00World"));
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
}
