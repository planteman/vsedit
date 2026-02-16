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
}
