//! Text file operations.

use std::collections::HashMap;
use std::fmt;

/// The type of a file system entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    SymbolicLink,
    Unknown,
}

/// Metadata about a file system entry.
#[derive(Debug, Clone)]
pub struct FileStat {
    pub file_type: FileType,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub readonly: bool,
}

/// An event produced by a file watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    Created(String),
    Changed(String),
    Deleted(String),
}

/// Configuration for a single file watcher.
#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    pub glob_pattern: String,
    pub recursive: bool,
    pub exclude_patterns: Vec<String>,
}

/// Service for files workbench functionality.
#[derive(Debug, Clone)]
pub struct FileService {
    pub watchers: Vec<FileWatcherConfig>,
    pub events: Vec<FileEvent>,
}

impl FileService {
    pub fn new() -> Self {
        Self {
            watchers: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn add_watcher(&mut self, config: FileWatcherConfig) {
        self.watchers.push(config);
    }

    pub fn remove_watcher(&mut self, pattern: &str) -> bool {
        let len = self.watchers.len();
        self.watchers.retain(|w| w.glob_pattern != pattern);
        self.watchers.len() != len
    }

    pub fn record_event(&mut self, event: FileEvent) {
        self.events.push(event);
    }

    pub fn get_events(&self) -> &[FileEvent] {
        &self.events
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Basic glob matching supporting `*` as a wildcard for any sequence of characters.
    pub fn matches_watcher(&self, path: &str) -> bool {
        self.watchers.iter().any(|w| glob_match(&w.glob_pattern, path))
    }

    pub fn get_events_for_uri(&self, uri: &str) -> Vec<&FileEvent> {
        self.events
            .iter()
            .filter(|e| match e {
                FileEvent::Created(u) | FileEvent::Changed(u) | FileEvent::Deleted(u) => u == uri,
            })
            .collect()
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub fn watcher_count(&self) -> usize {
        self.watchers.len()
    }

    pub fn get_stats(&self) -> FileServiceStats {
        let created = self.events.iter().filter(|e| matches!(e, FileEvent::Created(_))).count();
        let changed = self.events.iter().filter(|e| matches!(e, FileEvent::Changed(_))).count();
        let deleted = self.events.iter().filter(|e| matches!(e, FileEvent::Deleted(_))).count();
        FileServiceStats {
            total_events: self.events.len(),
            created_count: created,
            changed_count: changed,
            deleted_count: deleted,
            watcher_count: self.watchers.len(),
        }
    }
}

impl Default for FileService {
    fn default() -> Self {
        Self::new()
    }
}

/// Match `path` against a simple glob `pattern` where `*` matches any sequence of characters.
fn glob_match(pattern: &str, path: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == path;
    }

    let mut remaining = path;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if let Some(rest) = remaining.strip_prefix(part) {
                remaining = rest;
            } else {
                return false;
            }
        } else if i == parts.len() - 1 {
            if !remaining.ends_with(part) {
                return false;
            }
            remaining = "";
        } else {
            if let Some(pos) = remaining.find(part) {
                remaining = &remaining[pos + part.len()..];
            } else {
                return false;
            }
        }
    }

    true
}

impl FileWatcherConfig {
    pub fn is_excluded(&self, path: &str) -> bool {
        self.exclude_patterns.iter().any(|p| glob_match(p, path))
    }
}

impl FileStat {
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File
    }

    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Directory
    }

    pub fn is_symlink(&self) -> bool {
        self.file_type == FileType::SymbolicLink
    }

    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.modified)
    }
}

#[derive(Debug, Clone)]
pub struct FileServiceStats {
    pub total_events: usize,
    pub created_count: usize,
    pub changed_count: usize,
    pub deleted_count: usize,
    pub watcher_count: usize,
}

pub trait FileSystemProvider {
    fn stat(&self, uri: &str) -> Option<FileStat>;

    fn read_file(&self, uri: &str) -> Option<Vec<u8>> {
        let _ = uri;
        None
    }

    fn write_file(&self, uri: &str, content: &[u8]) -> bool {
        let _ = (uri, content);
        false
    }

    fn delete(&self, uri: &str) -> bool {
        let _ = uri;
        false
    }

    fn rename(&self, old_uri: &str, new_uri: &str) -> bool {
        let _ = (old_uri, new_uri);
        false
    }

    fn create_directory(&self, uri: &str) -> bool {
        let _ = uri;
        false
    }
}

impl PartialEq for FileStat {
    fn eq(&self, other: &Self) -> bool {
        self.file_type == other.file_type
            && self.size == other.size
            && self.modified == other.modified
            && self.created == other.created
            && self.readonly == other.readonly
    }
}

impl Eq for FileStat {}

/// Utility methods for working with file paths represented as strings.
pub struct FilePathUtils;

impl FilePathUtils {
    /// Get the file extension after the last `.`.
    pub fn extension(path: &str) -> Option<&str> {
        let name = Self::file_name(path)?;
        let dot_pos = name.rfind('.')?;
        if dot_pos == 0 || dot_pos == name.len() - 1 {
            return None;
        }
        Some(&name[dot_pos + 1..])
    }

    /// Get the filename after the last `/`.
    pub fn file_name(path: &str) -> Option<&str> {
        let trimmed = path.trim_end_matches('/');
        if trimmed.is_empty() {
            return None;
        }
        match trimmed.rfind('/') {
            Some(pos) => Some(&trimmed[pos + 1..]),
            None => Some(trimmed),
        }
    }

    /// Get the parent directory (everything before the last `/`).
    pub fn parent(path: &str) -> Option<&str> {
        let trimmed = path.trim_end_matches('/');
        if trimmed.is_empty() {
            return None;
        }
        match trimmed.rfind('/') {
            Some(0) => Some("/"),
            Some(pos) => Some(&trimmed[..pos]),
            None => None,
        }
    }

    /// Join a base path with a child segment using `/`.
    pub fn join(base: &str, child: &str) -> String {
        if base.ends_with('/') {
            format!("{}{}", base, child)
        } else {
            format!("{}/{}", base, child)
        }
    }

    /// Check whether a path is absolute (starts with `/`).
    pub fn is_absolute(path: &str) -> bool {
        path.starts_with('/')
    }

    /// Collapse consecutive `/` to a single `/` and remove trailing `/`.
    pub fn normalize(path: &str) -> String {
        let mut result = String::with_capacity(path.len());
        let mut prev_slash = false;
        for ch in path.chars() {
            if ch == '/' {
                if !prev_slash {
                    result.push('/');
                }
                prev_slash = true;
            } else {
                result.push(ch);
                prev_slash = false;
            }
        }
        if result.len() > 1 && result.ends_with('/') {
            result.pop();
        }
        result
    }

    /// Count the number of path segments (split on `/`, skip empty).
    pub fn depth(path: &str) -> usize {
        path.split('/').filter(|s| !s.is_empty()).count()
    }
}

/// Utility methods for formatting and parsing file sizes.
pub struct FileSizeFormatter;

impl FileSizeFormatter {
    /// Format a byte count as a human-readable string.
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Parse a human-readable size string like "10 KB" back to bytes.
    pub fn parse_size(s: &str) -> Option<u64> {
        let s = s.trim();
        let (num_str, unit) = if let Some(pos) = s.find(|c: char| c.is_alphabetic()) {
            (s[..pos].trim(), s[pos..].trim())
        } else {
            return s.parse::<u64>().ok();
        };

        let value: f64 = num_str.parse().ok()?;
        let multiplier: u64 = match unit.to_uppercase().as_str() {
            "B" => 1,
            "KB" => 1024,
            "MB" => 1024 * 1024,
            "GB" => 1024 * 1024 * 1024,
            _ => return None,
        };
        Some((value * multiplier as f64) as u64)
    }
}

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

impl fmt::Display for FileEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileEvent::Created(path) => write!(f, "Created: {}", path),
            FileEvent::Changed(path) => write!(f, "Changed: {}", path),
            FileEvent::Deleted(path) => write!(f, "Deleted: {}", path),
        }
    }
}

impl FileService {
    /// Check whether a watcher with the given pattern exists.
    pub fn has_watcher(&self, pattern: &str) -> bool {
        self.watchers.iter().any(|w| w.glob_pattern == pattern)
    }

    /// Return references to all `Created` events.
    pub fn get_created_events(&self) -> Vec<&FileEvent> {
        self.events
            .iter()
            .filter(|e| matches!(e, FileEvent::Created(_)))
            .collect()
    }

    /// Return references to all `Changed` events.
    pub fn get_changed_events(&self) -> Vec<&FileEvent> {
        self.events
            .iter()
            .filter(|e| matches!(e, FileEvent::Changed(_)))
            .collect()
    }

    /// Return references to all `Deleted` events.
    pub fn get_deleted_events(&self) -> Vec<&FileEvent> {
        self.events
            .iter()
            .filter(|e| matches!(e, FileEvent::Deleted(_)))
            .collect()
    }

    /// Collect unique URIs across all events.
    pub fn unique_event_uris(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for event in &self.events {
            let uri = match event {
                FileEvent::Created(u) | FileEvent::Changed(u) | FileEvent::Deleted(u) => u,
            };
            if !seen.contains(uri) {
                seen.push(uri.clone());
            }
        }
        seen
    }
}

/// Detected file encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Ascii,
    Unknown,
}

impl FileEncoding {
    /// Detect encoding from a byte-order mark at the start of content.
    pub fn detect_from_bom(bytes: &[u8]) -> Self {
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            FileEncoding::Utf8Bom
        } else if bytes.starts_with(&[0xFF, 0xFE]) {
            FileEncoding::Utf16Le
        } else if bytes.starts_with(&[0xFE, 0xFF]) {
            FileEncoding::Utf16Be
        } else if bytes.iter().all(|&b| b.is_ascii()) {
            FileEncoding::Ascii
        } else if std::str::from_utf8(bytes).is_ok() {
            FileEncoding::Utf8
        } else {
            FileEncoding::Unknown
        }
    }

    /// Return the BOM bytes for this encoding, if any.
    pub fn bom_bytes(&self) -> &'static [u8] {
        match self {
            FileEncoding::Utf8Bom => &[0xEF, 0xBB, 0xBF],
            FileEncoding::Utf16Le => &[0xFF, 0xFE],
            FileEncoding::Utf16Be => &[0xFE, 0xFF],
            _ => &[],
        }
    }
}

impl fmt::Display for FileEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileEncoding::Utf8 => write!(f, "UTF-8"),
            FileEncoding::Utf8Bom => write!(f, "UTF-8 with BOM"),
            FileEncoding::Utf16Le => write!(f, "UTF-16 LE"),
            FileEncoding::Utf16Be => write!(f, "UTF-16 BE"),
            FileEncoding::Ascii => write!(f, "ASCII"),
            FileEncoding::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Result of comparing two file contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileContentComparison {
    pub are_equal: bool,
    pub size_difference: i64,
    pub first_differing_byte: Option<usize>,
}

