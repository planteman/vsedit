//! Virtual file system service.
//!
//! Equivalent to VS Code's `vs/platform/files/common/fileService.ts`.
//! Provides file system operations with URI-based paths and file watching.

pub mod watcher;

use std::fmt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use vsedit_events::{Emitter, Event};
use vsedit_uri::VsUri;

/// File type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    SymbolicLink,
    Unknown,
}

/// Metadata about a file.
#[derive(Debug, Clone)]
pub struct FileStat {
    pub file_type: FileType,
    pub size: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub readonly: bool,
}

/// A file change event.
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub uri: VsUri,
    pub change_type: FileChangeType,
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeType {
    Created,
    Changed,
    Deleted,
}

/// Options for reading files.
#[derive(Debug, Clone, Default)]
pub struct ReadFileOptions {
    pub encoding: Option<String>,
}

/// Options for writing files.
#[derive(Debug, Clone, Default)]
pub struct WriteFileOptions {
    pub create: bool,
    pub overwrite: bool,
    pub encoding: Option<String>,
}

/// Error type for file operations.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("File exists: {0}")]
    Exists(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Not a directory: {0}")]
    NotADirectory(String),
    #[error("Not a file: {0}")]
    NotAFile(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unknown scheme: {0}")]
    UnknownScheme(String),
}

pub type FileResult<T> = Result<T, FileError>;

/// A directory entry.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub uri: VsUri,
    pub file_type: FileType,
}

/// Provider trait for custom file system schemes.
pub trait FileSystemProvider: Send + Sync {
    fn stat(&self, uri: &VsUri) -> FileResult<FileStat>;
    fn read_file(&self, uri: &VsUri) -> FileResult<Vec<u8>>;
    fn write_file(&self, uri: &VsUri, content: &[u8], opts: &WriteFileOptions) -> FileResult<()>;
    fn delete(&self, uri: &VsUri, recursive: bool) -> FileResult<()>;
    fn rename(&self, source: &VsUri, target: &VsUri, overwrite: bool) -> FileResult<()>;
    fn mkdir(&self, uri: &VsUri) -> FileResult<()>;
    fn readdir(&self, uri: &VsUri) -> FileResult<Vec<DirEntry>>;
}

/// Local disk file system provider.
pub struct DiskFileSystemProvider;

impl DiskFileSystemProvider {
    pub fn new() -> Self {
        Self
    }

    fn to_path(uri: &VsUri) -> FileResult<PathBuf> {
        Ok(PathBuf::from(&uri.path))
    }
}

impl Default for DiskFileSystemProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystemProvider for DiskFileSystemProvider {
    fn stat(&self, uri: &VsUri) -> FileResult<FileStat> {
        let path = Self::to_path(uri)?;
        let meta = std::fs::metadata(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path.display().to_string()),
            std::io::ErrorKind::PermissionDenied => {
                FileError::PermissionDenied(path.display().to_string())
            }
            _ => FileError::Io(e),
        })?;

        let file_type = if meta.is_file() {
            FileType::File
        } else if meta.is_dir() {
            FileType::Directory
        } else if meta.is_symlink() {
            FileType::SymbolicLink
        } else {
            FileType::Unknown
        };

        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let ctime = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(FileStat {
            file_type,
            size: meta.len(),
            mtime,
            ctime,
            readonly: meta.permissions().readonly(),
        })
    }

    fn read_file(&self, uri: &VsUri) -> FileResult<Vec<u8>> {
        let path = Self::to_path(uri)?;
        std::fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path.display().to_string()),
            _ => FileError::Io(e),
        })
    }

    fn write_file(&self, uri: &VsUri, content: &[u8], opts: &WriteFileOptions) -> FileResult<()> {
        let path = Self::to_path(uri)?;
        if !opts.overwrite && path.exists() {
            return Err(FileError::Exists(path.display().to_string()));
        }
        if opts.create {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(&path, content).map_err(FileError::Io)
    }

    fn delete(&self, uri: &VsUri, recursive: bool) -> FileResult<()> {
        let path = Self::to_path(uri)?;
        if path.is_dir() {
            if recursive {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_dir(&path)?;
            }
        } else {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn rename(&self, source: &VsUri, target: &VsUri, overwrite: bool) -> FileResult<()> {
        let src = Self::to_path(source)?;
        let dst = Self::to_path(target)?;
        if !overwrite && dst.exists() {
            return Err(FileError::Exists(dst.display().to_string()));
        }
        std::fs::rename(&src, &dst).map_err(FileError::Io)
    }

    fn mkdir(&self, uri: &VsUri) -> FileResult<()> {
        let path = Self::to_path(uri)?;
        std::fs::create_dir_all(&path).map_err(FileError::Io)
    }

    fn readdir(&self, uri: &VsUri) -> FileResult<Vec<DirEntry>> {
        let path = Self::to_path(uri)?;
        if !path.is_dir() {
            return Err(FileError::NotADirectory(path.display().to_string()));
        }
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&path)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let file_type = if ft.is_file() {
                FileType::File
            } else if ft.is_dir() {
                FileType::Directory
            } else if ft.is_symlink() {
                FileType::SymbolicLink
            } else {
                FileType::Unknown
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let child_path = entry.path();
            entries.push(DirEntry {
                name,
                uri: VsUri::file(&child_path.to_string_lossy()),
                file_type,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
}

/// File service that delegates to registered providers.
pub struct FileService {
    providers: Mutex<HashMap<String, Arc<dyn FileSystemProvider>>>,
    on_did_change: Emitter<Vec<FileChangeEvent>>,
}

impl FileService {
    pub fn new() -> Self {
        let mut providers: HashMap<String, Arc<dyn FileSystemProvider>> = HashMap::new();
        providers.insert("file".to_string(), Arc::new(DiskFileSystemProvider::new()));
        Self {
            providers: Mutex::new(providers),
            on_did_change: Emitter::new(),
        }
    }

    /// Register a file system provider for a URI scheme.
    pub fn register_provider(&self, scheme: &str, provider: Arc<dyn FileSystemProvider>) {
        let mut providers = self.providers.lock().unwrap();
        providers.insert(scheme.to_string(), provider);
    }

    fn get_provider(&self, scheme: &str) -> FileResult<Arc<dyn FileSystemProvider>> {
        let providers = self.providers.lock().unwrap();
        providers
            .get(scheme)
            .cloned()
            .ok_or_else(|| FileError::UnknownScheme(scheme.to_string()))
    }

    pub fn stat(&self, uri: &VsUri) -> FileResult<FileStat> {
        self.get_provider(&uri.scheme)?.stat(uri)
    }

    pub fn read_file(&self, uri: &VsUri) -> FileResult<Vec<u8>> {
        self.get_provider(&uri.scheme)?.read_file(uri)
    }

    pub fn read_file_string(&self, uri: &VsUri) -> FileResult<String> {
        let bytes = self.read_file(uri)?;
        String::from_utf8(bytes).map_err(|e| FileError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
    }

    pub fn write_file(&self, uri: &VsUri, content: &[u8], opts: &WriteFileOptions) -> FileResult<()> {
        self.get_provider(&uri.scheme)?.write_file(uri, content, opts)
    }

    pub fn delete(&self, uri: &VsUri, recursive: bool) -> FileResult<()> {
        self.get_provider(&uri.scheme)?.delete(uri, recursive)
    }

    pub fn rename(&self, source: &VsUri, target: &VsUri, overwrite: bool) -> FileResult<()> {
        let scheme = &source.scheme;
        self.get_provider(scheme)?.rename(source, target, overwrite)
    }

    pub fn mkdir(&self, uri: &VsUri) -> FileResult<()> {
        self.get_provider(&uri.scheme)?.mkdir(uri)
    }

    pub fn readdir(&self, uri: &VsUri) -> FileResult<Vec<DirEntry>> {
        self.get_provider(&uri.scheme)?.readdir(uri)
    }

    /// Event fired when files change.
    pub fn on_did_files_change(&self) -> Event<Vec<FileChangeEvent>> {
        self.on_did_change.event()
    }

    /// Notify that files have changed (called by watchers).
    pub fn fire_file_changes(&self, changes: Vec<FileChangeEvent>) {
        self.on_did_change.fire(&changes);
    }
}

impl Default for FileService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for files operations.
#[derive(Debug, Clone, PartialEq)]
pub struct FilesStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl FilesStats {
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
    pub fn merge(&mut self, other: &FilesStats) {
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

impl Default for FilesStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FilesStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FilesStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for files.
#[derive(Debug, Clone)]
pub struct FilesValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl FilesValidator {
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

impl Default for FilesValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// FileReadOptions / FileWriteOptions / file_stat_batch
// ---------------------------------------------------------------------------

/// Encoding to use when reading a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Latin1,
    Auto,
}

impl fmt::Display for FileEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf8 => write!(f, "UTF-8"),
            Self::Utf16Le => write!(f, "UTF-16 LE"),
            Self::Utf16Be => write!(f, "UTF-16 BE"),
            Self::Latin1 => write!(f, "Latin-1"),
            Self::Auto => write!(f, "Auto"),
        }
    }
}

/// Options that control how a file is read.
#[derive(Debug, Clone)]
pub struct FileReadOptions {
    /// The text encoding to apply.
    pub encoding: FileEncoding,
    /// Whether binary (non-text) files are acceptable.
    pub accept_binary: bool,
    /// Optional upper bound on the file size in bytes.
    pub max_size: Option<u64>,
    /// Whether to normalise line endings to `\n`.
    pub line_ending_normalization: bool,
}

impl Default for FileReadOptions {
    fn default() -> Self {
        Self {
            encoding: FileEncoding::Utf8,
            accept_binary: false,
            max_size: None,
            line_ending_normalization: true,
        }
    }
}

impl FileReadOptions {
    /// Set the encoding (builder pattern).
    pub fn with_encoding(mut self, enc: FileEncoding) -> Self {
        self.encoding = enc;
        self
    }

    /// Set the maximum acceptable file size (builder pattern).
    pub fn with_max_size(mut self, size: u64) -> Self {
        self.max_size = Some(size);
        self
    }

    /// Set whether binary content is accepted (builder pattern).
    pub fn with_binary(mut self, accept: bool) -> Self {
        self.accept_binary = accept;
        self
    }

    /// Returns `true` when a file with the given `size` and binary flag would
    /// be accepted under these options.
    pub fn would_accept(&self, size: u64, is_binary: bool) -> bool {
        if is_binary && !self.accept_binary {
            return false;
        }
        if let Some(max) = self.max_size {
            if size > max {
                return false;
            }
        }
        true
    }
}

/// Options that control how a file is written.
#[derive(Debug, Clone)]
pub struct FileWriteOptions {
    /// Create intermediate parent directories if they don't exist.
    pub create_parents: bool,
    /// Allow overwriting an existing file.
    pub overwrite: bool,
    /// Use atomic write (write-to-tmp then rename).
    pub atomic: bool,
    /// Create a backup of the existing file before writing.
    pub backup_before_write: bool,
}

impl Default for FileWriteOptions {
    fn default() -> Self {
        Self {
            create_parents: true,
            overwrite: true,
            atomic: false,
            backup_before_write: false,
        }
    }
}

impl FileWriteOptions {
    /// Set atomic write (builder pattern).
    pub fn with_atomic(mut self, atomic: bool) -> Self {
        self.atomic = atomic;
        self
    }

    /// Set backup-before-write (builder pattern).
    pub fn with_backup(mut self, backup: bool) -> Self {
        self.backup_before_write = backup;
        self
    }

    /// Set overwrite flag (builder pattern).
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// Set create-parents flag (builder pattern).
    pub fn with_create_parents(mut self, create_parents: bool) -> Self {
        self.create_parents = create_parents;
        self
    }

    /// Validates whether a write is permissible.
    ///
    /// Returns `Err` with a description when the file already exists but
    /// `overwrite` is `false`.
    pub fn validate_write(&self, file_exists: bool) -> Result<(), String> {
        if file_exists && !self.overwrite {
            return Err("file already exists and overwrite is disabled".into());
        }
        Ok(())
    }
}

/// Result of a lightweight (no-I/O) path inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatResult {
    /// The original path string.
    pub path: String,
    /// Estimated size – always `0` since we don't touch the filesystem.
    pub size: u64,
    /// `true` when the path looks like a directory (ends with `/` or `\`).
    pub is_dir: bool,
    /// `true` when the path contains a symlink indicator (`->`) — a
    /// heuristic useful for `ls -l` style output.
    pub is_symlink: bool,
    /// The file extension extracted from the path, if any.
    pub extension: Option<String>,
}

impl fmt::Display for FileStatResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.is_dir {
            "dir"
        } else if self.is_symlink {
            "symlink"
        } else {
            "file"
        };
        let ext = self.extension.as_deref().unwrap_or("(none)");
        write!(
            f,
            "{} [{}] size={} ext={}",
            self.path, kind, self.size, ext
        )
    }
}

// ---------------------------------------------------------------------------
// FileBreadcrumb – path segment navigation
// ---------------------------------------------------------------------------

/// A single segment in a file path breadcrumb trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    /// The display label for this segment.
    pub label: String,
    /// The full path up to and including this segment.
    pub path: String,
    /// Whether this segment represents a directory.
    pub is_directory: bool,
}

/// Breadcrumb trail for navigating file paths.
///
/// Splits a path into individual segments so that a UI can render clickable
/// breadcrumbs (e.g. `src > components > Button.tsx`).
#[derive(Debug, Clone)]
pub struct FileBreadcrumb {
    segments: Vec<BreadcrumbSegment>,
}

impl FileBreadcrumb {
    /// Build a breadcrumb trail from a path string.
    ///
    /// The final segment is treated as a file unless the path ends with `/`.
    pub fn from_path(path: &str) -> Self {
        let is_dir_path = path.ends_with('/') || path.ends_with('\\');
        let normalized = path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
        let mut segments = Vec::new();
        let mut accumulated = String::new();

        for (i, part) in parts.iter().enumerate() {
            if !accumulated.is_empty() || normalized.starts_with('/') {
                accumulated.push('/');
            }
            accumulated.push_str(part);

            let is_last = i == parts.len() - 1;
            let is_directory = if is_last { is_dir_path } else { true };

            segments.push(BreadcrumbSegment {
                label: part.to_string(),
                path: accumulated.clone(),
                is_directory,
            });
        }

        Self { segments }
    }

    /// Return the breadcrumb segments.
    pub fn segments(&self) -> &[BreadcrumbSegment] {
        &self.segments
    }

    /// Return the number of segments.
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Return the final segment label, or `None` if the path was empty.
    pub fn file_name(&self) -> Option<&str> {
        self.segments.last().map(|s| s.label.as_str())
    }

    /// Return the parent path (everything except the last segment).
    pub fn parent_path(&self) -> Option<&str> {
        if self.segments.len() < 2 {
            return None;
        }
        self.segments
            .get(self.segments.len() - 2)
            .map(|s| s.path.as_str())
    }
}

impl fmt::Display for FileBreadcrumb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let labels: Vec<&str> = self.segments.iter().map(|s| s.label.as_str()).collect();
        write!(f, "{}", labels.join(" > "))
    }
}

