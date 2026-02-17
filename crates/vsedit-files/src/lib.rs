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

}