impl FileContentComparison {
    /// Compare two byte slices and return a comparison result.
    pub fn compare(a: &[u8], b: &[u8]) -> Self {
        let size_difference = a.len() as i64 - b.len() as i64;
        let first_differing_byte = a.iter().zip(b.iter())
            .position(|(x, y)| x != y)
            .or_else(|| if a.len() != b.len() { Some(a.len().min(b.len())) } else { None });
        FileContentComparison {
            are_equal: a == b,
            size_difference,
            first_differing_byte,
        }
    }
}

/// Tracks metadata changes for a file over time.
#[derive(Debug, Clone)]
pub struct FileMetadataTracker {
    entries: Vec<(String, FileStat)>,
}

impl FileMetadataTracker {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn record(&mut self, uri: String, stat: FileStat) {
        self.entries.push((uri, stat));
    }

    pub fn latest_for(&self, uri: &str) -> Option<&FileStat> {
        self.entries.iter().rev().find(|(u, _)| u == uri).map(|(_, s)| s)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn has_changed(&self, uri: &str) -> bool {
        let matching: Vec<_> = self.entries.iter().filter(|(u, _)| u == uri).collect();
        if matching.len() < 2 { return false; }
        matching.first().map(|(_, s)| s) != matching.last().map(|(_, s)| s)
    }
}

impl Default for FileMetadataTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a batch file operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchFileResult {
    pub uri: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Aggregated results for a batch of file operations.
#[derive(Debug, Clone)]
pub struct BatchFileResults {
    pub results: Vec<BatchFileResult>,
}

impl BatchFileResults {
    pub fn new() -> Self {
        Self { results: Vec::new() }
    }

    pub fn add(&mut self, result: BatchFileResult) {
        self.results.push(result);
    }

    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.success).count()
    }

    pub fn failure_count(&self) -> usize {
        self.results.iter().filter(|r| !r.success).count()
    }

    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(|r| r.success)
    }

    pub fn failed_uris(&self) -> Vec<&str> {
        self.results.iter().filter(|r| !r.success).map(|r| r.uri.as_str()).collect()
    }
}

impl Default for BatchFileResults {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// FileTreeBuilder – constructs a hierarchical file tree from flat paths
// ---------------------------------------------------------------------------

/// A node in a hierarchical file tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeNode {
    /// The name of this node (file or directory name, not full path).
    pub name: String,
    /// Whether this node represents a directory.
    pub is_directory: bool,
    /// Child nodes (empty for files).
    pub children: Vec<FileTreeNode>,
}

impl FileTreeNode {
    /// Create a new file node (leaf).
    pub fn new_file(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_directory: false,
            children: Vec::new(),
        }
    }

    /// Create a new directory node.
    pub fn new_dir(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_directory: true,
            children: Vec::new(),
        }
    }

    /// Find a direct child by name.
    pub fn find(&self, name: &str) -> Option<&FileTreeNode> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Find a direct child by name (mutable).
    pub fn find_mut(&mut self, name: &str) -> Option<&mut FileTreeNode> {
        self.children.iter_mut().find(|c| c.name == name)
    }

    /// Count all file nodes recursively (not counting directories).
    pub fn total_files(&self) -> usize {
        if !self.is_directory {
            return 1;
        }
        self.children.iter().map(|c| c.total_files()).sum()
    }

    /// Count all directory nodes recursively (including self if it is a directory).
    pub fn total_dirs(&self) -> usize {
        if !self.is_directory {
            return 0;
        }
        1 + self.children.iter().map(|c| c.total_dirs()).sum::<usize>()
    }

    /// Sort children: directories first, then alphabetically (case-insensitive).
    pub fn sort_recursive(&mut self) {
        self.children.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        for child in &mut self.children {
            child.sort_recursive();
        }
    }
}

/// Builds a hierarchical [`FileTreeNode`] from a flat list of paths.
pub struct FileTreeBuilder;

impl FileTreeBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self
    }

    /// Build a tree from a slice of path strings.
    ///
    /// Each path is split on `/`. Intermediate components become directory nodes
    /// and the final component becomes a file node. The returned root node
    /// represents the project root directory.
    pub fn build(paths: &[&str]) -> FileTreeNode {
        let mut root = FileTreeNode::new_dir("");

        for path in paths {
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() {
                continue;
            }

            let mut current = &mut root;

            for (i, part) in parts.iter().enumerate() {
                let is_last = i == parts.len() - 1;

                if is_last {
                    // Final component is a file.
                    if current.find(part).is_none() {
                        current.children.push(FileTreeNode::new_file(*part));
                    }
                } else {
                    // Intermediate component is a directory.
                    if current.find(part).is_none() {
                        current.children.push(FileTreeNode::new_dir(*part));
                    }
                    current = current.find_mut(part).unwrap();
                }
            }
        }

        root.sort_recursive();
        root
    }
}

impl Default for FileTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// file_icon_theme – maps file extensions to icon theme names
// ---------------------------------------------------------------------------

/// Map a file extension to an icon theme name.
///
/// Supports common extensions used in software projects. Unknown extensions
/// return `"file"`.
pub fn file_icon_theme(extension: &str) -> &'static str {
    match extension {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "md" => "markdown",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "html" => "html",
        "css" => "css",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cxx" | "cc" => "cpp",
        "h" | "hpp" => "header",
        "sh" | "bash" => "shell",
        "xml" => "xml",
        "svg" => "svg",
        "png" | "jpg" | "jpeg" | "gif" | "webp" => "image",
        "lock" => "lock",
        "txt" => "text",
        _ => "file",
    }
}

// ---------------------------------------------------------------------------
// file_sort – sorts (name, is_directory) tuples: dirs first, then alpha
// ---------------------------------------------------------------------------

/// Sort a list of `(name, is_directory)` tuples.
///
/// Directories are placed before files. Within each group entries are sorted
/// alphabetically using case-insensitive comparison.
pub fn file_sort(entries: &mut [(String, bool)]) {
    entries.sort_by(|a, b| {
        match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        }
    });
}

// ---------------------------------------------------------------------------
// FileGlobFilter
// ---------------------------------------------------------------------------

/// Matches filenames against simple glob patterns.
pub struct FileGlobFilter {
    patterns: Vec<String>,
}

impl FileGlobFilter {
    /// Create a filter with the given glob patterns.
    /// Supports `*` (any chars) and `?` (single char) wildcards.
    pub fn new(patterns: &[&str]) -> Self {
        Self {
            patterns: patterns.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Check if a filename matches any of the patterns.
    pub fn matches(&self, filename: &str) -> bool {
        self.patterns.iter().any(|p| Self::glob_match(p, filename))
    }

    /// Filter a list of filenames, returning only those that match.
    pub fn filter<'a>(&self, names: &[&'a str]) -> Vec<&'a str> {
        names.iter().copied().filter(|n| self.matches(n)).collect()
    }

    /// Filter a list of filenames, returning only those that do NOT match (exclusion).
    pub fn exclude<'a>(&self, names: &[&'a str]) -> Vec<&'a str> {
        names.iter().copied().filter(|n| !self.matches(n)).collect()
    }

    fn glob_match(pattern: &str, text: &str) -> bool {
        let p: Vec<char> = pattern.chars().collect();
        let t: Vec<char> = text.chars().collect();
        Self::glob_match_inner(&p, &t, 0, 0)
    }

    fn glob_match_inner(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
        if pi == pattern.len() {
            return ti == text.len();
        }
        if pattern[pi] == '*' {
            // Try matching * with 0..n characters
            for skip in 0..=(text.len() - ti) {
                if Self::glob_match_inner(pattern, text, pi + 1, ti + skip) {
                    return true;
                }
            }
            return false;
        }
        if ti >= text.len() {
            return false;
        }
        if pattern[pi] == '?' || pattern[pi] == text[ti] {
            return Self::glob_match_inner(pattern, text, pi + 1, ti + 1);
        }
        false
    }

    /// Get the patterns used by this filter.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

// ---------------------------------------------------------------------------
// FileStatistics
// ---------------------------------------------------------------------------

/// Tracks file statistics: counts by extension and total size tracking.
#[derive(Debug, Clone)]
pub struct FileStatistics {
    counts_by_ext: std::collections::HashMap<String, usize>,
    total_size: u64,
    file_count: usize,
    dir_count: usize,
}

impl FileStatistics {
    pub fn new() -> Self {
        Self {
            counts_by_ext: std::collections::HashMap::new(),
            total_size: 0,
            file_count: 0,
            dir_count: 0,
        }
    }

    /// Record a file with its extension and size.
    pub fn record_file(&mut self, filename: &str, size: u64) {
        self.file_count += 1;
        self.total_size += size;
        let ext = filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase();
        if !ext.is_empty() && ext != filename.to_lowercase() {
            *self.counts_by_ext.entry(ext).or_insert(0) += 1;
        }
    }

    /// Record a directory.
    pub fn record_directory(&mut self) {
        self.dir_count += 1;
    }

    /// Get the count for a specific extension.
    pub fn count_for_ext(&self, ext: &str) -> usize {
        self.counts_by_ext.get(&ext.to_lowercase()).copied().unwrap_or(0)
    }

    /// Get all extensions sorted by count (descending).
    pub fn extensions_by_count(&self) -> Vec<(String, usize)> {
        let mut exts: Vec<(String, usize)> = self.counts_by_ext.clone().into_iter().collect();
        exts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        exts
    }

    /// Total number of files recorded.
    pub fn total_files(&self) -> usize {
        self.file_count
    }

    /// Total number of directories recorded.
    pub fn total_dirs(&self) -> usize {
        self.dir_count
    }

    /// Total size in bytes.
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Average file size in bytes.
    pub fn average_file_size(&self) -> f64 {
        if self.file_count == 0 {
            return 0.0;
        }
        self.total_size as f64 / self.file_count as f64
    }

    /// Number of distinct extensions.
    pub fn extension_count(&self) -> usize {
        self.counts_by_ext.len()
    }

    /// Merge another statistics into this one.
    pub fn merge(&mut self, other: &FileStatistics) {
        self.file_count += other.file_count;
        self.dir_count += other.dir_count;
        self.total_size += other.total_size;
        for (ext, count) in &other.counts_by_ext {
            *self.counts_by_ext.entry(ext.clone()).or_insert(0) += count;
        }
    }
}

impl Default for FileStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FileStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} files, {} dirs, {} bytes, {} extensions",
            self.file_count, self.dir_count, self.total_size, self.counts_by_ext.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// FileTreeNode depth / flatten
// ---------------------------------------------------------------------------

impl FileTreeNode {
    /// Maximum depth of the tree (root = 0).
    pub fn max_depth(&self) -> usize {
        if self.children.is_empty() {
            return 0;
        }
        1 + self.children.iter().map(|c| c.max_depth()).max().unwrap_or(0)
    }