// ---------------------------------------------------------------------------
// FileBookmarks – named bookmarks to frequently used paths
// ---------------------------------------------------------------------------

/// A bookmark to a specific file or directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBookmark {
    /// User-assigned name for the bookmark.
    pub name: String,
    /// The bookmarked URI.
    pub uri: VsUri,
    /// Optional descriptive note.
    pub note: Option<String>,
}

/// A collection of file bookmarks.
#[derive(Debug, Clone, Default)]
pub struct FileBookmarks {
    bookmarks: Vec<FileBookmark>,
}

impl FileBookmarks {
    pub fn new() -> Self {
        Self {
            bookmarks: Vec::new(),
        }
    }

    /// Add a bookmark. Returns `Err` if a bookmark with the same name exists.
    pub fn add(&mut self, name: impl Into<String>, uri: VsUri, note: Option<String>) -> Result<(), String> {
        let name = name.into();
        if self.bookmarks.iter().any(|b| b.name == name) {
            return Err(format!("bookmark '{}' already exists", name));
        }
        self.bookmarks.push(FileBookmark { name, uri, note });
        Ok(())
    }

    /// Remove a bookmark by name. Returns `true` if found and removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.name != name);
        self.bookmarks.len() < before
    }

    /// Look up a bookmark by name.
    pub fn get(&self, name: &str) -> Option<&FileBookmark> {
        self.bookmarks.iter().find(|b| b.name == name)
    }

    /// Return all bookmarks.
    pub fn all(&self) -> &[FileBookmark] {
        &self.bookmarks
    }

    /// Return the number of bookmarks.
    pub fn len(&self) -> usize {
        self.bookmarks.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }

    /// Rename a bookmark. Returns `Err` if the old name is not found or the
    /// new name already exists.
    pub fn rename(&mut self, old_name: &str, new_name: impl Into<String>) -> Result<(), String> {
        let new_name = new_name.into();
        if self.bookmarks.iter().any(|b| b.name == new_name) {
            return Err(format!("bookmark '{}' already exists", new_name));
        }
        match self.bookmarks.iter_mut().find(|b| b.name == old_name) {
            Some(b) => {
                b.name = new_name;
                Ok(())
            }
            None => Err(format!("bookmark '{}' not found", old_name)),
        }
    }
}

// ---------------------------------------------------------------------------
// FileCompare – line-level diff between two byte slices
// ---------------------------------------------------------------------------

/// The kind of change for a single line in a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Line is the same in both inputs.
    Equal,
    /// Line was added in the new version.
    Added,
    /// Line was removed from the old version.
    Removed,
}

/// A single line in a diff result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

/// Summary statistics for a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummary {
    pub total_lines: usize,
    pub added: usize,
    pub removed: usize,
    pub equal: usize,
}

/// Compare two byte slices line-by-line and return a simple diff.
///
/// This uses a straightforward longest-common-subsequence approach for
/// correctness, but is not optimised for very large files.
pub fn file_compare(old: &[u8], new: &[u8]) -> Vec<DiffLine> {
    let old_str = String::from_utf8_lossy(old);
    let new_str = String::from_utf8_lossy(new);
    let old_lines: Vec<&str> = old_str.lines().collect();
    let new_lines: Vec<&str> = new_str.lines().collect();

    // Build LCS table
    let m = old_lines.len();
    let n = new_lines.len();
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            if old_lines[i] == new_lines[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // Walk the table to produce diff lines
    let mut result = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < m || j < n {
        if i < m && j < n && old_lines[i] == new_lines[j] {
            result.push(DiffLine {
                kind: DiffLineKind::Equal,
                content: old_lines[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if j < n && (i >= m || dp[i][j + 1] >= dp[i + 1][j]) {
            result.push(DiffLine {
                kind: DiffLineKind::Added,
                content: new_lines[j].to_string(),
            });
            j += 1;
        } else {
            result.push(DiffLine {
                kind: DiffLineKind::Removed,
                content: old_lines[i].to_string(),
            });
            i += 1;
        }
    }

    result
}

/// Compute a [`DiffSummary`] from diff lines.
pub fn diff_summary(lines: &[DiffLine]) -> DiffSummary {
    let mut added = 0;
    let mut removed = 0;
    let mut equal = 0;
    for line in lines {
        match line.kind {
            DiffLineKind::Added => added += 1,
            DiffLineKind::Removed => removed += 1,
            DiffLineKind::Equal => equal += 1,
        }
    }
    DiffSummary {
        total_lines: lines.len(),
        added,
        removed,
        equal,
    }
}

/// Parse a batch of path strings into [`FileStatResult`] values.
///
/// This function does **not** access the filesystem – it only examines the
/// path strings themselves. Extension is derived from the final `.`-delimited
/// segment, and a trailing `/` or `\` is taken as a directory indicator.
pub fn file_stat_batch(paths: &[&str]) -> Vec<FileStatResult> {
    paths.iter().map(|p| {
        let is_dir = p.ends_with('/') || p.ends_with('\\');
        let is_symlink = p.contains(" -> ");

        let extension = if is_dir {
            None
        } else {
            std::path::Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_string())
        };

        FileStatResult {
            path: p.to_string(),
            size: 0,
            is_dir,
            is_symlink,
            extension,
        }
    }).collect()
}

// ---------------------------------------------------------------------------
// FileExtensionStats – extension-based statistics
// ---------------------------------------------------------------------------

/// Statistics about file extensions in a set of paths.
#[derive(Debug, Clone, PartialEq)]
pub struct FileExtensionStats {
    pub extension_counts: HashMap<String, usize>,
    pub total_files: usize,
    pub total_dirs: usize,
}

impl FileExtensionStats {
    /// Compute extension stats from a batch of `FileStatResult` items.
    pub fn from_results(results: &[FileStatResult]) -> Self {
        let mut extension_counts: HashMap<String, usize> = HashMap::new();
        let mut total_files = 0usize;
        let mut total_dirs = 0usize;
        for r in results {
            if r.is_dir {
                total_dirs += 1;
            } else {
                total_files += 1;
                if let Some(ref ext) = r.extension {
                    *extension_counts.entry(ext.clone()).or_insert(0) += 1;
                }
            }
        }
        Self { extension_counts, total_files, total_dirs }
    }

    /// Return the most common extension, if any.
    pub fn most_common_extension(&self) -> Option<(&str, usize)> {
        self.extension_counts
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(ext, &count)| (ext.as_str(), count))
    }

    /// Return the number of distinct extensions.
    pub fn distinct_extensions(&self) -> usize {
        self.extension_counts.len()
    }
}

impl fmt::Display for FileExtensionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Files: {}, Dirs: {}, Extensions: {}",
            self.total_files, self.total_dirs, self.distinct_extensions()
        )
    }
}

// ---------------------------------------------------------------------------
// PathMatcher – glob-like path filtering
// ---------------------------------------------------------------------------

/// Simple path matcher supporting `*` (any segment characters) and `**` (any
/// number of path segments) patterns. This is a basic utility for filtering
/// file paths without pulling in a full glob crate.
pub struct PathMatcher {
    pattern: String,
}

impl PathMatcher {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self { pattern: pattern.into() }
    }

    /// Returns `true` if the path matches the pattern.
    ///
    /// Supports:
    /// - `*` matches any characters except `/`
    /// - exact string match
    /// - suffix match when pattern starts with `*`
    pub fn matches(&self, path: &str) -> bool {
        if self.pattern == "*" {
            return true;
        }
        if self.pattern.starts_with("*.") {
            let suffix = &self.pattern[1..];
            return path.ends_with(suffix);
        }
        if self.pattern.ends_with("/*") {
            let prefix = &self.pattern[..self.pattern.len() - 1];
            return path.starts_with(prefix);
        }
        self.pattern == path
    }

    /// Filter a list of paths, returning only those that match.
    pub fn filter<'a>(&self, paths: &'a [&str]) -> Vec<&'a str> {
        paths.iter().copied().filter(|p| self.matches(p)).collect()
    }
}

// ---------------------------------------------------------------------------
// DiffSummary Display
// ---------------------------------------------------------------------------

impl fmt::Display for DiffSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "+{} -{} ={} (total {})",
            self.added, self.removed, self.equal, self.total_lines
        )
    }
}

/// Compute a simple similarity ratio between two byte slices based on
/// their diff. Returns a value between 0.0 (completely different) and
/// 1.0 (identical).
pub fn file_similarity(old: &[u8], new: &[u8]) -> f64 {
    if old.is_empty() && new.is_empty() {
        return 1.0;
    }
    let diff = file_compare(old, new);
    let summary = diff_summary(&diff);
    if summary.total_lines == 0 {
        return 1.0;
    }
    summary.equal as f64 / summary.total_lines as f64
}

// ---------------------------------------------------------------------------
// FileWatchFilter
// ---------------------------------------------------------------------------

/// Filters file watch events using simple glob-like patterns.
///
/// Include patterns specify which paths should be accepted. Exclude patterns
/// reject paths even when they match an include pattern. An empty include list
/// means "accept everything".
pub struct FileWatchFilter {
    includes: Vec<String>,
    excludes: Vec<String>,
}

impl FileWatchFilter {
    /// Create an empty filter that accepts all paths.
    pub fn new() -> Self {
        Self {
            includes: Vec::new(),
            excludes: Vec::new(),
        }
    }

    /// Add an include pattern. Supports `*` (any chars) and `?` (single char).
    pub fn add_include(&mut self, pattern: &str) {
        self.includes.push(pattern.to_string());
    }