    /// Flatten the tree into a list of (path, is_directory) pairs.
    pub fn flatten(&self, prefix: &str) -> Vec<(String, bool)> {
        let mut result = Vec::new();
        let path = if prefix.is_empty() {
            self.name.clone()
        } else if self.name.is_empty() {
            prefix.to_string()
        } else {
            format!("{}/{}", prefix, self.name)
        };
        if !self.name.is_empty() {
            result.push((path.clone(), self.is_directory));
        }
        for child in &self.children {
            let child_prefix = if self.name.is_empty() { prefix } else { &path };
            result.extend(child.flatten(child_prefix));
        }
        result
    }

    /// Add a child node if one with the same name does not already exist.
    pub fn add_child(&mut self, child: FileTreeNode) -> bool {
        if self.find(&child.name).is_some() {
            return false;
        }
        self.children.push(child);
        true
    }

    /// Remove a child by name, returning the removed node.
    pub fn remove_child(&mut self, name: &str) -> Option<FileTreeNode> {
        if let Some(pos) = self.children.iter().position(|c| c.name == name) {
            Some(self.children.remove(pos))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// File path analysis utilities
// ---------------------------------------------------------------------------

/// Return all unique file extensions from a slice of paths.
pub fn file_unique_extensions(paths: &[&str]) -> Vec<String> {
    let mut exts: Vec<String> = paths
        .iter()
        .filter_map(|p| FilePathUtils::extension(p))
        .map(|e| e.to_string())
        .collect();
    exts.sort();
    exts.dedup();
    exts
}

/// Classify a list of paths into files and directories based on extension heuristic.
/// Paths with an extension are treated as files, paths without as directories.
pub fn file_classify_paths(paths: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for &p in paths {
        if FilePathUtils::extension(p).is_some() {
            files.push(p.to_string());
        } else {
            dirs.push(p.to_string());
        }
    }
    (files, dirs)
}

/// Count how many paths share the same parent directory.
pub fn file_count_by_parent<'a>(paths: &[&'a str]) -> std::collections::HashMap<&'a str, usize> {
    let mut counts = std::collections::HashMap::new();
    for &p in paths {
        let parent = FilePathUtils::parent(p).unwrap_or("/");
        *counts.entry(parent).or_insert(0) += 1;
    }
    counts
}