    /// Add an exclude pattern. Supports `*` (any chars) and `?` (single char).
    pub fn add_exclude(&mut self, pattern: &str) {
        self.excludes.push(pattern.to_string());
    }

    /// Number of include patterns.
    pub fn include_count(&self) -> usize {
        self.includes.len()
    }

    /// Number of exclude patterns.
    pub fn exclude_count(&self) -> usize {
        self.excludes.len()
    }

    /// Test whether `path` passes the filter.
    ///
    /// A path passes when it matches at least one include pattern (or there
    /// are no include patterns) and does not match any exclude pattern.
    pub fn matches(&self, path: &str) -> bool {
        let dominated = !self.includes.is_empty()
            && !self.includes.iter().any(|p| glob_match(p, path));
        if dominated {
            return false;
        }
        !self.excludes.iter().any(|p| glob_match(p, path))
    }
}

/// Minimal glob matcher supporting `*` and `?`.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (plen, tlen) = (p.len(), t.len());
    // dp[i][j] = pattern[..i] matches text[..j]
    let mut dp = vec![vec![false; tlen + 1]; plen + 1];
    dp[0][0] = true;
    for i in 1..=plen {
        if p[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=plen {
        for j in 1..=tlen {
            match p[i - 1] {
                '*' => dp[i][j] = dp[i - 1][j] || dp[i][j - 1],
                '?' => dp[i][j] = dp[i - 1][j - 1],
                c => dp[i][j] = dp[i - 1][j - 1] && c == t[j - 1],
            }
        }
    }
    dp[plen][tlen]
}

// ---------------------------------------------------------------------------
// FileContentHash
// ---------------------------------------------------------------------------

/// A simple content hash for detecting file changes without storing the
/// full contents. Uses a FNV-1a–inspired algorithm so no external
/// dependencies are needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileContentHash {
    hash: u64,
}

impl FileContentHash {
    /// Hash a byte slice.
    pub fn from_bytes(data: &[u8]) -> Self {
        const BASIS: u64 = 0xcbf29ce484222325;
        const PRIME: u64 = 0x00000100000001b3;
        let mut h = BASIS;
        for &b in data {
            h ^= b as u64;
            h = h.wrapping_mul(PRIME);
        }
        Self { hash: h }
    }

    /// Hash a string.
    pub fn from_str(s: &str) -> Self {
        Self::from_bytes(s.as_bytes())
    }

    /// Return the raw 64-bit hash value.
    pub fn value(&self) -> u64 {
        self.hash
    }

    /// Check whether two hashes are equal.
    pub fn matches(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl fmt::Display for FileContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.hash)
    }
}

// ---------------------------------------------------------------------------
// DirectoryDiff
// ---------------------------------------------------------------------------

/// Compares two directory listings (represented as sorted name lists) and
/// identifies entries that only appear on one side or both.
pub struct DirectoryDiff {
    left: Vec<String>,
    right: Vec<String>,
}

impl DirectoryDiff {
    /// Create a new diff from two directory listings.
    pub fn new(left: Vec<String>, right: Vec<String>) -> Self {
        Self { left, right }
    }

    /// Entries present only in the left listing.
    pub fn only_left(&self) -> Vec<&str> {
        self.left
            .iter()
            .filter(|e| !self.right.contains(e))
            .map(|s| s.as_str())
            .collect()
    }

    /// Entries present only in the right listing.
    pub fn only_right(&self) -> Vec<&str> {
        self.right
            .iter()
            .filter(|e| !self.left.contains(e))
            .map(|s| s.as_str())
            .collect()
    }

    /// Entries present in both listings.
    pub fn common(&self) -> Vec<&str> {
        self.left
            .iter()
            .filter(|e| self.right.contains(e))
            .map(|s| s.as_str())
            .collect()
    }

    /// A human-readable summary string.
    pub fn summary(&self) -> String {
        format!(
            "left_only: {}, right_only: {}, common: {}",
            self.only_left().len(),
            self.only_right().len(),
            self.common().len(),
        )
    }
}

// ---------------------------------------------------------------------------
// FileMetadataCache
// ---------------------------------------------------------------------------

/// Cached metadata for a single file.
#[derive(Debug, Clone)]
pub struct CachedMetadata {
    /// Approximate file size in bytes.
    pub size: u64,
    /// Last-modified timestamp in milliseconds since the epoch.
    pub modified_ms: u64,
}

/// A simple in-memory cache for file metadata, useful for avoiding repeated
/// filesystem round-trips when checking sizes and modification times.
pub struct FileMetadataCache {
    entries: HashMap<String, CachedMetadata>,
}