/// Find the longest common prefix among a set of paths.
pub fn file_common_prefix(paths: &[&str]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let first = paths[0];
    let mut prefix_len = first.len();
    for p in &paths[1..] {
        prefix_len = first
            .chars()
            .zip(p.chars())
            .take(prefix_len)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let prefix = &first[..first.char_indices().nth(prefix_len).map(|(i, _)| i).unwrap_or(first.len())];
    // Trim to last separator so we return a complete path segment
    match prefix.rfind('/') {
        Some(idx) => prefix[..=idx].to_string(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// FileEvent helpers
// ---------------------------------------------------------------------------

impl FileEvent {
    /// Return the path associated with this event.
    pub fn path(&self) -> &str {
        match self {
            FileEvent::Created(p) | FileEvent::Changed(p) | FileEvent::Deleted(p) => p,
        }
    }

    /// Return `true` if this is a `Created` event.
    pub fn is_created(&self) -> bool {
        matches!(self, FileEvent::Created(_))
    }

    /// Return `true` if this is a `Changed` event.
    pub fn is_changed(&self) -> bool {
        matches!(self, FileEvent::Changed(_))
    }

    /// Return `true` if this is a `Deleted` event.
    pub fn is_deleted(&self) -> bool {
        matches!(self, FileEvent::Deleted(_))
    }
}

// ---------------------------------------------------------------------------
// FileWatcherConfig builder helpers
// ---------------------------------------------------------------------------

impl FileWatcherConfig {
    /// Create a new watcher config with the given pattern.
    pub fn new(glob_pattern: impl Into<String>) -> Self {
        Self {
            glob_pattern: glob_pattern.into(),
            recursive: false,
            exclude_patterns: Vec::new(),
        }
    }

    /// Set recursive flag.
    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Add an exclusion pattern.
    pub fn with_exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Return `true` if `path` matches the glob and is not excluded.
    pub fn accepts(&self, path: &str) -> bool {
        glob_match(&self.glob_pattern, path) && !self.is_excluded(path)
    }
}

// ---------------------------------------------------------------------------
// FileStat helpers
// ---------------------------------------------------------------------------

impl FileStat {
    /// Return a human-readable size string.
    pub fn human_size(&self) -> String {
        FileSizeFormatter::format_bytes(self.size)
    }

    /// Return `true` if the file was modified after it was created.
    pub fn was_modified_after_creation(&self) -> bool {
        self.modified > self.created
    }
}

// ---------------------------------------------------------------------------
// FileService – filtering and replay
// ---------------------------------------------------------------------------

impl FileService {
    /// Return events that match a predicate.
    pub fn filter_events<F>(&self, predicate: F) -> Vec<&FileEvent>
    where
        F: Fn(&FileEvent) -> bool,
    {
        self.events.iter().filter(|e| predicate(e)).collect()
    }

    /// Remove all events whose path matches the given glob pattern.
    pub fn remove_events_matching(&mut self, pattern: &str) {
        self.events.retain(|e| !glob_match(pattern, e.path()));
    }

    /// Replay all recorded events into the supplied closure.
    pub fn replay_events<F>(&self, mut handler: F)
    where
        F: FnMut(&FileEvent),
    {
        for event in &self.events {
            handler(event);
        }
    }

    /// Return the most recent event (last recorded).
    pub fn last_event(&self) -> Option<&FileEvent> {
        self.events.last()
    }

    /// Find watchers whose pattern matches the given path.
    pub fn matching_watchers(&self, path: &str) -> Vec<&FileWatcherConfig> {
        self.watchers
            .iter()
            .filter(|w| glob_match(&w.glob_pattern, path))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// FilePathUtils – additional path manipulation
// ---------------------------------------------------------------------------

impl FilePathUtils {
    /// Return the file name without its extension.
    pub fn stem(path: &str) -> Option<&str> {
        let name = Self::file_name(path)?;
        match name.rfind('.') {
            Some(0) | None => Some(name),
            Some(pos) => Some(&name[..pos]),
        }
    }

    /// Return `true` if `child` is a direct or nested child of `parent`.
    pub fn is_descendant(parent: &str, child: &str) -> bool {
        let p = parent.trim_end_matches('/');
        let c = child.trim_end_matches('/');
        if p.is_empty() {
            return true;
        }
        c.starts_with(p) && c.as_bytes().get(p.len()) == Some(&b'/')
    }

    /// Return the relative path of `full` with respect to `base`.
    /// Returns `None` if `full` is not under `base`.
    pub fn relative(base: &str, full: &str) -> Option<String> {
        let b = base.trim_end_matches('/');
        let f = full.trim_end_matches('/');
        if !f.starts_with(b) {
            return None;
        }
        let rest = &f[b.len()..];
        if rest.is_empty() {
            return Some(String::new());
        }
        if rest.starts_with('/') {
            Some(rest[1..].to_string())
        } else {
            None
        }
    }

    /// Split a path into all of its segments.
    pub fn segments(path: &str) -> Vec<&str> {
        path.split('/').filter(|s| !s.is_empty()).collect()
    }

    /// Return `true` if the filename starts with `.` (hidden file on Unix).
    pub fn is_hidden(path: &str) -> bool {
        Self::file_name(path)
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
    }
}

/// Return `true` if the file extension matches any in the given set (case-insensitive).
pub fn file_has_extension(path: &str, extensions: &[&str]) -> bool {
    if let Some(ext) = FilePathUtils::extension(path) {
        extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
    } else {
        false
    }
}


// ---------------------------------------------------------------------------
// FileSystemTreeDiff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum FileTreeChange {
    Added(String),
    Removed(String),
    Renamed { old_path: String, new_path: String },
    Modified(String),
}

impl fmt::Display for FileTreeChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileTreeChange::Added(p) => write!(f, "+ {p}"),
            FileTreeChange::Removed(p) => write!(f, "- {p}"),
            FileTreeChange::Renamed { old_path, new_path } => write!(f, "{old_path} -> {new_path}"),
            FileTreeChange::Modified(p) => write!(f, "~ {p}"),
        }
    }
}

pub struct FileSystemTreeDiff;

impl FileSystemTreeDiff {
    pub fn diff(old: &[String], new: &[String]) -> Vec<FileTreeChange> {
        let mut changes = Vec::new();
        let old_set: std::collections::HashSet<&str> = old.iter().map(|s| s.as_str()).collect();
        let new_set: std::collections::HashSet<&str> = new.iter().map(|s| s.as_str()).collect();
        for path in old {
            if !new_set.contains(path.as_str()) { changes.push(FileTreeChange::Removed(path.clone())); }
        }
        for path in new {
            if !old_set.contains(path.as_str()) { changes.push(FileTreeChange::Added(path.clone())); }
        }
        changes
    }

    pub fn detect_renames(changes: &mut Vec<FileTreeChange>) {
        let removed: Vec<String> = changes.iter().filter_map(|c| {
            if let FileTreeChange::Removed(p) = c { Some(p.clone()) } else { None }
        }).collect();
        let added: Vec<String> = changes.iter().filter_map(|c| {
            if let FileTreeChange::Added(p) = c { Some(p.clone()) } else { None }
        }).collect();
        let mut renames = Vec::new();
        for r in &removed {
            let r_name = FilePathUtils::file_name(r);
            for a in &added {
                if FilePathUtils::file_name(a) == r_name && r_name.is_some() {
                    renames.push((r.clone(), a.clone()));
                }
            }
        }
        for (old, new) in &renames {
            changes.retain(|c| !matches!(c, FileTreeChange::Removed(p) if p == old));
            changes.retain(|c| !matches!(c, FileTreeChange::Added(p) if p == new));
            changes.push(FileTreeChange::Renamed { old_path: old.clone(), new_path: new.clone() });
        }
    }
}

// ---------------------------------------------------------------------------
// FileIconResolver
// ---------------------------------------------------------------------------

pub struct FileIconResolver {
    icon_map: std::collections::HashMap<String, String>,
}

impl FileIconResolver {
    pub fn new() -> Self {
        let mut icon_map = std::collections::HashMap::new();
        for (ext, icon) in [("rs","rust"),("py","python"),("js","javascript"),("ts","typescript"),
            ("html","html"),("css","css"),("json","json"),("toml","toml"),("yaml","yaml"),
            ("yml","yaml"),("md","markdown"),("txt","text"),("sh","shell"),("go","go"),
            ("c","c"),("cpp","cpp"),("java","java"),("rb","ruby")] {
            icon_map.insert(ext.to_string(), icon.to_string());
        }
        Self { icon_map }
    }

    pub fn resolve(&self, path: &str) -> &str {
        FilePathUtils::extension(path)
            .and_then(|ext| self.icon_map.get(ext))
            .map(|s| s.as_str())
            .unwrap_or("file")
    }

    pub fn register(&mut self, ext: impl Into<String>, icon: impl Into<String>) {
        self.icon_map.insert(ext.into(), icon.into());
    }

    pub fn len(&self) -> usize { self.icon_map.len() }
    pub fn is_empty(&self) -> bool { self.icon_map.is_empty() }
}

impl Default for FileIconResolver { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// FileExcludePattern
// ---------------------------------------------------------------------------

pub struct FileExcludePattern {
    patterns: Vec<String>,
}

impl FileExcludePattern {
    pub fn new() -> Self { Self { patterns: Vec::new() } }

    pub fn add_pattern(&mut self, pattern: impl Into<String>) { self.patterns.push(pattern.into()); }

    pub fn is_excluded(&self, path: &str) -> bool {
        self.patterns.iter().any(|p| {
            if p.starts_with("*.") { path.ends_with(&p[1..]) } else { path.contains(p) }
        })
    }

    pub fn len(&self) -> usize { self.patterns.len() }
    pub fn is_empty(&self) -> bool { self.patterns.is_empty() }
    pub fn clear(&mut self) { self.patterns.clear(); }

    pub fn remove_pattern(&mut self, pattern: &str) -> bool {
        if let Some(i) = self.patterns.iter().position(|p| p == pattern) { self.patterns.remove(i); true } else { false }
    }
}

impl Default for FileExcludePattern { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// FileReadonlyIndicator
// ---------------------------------------------------------------------------

pub struct FileReadonlyIndicator {
    readonly_paths: Vec<String>,
}

impl FileReadonlyIndicator {
    pub fn new() -> Self { Self { readonly_paths: Vec::new() } }

    pub fn mark_readonly(&mut self, path: impl Into<String>) {
        let p = path.into();
        if !self.readonly_paths.contains(&p) { self.readonly_paths.push(p); }
    }

    pub fn unmark_readonly(&mut self, path: &str) -> bool {
        if let Some(i) = self.readonly_paths.iter().position(|p| p == path) { self.readonly_paths.remove(i); true } else { false }
    }

    pub fn is_readonly(&self, path: &str) -> bool { self.readonly_paths.iter().any(|p| p == path) }
    pub fn readonly_count(&self) -> usize { self.readonly_paths.len() }
}

impl Default for FileReadonlyIndicator { fn default() -> Self { Self::new() } }


// === File Icon Theme Loader ===

/// File Icon Theme Loader implementation.
#[derive(Debug, Clone)]
pub struct FileIconThemeLoader {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: FileIconThemeLoaderStats,
}

/// Statistics for FileIconThemeLoader.
#[derive(Debug, Clone, Default)]
pub struct FileIconThemeLoaderStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl FileIconThemeLoaderStats {
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

impl FileIconThemeLoader {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: FileIconThemeLoaderStats::default(),
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

    pub fn stats(&self) -> &FileIconThemeLoaderStats {
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

impl Default for FileIconThemeLoader {
    fn default() -> Self {
        Self::new()
    }
}

// === File System Event Aggregator ===

/// Priority level for FileSystemEventAggregator items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileSystemEventAggregatorPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl FileSystemEventAggregatorPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for FileSystemEventAggregatorPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// File System Event Aggregator implementation.
#[derive(Debug, Clone)]
pub struct FileSystemEventAggregator {
    items: Vec<FileSystemEventAggregatorItem>,
    max_items: usize,
    default_priority: FileSystemEventAggregatorPriority,
}

/// A single item in FileSystemEventAggregator.
#[derive(Debug, Clone)]
pub struct FileSystemEventAggregatorItem {
    pub id: String,
    pub label: String,
    pub priority: FileSystemEventAggregatorPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl FileSystemEventAggregatorItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: FileSystemEventAggregatorPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: FileSystemEventAggregatorPriority) -> Self {
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

impl FileSystemEventAggregator {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: FileSystemEventAggregatorPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: FileSystemEventAggregatorItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<FileSystemEventAggregatorItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&FileSystemEventAggregatorItem> {
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

    pub fn by_priority(&self, priority: FileSystemEventAggregatorPriority) -> Vec<&FileSystemEventAggregatorItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&FileSystemEventAggregatorItem> {
        let mut sorted: Vec<&FileSystemEventAggregatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&FileSystemEventAggregatorItem> {
        let mut sorted: Vec<&FileSystemEventAggregatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&FileSystemEventAggregatorItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: FileSystemEventAggregatorPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> FileSystemEventAggregatorPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &FileSystemEventAggregatorItem> {
        self.items.iter()
    }
}

impl Default for FileSystemEventAggregator {
    fn default() -> Self {
        Self::new()
    }
}


// ─── WbFile LRU Cache ───────────────────────────────────────

/// A simple LRU cache for file metadata.
#[derive(Debug)]
pub struct WbFileLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> WbFileLruCache<V> {
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

impl<V: Clone + fmt::Display> fmt::Display for WbFileLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WbFileLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── WbFile Builder & Validator ─────────────────────────────

/// Builder for constructing workbench files configurations.
#[derive(Debug, Clone)]
pub struct WbFileBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl WbFileBuilder {
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

    pub fn build(self) -> Result<WbFileCfg, WbFileBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(WbFileBuildErr { errors }); }
        Ok(WbFileCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated workbench files configuration.
#[derive(Debug, Clone)]
pub struct WbFileCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl WbFileCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &WbFileCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for WbFileCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WbFileCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct WbFileBuildErr { pub errors: Vec<String> }

impl fmt::Display for WbFileBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WbFileBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for WbFileBuildErr {}



// ---------------------------------------------------------------------------
// wb_files – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench file operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbFilesFileWatchEvent {
    Created,
    Modified,
    Deleted,
    Renamed,
}

impl YWbFilesFileWatchEvent {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Created => 0,
            Self::Modified => 1,
            Self::Deleted => 2,
            Self::Renamed => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Modified => "Modified",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbFilesFileWatchEvent] {
        &[
            YWbFilesFileWatchEvent::Created,
            YWbFilesFileWatchEvent::Modified,
            YWbFilesFileWatchEvent::Deleted,
            YWbFilesFileWatchEvent::Renamed,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbFilesFileWatchEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks file watcher stats data.
#[derive(Debug, Clone)]
pub struct YWbFilesFileWatcherStats {
    pub events_seen: u64,
    pub paths_watched: usize,
    pub errors: u64,
}

impl YWbFilesFileWatcherStats {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            events_seen: 0,
            paths_watched: 0,
            errors: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbFilesFileWatcherStats({}: {:?})", "events_seen", self.events_seen)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_files_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_files_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_files_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_files_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_files_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_files_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_files_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_files_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_files – Extended file index entry helpers
// ---------------------------------------------------------------------------

/// Priority levels for file index entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbFilesPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbFilesPriority {
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
    pub fn all_asc() -> [ZWbFilesPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbFilesPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks file index entry data.
#[derive(Debug, Clone)]
pub struct ZWbFilesFileIndexEntry {
    pub paths: Vec<(String, u64)>,
    pub total_size: u64,
    pub indexed_at_ms: u64,
}

impl ZWbFilesFileIndexEntry {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            total_size: 0,
            indexed_at_ms: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbFilesFileIndexEntry[total_size={:?}, indexed_at_ms={:?}]", self.total_size, self.indexed_at_ms)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for file index entry.
pub fn z_wb_files_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_files_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_files_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_files_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_wb_files_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_files_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_files_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 104
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer104 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer104 {
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
pub fn xb_fnv1a_104(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_104<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_104<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_104(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_104(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 208
// ---------------------------------------------------------------------------

/// Generic object pool `Xc208Pool<T>`.
pub struct Xc208Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc208Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc208PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc208Pool<T> {
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
    pub fn stats(&self) -> Xc208PoolStats {
        Xc208PoolStats {
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

impl<T> Default for Xc208Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc208Scheduler`.
pub struct Xc208Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc208Scheduler {
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

impl Default for Xc208Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_208 hash for the given byte slice.
pub fn xc_208_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_208 convention.
pub fn xc_208_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe117 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe117Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe117PipelineError {
    pub stage: Xe117Stage,
    pub message: String,
}

impl std::fmt::Display for Xe117PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe117Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe117Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError>>>,
    stage_names: Vec<Xe117Stage>,
}

impl Xe117Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe117Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe117Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe117Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe117Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> {
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

    pub fn compose(mut self, other: Xe117Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe117CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe117CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe117Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe117CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe117CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe117Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe117CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_117_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe117CacheEntry {
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

    fn xe_117_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe117CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_117_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> {
    Ok(data)
}

pub fn xe_117_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_117_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_117_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_117_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe117PipelineError> {
    Err(Xe117PipelineError {
        stage: Xe117Stage::Parse,
        message: "intentional failure".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_watchers() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: true,
            exclude_patterns: vec![],
        });
        assert_eq!(svc.watchers.len(), 1);
        assert!(svc.remove_watcher("*.rs"));
        assert!(svc.watchers.is_empty());
        assert!(!svc.remove_watcher("*.rs"));
    }

    #[test]
    fn record_and_clear_events() {
        let mut svc = FileService::new();
        svc.record_event(FileEvent::Created("a.txt".into()));
        svc.record_event(FileEvent::Changed("a.txt".into()));
        svc.record_event(FileEvent::Deleted("b.txt".into()));
        assert_eq!(svc.get_events().len(), 3);
        svc.clear_events();
        assert!(svc.get_events().is_empty());
    }

    #[test]
    fn matches_watcher_glob() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: false,
            exclude_patterns: vec![],
        });
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "src/*/*.ts".into(),
            recursive: true,
            exclude_patterns: vec![],
        });
        assert!(svc.matches_watcher("main.rs"));
        assert!(svc.matches_watcher("lib.rs"));
        assert!(!svc.matches_watcher("main.py"));
        assert!(svc.matches_watcher("src/components/app.ts"));
        assert!(!svc.matches_watcher("src/app.py"));
    }

    #[test]
    fn is_excluded_matches_patterns() {
        let config = FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: true,
            exclude_patterns: vec!["*.test.rs".into(), "target/*".into()],
        };
        assert!(config.is_excluded("foo.test.rs"));
        assert!(config.is_excluded("target/debug.rs"));
        assert!(!config.is_excluded("main.rs"));
    }

    #[test]
    fn get_events_for_uri_filters() {
        let mut svc = FileService::new();
        svc.record_event(FileEvent::Created("a.txt".into()));
        svc.record_event(FileEvent::Changed("a.txt".into()));
        svc.record_event(FileEvent::Deleted("b.txt".into()));
        let events = svc.get_events_for_uri("a.txt");
        assert_eq!(events.len(), 2);
        assert_eq!(svc.get_events_for_uri("b.txt").len(), 1);
        assert_eq!(svc.get_events_for_uri("c.txt").len(), 0);
    }

    #[test]
    fn event_and_watcher_counts() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: false,
            exclude_patterns: vec![],
        });
        svc.record_event(FileEvent::Created("a.rs".into()));
        assert_eq!(svc.event_count(), 1);
        assert_eq!(svc.watcher_count(), 1);
    }

    #[test]
    fn file_service_stats() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: false,
            exclude_patterns: vec![],
        });
        svc.record_event(FileEvent::Created("a.rs".into()));
        svc.record_event(FileEvent::Created("b.rs".into()));
        svc.record_event(FileEvent::Changed("a.rs".into()));
        svc.record_event(FileEvent::Deleted("c.rs".into()));
        let stats = svc.get_stats();
        assert_eq!(stats.total_events, 4);
        assert_eq!(stats.created_count, 2);
        assert_eq!(stats.changed_count, 1);
        assert_eq!(stats.deleted_count, 1);
        assert_eq!(stats.watcher_count, 1);
    }

    #[test]
    fn file_stat_type_checks() {
        let stat = FileStat {
            file_type: FileType::File,
            size: 100,
            modified: 1000,
            created: 900,
            readonly: false,
        };
        assert!(stat.is_file());
        assert!(!stat.is_directory());
        assert!(!stat.is_symlink());

        let dir = FileStat { file_type: FileType::Directory, ..stat.clone() };
        assert!(dir.is_directory());

        let link = FileStat { file_type: FileType::SymbolicLink, ..stat };
        assert!(link.is_symlink());
    }

    #[test]
    fn file_stat_age_seconds() {
        let stat = FileStat {
            file_type: FileType::File,
            size: 50,
            modified: 1000,
            created: 900,
            readonly: false,
        };
        assert_eq!(stat.age_seconds(1500), 500);
        assert_eq!(stat.age_seconds(500), 0);
    }

    #[test]
    fn file_system_provider_defaults() {
        struct NullProvider;
        impl FileSystemProvider for NullProvider {
            fn stat(&self, _uri: &str) -> Option<FileStat> {
                None
            }
        }
        let provider = NullProvider;
        assert!(provider.stat("file:///a").is_none());
        assert!(provider.read_file("file:///a").is_none());
        assert!(!provider.write_file("file:///a", b"data"));
        assert!(!provider.delete("file:///a"));
        assert!(!provider.rename("file:///a", "file:///b"));
        assert!(!provider.create_directory("file:///dir"));
    }

    #[test]
    fn file_stat_clone_and_debug() {
        let stat = FileStat {
            file_type: FileType::File,
            size: 42,
            modified: 100,
            created: 50,
            readonly: true,
        };
        let cloned = stat.clone();
        assert_eq!(cloned.size, 42);
        assert!(cloned.readonly);
        let debug = format!("{:?}", cloned);
        assert!(debug.contains("FileStat"));
    }

    #[test]
    fn test_extension() {
        assert_eq!(FilePathUtils::extension("main.rs"), Some("rs"));
        assert_eq!(FilePathUtils::extension("archive.tar.gz"), Some("gz"));
        assert_eq!(FilePathUtils::extension("no_ext"), None);
        assert_eq!(FilePathUtils::extension("/path/to/file.txt"), Some("txt"));
        assert_eq!(FilePathUtils::extension(".hidden"), None);
    }

    #[test]
    fn test_file_name() {
        assert_eq!(FilePathUtils::file_name("/path/to/file.rs"), Some("file.rs"));
        assert_eq!(FilePathUtils::file_name("file.rs"), Some("file.rs"));
        assert_eq!(FilePathUtils::file_name("/path/to/dir/"), Some("dir"));
        assert_eq!(FilePathUtils::file_name("/"), None);
    }

    #[test]
    fn test_parent() {
        assert_eq!(FilePathUtils::parent("/path/to/file.rs"), Some("/path/to"));
        assert_eq!(FilePathUtils::parent("/file.rs"), Some("/"));
        assert_eq!(FilePathUtils::parent("file.rs"), None);
        assert_eq!(FilePathUtils::parent("/"), None);
    }

    #[test]
    fn test_join() {
        assert_eq!(FilePathUtils::join("/path/to", "file.rs"), "/path/to/file.rs");
        assert_eq!(FilePathUtils::join("/path/to/", "file.rs"), "/path/to/file.rs");
        assert_eq!(FilePathUtils::join("base", "child"), "base/child");
    }

    #[test]
    fn test_is_absolute() {
        assert!(FilePathUtils::is_absolute("/usr/bin"));
        assert!(!FilePathUtils::is_absolute("relative/path"));
        assert!(!FilePathUtils::is_absolute(""));
    }

    #[test]
    fn test_normalize() {
        assert_eq!(FilePathUtils::normalize("/path//to///file"), "/path/to/file");
        assert_eq!(FilePathUtils::normalize("/path/to/dir/"), "/path/to/dir");
        assert_eq!(FilePathUtils::normalize("/"), "/");
        assert_eq!(FilePathUtils::normalize("a//b"), "a/b");
    }

    #[test]
    fn test_depth() {
        assert_eq!(FilePathUtils::depth("/usr/local/bin"), 3);
        assert_eq!(FilePathUtils::depth("a/b"), 2);
        assert_eq!(FilePathUtils::depth("/"), 0);
        assert_eq!(FilePathUtils::depth("single"), 1);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(FileSizeFormatter::format_bytes(0), "0 B");
        assert_eq!(FileSizeFormatter::format_bytes(512), "512 B");
        assert_eq!(FileSizeFormatter::format_bytes(1024), "1.0 KB");
        assert_eq!(FileSizeFormatter::format_bytes(1048576), "1.0 MB");
        assert_eq!(FileSizeFormatter::format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(FileSizeFormatter::parse_size("100 B"), Some(100));
        assert_eq!(FileSizeFormatter::parse_size("1 KB"), Some(1024));
        assert_eq!(FileSizeFormatter::parse_size("1 MB"), Some(1048576));
        assert_eq!(FileSizeFormatter::parse_size("1 GB"), Some(1073741824));
        assert_eq!(FileSizeFormatter::parse_size("invalid"), None);
    }

    #[test]
    fn test_file_type_display() {
        assert_eq!(format!("{}", FileType::File), "file");
        assert_eq!(format!("{}", FileType::Directory), "directory");
        assert_eq!(format!("{}", FileType::SymbolicLink), "symlink");
        assert_eq!(format!("{}", FileType::Unknown), "unknown");
    }

    #[test]
    fn test_file_event_display() {
        assert_eq!(
            format!("{}", FileEvent::Created("a.txt".into())),
            "Created: a.txt"
        );
        assert_eq!(
            format!("{}", FileEvent::Changed("b.txt".into())),
            "Changed: b.txt"
        );
        assert_eq!(
            format!("{}", FileEvent::Deleted("c.txt".into())),
            "Deleted: c.txt"
        );
    }

    #[test]
    fn test_has_watcher() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig {
            glob_pattern: "*.rs".into(),
            recursive: false,
            exclude_patterns: vec![],
        });
        assert!(svc.has_watcher("*.rs"));
        assert!(!svc.has_watcher("*.py"));
    }

    #[test]
    fn test_get_event_types() {
        let mut svc = FileService::new();
        svc.record_event(FileEvent::Created("a.rs".into()));
        svc.record_event(FileEvent::Changed("b.rs".into()));
        svc.record_event(FileEvent::Deleted("c.rs".into()));
        svc.record_event(FileEvent::Created("d.rs".into()));

        assert_eq!(svc.get_created_events().len(), 2);
        assert_eq!(svc.get_changed_events().len(), 1);
        assert_eq!(svc.get_deleted_events().len(), 1);
    }

    #[test]
    fn test_unique_event_uris() {
        let mut svc = FileService::new();
        svc.record_event(FileEvent::Created("a.rs".into()));
        svc.record_event(FileEvent::Changed("a.rs".into()));
        svc.record_event(FileEvent::Deleted("b.rs".into()));
        svc.record_event(FileEvent::Created("c.rs".into()));

        let uris = svc.unique_event_uris();
        assert_eq!(uris.len(), 3);
        assert!(uris.contains(&"a.rs".to_string()));
        assert!(uris.contains(&"b.rs".to_string()));
        assert!(uris.contains(&"c.rs".to_string()));
    }

    #[test]
    fn test_encoding_detect_utf8_bom() {
        let bytes = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(FileEncoding::detect_from_bom(&bytes), FileEncoding::Utf8Bom);
    }

    #[test]
    fn test_encoding_detect_ascii() {
        assert_eq!(FileEncoding::detect_from_bom(b"hello"), FileEncoding::Ascii);
    }

    #[test]
    fn test_encoding_detect_utf16le() {
        let bytes = [0xFF, 0xFE, 0x00, 0x41];
        assert_eq!(FileEncoding::detect_from_bom(&bytes), FileEncoding::Utf16Le);
    }

    #[test]
    fn test_encoding_detect_utf16be() {
        let bytes = [0xFE, 0xFF, 0x00, 0x41];
        assert_eq!(FileEncoding::detect_from_bom(&bytes), FileEncoding::Utf16Be);
    }

    #[test]
    fn test_encoding_detect_utf8() {
        let bytes = "héllo".as_bytes();
        assert_eq!(FileEncoding::detect_from_bom(bytes), FileEncoding::Utf8);
    }

    #[test]
    fn test_encoding_display() {
        assert_eq!(format!("{}", FileEncoding::Utf8), "UTF-8");
        assert_eq!(format!("{}", FileEncoding::Ascii), "ASCII");
        assert_eq!(format!("{}", FileEncoding::Unknown), "Unknown");
    }

    #[test]
    fn test_encoding_bom_bytes() {
        assert_eq!(FileEncoding::Utf8Bom.bom_bytes(), &[0xEF, 0xBB, 0xBF]);
        assert!(FileEncoding::Utf8.bom_bytes().is_empty());
    }

    #[test]
    fn test_file_content_comparison_equal() {
        let cmp = FileContentComparison::compare(b"hello", b"hello");
        assert!(cmp.are_equal);
        assert_eq!(cmp.size_difference, 0);
        assert_eq!(cmp.first_differing_byte, None);
    }

    #[test]
    fn test_file_content_comparison_different() {
        let cmp = FileContentComparison::compare(b"hello", b"hXllo");
        assert!(!cmp.are_equal);
        assert_eq!(cmp.first_differing_byte, Some(1));
    }

    #[test]
    fn test_file_content_comparison_size_diff() {
        let cmp = FileContentComparison::compare(b"hi", b"hello");
        assert!(!cmp.are_equal);
        assert_eq!(cmp.size_difference, -3);
    }

    #[test]
    fn test_metadata_tracker_basic() {
        let mut tracker = FileMetadataTracker::new();
        let stat = FileStat {
            file_type: FileType::File, size: 100, modified: 1000, created: 900, readonly: false,
        };
        tracker.record("a.rs".into(), stat.clone());
        assert_eq!(tracker.entry_count(), 1);
        assert_eq!(tracker.latest_for("a.rs").unwrap().size, 100);
        assert!(tracker.latest_for("b.rs").is_none());
    }

    #[test]
    fn test_metadata_tracker_has_changed() {
        let mut tracker = FileMetadataTracker::new();
        let stat1 = FileStat {
            file_type: FileType::File, size: 100, modified: 1000, created: 900, readonly: false,
        };
        let stat2 = FileStat {
            file_type: FileType::File, size: 200, modified: 2000, created: 900, readonly: false,
        };
        tracker.record("a.rs".into(), stat1);
        assert!(!tracker.has_changed("a.rs"));
        tracker.record("a.rs".into(), stat2);
        assert!(tracker.has_changed("a.rs"));
    }

    #[test]
    fn test_batch_file_results_all_success() {
        let mut results = BatchFileResults::new();
        results.add(BatchFileResult { uri: "a.rs".into(), success: true, error_message: None });
        results.add(BatchFileResult { uri: "b.rs".into(), success: true, error_message: None });
        assert!(results.all_succeeded());
        assert_eq!(results.success_count(), 2);
        assert_eq!(results.failure_count(), 0);
    }

    #[test]
    fn test_batch_file_results_with_failures() {
        let mut results = BatchFileResults::new();
        results.add(BatchFileResult { uri: "a.rs".into(), success: true, error_message: None });
        results.add(BatchFileResult { uri: "b.rs".into(), success: false, error_message: Some("err".into()) });
        assert!(!results.all_succeeded());
        assert_eq!(results.failure_count(), 1);
        assert_eq!(results.failed_uris(), vec!["b.rs"]);
    }

    // -----------------------------------------------------------------------
    // FileTreeBuilder tests
    // -----------------------------------------------------------------------

    #[test]
    fn tree_builder_simple() {
        let tree = FileTreeBuilder::build(&["README.md", "Cargo.toml"]);
        assert!(tree.is_directory);
        assert_eq!(tree.children.len(), 2);
        // Sorted alphabetically: Cargo.toml < README.md (case-insensitive)
        assert_eq!(tree.children[0].name, "Cargo.toml");
        assert_eq!(tree.children[1].name, "README.md");
        assert!(!tree.children[0].is_directory);
    }

    #[test]
    fn tree_builder_nested_paths() {
        let tree = FileTreeBuilder::build(&[
            "src/main.rs",
            "src/lib.rs",
            "src/utils/helpers.rs",
            "README.md",
        ]);
        // Root should have: src/ dir + README.md file. Dirs come first.
        assert_eq!(tree.children.len(), 2);
        let src = &tree.children[0];
        assert_eq!(src.name, "src");
        assert!(src.is_directory);
        // src has: utils/ dir + lib.rs, main.rs files → dirs first
        assert_eq!(src.children.len(), 3);
        assert_eq!(src.children[0].name, "utils");
        assert!(src.children[0].is_directory);
        // utils/helpers.rs
        assert_eq!(src.children[0].children.len(), 1);
        assert_eq!(src.children[0].children[0].name, "helpers.rs");
    }

    #[test]
    fn tree_node_find_child() {
        let tree = FileTreeBuilder::build(&["src/main.rs", "README.md"]);
        assert!(tree.find("src").is_some());
        assert!(tree.find("README.md").is_some());
        assert!(tree.find("nonexistent").is_none());
    }

    #[test]
    fn tree_node_total_files() {
        let tree = FileTreeBuilder::build(&[
            "src/main.rs",
            "src/lib.rs",
            "src/utils/helpers.rs",
            "README.md",
        ]);
        assert_eq!(tree.total_files(), 4);
    }

    #[test]
    fn tree_node_total_dirs() {
        let tree = FileTreeBuilder::build(&[
            "src/main.rs",
            "src/lib.rs",
            "src/utils/helpers.rs",
            "README.md",
        ]);
        // root, src, utils → 3 directories
        assert_eq!(tree.total_dirs(), 3);
    }

    // -----------------------------------------------------------------------
    // file_icon_theme tests
    // -----------------------------------------------------------------------

    #[test]
    fn icon_theme_rust() {
        assert_eq!(file_icon_theme("rs"), "rust");
        assert_eq!(file_icon_theme("py"), "python");
        assert_eq!(file_icon_theme("js"), "javascript");
        assert_eq!(file_icon_theme("ts"), "typescript");
        assert_eq!(file_icon_theme("md"), "markdown");
        assert_eq!(file_icon_theme("json"), "json");
        assert_eq!(file_icon_theme("toml"), "toml");
        assert_eq!(file_icon_theme("yaml"), "yaml");
        assert_eq!(file_icon_theme("yml"), "yaml");
        assert_eq!(file_icon_theme("html"), "html");
        assert_eq!(file_icon_theme("css"), "css");
    }

    #[test]
    fn icon_theme_unknown_ext() {
        assert_eq!(file_icon_theme("xyz"), "file");
        assert_eq!(file_icon_theme(""), "file");
        assert_eq!(file_icon_theme("zzz"), "file");
    }

    // -----------------------------------------------------------------------
    // file_sort tests
    // -----------------------------------------------------------------------

    #[test]
    fn file_sort_dirs_first() {
        let mut entries = vec![
            ("main.rs".to_string(), false),
            ("src".to_string(), true),
            ("lib.rs".to_string(), false),
            ("tests".to_string(), true),
        ];
        file_sort(&mut entries);
        // Dirs first
        assert!(entries[0].1); // directory
        assert!(entries[1].1); // directory
        assert!(!entries[2].1); // file
        assert!(!entries[3].1); // file
    }

    #[test]
    fn file_sort_alphabetical() {
        let mut entries = vec![
            ("Zebra.rs".to_string(), false),
            ("alpha.rs".to_string(), false),
            ("Beta.rs".to_string(), false),
            ("zdir".to_string(), true),
            ("Adir".to_string(), true),
        ];
        file_sort(&mut entries);
        // Dirs first, then alphabetically case-insensitive
        assert_eq!(entries[0].0, "Adir");
        assert_eq!(entries[1].0, "zdir");
        assert_eq!(entries[2].0, "alpha.rs");
        assert_eq!(entries[3].0, "Beta.rs");
        assert_eq!(entries[4].0, "Zebra.rs");
    }

    // ── FileGlobFilter / FileStatistics / TreeNode extension tests ──

    #[test]
    fn glob_filter_matches_wildcard() {
        let filter = FileGlobFilter::new(&["*.rs", "*.toml"]);
        assert!(filter.matches("main.rs"));
        assert!(filter.matches("Cargo.toml"));
        assert!(!filter.matches("readme.md"));
    }

    #[test]
    fn glob_filter_question_mark() {
        let filter = FileGlobFilter::new(&["file?.txt"]);
        assert!(filter.matches("file1.txt"));
        assert!(filter.matches("fileA.txt"));
        assert!(!filter.matches("file12.txt"));
    }

    #[test]
    fn glob_filter_exclude() {
        let filter = FileGlobFilter::new(&["*.lock"]);
        let names = vec!["Cargo.toml", "Cargo.lock", "main.rs"];
        let excluded = filter.exclude(&names);
        assert_eq!(excluded, vec!["Cargo.toml", "main.rs"]);
    }

    #[test]
    fn file_statistics_tracks_extensions() {
        let mut stats = FileStatistics::new();
        stats.record_file("main.rs", 1000);
        stats.record_file("lib.rs", 2000);
        stats.record_file("readme.md", 500);
        stats.record_directory();
        assert_eq!(stats.count_for_ext("rs"), 2);
        assert_eq!(stats.count_for_ext("md"), 1);
        assert_eq!(stats.total_files(), 3);
        assert_eq!(stats.total_dirs(), 1);
        assert_eq!(stats.total_size(), 3500);
        assert!((stats.average_file_size() - 1166.666).abs() < 1.0);
    }

    #[test]
    fn file_statistics_merge() {
        let mut a = FileStatistics::new();
        a.record_file("a.rs", 100);
        let mut b = FileStatistics::new();
        b.record_file("b.rs", 200);
        b.record_file("c.py", 300);
        a.merge(&b);
        assert_eq!(a.total_files(), 3);
        assert_eq!(a.total_size(), 600);
        assert_eq!(a.count_for_ext("rs"), 2);
    }

    #[test]
    fn file_tree_node_flatten() {
        let tree = FileTreeBuilder::build(&["src/main.rs", "src/lib.rs", "Cargo.toml"]);
        let flat = tree.flatten("");
        assert!(flat.iter().any(|(p, d)| p == "src" && *d));
        assert!(flat.iter().any(|(p, d)| p == "src/main.rs" && !*d));
    }

    #[test]
    fn file_tree_node_max_depth() {
        let tree = FileTreeBuilder::build(&["a/b/c.txt"]);
        assert!(tree.max_depth() >= 2);
    }

    #[test]
    fn file_tree_node_add_remove_child() {
        let mut root = FileTreeNode::new_dir("root");
        assert!(root.add_child(FileTreeNode::new_file("a.txt")));
        assert!(!root.add_child(FileTreeNode::new_file("a.txt")));
        assert!(root.find("a.txt").is_some());
        let removed = root.remove_child("a.txt");
        assert!(removed.is_some());
        assert!(root.find("a.txt").is_none());
    }

    #[test]
    fn file_unique_extensions_deduplicates() {
        let paths = vec!["a.rs", "b.rs", "c.txt", "d.txt", "e.py"];
        let exts = file_unique_extensions(&paths);
        assert_eq!(exts, vec!["py", "rs", "txt"]);
    }

    #[test]
    fn file_unique_extensions_empty() {
        let exts = file_unique_extensions(&[]);
        assert!(exts.is_empty());
    }

    #[test]
    fn file_classify_paths_separates() {
        let paths = vec!["/src/main.rs", "/src", "/lib.rs", "/docs"];
        let (files, dirs) = file_classify_paths(&paths);
        assert_eq!(files.len(), 2);
        assert_eq!(dirs.len(), 2);
        assert!(files.contains(&"/src/main.rs".to_string()));
        assert!(dirs.contains(&"/src".to_string()));
    }

    #[test]
    fn file_count_by_parent_groups() {
        let paths = vec!["/src/a.rs", "/src/b.rs", "/tests/t.rs"];
        let counts = file_count_by_parent(&paths);
        assert_eq!(counts.get("/src"), Some(&2));
        assert_eq!(counts.get("/tests"), Some(&1));
    }

    #[test]
    fn file_common_prefix_finds_shared() {
        let paths = vec!["/home/user/project/src/a.rs", "/home/user/project/src/b.rs"];
        let prefix = file_common_prefix(&paths);
        assert_eq!(prefix, "/home/user/project/src/");
    }

    #[test]
    fn file_common_prefix_empty_input() {
        let prefix = file_common_prefix(&[]);
        assert_eq!(prefix, "");
    }

    #[test]
    fn file_common_prefix_no_common() {
        let paths = vec!["abc", "xyz"];
        let prefix = file_common_prefix(&paths);
        assert_eq!(prefix, "");
    }

    #[test]
    fn file_has_extension_matches_case_insensitive() {
        assert!(file_has_extension("main.RS", &["rs", "py"]));
        assert!(file_has_extension("lib.py", &["rs", "py"]));
        assert!(!file_has_extension("readme.md", &["rs", "py"]));
    }

    #[test]
    fn file_has_extension_no_ext() {
        assert!(!file_has_extension("Makefile", &["rs"]));
    }

    // -----------------------------------------------------------------------
    // New tests for added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn file_event_path_accessor() {
        let c = FileEvent::Created("/src/main.rs".into());
        let ch = FileEvent::Changed("/src/lib.rs".into());
        let d = FileEvent::Deleted("/old.rs".into());
        assert_eq!(c.path(), "/src/main.rs");
        assert_eq!(ch.path(), "/src/lib.rs");
        assert_eq!(d.path(), "/old.rs");
        assert!(c.is_created());
        assert!(!c.is_changed());
        assert!(!c.is_deleted());
        assert!(ch.is_changed());
        assert!(d.is_deleted());
    }

    #[test]
    fn file_watcher_config_builder() {
        let cfg = FileWatcherConfig::new("*.rs")
            .with_recursive(true)
            .with_exclude("*.test.rs");
        assert_eq!(cfg.glob_pattern, "*.rs");
        assert!(cfg.recursive);
        assert_eq!(cfg.exclude_patterns, vec!["*.test.rs"]);
        assert!(cfg.accepts("main.rs"));
        assert!(!cfg.accepts("foo.test.rs"));
        assert!(!cfg.accepts("readme.md"));
    }

    #[test]
    fn file_stat_human_size_and_modified() {
        let stat = FileStat {
            file_type: FileType::File,
            size: 2048,
            modified: 200,
            created: 100,
            readonly: false,
        };
        assert_eq!(stat.human_size(), "2.0 KB");
        assert!(stat.was_modified_after_creation());

        let unmodified = FileStat { modified: 100, ..stat };
        assert!(!unmodified.was_modified_after_creation());
    }

    #[test]
    fn file_service_filter_and_remove_events() {
        let mut svc = FileService::new();
        svc.record_event(FileEvent::Created("a.rs".into()));
        svc.record_event(FileEvent::Changed("b.py".into()));
        svc.record_event(FileEvent::Deleted("c.rs".into()));

        let rs_events = svc.filter_events(|e| e.path().ends_with(".rs"));
        assert_eq!(rs_events.len(), 2);

        svc.remove_events_matching("*.py");
        assert_eq!(svc.event_count(), 2);
        assert!(svc.get_events().iter().all(|e| e.path().ends_with(".rs")));
    }

    #[test]
    fn file_service_replay_and_last_event() {
        let mut svc = FileService::new();
        assert!(svc.last_event().is_none());

        svc.record_event(FileEvent::Created("x.rs".into()));
        svc.record_event(FileEvent::Changed("y.rs".into()));

        let mut replayed = Vec::new();
        svc.replay_events(|e| replayed.push(e.path().to_string()));
        assert_eq!(replayed, vec!["x.rs", "y.rs"]);

        assert_eq!(svc.last_event().unwrap().path(), "y.rs");
    }

    #[test]
    fn file_service_matching_watchers() {
        let mut svc = FileService::new();
        svc.add_watcher(FileWatcherConfig::new("*.rs"));
        svc.add_watcher(FileWatcherConfig::new("*.toml"));
        svc.add_watcher(FileWatcherConfig::new("src/*"));

        let matches = svc.matching_watchers("main.rs");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].glob_pattern, "*.rs");

        assert!(svc.matching_watchers("readme.md").is_empty());
    }

    #[test]
    fn file_path_utils_stem() {
        assert_eq!(FilePathUtils::stem("main.rs"), Some("main"));
        assert_eq!(FilePathUtils::stem("archive.tar.gz"), Some("archive.tar"));
        assert_eq!(FilePathUtils::stem("noext"), Some("noext"));
        assert_eq!(FilePathUtils::stem(".hidden"), Some(".hidden"));
        assert_eq!(FilePathUtils::stem("/path/to/file.txt"), Some("file"));
    }

    #[test]
    fn file_path_utils_is_descendant() {
        assert!(FilePathUtils::is_descendant("/src", "/src/main.rs"));
        assert!(FilePathUtils::is_descendant("/src", "/src/sub/deep.rs"));
        assert!(!FilePathUtils::is_descendant("/src", "/other/main.rs"));
        assert!(!FilePathUtils::is_descendant("/src", "/src"));
        assert!(FilePathUtils::is_descendant("", "/anything"));
    }

    #[test]
    fn file_path_utils_relative() {
        assert_eq!(
            FilePathUtils::relative("/home/user", "/home/user/project/a.rs"),
            Some("project/a.rs".to_string())
        );
        assert_eq!(
            FilePathUtils::relative("/home/user", "/home/user"),
            Some(String::new())
        );
        assert_eq!(
            FilePathUtils::relative("/home/user", "/other/path"),
            None
        );
    }

    #[test]
    fn file_path_utils_segments() {
        assert_eq!(FilePathUtils::segments("/usr/local/bin"), vec!["usr", "local", "bin"]);
        assert_eq!(FilePathUtils::segments("a/b"), vec!["a", "b"]);
        assert!(FilePathUtils::segments("/").is_empty());
    }

    #[test]
    fn file_path_utils_is_hidden() {
        assert!(FilePathUtils::is_hidden(".gitignore"));
        assert!(FilePathUtils::is_hidden("/home/.config"));
        assert!(!FilePathUtils::is_hidden("visible.txt"));
        assert!(!FilePathUtils::is_hidden("/path/to/normal"));
    }


    #[test]
    fn tree_diff_basic() {
        let old = vec!["a.rs".into(), "b.rs".into()];
        let new = vec!["b.rs".into(), "c.rs".into()];
        let changes = FileSystemTreeDiff::diff(&old, &new);
        assert!(changes.iter().any(|c| matches!(c, FileTreeChange::Removed(p) if p == "a.rs")));
        assert!(changes.iter().any(|c| matches!(c, FileTreeChange::Added(p) if p == "c.rs")));
    }

    #[test]
    fn tree_diff_renames() {
        let old = vec!["/old/file.rs".into()];
        let new = vec!["/new/file.rs".into()];
        let mut changes = FileSystemTreeDiff::diff(&old, &new);
        FileSystemTreeDiff::detect_renames(&mut changes);
        assert!(changes.iter().any(|c| matches!(c, FileTreeChange::Renamed { .. })));
    }

    #[test]
    fn tree_change_display() {
        assert_eq!(format!("{}", FileTreeChange::Added("foo.rs".into())), "+ foo.rs");
    }

    #[test]
    fn icon_resolver_defaults() {
        let r = FileIconResolver::new();
        assert_eq!(r.resolve("main.rs"), "rust");
        assert_eq!(r.resolve("unknown"), "file");
    }

    #[test]
    fn icon_resolver_custom() {
        let mut r = FileIconResolver::new();
        r.register("vue", "vue");
        assert_eq!(r.resolve("app.vue"), "vue");
    }

    #[test]
    fn exclude_pattern_basic() {
        let mut e = FileExcludePattern::new();
        e.add_pattern("*.tmp");
        e.add_pattern("node_modules");
        assert!(e.is_excluded("foo.tmp"));
        assert!(!e.is_excluded("foo.rs"));
    }

    #[test]
    fn exclude_pattern_remove() {
        let mut e = FileExcludePattern::new();
        e.add_pattern("*.log");
        assert!(e.remove_pattern("*.log"));
        assert!(e.is_empty());
    }

    #[test]
    fn readonly_basic() {
        let mut ri = FileReadonlyIndicator::new();
        ri.mark_readonly("/etc/config");
        assert!(ri.is_readonly("/etc/config"));
        assert!(!ri.is_readonly("/tmp/data"));
    }

    #[test]
    fn readonly_unmark() {
        let mut ri = FileReadonlyIndicator::new();
        ri.mark_readonly("a.txt");
        assert!(ri.unmark_readonly("a.txt"));
        assert!(!ri.is_readonly("a.txt"));
    }

    #[test]
    fn readonly_no_duplicate() {
        let mut ri = FileReadonlyIndicator::new();
        ri.mark_readonly("a.txt");
        ri.mark_readonly("a.txt");
        assert_eq!(ri.readonly_count(), 1);
    }

    #[test]
    fn icon_resolver_len() {
        let r = FileIconResolver::new();
        assert!(r.len() > 10);
    }

    #[test]
    fn exclude_pattern_clear() {
        let mut e = FileExcludePattern::new();
        e.add_pattern("*.tmp");
        e.clear();
        assert!(e.is_empty());
    }


    #[test]
    fn fileIconThemeLoader_new() {
        let s = FileIconThemeLoader::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn fileIconThemeLoader_add_contains() {
        let mut s = FileIconThemeLoader::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn fileIconThemeLoader_add_duplicate() {
        let mut s = FileIconThemeLoader::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn fileIconThemeLoader_remove() {
        let mut s = FileIconThemeLoader::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn fileIconThemeLoader_capacity() {
        let s = FileIconThemeLoader::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn fileIconThemeLoader_search() {
        let mut s = FileIconThemeLoader::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn fileIconThemeLoader_stats() {
        let mut s = FileIconThemeLoader::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn fileSystemEventAggregator_new() {
        let m = FileSystemEventAggregator::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn fileSystemEventAggregator_add_find() {
        let mut m = FileSystemEventAggregator::new();
        m.add(FileSystemEventAggregatorItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn fileSystemEventAggregator_priority_filter() {
        let mut m = FileSystemEventAggregator::new();
        m.add(FileSystemEventAggregatorItem::new("a", "A").with_priority(FileSystemEventAggregatorPriority::High));
        m.add(FileSystemEventAggregatorItem::new("b", "B").with_priority(FileSystemEventAggregatorPriority::Low));
        m.add(FileSystemEventAggregatorItem::new("c", "C").with_priority(FileSystemEventAggregatorPriority::High));
        assert_eq!(m.by_priority(FileSystemEventAggregatorPriority::High).len(), 2);
    }

    #[test]
    fn fileSystemEventAggregator_remove() {
        let mut m = FileSystemEventAggregator::new();
        m.add(FileSystemEventAggregatorItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn fileSystemEventAggregator_search() {
        let mut m = FileSystemEventAggregator::new();
        m.add(FileSystemEventAggregatorItem::new("id1", "Hello World"));
        m.add(FileSystemEventAggregatorItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn fileSystemEventAggregator_total_weight() {
        let mut m = FileSystemEventAggregator::new();
        m.add(FileSystemEventAggregatorItem::new("a", "A").with_priority(FileSystemEventAggregatorPriority::Critical));
        m.add(FileSystemEventAggregatorItem::new("b", "B").with_priority(FileSystemEventAggregatorPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn fileSystemEventAggregator_capacity_limit() {
        let mut m = FileSystemEventAggregator::new().with_max_items(2);
        m.add(FileSystemEventAggregatorItem::new("1", "one"));
        m.add(FileSystemEventAggregatorItem::new("2", "two"));
        assert!(!m.add(FileSystemEventAggregatorItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn fileSystemEventAggregator_sorted_by_priority() {
        let mut m = FileSystemEventAggregator::new();
        m.add(FileSystemEventAggregatorItem::new("lo", "Low").with_priority(FileSystemEventAggregatorPriority::Low));
        m.add(FileSystemEventAggregatorItem::new("hi", "High").with_priority(FileSystemEventAggregatorPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn fileSystemEventAggregator_item_metadata() {
        let mut item = FileSystemEventAggregatorItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn fileIconThemeLoader_enabled_toggle() {
        let mut s = FileIconThemeLoader::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn fileSystemEventAggregator_priority_display() {
        assert_eq!(format!("{}", FileSystemEventAggregatorPriority::High), "high");
        assert_eq!(format!("{}", FileSystemEventAggregatorPriority::Low), "low");
    }


    #[test]
    fn wbfile_lru_insert_get() {
        let mut c = WbFileLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn wbfile_lru_eviction() {
        let mut c = WbFileLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn wbfile_lru_hit_ratio() {
        let mut c = WbFileLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn wbfile_lru_clear() {
        let mut c = WbFileLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn wbfile_lru_remove() {
        let mut c = WbFileLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn wbfile_lru_peek() {
        let mut c = WbFileLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn wbfile_builder_valid() {
        let cfg = WbFileBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn wbfile_builder_empty_name() {
        let r = WbFileBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn wbfile_builder_bad_priority() {
        assert!(WbFileBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn wbfile_builder_zero_max() {
        assert!(WbFileBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn wbfile_cfg_merge() {
        let mut a = WbFileBuilder::new("a").property("x", "1").build().unwrap();
        let b = WbFileBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn wbfile_cfg_display() {
        let cfg = WbFileBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- wb_files extended domain tests ----------------------------------------

    #[test]
    fn y_wb_files_enum_index() {
        assert_eq!(YWbFilesFileWatchEvent::Created.index(), 0);
        assert_eq!(YWbFilesFileWatchEvent::Modified.index(), 1);
        assert_eq!(YWbFilesFileWatchEvent::Deleted.index(), 2);
        assert_eq!(YWbFilesFileWatchEvent::Renamed.index(), 3);
    }

    #[test]
    fn y_wb_files_enum_label() {
        assert_eq!(YWbFilesFileWatchEvent::Created.label(), "Created");
        assert_eq!(YWbFilesFileWatchEvent::Modified.label(), "Modified");
        assert_eq!(YWbFilesFileWatchEvent::Deleted.label(), "Deleted");
        assert_eq!(YWbFilesFileWatchEvent::Renamed.label(), "Renamed");
    }

    #[test]
    fn y_wb_files_enum_all() {
        let all = YWbFilesFileWatchEvent::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_files_enum_is_default() {
        assert!(YWbFilesFileWatchEvent::Created.is_default());
        assert!(!YWbFilesFileWatchEvent::Renamed.is_default());
    }

    #[test]
    fn y_wb_files_enum_display() {
        assert_eq!(format!("{}", YWbFilesFileWatchEvent::Created), "Created");
    }

    #[test]
    fn y_wb_files_struct_new() {
        let s = YWbFilesFileWatcherStats::new();
        let _ = s.summary();
    }

    #[test]
    fn y_wb_files_fingerprint_deterministic() {
        let h1 = y_wb_files_fingerprint("hello");
        let h2 = y_wb_files_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_files_fingerprint("a"), y_wb_files_fingerprint("b"));
    }

    #[test]
    fn y_wb_files_truncate_short() {
        assert_eq!(y_wb_files_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_files_truncate_long() {
        let r = y_wb_files_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_files_normalize_key_basic() {
        assert_eq!(y_wb_files_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_files_split_path_basic() {
        let parts = y_wb_files_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_files_count_occurrences_basic() {
        assert_eq!(y_wb_files_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_files_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_files_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_files_in_range_basic() {
        assert!(y_wb_files_in_range(5, 1, 10));
        assert!(y_wb_files_in_range(1, 1, 10));
        assert!(y_wb_files_in_range(10, 1, 10));
        assert!(!y_wb_files_in_range(0, 1, 10));
        assert!(!y_wb_files_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_files_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_files_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_files_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_files_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_files Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_files_priority_weight() {
        assert_eq!(ZWbFilesPriority::Idle.weight(), 0);
        assert_eq!(ZWbFilesPriority::Normal.weight(), 2);
        assert_eq!(ZWbFilesPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_files_priority_label() {
        assert_eq!(ZWbFilesPriority::Low.label(), "low");
        assert_eq!(ZWbFilesPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_files_priority_is_elevated() {
        assert!(!ZWbFilesPriority::Normal.is_elevated());
        assert!(ZWbFilesPriority::High.is_elevated());
        assert!(ZWbFilesPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_files_priority_display() {
        assert_eq!(format!("{}", ZWbFilesPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_files_priority_all_asc() {
        let all = ZWbFilesPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbFilesPriority::Idle);
        assert_eq!(all[4], ZWbFilesPriority::Realtime);
    }

    #[test]
    fn z_wb_files_struct_new() {
        let s = ZWbFilesFileIndexEntry::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_files_struct_toggled_clone() {
        let s = ZWbFilesFileIndexEntry::new();
        let t = s.toggled_clone();
        let _ = t.indexed_at_ms;
    }

    #[test]
    fn z_wb_files_rolling_hash_deterministic() {
        let h1 = z_wb_files_rolling_hash(b"test");
        let h2 = z_wb_files_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_files_rolling_hash(b"a"), z_wb_files_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_files_pad_to_basic() {
        assert_eq!(z_wb_files_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_files_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_files_is_identifier_basic() {
        assert!(z_wb_files_is_identifier("foo_bar"));
        assert!(z_wb_files_is_identifier("abc123"));
        assert!(!z_wb_files_is_identifier(""));
        assert!(!z_wb_files_is_identifier("has space"));
    }

    #[test]
    fn z_wb_files_levenshtein_basic() {
        assert_eq!(z_wb_files_levenshtein("", ""), 0);
        assert_eq!(z_wb_files_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_files_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_files_unique_words_basic() {
        let w = z_wb_files_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_files_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_files_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_files_common_prefix_basic() {
        assert_eq!(z_wb_files_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_files_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_files_struct_clear() {
        let mut s = ZWbFilesFileIndexEntry::new();
        s.paths.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_files_rolling_hash_empty() {
        let h = z_wb_files_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_104_push_and_len() {
        let mut rb = super::XbRingBuffer104::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_104_overwrite() {
        let mut rb = super::XbRingBuffer104::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_104_get_out_of_bounds() {
        let rb = super::XbRingBuffer104::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_104_drain_all() {
        let mut rb = super::XbRingBuffer104::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_104_peek_front_back() {
        let mut rb = super::XbRingBuffer104::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_104_clear() {
        let mut rb = super::XbRingBuffer104::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_104_capacity() {
        let rb = super::XbRingBuffer104::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_104_basic() {
        let h = super::xb_fnv1a_104(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_104(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_104_different_inputs() {
        let h1 = super::xb_fnv1a_104(b"abc");
        let h2 = super::xb_fnv1a_104(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_104_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_104(&data);
        let dec = super::xb_rle_decode_104(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_104_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_104(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_104(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_104_values() {
        assert!((super::xb_clamp_104(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_104(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_104(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_104_values() {
        assert!((super::xb_lerp_104(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_104(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_104(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_104_wrap_around_twice() {
        let mut rb = super::XbRingBuffer104::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 208 ----

    #[test]
    fn xc_208_pool_new_empty() {
        let pool: super::Xc208Pool<i32> = super::Xc208Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_208_pool_release_acquire() {
        let mut pool = super::Xc208Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_208_pool_acquire_empty() {
        let mut pool: super::Xc208Pool<i32> = super::Xc208Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_208_pool_full() {
        let mut pool = super::Xc208Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_208_pool_drain() {
        let mut pool = super::Xc208Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_208_pool_stats() {
        let mut pool = super::Xc208Pool::new(8);
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
    fn xc_208_pool_clear() {
        let mut pool = super::Xc208Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_208_pool_shrink() {
        let mut pool = super::Xc208Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_208_pool_default() {
        let pool: super::Xc208Pool<String> = super::Xc208Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_208_pool_extend() {
        let mut pool = super::Xc208Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_208_pool_retain() {
        let mut pool = super::Xc208Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_208_scheduler_round_robin() {
        let mut sched = super::Xc208Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_208_scheduler_empty() {
        let mut sched = super::Xc208Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_208_scheduler_reset() {
        let mut sched = super::Xc208Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_208_scheduler_add_remove() {
        let mut sched = super::Xc208Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_208_scheduler_targets() {
        let sched = super::Xc208Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_208_hash_empty() {
        assert_eq!(super::xc_208_hash(b""), 5381);
    }

    #[test]
    fn xc_208_hash_data() {
        let h = super::xc_208_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_208_hash(b"hello"), h);
    }

    #[test]
    fn xc_208_reverse_str() {
        assert_eq!(super::xc_208_reverse("abc"), "cba");
        assert_eq!(super::xc_208_reverse(""), "");
    }


    #[test]
    fn xe_117_pipeline_empty() {
        let p = super::Xe117Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_117_pipeline_parse_stage() {
        let p = super::Xe117Pipeline::new()
            .add_parse(super::xe_117_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_117_pipeline_transform_double() {
        let p = super::Xe117Pipeline::new()
            .add_transform(super::xe_117_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_117_pipeline_validate_reverse() {
        let p = super::Xe117Pipeline::new()
            .add_validate(super::xe_117_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_117_pipeline_emit_filter() {
        let p = super::Xe117Pipeline::new()
            .add_emit(super::xe_117_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_117_pipeline_multi_stage() {
        let p = super::Xe117Pipeline::new()
            .add_parse(super::xe_117_pipeline_identity)
            .add_transform(super::xe_117_pipeline_double)
            .add_validate(super::xe_117_pipeline_reverse)
            .add_emit(super::xe_117_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_117_pipeline_error_propagation() {
        let p = super::Xe117Pipeline::new()
            .add_parse(super::xe_117_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe117Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_117_pipeline_compose() {
        let p1 = super::Xe117Pipeline::new()
            .add_parse(super::xe_117_pipeline_identity);
        let p2 = super::Xe117Pipeline::new()
            .add_transform(super::xe_117_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_117_pipeline_error_display() {
        let e = super::Xe117PipelineError {
            stage: super::Xe117Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_117_cache_put_get() {
        let mut c = super::Xe117Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_117_cache_miss() {
        let mut c: super::Xe117Cache<&str, i32> = super::Xe117Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_117_cache_ttl_expiry() {
        let mut c = super::Xe117Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_117_cache_evict() {
        let mut c = super::Xe117Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_117_cache_capacity() {
        let mut c = super::Xe117Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_117_cache_stats() {
        let mut c = super::Xe117Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_117_cache_clear() {
        let mut c = super::Xe117Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}