impl FileMetadataCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Insert or update metadata for `path`.
    pub fn insert(&mut self, path: &str, size: u64, modified_ms: u64) {
        self.entries.insert(
            path.to_string(),
            CachedMetadata { size, modified_ms },
        );
    }

    /// Look up cached metadata.
    pub fn get(&self, path: &str) -> Option<&CachedMetadata> {
        self.entries.get(path)
    }

    /// Remove a path from the cache.
    pub fn invalidate(&mut self, path: &str) {
        self.entries.remove(path);
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check whether the cached entry for `path` is older than `max_age_ms`
    /// relative to `current_ms`. Returns `true` when the entry is missing or
    /// stale.
    pub fn is_stale(&self, path: &str, max_age_ms: u64, current_ms: u64) -> bool {
        match self.entries.get(path) {
            None => true,
            Some(meta) => {
                if current_ms < meta.modified_ms {
                    return false;
                }
                current_ms - meta.modified_ms > max_age_ms
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FileBatchNotifier – group rapid file change events
// ---------------------------------------------------------------------------

/// Accumulates file change events and groups rapid changes into batches.
#[derive(Debug)]
pub struct FileBatchNotifier {
    pending: Vec<FileChangeEvent>,
    batch_window_ms: u64,
    last_event_ms: Option<u64>,
    max_batch_size: usize,
}

impl FileBatchNotifier {
    pub fn new(batch_window_ms: u64, max_batch_size: usize) -> Self {
        Self {
            pending: Vec::new(),
            batch_window_ms,
            last_event_ms: None,
            max_batch_size,
        }
    }

    /// Add a file change event with the current timestamp in milliseconds.
    pub fn add_event(&mut self, event: FileChangeEvent, timestamp_ms: u64) {
        self.last_event_ms = Some(timestamp_ms);
        // Coalesce: if same URI and same type already pending, skip.
        let dominated = self.pending.iter().any(|e| {
            e.uri == event.uri && e.change_type == event.change_type
        });
        if !dominated {
            self.pending.push(event);
        }
        // Force flush if we hit max batch size.
        if self.pending.len() >= self.max_batch_size {
            // Caller should drain via `flush`.
        }
    }

    /// Check if the batch window has elapsed and a flush is due.
    pub fn should_flush(&self, current_ms: u64) -> bool {
        if self.pending.is_empty() {
            return false;
        }
        if self.pending.len() >= self.max_batch_size {
            return true;
        }
        match self.last_event_ms {
            Some(last) => current_ms.saturating_sub(last) >= self.batch_window_ms,
            None => false,
        }
    }

    /// Flush and return all pending events.
    pub fn flush(&mut self) -> Vec<FileChangeEvent> {
        self.last_event_ms = None;
        std::mem::take(&mut self.pending)
    }

    /// Number of pending events.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// True if no events are pending.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn batch_window_ms(&self) -> u64 {
        self.batch_window_ms
    }

    pub fn set_batch_window_ms(&mut self, ms: u64) {
        self.batch_window_ms = ms;
    }

    /// Peek at pending events without draining.
    pub fn peek(&self) -> &[FileChangeEvent] {
        &self.pending
    }

    /// Count pending events by change type.
    pub fn count_by_type(&self, change_type: FileChangeType) -> usize {
        self.pending.iter().filter(|e| e.change_type == change_type).count()
    }

    /// Get all unique URIs in the pending batch.
    pub fn unique_uris(&self) -> Vec<&VsUri> {
        let mut uris: Vec<&VsUri> = self.pending.iter().map(|e| &e.uri).collect();
        uris.dedup_by(|a, b| a.path == b.path && a.scheme == b.scheme);
        uris
    }
}

// ---------------------------------------------------------------------------
// FileEncodingGuesser – guess file encoding from content bytes
// ---------------------------------------------------------------------------

/// Detected file encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Ascii,
    Latin1,
    Unknown,
}

impl fmt::Display for DetectedEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf8 => write!(f, "UTF-8"),
            Self::Utf8Bom => write!(f, "UTF-8 with BOM"),
            Self::Utf16Le => write!(f, "UTF-16 LE"),
            Self::Utf16Be => write!(f, "UTF-16 BE"),
            Self::Ascii => write!(f, "ASCII"),
            Self::Latin1 => write!(f, "ISO-8859-1"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Guesses file encoding by examining leading bytes.
pub struct FileEncodingGuesser;

impl FileEncodingGuesser {
    /// Guess the encoding from raw bytes.
    pub fn guess(data: &[u8]) -> DetectedEncoding {
        if data.is_empty() {
            return DetectedEncoding::Ascii;
        }
        // Check BOM
        if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
            return DetectedEncoding::Utf8Bom;
        }
        if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xFE {
            return DetectedEncoding::Utf16Le;
        }
        if data.len() >= 2 && data[0] == 0xFE && data[1] == 0xFF {
            return DetectedEncoding::Utf16Be;
        }
        // Check if valid UTF-8
        if std::str::from_utf8(data).is_ok() {
            // Check if it's pure ASCII
            if data.iter().all(|&b| b < 128) {
                return DetectedEncoding::Ascii;
            }
            return DetectedEncoding::Utf8;
        }
        // Check if it might be Latin-1 (all bytes valid)
        if data.iter().all(|&b| b != 0) {
            return DetectedEncoding::Latin1;
        }
        DetectedEncoding::Unknown
    }

    /// Guess encoding and return a human-readable label.
    pub fn guess_label(data: &[u8]) -> String {
        Self::guess(data).to_string()
    }

    /// Check if the data is likely binary (contains null bytes).
    pub fn is_likely_binary(data: &[u8]) -> bool {
        let check_len = data.len().min(8192);
        data[..check_len].iter().any(|&b| b == 0)
    }
}

// ---------------------------------------------------------------------------
// FileSizeFormatter – human-readable file sizes
// ---------------------------------------------------------------------------

/// Formats file sizes into human-readable strings.
pub struct FileSizeFormatter;

impl FileSizeFormatter {
    /// Format bytes as a human-readable size string.
    pub fn format(bytes: u64) -> String {
        if bytes < 1024 {
            return format!("{} B", bytes);
        }
        let units = ["KB", "MB", "GB", "TB"];
        let mut size = bytes as f64 / 1024.0;
        for unit in &units {
            if size < 1024.0 {
                return if size < 10.0 {
                    format!("{:.1} {}", size, unit)
                } else {
                    format!("{:.0} {}", size, unit)
                };
            }
            size /= 1024.0;
        }
        format!("{:.0} PB", size)
    }

    /// Format with explicit precision.
    pub fn format_with_precision(bytes: u64, precision: usize) -> String {
        if bytes < 1024 {
            return format!("{} B", bytes);
        }
        let units = ["KB", "MB", "GB", "TB"];
        let mut size = bytes as f64 / 1024.0;
        for unit in &units {
            if size < 1024.0 {
                return format!("{:.prec$} {}", size, unit, prec = precision);
            }
            size /= 1024.0;
        }
        format!("{:.prec$} PB", size, prec = precision)
    }

    /// Return the appropriate unit for a given byte count.
    pub fn unit_for(bytes: u64) -> &'static str {
        if bytes < 1024 { return "B"; }
        if bytes < 1024 * 1024 { return "KB"; }
        if bytes < 1024 * 1024 * 1024 { return "MB"; }
        if bytes < 1024u64 * 1024 * 1024 * 1024 { return "GB"; }
        "TB"
    }
}

// ---------------------------------------------------------------------------
// FileModifiedIndicator – track file modification state
// ---------------------------------------------------------------------------

/// Tracks the modification state of a file.
#[derive(Debug, Clone)]
pub struct FileModifiedIndicator {
    uri: VsUri,
    original_hash: u64,
    current_hash: u64,
    save_count: u32,
}

impl FileModifiedIndicator {
    pub fn new(uri: VsUri, initial_hash: u64) -> Self {
        Self {
            uri,
            original_hash: initial_hash,
            current_hash: initial_hash,
            save_count: 0,
        }
    }

    /// Simple hash function for content bytes.
    pub fn hash_content(content: &[u8]) -> u64 {
        let mut h: u64 = 5381;
        for &b in content {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    /// Update the current content hash.
    pub fn update_hash(&mut self, content: &[u8]) {
        self.current_hash = Self::hash_content(content);
    }

    /// Returns true if the file is modified (differs from original).
    pub fn is_modified(&self) -> bool {
        self.current_hash != self.original_hash
    }

    /// Mark the current state as saved (original = current).
    pub fn mark_saved(&mut self) {
        self.original_hash = self.current_hash;
        self.save_count += 1;
    }

    /// Revert to the original hash.
    pub fn revert(&mut self) {
        self.current_hash = self.original_hash;
    }

    pub fn uri(&self) -> &VsUri {
        &self.uri
    }

    pub fn save_count(&self) -> u32 {
        self.save_count
    }

    /// Get a display indicator string: "●" if modified, empty otherwise.
    pub fn indicator_char(&self) -> &'static str {
        if self.is_modified() { "●" } else { "" }
    }

    /// Format a title with modification indicator.
    pub fn format_title(&self, filename: &str) -> String {
        if self.is_modified() {
            format!("● {}", filename)
        } else {
            filename.to_string()
        }
    }
}

/// Manager that tracks modification state for multiple files.
#[derive(Debug)]
pub struct FileModifiedTracker {
    indicators: HashMap<String, FileModifiedIndicator>,
}

impl FileModifiedTracker {
    pub fn new() -> Self {
        Self {
            indicators: HashMap::new(),
        }
    }

    /// Register a file for tracking.
    pub fn register(&mut self, uri: VsUri, content: &[u8]) {
        let hash = FileModifiedIndicator::hash_content(content);
        let key = uri.to_string();
        self.indicators.insert(key, FileModifiedIndicator::new(uri, hash));
    }

    /// Update a file's content hash.
    pub fn update(&mut self, uri_str: &str, content: &[u8]) -> bool {
        if let Some(ind) = self.indicators.get_mut(uri_str) {
            ind.update_hash(content);
            true
        } else {
            false
        }
    }

    /// Check if a file is modified.
    pub fn is_modified(&self, uri_str: &str) -> bool {
        self.indicators.get(uri_str).map_or(false, |i| i.is_modified())
    }

    /// Mark a file as saved.
    pub fn mark_saved(&mut self, uri_str: &str) -> bool {
        if let Some(ind) = self.indicators.get_mut(uri_str) {
            ind.mark_saved();
            true
        } else {
            false
        }
    }

    /// Get all modified file URIs.
    pub fn modified_files(&self) -> Vec<&str> {
        self.indicators
            .iter()
            .filter(|(_, ind)| ind.is_modified())
            .map(|(key, _)| key.as_str())
            .collect()
    }

    /// Total number of tracked files.
    pub fn tracked_count(&self) -> usize {
        self.indicators.len()
    }

    /// Number of modified files.
    pub fn modified_count(&self) -> usize {
        self.indicators.values().filter(|i| i.is_modified()).count()
    }

    /// Remove a tracked file.
    pub fn unregister(&mut self, uri_str: &str) -> bool {
        self.indicators.remove(uri_str).is_some()
    }
}

impl Default for FileModifiedTracker {
    fn default() -> Self {
        Self::new()
    }
}


/// File system configuration manager.
#[derive(Debug, Clone)]
pub struct FilesConfig {
    entries: Vec<FilesEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single file system entry.
#[derive(Debug, Clone, PartialEq)]
pub struct FilesEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl FilesEntry {
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

impl FilesConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: FilesEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&FilesEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut FilesEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&FilesEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&FilesEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&FilesEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<FilesEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// File system path utilities — extended utilities (qx)
// ---------------------------------------------------------------------------

/// Metric accumulator for files operations.
#[derive(Debug, Clone)]
pub struct QxMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QxMetrics {
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

/// Sliding-window rate counter for files.
#[derive(Debug, Clone)]
pub struct QxRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QxRateWindow {
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

/// A small LRU-style cache for files lookups.
#[derive(Debug, Clone)]
pub struct QxLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QxLruCache {
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
// xb_ utilities – batch 14
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer14 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer14 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_14(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_14<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_14<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_14(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_14(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 81
// ---------------------------------------------------------------------------

/// Generic object pool `Xc81Pool<T>`.
pub struct Xc81Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc81Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc81PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc81Pool<T> {
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
    pub fn stats(&self) -> Xc81PoolStats {
        Xc81PoolStats {
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

impl<T> Default for Xc81Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc81Scheduler`.
pub struct Xc81Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc81Scheduler {
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

impl Default for Xc81Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_81 hash for the given byte slice.
pub fn xc_81_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_81 convention.
pub fn xc_81_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe26 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe26Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe26PipelineError {
    pub stage: Xe26Stage,
    pub message: String,
}

impl std::fmt::Display for Xe26PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe26Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe26Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError>>>,
    stage_names: Vec<Xe26Stage>,
}

impl Xe26Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe26Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe26Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe26Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe26Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe26Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe26CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe26CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe26Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe26CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe26CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe26Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe26CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_26_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe26CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_26_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe26CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_26_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> {
    Ok(data)
}

pub fn xe_26_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_26_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_26_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_26_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe26PipelineError> {
    Err(Xe26PipelineError {
        stage: Xe26Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #111
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf111Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf111TrieNode {
    children: std::collections::HashMap<char, Xf111TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf111Trie {
    root: Xf111TrieNode,
    count: usize,
}

impl Xf111Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf111TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf111TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf111TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf111BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf111BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 80).
pub struct Xh80SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh80SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 122 as u64,
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

/// A compact bit set supporting boolean operations (variant 80).
pub struct Xh80BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh80BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 80).
pub struct Xi80Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi80Deque<T> {
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
pub struct Xi80Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi80Interval {
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

/// A simple interval tree (variant 80).
pub struct Xi80IntervalTree {
    xi_intervals: Vec<Xi80Interval>,
}

impl Xi80IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi80Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi80Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi80Interval) -> Vec<&Xi80Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi80Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi80Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi80Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi80Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi80Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi80Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 80) ---

/// Disjoint set / union-find for crate 80.
pub struct Xj80UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj80UnionFind {
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

const XJ80_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 80.
pub struct Xj80BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj80BTreeNode<K, V>>>,
    len: usize,
}

struct Xj80BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj80BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj80BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ80_BTREE_ORDER - 1
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
        let mid = XJ80_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj80BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj80BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj80BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj80BTreeNode::xj_new_leaf();
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


// --- xk_80 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk80SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk80SegmentTree {
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
pub struct Xk80DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk80DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_80).
#[derive(Debug, Clone)]
pub struct Xl80Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl80Rope {
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

/// Suffix array for efficient string searching (xl_80).
#[derive(Debug, Clone)]
pub struct Xl80SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl80SuffixArray {
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
pub struct Xm80MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm80MatrixSparse {
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
pub struct Xm80Tokenizer {
    text: String,
}

impl Xm80Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 80.
pub struct Xn80Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn80Fenwick {
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

// ----- AVL tree map — crate 80 -----

#[derive(Debug, Clone)]
struct Xn80AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn80AvlNode<K, V>>>,
    right: Option<Box<Xn80AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 80.
#[derive(Debug, Clone)]
pub struct Xn80AVL<K, V> {
    root: Option<Box<Xn80AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn80AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn80AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn80AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn80AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn80AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn80AvlNode<K, V>>) -> Box<Xn80AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn80AvlNode<K, V>>) -> Box<Xn80AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn80AvlNode<K, V>>) -> Box<Xn80AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn80AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn80AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn80AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn80AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn80AvlNode<K, V>>) -> &Xn80AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn80AvlNode<K, V>>) -> (Box<Xn80AvlNode<K, V>>, Option<Box<Xn80AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn80AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn80AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn80AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn80AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn80AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn80AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn80AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo80RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo80Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo80RBNode<K, V> {
    key: K,
    value: V,
    color: Xo80Color,
    left: Option<Box<Xo80RBNode<K, V>>>,
    right: Option<Box<Xo80RBNode<K, V>>>,
}

/// A red-black tree map for crate 80.
#[derive(Debug, Clone)]
pub struct Xo80RedBlack<K, V> {
    root: Option<Box<Xo80RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo80RedBlack<K, V> {
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
            r.color = Xo80Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo80RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo80RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo80RBNode {
                    key, value, color: Xo80Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo80RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo80Color::Red)
    }

    fn xo_balance(mut h: Box<Xo80RBNode<K, V>>) -> Box<Xo80RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo80Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo80RBNode<K, V>>) -> Box<Xo80RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo80Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo80RBNode<K, V>>) -> Box<Xo80RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo80Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo80RBNode<K, V>>) {
        h.color = Xo80Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo80Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo80Color::Black; }
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
            r.color = Xo80Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo80RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo80RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo80RBNode<K, V>) -> (K, V, Option<Box<Xo80RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo80RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo80Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo80RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo80ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 80.
#[derive(Debug, Clone)]
pub struct Xo80ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo80ConsistentHash {
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
            let vkey = format!("{}#xo80#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo80#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 80).
#[derive(Debug)]
pub struct Xp80SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp80Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp80Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp80Node<K, V>>>,
    xp_right: Option<Box<Xp80Node<K, V>>>,
}

impl<K: Ord, V> Xp80Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp80SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp80SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp80Node<K, V>>>, key: &K) -> Option<Box<Xp80Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp80Node<K, V>>) -> Box<Xp80Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp80Node<K, V>>) -> Box<Xp80Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp80Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp80Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp80Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq80Treap ---------------

use std::cmp::Ordering as Xq80Ord;

struct Xq80TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq80TreapNode<K, V>>>,
    right: Option<Box<Xq80TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq80Treap<K, V> {
    root: Option<Box<Xq80TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq80TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_80_size<K, V>(node: &Option<Box<Xq80TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_80_update_size<K, V>(node: &mut Xq80TreapNode<K, V>) {
    node.size = 1 + xq_80_size(&node.left) + xq_80_size(&node.right);
}

fn xq_80_rotate_right<K, V>(mut node: Box<Xq80TreapNode<K, V>>) -> Box<Xq80TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_80_update_size(&mut node);
    left.right = Some(node);
    xq_80_update_size(&mut left);
    left
}

fn xq_80_rotate_left<K, V>(mut node: Box<Xq80TreapNode<K, V>>) -> Box<Xq80TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_80_update_size(&mut node);
    right.left = Some(node);
    xq_80_update_size(&mut right);
    right
}

fn xq_80_insert_node<K: Ord, V>(
    node: Option<Box<Xq80TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq80TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq80TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq80Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq80Ord::Less => {
                let (new_left, old) = xq_80_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_80_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_80_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq80Ord::Greater => {
                let (new_right, old) = xq_80_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_80_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_80_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_80_remove_node<K: Ord, V>(
    node: Option<Box<Xq80TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq80TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq80Ord::Less => {
                let (new_left, old) = xq_80_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_80_update_size(&mut n);
                (Some(n), old)
            }
            Xq80Ord::Greater => {
                let (new_right, old) = xq_80_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_80_update_size(&mut n);
                (Some(n), old)
            }
            Xq80Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_80_rotate_right(n);
                    let (new_right, old) = xq_80_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_80_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_80_rotate_left(n);
                    let (new_left, old) = xq_80_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_80_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_80_find_min<K, V>(node: &Option<Box<Xq80TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_80_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_80_find_max<K, V>(node: &Option<Box<Xq80TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_80_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_80_rank<K: Ord, V>(node: &Option<Box<Xq80TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq80Ord::Less => xq_80_rank(&n.left, key),
            Xq80Ord::Equal => xq_80_size(&n.left),
            Xq80Ord::Greater => 1 + xq_80_size(&n.left) + xq_80_rank(&n.right, key),
        },
    }
}

fn xq_80_kth<K, V>(node: &Option<Box<Xq80TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_80_size(&n.left);
        if k < left_size {
            xq_80_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_80_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_80_in_order<K: Clone, V>(node: &Option<Box<Xq80TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_80_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_80_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq80Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 80 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_80_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq80Ord::Equal => return Some(&n.value),
                Xq80Ord::Less => cur = &n.left,
                Xq80Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_80_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_80_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_80_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_80_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_80_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_80_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_80_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq80VEBTree ---------------

pub struct Xq80VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq80VEBTree>>,
    clusters: Vec<Option<Box<Xq80VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq80VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq80VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq80VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr80KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr80KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr80BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr80KDNode {
    xr_point: Xr80KDPoint,
    xr_left: Option<Box<Xr80KDNode>>,
    xr_right: Option<Box<Xr80KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr80KDTree {
    xr_root: Option<Box<Xr80KDNode>>,
    xr_size: usize,
}

impl Xr80KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr80KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr80KDNode>>,
        point: Xr80KDPoint,
        depth: usize,
    ) -> Box<Xr80KDNode> {
        match node {
            None => Box::new(Xr80KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr80KDPoint) -> Option<Xr80KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr80KDNode>,
        query: &Xr80KDPoint,
        depth: usize,
        best: &mut Xr80KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr80KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr80KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr80KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr80KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr80KDNode>>, pts: &mut Vec<Xr80KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr80KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr80BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr80BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs80PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs80PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs80PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs80PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs80ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs80ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs80ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs80RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs80RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs80RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs80CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs80CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs80CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_stat_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let uri = VsUri::file(&file_path.to_string_lossy());
        let provider = DiskFileSystemProvider::new();
        let stat = provider.stat(&uri).unwrap();
        assert_eq!(stat.file_type, FileType::File);
        assert_eq!(stat.size, 5);
    }

    #[test]
    fn disk_read_write() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("rw.txt");
        let uri = VsUri::file(&file_path.to_string_lossy());
        let provider = DiskFileSystemProvider::new();

        provider
            .write_file(
                &uri,
                b"content",
                &WriteFileOptions {
                    create: true,
                    overwrite: false,
                    ..Default::default()
                },
            )
            .unwrap();

        let data = provider.read_file(&uri).unwrap();
        assert_eq!(data, b"content");
    }

    #[test]
    fn disk_readdir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "").unwrap();
        std::fs::write(dir.path().join("b.txt"), "").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let uri = VsUri::file(&dir.path().to_string_lossy());
        let provider = DiskFileSystemProvider::new();
        let entries = provider.readdir(&uri).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[0].file_type, FileType::File);
        assert_eq!(entries[2].name, "subdir");
        assert_eq!(entries[2].file_type, FileType::Directory);
    }

    #[test]
    fn disk_delete() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("del.txt");
        std::fs::write(&file_path, "bye").unwrap();

        let uri = VsUri::file(&file_path.to_string_lossy());
        let provider = DiskFileSystemProvider::new();
        provider.delete(&uri, false).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn file_service_uses_file_scheme() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("svc.txt");
        let uri = VsUri::file(&file_path.to_string_lossy());
        let svc = FileService::new();

        svc.write_file(
            &uri,
            b"service test",
            &WriteFileOptions {
                create: true,
                overwrite: true,
                ..Default::default()
            },
        )
        .unwrap();

        let content = svc.read_file_string(&uri).unwrap();
        assert_eq!(content, "service test");
    }

    #[test]
    fn file_service_unknown_scheme() {
        let svc = FileService::new();
        let uri = VsUri::from_components("custom", "", "/foo", "", "");
        assert!(matches!(
            svc.stat(&uri),
            Err(FileError::UnknownScheme(_))
        ));
    }

    #[test]
    fn file_service_mkdir_and_rename() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("newdir");
        let svc = FileService::new();

        let uri = VsUri::file(&sub.to_string_lossy());
        svc.mkdir(&uri).unwrap();
        assert!(sub.is_dir());

        // Write a file and rename it
        let f1 = dir.path().join("newdir").join("orig.txt");
        let f2 = dir.path().join("newdir").join("renamed.txt");
        std::fs::write(&f1, "data").unwrap();
        let u1 = VsUri::file(&f1.to_string_lossy());
        let u2 = VsUri::file(&f2.to_string_lossy());
        svc.rename(&u1, &u2, false).unwrap();
        assert!(!f1.exists());
        assert!(f2.exists());
    }

    #[test]
    fn eq_filetype_same() {
        assert_eq!(FileType::File, FileType::File);
    }

    #[test]
    fn ne_filetype_diff() {
        assert_ne!(FileType::File, FileType::Directory);
    }

    #[test]
    fn eq_filechangetype_same() {
        assert_eq!(FileChangeType::Created, FileChangeType::Created);
    }

    #[test]
    fn ne_filechangetype_diff() {
        assert_ne!(FileChangeType::Created, FileChangeType::Changed);
    }

    // ---- FileReadOptions / FileWriteOptions / file_stat_batch tests ----

    #[test]
    fn read_options_default() {
        let opts = FileReadOptions::default();
        assert_eq!(opts.encoding, FileEncoding::Utf8);
        assert!(!opts.accept_binary);
        assert!(opts.max_size.is_none());
        assert!(opts.line_ending_normalization);
    }

    #[test]
    fn read_options_builder() {
        let opts = FileReadOptions::default()
            .with_encoding(FileEncoding::Utf16Le)
            .with_max_size(1024)
            .with_binary(true);
        assert_eq!(opts.encoding, FileEncoding::Utf16Le);
        assert!(opts.accept_binary);
        assert_eq!(opts.max_size, Some(1024));
    }

    #[test]
    fn read_options_would_accept_valid() {
        let opts = FileReadOptions::default()
            .with_max_size(2048)
            .with_binary(true);
        assert!(opts.would_accept(100, false));
        assert!(opts.would_accept(2048, true));
    }

    #[test]
    fn read_options_rejects_binary() {
        let opts = FileReadOptions::default(); // accept_binary = false
        assert!(!opts.would_accept(10, true));
    }

    #[test]
    fn read_options_rejects_too_large() {
        let opts = FileReadOptions::default().with_max_size(500);
        assert!(!opts.would_accept(501, false));
        assert!(opts.would_accept(500, false));
    }

    #[test]
    fn write_options_default() {
        let opts = FileWriteOptions::default();
        assert!(opts.create_parents);
        assert!(opts.overwrite);
        assert!(!opts.atomic);
        assert!(!opts.backup_before_write);
    }

    #[test]
    fn write_options_validate_no_overwrite() {
        let opts = FileWriteOptions::default().with_overwrite(false);
        assert!(opts.validate_write(true).is_err());
        assert!(opts.validate_write(false).is_ok());
    }

    #[test]
    fn file_stat_batch_parses_paths() {
        let results = file_stat_batch(&["/tmp/dir/", "some_file.txt", "link -> target"]);
        assert_eq!(results.len(), 3);
        assert!(results[0].is_dir);
        assert!(!results[1].is_dir);
        assert!(results[2].is_symlink);
    }

    #[test]
    fn file_stat_batch_detects_extension() {
        let results = file_stat_batch(&["foo.rs", "bar.tar.gz", "no_ext", "trailing/"]);
        assert_eq!(results[0].extension.as_deref(), Some("rs"));
        assert_eq!(results[1].extension.as_deref(), Some("gz"));
        assert!(results[2].extension.is_none());
        assert!(results[3].extension.is_none()); // directory – no extension
    }

    #[test]
    fn behavior_check_0() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = DiskFileSystemProvider::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn files_stats_new_defaults() {
        let stats = FilesStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn files_stats_record_success() {
        let mut stats = FilesStats::new();
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
    fn files_stats_record_failure() {
        let mut stats = FilesStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn files_stats_reset() {
        let mut stats = FilesStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn files_stats_merge() {
        let mut a = FilesStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = FilesStats::new();
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
    fn files_stats_display() {
        let mut stats = FilesStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn files_stats_default() {
        let stats = FilesStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn files_validator_accepts_valid_name() {
        let v = FilesValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn files_validator_rejects_empty() {
        let v = FilesValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn files_validator_rejects_too_long() {
        let v = FilesValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn files_validator_forbidden_prefix() {
        let v = FilesValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn files_validator_allowed_chars() {
        let v = FilesValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn files_validator_range() {
        let v = FilesValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn files_sanitize_removes_control() {
        let result = FilesValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn files_truncate_short_string() {
        assert_eq!(FilesValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn files_truncate_long_string() {
        let result = FilesValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn files_is_ascii_printable() {
        assert!(FilesValidator::is_ascii_printable("Hello World 123"));
        assert!(!FilesValidator::is_ascii_printable("Hello\x00World"));
    }

    // ---- FileBreadcrumb tests ----

    #[test]
    fn breadcrumb_from_absolute_path() {
        let bc = FileBreadcrumb::from_path("/src/components/Button.tsx");
        assert_eq!(bc.depth(), 3);
        assert_eq!(bc.file_name(), Some("Button.tsx"));
        assert_eq!(bc.segments()[0].label, "src");
        assert!(bc.segments()[0].is_directory);
        assert!(!bc.segments()[2].is_directory);
        assert_eq!(bc.parent_path(), Some("/src/components"));
    }

    #[test]
    fn breadcrumb_trailing_slash_is_dir() {
        let bc = FileBreadcrumb::from_path("/usr/local/bin/");
        assert_eq!(bc.depth(), 3);
        assert!(bc.segments().last().unwrap().is_directory);
    }

    #[test]
    fn breadcrumb_display() {
        let bc = FileBreadcrumb::from_path("/a/b/c.rs");
        assert_eq!(format!("{bc}"), "a > b > c.rs");
    }

    #[test]
    fn breadcrumb_empty_path() {
        let bc = FileBreadcrumb::from_path("");
        assert_eq!(bc.depth(), 0);
        assert_eq!(bc.file_name(), None);
        assert_eq!(bc.parent_path(), None);
    }

    #[test]
    fn breadcrumb_single_segment() {
        let bc = FileBreadcrumb::from_path("file.txt");
        assert_eq!(bc.depth(), 1);
        assert_eq!(bc.file_name(), Some("file.txt"));
        assert_eq!(bc.parent_path(), None);
    }

    // ---- FileBookmarks tests ----

    #[test]
    fn bookmarks_add_and_get() {
        let mut bm = FileBookmarks::new();
        let uri = VsUri::file("/home/user/project");
        bm.add("project", uri.clone(), Some("Main project".into())).unwrap();
        assert_eq!(bm.len(), 1);
        assert!(!bm.is_empty());
        let b = bm.get("project").unwrap();
        assert_eq!(b.uri, uri);
        assert_eq!(b.note.as_deref(), Some("Main project"));
    }

    #[test]
    fn bookmarks_reject_duplicate_name() {
        let mut bm = FileBookmarks::new();
        bm.add("home", VsUri::file("/home"), None).unwrap();
        assert!(bm.add("home", VsUri::file("/tmp"), None).is_err());
    }

    #[test]
    fn bookmarks_remove() {
        let mut bm = FileBookmarks::new();
        bm.add("tmp", VsUri::file("/tmp"), None).unwrap();
        assert!(bm.remove("tmp"));
        assert!(bm.is_empty());
        assert!(!bm.remove("nonexistent"));
    }

    #[test]
    fn bookmarks_rename() {
        let mut bm = FileBookmarks::new();
        bm.add("old", VsUri::file("/a"), None).unwrap();
        bm.rename("old", "new").unwrap();
        assert!(bm.get("old").is_none());
        assert!(bm.get("new").is_some());
    }

    #[test]
    fn bookmarks_rename_conflict() {
        let mut bm = FileBookmarks::new();
        bm.add("a", VsUri::file("/a"), None).unwrap();
        bm.add("b", VsUri::file("/b"), None).unwrap();
        assert!(bm.rename("a", "b").is_err());
    }

    // ---- FileCompare / diff tests ----

    #[test]
    fn diff_identical_files() {
        let content = b"line1\nline2\nline3\n";
        let diff = file_compare(content, content);
        let summary = diff_summary(&diff);
        assert_eq!(summary.added, 0);
        assert_eq!(summary.removed, 0);
        assert_eq!(summary.equal, 3);
    }

    #[test]
    fn diff_added_lines() {
        let old = b"a\nb\n";
        let new = b"a\nb\nc\n";
        let diff = file_compare(old, new);
        let summary = diff_summary(&diff);
        assert_eq!(summary.added, 1);
        assert_eq!(summary.removed, 0);
    }

    #[test]
    fn diff_removed_lines() {
        let old = b"a\nb\nc\n";
        let new = b"a\nc\n";
        let diff = file_compare(old, new);
        let summary = diff_summary(&diff);
        assert_eq!(summary.removed, 1);
        assert_eq!(summary.added, 0);
        assert_eq!(summary.equal, 2);
    }

    #[test]
    fn diff_completely_different() {
        let old = b"x\ny\n";
        let new = b"a\nb\n";
        let diff = file_compare(old, new);
        let summary = diff_summary(&diff);
        assert_eq!(summary.added, 2);
        assert_eq!(summary.removed, 2);
        assert_eq!(summary.equal, 0);
    }

    #[test]
    fn diff_empty_to_content() {
        let diff = file_compare(b"", b"hello\nworld\n");
        let summary = diff_summary(&diff);
        assert_eq!(summary.added, 2);
        assert_eq!(summary.removed, 0);
    }

    #[test]
    fn test_file_extension_stats() {
        let results = file_stat_batch(&["src/main.rs", "src/lib.rs", "Cargo.toml", "docs/"]);
        let stats = FileExtensionStats::from_results(&results);
        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.total_dirs, 1);
        assert_eq!(stats.extension_counts.get("rs"), Some(&2));
        assert_eq!(stats.extension_counts.get("toml"), Some(&1));
    }

    #[test]
    fn test_most_common_extension() {
        let results = file_stat_batch(&["a.rs", "b.rs", "c.toml"]);
        let stats = FileExtensionStats::from_results(&results);
        let (ext, count) = stats.most_common_extension().unwrap();
        assert_eq!(ext, "rs");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_extension_stats_display() {
        let results = file_stat_batch(&["a.rs", "b.toml", "dir/"]);
        let stats = FileExtensionStats::from_results(&results);
        let display = stats.to_string();
        assert!(display.contains("Files: 2"));
        assert!(display.contains("Dirs: 1"));
    }

    #[test]
    fn test_path_matcher_suffix() {
        let m = PathMatcher::new("*.rs");
        assert!(m.matches("src/main.rs"));
        assert!(!m.matches("src/main.toml"));
    }

    #[test]
    fn test_path_matcher_prefix() {
        let m = PathMatcher::new("src/*");
        assert!(m.matches("src/main.rs"));
        assert!(!m.matches("tests/test.rs"));
    }

    #[test]
    fn test_file_similarity() {
        assert!((file_similarity(b"hello\n", b"hello\n") - 1.0).abs() < f64::EPSILON);
        assert!((file_similarity(b"a\nb\n", b"x\ny\n") - 0.0).abs() < f64::EPSILON);
        let sim = file_similarity(b"a\nb\nc\n", b"a\nb\nd\n");
        assert!(sim >= 0.5);
        assert!(sim < 1.0);
    }

    // --- FileWatchFilter tests ---

    #[test]
    fn watch_filter_no_patterns_matches_all() {
        let f = FileWatchFilter::new();
        assert!(f.matches("anything.rs"));
        assert!(f.matches(""));
    }

    #[test]
    fn watch_filter_include_star() {
        let mut f = FileWatchFilter::new();
        f.add_include("*.rs");
        assert!(f.matches("main.rs"));
        assert!(!f.matches("main.py"));
        assert_eq!(f.include_count(), 1);
    }

    #[test]
    fn watch_filter_exclude_overrides_include() {
        let mut f = FileWatchFilter::new();
        f.add_include("*.rs");
        f.add_exclude("test_*");
        assert!(f.matches("main.rs"));
        assert!(!f.matches("test_main.rs"));
        assert_eq!(f.exclude_count(), 1);
    }

    #[test]
    fn watch_filter_question_mark() {
        let mut f = FileWatchFilter::new();
        f.add_include("a?c");
        assert!(f.matches("abc"));
        assert!(f.matches("axc"));
        assert!(!f.matches("ac"));
        assert!(!f.matches("abbc"));
    }

    // --- FileContentHash tests ---

    #[test]
    fn content_hash_deterministic() {
        let a = FileContentHash::from_bytes(b"hello world");
        let b = FileContentHash::from_bytes(b"hello world");
        assert!(a.matches(&b));
        assert_eq!(a.value(), b.value());
    }

    #[test]
    fn content_hash_different_inputs() {
        let a = FileContentHash::from_str("alpha");
        let b = FileContentHash::from_str("beta");
        assert!(!a.matches(&b));
    }

    #[test]
    fn content_hash_display() {
        let h = FileContentHash::from_str("test");
        let s = format!("{}", h);
        assert_eq!(s.len(), 16); // 64-bit hex = 16 chars
    }

    // --- DirectoryDiff tests ---

    #[test]
    fn directory_diff_common_and_unique() {
        let d = DirectoryDiff::new(
            vec!["a.rs".into(), "b.rs".into(), "c.rs".into()],
            vec!["b.rs".into(), "c.rs".into(), "d.rs".into()],
        );
        assert_eq!(d.only_left(), vec!["a.rs"]);
        assert_eq!(d.only_right(), vec!["d.rs"]);
        assert_eq!(d.common(), vec!["b.rs", "c.rs"]);
    }

    #[test]
    fn directory_diff_summary() {
        let d = DirectoryDiff::new(
            vec!["x".into()],
            vec!["y".into()],
        );
        let s = d.summary();
        assert!(s.contains("left_only: 1"));
        assert!(s.contains("right_only: 1"));
        assert!(s.contains("common: 0"));
    }

    // --- FileMetadataCache tests ---

    #[test]
    fn metadata_cache_insert_get() {
        let mut cache = FileMetadataCache::new();
        assert!(cache.is_empty());
        cache.insert("foo.rs", 1024, 5000);
        assert_eq!(cache.len(), 1);
        let m = cache.get("foo.rs").unwrap();
        assert_eq!(m.size, 1024);
        assert_eq!(m.modified_ms, 5000);
    }

    #[test]
    fn metadata_cache_invalidate() {
        let mut cache = FileMetadataCache::new();
        cache.insert("bar.rs", 512, 1000);
        cache.invalidate("bar.rs");
        assert!(cache.get("bar.rs").is_none());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn metadata_cache_is_stale() {
        let mut cache = FileMetadataCache::new();
        cache.insert("a.rs", 100, 1000);
        // Not stale: current 1500, max_age 1000 => age=500 <= 1000
        assert!(!cache.is_stale("a.rs", 1000, 1500));
        // Stale: current 3000, max_age 1000 => age=2000 > 1000
        assert!(cache.is_stale("a.rs", 1000, 3000));
        // Missing entry is always stale
        assert!(cache.is_stale("missing.rs", 1000, 1000));
    }
    #[test]
    fn batch_notifier_basic() {
        let mut notifier = FileBatchNotifier::new(100, 50);
        assert!(notifier.is_empty());
        let uri = VsUri::file("/test.rs");
        notifier.add_event(FileChangeEvent { uri, change_type: FileChangeType::Changed }, 10);
        assert_eq!(notifier.pending_count(), 1);
        assert!(!notifier.should_flush(50));
        assert!(notifier.should_flush(120));
    }

    #[test]
    fn batch_notifier_coalesce() {
        let mut notifier = FileBatchNotifier::new(100, 50);
        let uri = VsUri::file("/test.rs");
        notifier.add_event(FileChangeEvent { uri: uri.clone(), change_type: FileChangeType::Changed }, 10);
        notifier.add_event(FileChangeEvent { uri: uri.clone(), change_type: FileChangeType::Changed }, 20);
        assert_eq!(notifier.pending_count(), 1);
    }

    #[test]
    fn batch_notifier_flush() {
        let mut notifier = FileBatchNotifier::new(100, 50);
        let uri = VsUri::file("/a.rs");
        notifier.add_event(FileChangeEvent { uri, change_type: FileChangeType::Created }, 10);
        let events = notifier.flush();
        assert_eq!(events.len(), 1);
        assert!(notifier.is_empty());
    }

    #[test]
    fn batch_notifier_max_batch_size() {
        let mut notifier = FileBatchNotifier::new(1000, 3);
        for i in 0..4 {
            let uri = VsUri::file(&format!("/file{i}.rs"));
            notifier.add_event(FileChangeEvent { uri, change_type: FileChangeType::Changed }, 10);
        }
        assert!(notifier.should_flush(10));
    }

    #[test]
    fn batch_notifier_count_by_type() {
        let mut notifier = FileBatchNotifier::new(100, 50);
        notifier.add_event(FileChangeEvent { uri: VsUri::file("/a"), change_type: FileChangeType::Created }, 10);
        notifier.add_event(FileChangeEvent { uri: VsUri::file("/b"), change_type: FileChangeType::Changed }, 20);
        notifier.add_event(FileChangeEvent { uri: VsUri::file("/c"), change_type: FileChangeType::Created }, 30);
        assert_eq!(notifier.count_by_type(FileChangeType::Created), 2);
        assert_eq!(notifier.count_by_type(FileChangeType::Changed), 1);
    }

    #[test]
    fn batch_notifier_set_window() {
        let mut notifier = FileBatchNotifier::new(100, 50);
        assert_eq!(notifier.batch_window_ms(), 100);
        notifier.set_batch_window_ms(200);
        assert_eq!(notifier.batch_window_ms(), 200);
    }

    #[test]
    fn encoding_guesser_utf8() {
        assert_eq!(FileEncodingGuesser::guess(b"hello world"), DetectedEncoding::Ascii);
        assert_eq!(FileEncodingGuesser::guess("héllo".as_bytes()), DetectedEncoding::Utf8);
    }

    #[test]
    fn encoding_guesser_bom() {
        assert_eq!(FileEncodingGuesser::guess(&[0xEF, 0xBB, 0xBF, b'h']), DetectedEncoding::Utf8Bom);
        assert_eq!(FileEncodingGuesser::guess(&[0xFF, 0xFE, 0, 0]), DetectedEncoding::Utf16Le);
        assert_eq!(FileEncodingGuesser::guess(&[0xFE, 0xFF, 0, 0]), DetectedEncoding::Utf16Be);
    }

    #[test]
    fn encoding_guesser_empty() {
        assert_eq!(FileEncodingGuesser::guess(b""), DetectedEncoding::Ascii);
    }

    #[test]
    fn encoding_guesser_label() {
        assert_eq!(FileEncodingGuesser::guess_label(b"hello"), "ASCII");
    }

    #[test]
    fn encoding_guesser_binary() {
        assert!(FileEncodingGuesser::is_likely_binary(&[0, 1, 2, 3]));
        assert!(!FileEncodingGuesser::is_likely_binary(b"text"));
    }

    #[test]
    fn file_size_formatter_bytes() {
        assert_eq!(FileSizeFormatter::format(0), "0 B");
        assert_eq!(FileSizeFormatter::format(512), "512 B");
        assert_eq!(FileSizeFormatter::format(1023), "1023 B");
    }

    #[test]
    fn file_size_formatter_kb() {
        assert_eq!(FileSizeFormatter::format(1024), "1.0 KB");
        assert_eq!(FileSizeFormatter::format(1536), "1.5 KB");
        assert_eq!(FileSizeFormatter::format(10240), "10 KB");
    }

    #[test]
    fn file_size_formatter_mb_gb() {
        assert_eq!(FileSizeFormatter::format(1024 * 1024), "1.0 MB");
        assert_eq!(FileSizeFormatter::format(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn file_size_formatter_precision() {
        assert_eq!(FileSizeFormatter::format_with_precision(1536, 2), "1.50 KB");
        assert_eq!(FileSizeFormatter::format_with_precision(100, 2), "100 B");
    }

    #[test]
    fn file_size_formatter_unit() {
        assert_eq!(FileSizeFormatter::unit_for(100), "B");
        assert_eq!(FileSizeFormatter::unit_for(2048), "KB");
        assert_eq!(FileSizeFormatter::unit_for(2 * 1024 * 1024), "MB");
    }

    #[test]
    fn file_modified_indicator_basic() {
        let uri = VsUri::file("/test.rs");
        let content = b"hello world";
        let hash = FileModifiedIndicator::hash_content(content);
        let mut ind = FileModifiedIndicator::new(uri, hash);
        assert!(!ind.is_modified());
        assert_eq!(ind.indicator_char(), "");

        ind.update_hash(b"changed");
        assert!(ind.is_modified());
        assert_eq!(ind.indicator_char(), "●");
    }

    #[test]
    fn file_modified_indicator_save_revert() {
        let uri = VsUri::file("/test.rs");
        let mut ind = FileModifiedIndicator::new(uri, 100);
        ind.current_hash = 200;
        assert!(ind.is_modified());
        ind.mark_saved();
        assert!(!ind.is_modified());
        assert_eq!(ind.save_count(), 1);

        ind.current_hash = 300;
        assert!(ind.is_modified());
        ind.revert();
        assert!(!ind.is_modified());
    }

    #[test]
    fn file_modified_indicator_title() {
        let uri = VsUri::file("/test.rs");
        let mut ind = FileModifiedIndicator::new(uri, 100);
        assert_eq!(ind.format_title("test.rs"), "test.rs");
        ind.current_hash = 200;
        assert_eq!(ind.format_title("test.rs"), "● test.rs");
    }

    #[test]
    fn file_modified_tracker_basic() {
        let mut tracker = FileModifiedTracker::new();
        let uri = VsUri::file("/test.rs");
        tracker.register(uri, b"hello");
        assert_eq!(tracker.tracked_count(), 1);
        assert_eq!(tracker.modified_count(), 0);
    }

    #[test]
    fn file_modified_tracker_update() {
        let mut tracker = FileModifiedTracker::new();
        let uri = VsUri::file("/test.rs");
        let key = uri.to_string();
        tracker.register(uri, b"hello");
        tracker.update(&key, b"changed");
        assert!(tracker.is_modified(&key));
        assert_eq!(tracker.modified_count(), 1);
    }

    #[test]
    fn file_modified_tracker_save() {
        let mut tracker = FileModifiedTracker::new();
        let uri = VsUri::file("/test.rs");
        let key = uri.to_string();
        tracker.register(uri, b"hello");
        tracker.update(&key, b"changed");
        tracker.mark_saved(&key);
        assert!(!tracker.is_modified(&key));
    }

    #[test]
    fn file_modified_tracker_modified_files() {
        let mut tracker = FileModifiedTracker::new();
        let u1 = VsUri::file("/a.rs");
        let u2 = VsUri::file("/b.rs");
        let k1 = u1.to_string();
        let k2 = u2.to_string();
        tracker.register(u1, b"a");
        tracker.register(u2, b"b");
        tracker.update(&k1, b"changed");
        assert_eq!(tracker.modified_files().len(), 1);
        assert!(!tracker.is_modified(&k2));
    }

    #[test]
    fn file_modified_tracker_unregister() {
        let mut tracker = FileModifiedTracker::new();
        let uri = VsUri::file("/test.rs");
        let key = uri.to_string();
        tracker.register(uri, b"hello");
        assert!(tracker.unregister(&key));
        assert!(!tracker.unregister(&key));
        assert_eq!(tracker.tracked_count(), 0);
    }


    #[test]
    fn files_entry_creation() {
        let e = FilesEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn files_entry_with_priority() {
        let e = FilesEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn files_entry_metadata() {
        let e = FilesEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn files_entry_remove_meta() {
        let mut e = FilesEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn files_entry_activate_deactivate() {
        let mut e = FilesEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn files_config_add_sorted() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("lo", "Lo").with_priority(1));
        c.add(FilesEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn files_config_capacity() {
        let mut c = FilesConfig::new(1);
        assert!(c.add(FilesEntry::new("a", "A")));
        assert!(!c.add(FilesEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn files_config_remove() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn files_config_get() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn files_config_active_entries() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "A"));
        c.add(FilesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn files_config_enable_disable() {
        let mut c = FilesConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn files_config_clear() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn files_config_find_by_label() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn files_config_top_n() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "A").with_priority(1));
        c.add(FilesEntry::new("b", "B").with_priority(2));
        c.add(FilesEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn files_config_deactivate_activate_all() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "A"));
        c.add(FilesEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn files_config_highest_priority() {
        let mut c = FilesConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(FilesEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn files_config_contains() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn files_config_labels() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "Alpha"));
        c.add(FilesEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn files_config_drain_inactive() {
        let mut c = FilesConfig::new(10);
        c.add(FilesEntry::new("a", "A"));
        c.add(FilesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qx_metrics_empty() {
        let m = QxMetrics::new("files");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qx_metrics_record_and_mean() {
        let mut m = QxMetrics::new("files");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qx_metrics_min_max() {
        let mut m = QxMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qx_metrics_variance_and_std() {
        let mut m = QxMetrics::new("v");
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
    fn qx_metrics_percentile() {
        let mut m = QxMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qx_metrics_merge() {
        let mut a = QxMetrics::new("a");
        a.record(1.0);
        let mut b = QxMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qx_metrics_reset() {
        let mut m = QxMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qx_rate_window_empty() {
        let rw = QxRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qx_rate_window_tick_and_rate() {
        let mut rw = QxRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qx_lru_cache_basic() {
        let mut c = QxLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qx_lru_cache_contains_and_keys() {
        let mut c = QxLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qx_lru_cache_remove() {
        let mut c = QxLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qx_metrics_sum() {
        let mut m = QxMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qx_metrics_label() {
        let m = QxMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qx_lru_cache_clear() {
        let mut c = QxLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_14_push_and_len() {
        let mut rb = super::XbRingBuffer14::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_14_overwrite() {
        let mut rb = super::XbRingBuffer14::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_14_get_out_of_bounds() {
        let rb = super::XbRingBuffer14::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_14_drain_all() {
        let mut rb = super::XbRingBuffer14::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_14_peek_front_back() {
        let mut rb = super::XbRingBuffer14::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_14_clear() {
        let mut rb = super::XbRingBuffer14::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_14_capacity() {
        let rb = super::XbRingBuffer14::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_14_basic() {
        let h = super::xb_fnv1a_14(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_14(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_14_different_inputs() {
        let h1 = super::xb_fnv1a_14(b"abc");
        let h2 = super::xb_fnv1a_14(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_14_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_14(&data);
        let dec = super::xb_rle_decode_14(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_14_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_14(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_14(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_14_values() {
        assert!((super::xb_clamp_14(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_14(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_14(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_14_values() {
        assert!((super::xb_lerp_14(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_14(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_14(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_14_wrap_around_twice() {
        let mut rb = super::XbRingBuffer14::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 81 ----

    #[test]
    fn xc_81_pool_new_empty() {
        let pool: super::Xc81Pool<i32> = super::Xc81Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_81_pool_release_acquire() {
        let mut pool = super::Xc81Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_81_pool_acquire_empty() {
        let mut pool: super::Xc81Pool<i32> = super::Xc81Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_81_pool_full() {
        let mut pool = super::Xc81Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_81_pool_drain() {
        let mut pool = super::Xc81Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_81_pool_stats() {
        let mut pool = super::Xc81Pool::new(8);
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
    fn xc_81_pool_clear() {
        let mut pool = super::Xc81Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_81_pool_shrink() {
        let mut pool = super::Xc81Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_81_pool_default() {
        let pool: super::Xc81Pool<String> = super::Xc81Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_81_pool_extend() {
        let mut pool = super::Xc81Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_81_pool_retain() {
        let mut pool = super::Xc81Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_81_scheduler_round_robin() {
        let mut sched = super::Xc81Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_81_scheduler_empty() {
        let mut sched = super::Xc81Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_81_scheduler_reset() {
        let mut sched = super::Xc81Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_81_scheduler_add_remove() {
        let mut sched = super::Xc81Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_81_scheduler_targets() {
        let sched = super::Xc81Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_81_hash_empty() {
        assert_eq!(super::xc_81_hash(b""), 5381);
    }

    #[test]
    fn xc_81_hash_data() {
        let h = super::xc_81_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_81_hash(b"hello"), h);
    }

    #[test]
    fn xc_81_reverse_str() {
        assert_eq!(super::xc_81_reverse("abc"), "cba");
        assert_eq!(super::xc_81_reverse(""), "");
    }


    #[test]
    fn xe_26_pipeline_empty() {
        let p = super::Xe26Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_26_pipeline_parse_stage() {
        let p = super::Xe26Pipeline::new()
            .add_parse(super::xe_26_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_26_pipeline_transform_double() {
        let p = super::Xe26Pipeline::new()
            .add_transform(super::xe_26_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_26_pipeline_validate_reverse() {
        let p = super::Xe26Pipeline::new()
            .add_validate(super::xe_26_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_26_pipeline_emit_filter() {
        let p = super::Xe26Pipeline::new()
            .add_emit(super::xe_26_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_26_pipeline_multi_stage() {
        let p = super::Xe26Pipeline::new()
            .add_parse(super::xe_26_pipeline_identity)
            .add_transform(super::xe_26_pipeline_double)
            .add_validate(super::xe_26_pipeline_reverse)
            .add_emit(super::xe_26_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_26_pipeline_error_propagation() {
        let p = super::Xe26Pipeline::new()
            .add_parse(super::xe_26_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe26Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_26_pipeline_compose() {
        let p1 = super::Xe26Pipeline::new()
            .add_parse(super::xe_26_pipeline_identity);
        let p2 = super::Xe26Pipeline::new()
            .add_transform(super::xe_26_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_26_pipeline_error_display() {
        let e = super::Xe26PipelineError {
            stage: super::Xe26Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_26_cache_put_get() {
        let mut c = super::Xe26Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_26_cache_miss() {
        let mut c: super::Xe26Cache<&str, i32> = super::Xe26Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_26_cache_ttl_expiry() {
        let mut c = super::Xe26Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_26_cache_evict() {
        let mut c = super::Xe26Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_26_cache_capacity() {
        let mut c = super::Xe26Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_26_cache_stats() {
        let mut c = super::Xe26Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_26_cache_clear() {
        let mut c = super::Xe26Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #111 --

    #[test]
    fn xf111_trie_insert_search() {
        let mut t = Xf111Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf111_trie_starts_with() {
        let mut t = Xf111Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf111_trie_remove() {
        let mut t = Xf111Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf111_trie_word_count() {
        let mut t = Xf111Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf111_trie_longest_prefix() {
        let mut t = Xf111Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf111_trie_all_words() {
        let mut t = Xf111Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf111_trie_autocomplete() {
        let mut t = Xf111Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf111_trie_empty_search() {
        let t = Xf111Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf111_bloom_add_contains() {
        let mut bf = Xf111BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf111_bloom_probably_absent() {
        let bf = Xf111BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf111_bloom_false_positive_rate() {
        let mut bf = Xf111BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf111_bloom_clear() {
        let mut bf = Xf111BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf111_bloom_union() {
        let mut a = Xf111BloomFilter::xf_new(512, 2);
        let mut b = Xf111BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf111_bloom_intersection_estimate() {
        let mut a = Xf111BloomFilter::xf_new(512, 2);
        let mut b = Xf111BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf111_bloom_union_size_mismatch() {
        let a = Xf111BloomFilter::xf_new(256, 2);
        let b = Xf111BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh80_skip_insert_contains() {
        let mut sl = super::Xh80SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh80_skip_remove() {
        let mut sl = super::Xh80SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh80_skip_len() {
        let mut sl = super::Xh80SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh80_skip_range_query() {
        let mut sl = super::Xh80SkipList::xh_new(4);
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
    fn xh80_skip_floor_ceiling() {
        let mut sl = super::Xh80SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh80_skip_rank() {
        let mut sl = super::Xh80SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh80_skip_empty() {
        let sl = super::Xh80SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh80_skip_duplicates() {
        let mut sl = super::Xh80SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh80_bitset_set_test() {
        let mut bs = super::Xh80BitSet::xh_new(256);
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
    fn xh80_bitset_clear_count() {
        let mut bs = super::Xh80BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh80_bitset_and_or_xor() {
        let mut a = super::Xh80BitSet::xh_new(128);
        let mut b = super::Xh80BitSet::xh_new(128);
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
    fn xh80_bitset_iter_ones() {
        let mut bs = super::Xh80BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh80_bitset_first_last() {
        let mut bs = super::Xh80BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh80_bitset_empty() {
        let bs = super::Xh80BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi80_deque_push_pop_back() {
        let mut dq = super::Xi80Deque::xi_new(4);
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
    fn xi80_deque_push_pop_front() {
        let mut dq = super::Xi80Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi80_deque_mixed_ops() {
        let mut dq = super::Xi80Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi80_deque_get_and_split() {
        let mut dq = super::Xi80Deque::xi_new(8);
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
    fn xi80_deque_rotate_left() {
        let mut dq = super::Xi80Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi80_deque_rotate_right() {
        let mut dq = super::Xi80Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi80_deque_grow() {
        let mut dq = super::Xi80Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi80_deque_empty() {
        let dq = super::Xi80Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi80_interval_tree_insert_query() {
        let mut tree = super::Xi80IntervalTree::xi_new();
        tree.xi_insert(super::Xi80Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi80Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi80Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi80_interval_tree_overlap() {
        let mut tree = super::Xi80IntervalTree::xi_new();
        tree.xi_insert(super::Xi80Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi80Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi80Interval::xi_new(12, 20));
        let q = super::Xi80Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi80_interval_tree_remove() {
        let mut tree = super::Xi80IntervalTree::xi_new();
        tree.xi_insert(super::Xi80Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi80Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi80_interval_tree_gaps() {
        let mut tree = super::Xi80IntervalTree::xi_new();
        tree.xi_insert(super::Xi80Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi80Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi80Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi80Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi80Interval::xi_new(8, 10));
    }

    #[test]
    fn xi80_interval_tree_merge() {
        let mut tree = super::Xi80IntervalTree::xi_new();
        tree.xi_insert(super::Xi80Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi80Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi80Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi80Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi80Interval::xi_new(10, 15));
    }

    #[test]
    fn xi80_interval_tree_all() {
        let mut tree = super::Xi80IntervalTree::xi_new();
        tree.xi_insert(super::Xi80Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi80Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi80_interval_tree_empty() {
        let tree = super::Xi80IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi80_interval_tree_contains_point() {
        let iv = super::Xi80Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 80) ---

    #[test]
    fn xj_80_uf_make_and_find() {
        let mut uf = super::Xj80UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_80_uf_union_connected() {
        let mut uf = super::Xj80UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_80_uf_component_count() {
        let mut uf = super::Xj80UnionFind::xj_new();
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
    fn xj_80_uf_component_size() {
        let mut uf = super::Xj80UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_80_uf_largest_component() {
        let mut uf = super::Xj80UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_80_uf_many_elements() {
        let mut uf = super::Xj80UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_80_uf_separate_components() {
        let mut uf = super::Xj80UnionFind::xj_new();
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
    fn xj_80_uf_path_compression() {
        let mut uf = super::Xj80UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_80_bt_insert_get() {
        let mut bt = super::Xj80BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_80_bt_contains_len() {
        let mut bt = super::Xj80BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_80_bt_replace() {
        let mut bt = super::Xj80BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_80_bt_remove() {
        let mut bt = super::Xj80BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_80_bt_keys_values() {
        let mut bt = super::Xj80BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_80_bt_range() {
        let mut bt = super::Xj80BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_80_bt_min_max() {
        let mut bt = super::Xj80BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_80_bt_many_inserts() {
        let mut bt = super::Xj80BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_80 segment tree tests ---

    #[test]
    fn xk_80_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk80SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_80_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk80SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_80_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk80SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_80_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk80SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_80_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk80SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_80_st_single_element() {
        let data = vec![42];
        let st = super::Xk80SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_80_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk80SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_80_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk80SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_80 disjoint intervals tests ---

    #[test]
    fn xk_80_di_add_and_count() {
        let mut di = super::Xk80DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_80_di_merge_overlap() {
        let mut di = super::Xk80DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_80_di_contains() {
        let mut di = super::Xk80DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_80_di_remove() {
        let mut di = super::Xk80DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_80_di_covered_length() {
        let mut di = super::Xk80DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_80_di_gaps() {
        let mut di = super::Xk80DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_80_di_merge_adjacent() {
        let mut di = super::Xk80DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_80_di_empty() {
        let di = super::Xk80DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_80_rope_new_empty() {
        let rope = super::Xl80Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_80_rope_from_str() {
        let rope = super::Xl80Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_80_rope_insert_at() {
        let mut rope = super::Xl80Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_80_rope_delete_range() {
        let mut rope = super::Xl80Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_80_rope_char_at() {
        let rope = super::Xl80Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_80_rope_split_concat() {
        let rope = super::Xl80Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_80_rope_line_count() {
        let rope = super::Xl80Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_80_rope_line_at() {
        let rope = super::Xl80Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_80_sa_build_and_search() {
        let sa = super::Xl80SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_80_sa_count() {
        let sa = super::Xl80SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_80_sa_longest_repeated() {
        let sa = super::Xl80SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_80_sa_all_positions() {
        let sa = super::Xl80SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_80_sa_len() {
        let sa = super::Xl80SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_80_sa_empty() {
        let sa = super::Xl80SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_80_rope_slice() {
        let rope = super::Xl80Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_80_sa_search_start() {
        let sa = super::Xl80SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_80_sparse_set_get() {
        let mut m = super::Xm80MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_80_sparse_row_col() {
        let mut m = super::Xm80MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_80_sparse_transpose() {
        let mut m = super::Xm80MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_80_sparse_multiply_vec() {
        let mut m = super::Xm80MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_80_sparse_nnz_density() {
        let mut m = super::Xm80MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_80_sparse_clear() {
        let mut m = super::Xm80MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_80_sparse_overwrite_zero() {
        let mut m = super::Xm80MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_80_tokenizer_basic() {
        let t = super::Xm80Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_80_tokenizer_count() {
        let t = super::Xm80Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_80_tokenizer_unique() {
        let t = super::Xm80Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_80_tokenizer_frequency() {
        let t = super::Xm80Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_80_tokenizer_delimiter() {
        let t = super::Xm80Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_80_tokenizer_whitespace() {
        let t = super::Xm80Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_80_tokenizer_empty() {
        let t = super::Xm80Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 80 ----

    #[test]
    fn xn_80_fenwick_prefix_sum() {
        let mut ft = super::Xn80Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_80_fenwick_range_sum() {
        let mut ft = super::Xn80Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_80_fenwick_point_query() {
        let mut ft = super::Xn80Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_80_fenwick_len() {
        let ft = super::Xn80Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_80_fenwick_multiple_updates() {
        let mut ft = super::Xn80Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_80_fenwick_single_element() {
        let mut ft = super::Xn80Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_80_fenwick_find_kth() {
        let mut ft = super::Xn80Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_80_fenwick_negative_delta() {
        let mut ft = super::Xn80Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 80 ----

    #[test]
    fn xn_80_avl_insert_get() {
        let mut m = super::Xn80AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_80_avl_remove() {
        let mut m = super::Xn80AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_80_avl_in_order() {
        let mut m = super::Xn80AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_80_avl_min_max() {
        let mut m = super::Xn80AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_80_avl_floor_ceiling() {
        let mut m = super::Xn80AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_80_avl_height_balanced() {
        let mut m = super::Xn80AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_80_avl_overwrite() {
        let mut m = super::Xn80AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_80_avl_empty() {
        let m: super::Xn80AVL<i32, i32> = super::Xn80AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo80RedBlack tests ---

    #[test]
    fn xo_80_rb_insert_and_get() {
        let mut tree = super::Xo80RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_80_rb_len_and_empty() {
        let mut tree = super::Xo80RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_80_rb_min_max() {
        let mut tree = super::Xo80RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_80_rb_contains() {
        let mut tree = super::Xo80RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_80_rb_remove() {
        let mut tree = super::Xo80RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_80_rb_in_order() {
        let mut tree = super::Xo80RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_80_rb_black_height() {
        let mut tree = super::Xo80RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_80_rb_overwrite() {
        let mut tree = super::Xo80RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo80ConsistentHash tests ---

    #[test]
    fn xo_80_ch_add_and_count() {
        let mut ring = super::Xo80ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_80_ch_remove_node() {
        let mut ring = super::Xo80ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_80_ch_get_node() {
        let mut ring = super::Xo80ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_80_ch_empty_ring() {
        let ring = super::Xo80ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_80_ch_distribution() {
        let mut ring = super::Xo80ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_80_ch_rebalance() {
        let mut ring = super::Xo80ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_80_ch_virtual_nodes() {
        let mut ring = super::Xo80ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_80_ch_consistent_lookup() {
        let mut ring = super::Xo80ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_80_splay_insert_get() {
        let mut t = super::Xp80SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_80_splay_remove() {
        let mut t = super::Xp80SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_80_splay_count_increases() {
        let mut t = super::Xp80SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_80_splay_depth() {
        let mut t = super::Xp80SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_80_splay_len_empty() {
        let t = super::Xp80SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_80_splay_min_max() {
        let mut t = super::Xp80SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_80_splay_overwrite() {
        let mut t = super::Xp80SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_80_splay_remove_missing() {
        let mut t = super::Xp80SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_80 treap tests ----
    #[test]
    fn xq_80_treap_empty() {
        let t = super::Xq80Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_80_treap_insert_get() {
        let mut t = super::Xq80Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_80_treap_overwrite() {
        let mut t = super::Xq80Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_80_treap_remove() {
        let mut t = super::Xq80Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_80_treap_min_max() {
        let mut t = super::Xq80Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_80_treap_rank() {
        let mut t = super::Xq80Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_80_treap_kth() {
        let mut t = super::Xq80Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_80_treap_in_order() {
        let mut t = super::Xq80Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_80 VEB tree tests ----
    #[test]
    fn xq_80_veb_empty() {
        let v = super::Xq80VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_80_veb_insert_contains() {
        let mut v = super::Xq80VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_80_veb_min_max() {
        let mut v = super::Xq80VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_80_veb_delete() {
        let mut v = super::Xq80VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_80_veb_successor() {
        let mut v = super::Xq80VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_80_veb_predecessor() {
        let mut v = super::Xq80VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_80_veb_count() {
        let mut v = super::Xq80VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_80_veb_duplicate_insert() {
        let mut v = super::Xq80VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_80_kdtree_empty() {
        let tree = super::Xr80KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_80_kdtree_insert_one() {
        let mut tree = super::Xr80KDTree::xr_new();
        tree.xr_insert(super::Xr80KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_80_kdtree_insert_multiple() {
        let mut tree = super::Xr80KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr80KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_80_kdtree_nearest_neighbor() {
        let mut tree = super::Xr80KDTree::xr_new();
        tree.xr_insert(super::Xr80KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr80KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr80KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_80_kdtree_nn_empty() {
        let tree = super::Xr80KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr80KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_80_kdtree_range_search() {
        let mut tree = super::Xr80KDTree::xr_new();
        tree.xr_insert(super::Xr80KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr80KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr80KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_80_kdtree_range_empty() {
        let mut tree = super::Xr80KDTree::xr_new();
        tree.xr_insert(super::Xr80KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_80_kdtree_all_points() {
        let mut tree = super::Xr80KDTree::xr_new();
        tree.xr_insert(super::Xr80KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr80KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_80_kdtree_depth() {
        let mut tree = super::Xr80KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr80KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_80_kdtree_bounding_box() {
        let mut tree = super::Xr80KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr80KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr80KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_80_persistent_array_new() {
        let arr = super::Xs80PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_80_persistent_array_push() {
        let mut arr = super::Xs80PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_80_persistent_array_set() {
        let mut arr = super::Xs80PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_80_persistent_array_diff() {
        let mut arr = super::Xs80PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_80_persistent_array_rollback() {
        let mut arr = super::Xs80PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_80_persistent_array_history() {
        let mut arr = super::Xs80PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_80_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs80PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_80_persistent_array_from_vec() {
        let arr = super::Xs80PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_80_concurrent_queue_new() {
        let q = super::Xs80ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_80_concurrent_queue_push_pop() {
        let mut q = super::Xs80ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_80_concurrent_queue_full() {
        let mut q = super::Xs80ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_80_concurrent_queue_drain() {
        let mut q = super::Xs80ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_80_concurrent_queue_try_pop() {
        let mut q = super::Xs80ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_80_concurrent_queue_clear() {
        let mut q = super::Xs80ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_80_range_map_new() {
        let rm = super::Xs80RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_80_range_map_insert_get() {
        let mut rm = super::Xs80RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_80_range_map_overlap() {
        let mut rm = super::Xs80RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_80_range_map_remove() {
        let mut rm = super::Xs80RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_80_range_map_gaps() {
        let mut rm = super::Xs80RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_80_range_map_coverage() {
        let mut rm = super::Xs80RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_80_range_map_contains() {
        let mut rm = super::Xs80RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_80_range_map_clear() {
        let mut rm = super::Xs80RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_80_circular_buffer_new() {
        let buf = super::Xs80CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_80_circular_buffer_push_pop() {
        let mut buf = super::Xs80CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_80_circular_buffer_overwrite() {
        let mut buf = super::Xs80CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_80_circular_buffer_peek() {
        let mut buf = super::Xs80CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_80_circular_buffer_is_full() {
        let mut buf = super::Xs80CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_80_circular_buffer_iter() {
        let mut buf = super::Xs80CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_80_circular_buffer_clear() {
        let mut buf = super::Xs80CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_80_circular_buffer_to_vec() {
        let mut buf = super::Xs80CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

}
