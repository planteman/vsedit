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


// ---------------------------------------------------------------------------
// xg_115: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg115Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg115Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg115Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_115: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg115Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg115Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg115Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg115Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 207).
pub struct Xh207SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh207SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 249 as u64,
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

/// A compact bit set supporting boolean operations (variant 207).
pub struct Xh207BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh207BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 207).
pub struct Xi207Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi207Deque<T> {
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
pub struct Xi207Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi207Interval {
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

/// A simple interval tree (variant 207).
pub struct Xi207IntervalTree {
    xi_intervals: Vec<Xi207Interval>,
}

impl Xi207IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi207Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi207Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi207Interval) -> Vec<&Xi207Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi207Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi207Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi207Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi207Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi207Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi207Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 207) ---

/// Disjoint set / union-find for crate 207.
pub struct Xj207UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj207UnionFind {
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

const XJ207_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 207.
pub struct Xj207BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj207BTreeNode<K, V>>>,
    len: usize,
}

struct Xj207BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj207BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj207BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ207_BTREE_ORDER - 1
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
        let mid = XJ207_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj207BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj207BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj207BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj207BTreeNode::xj_new_leaf();
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


// --- xk_207 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk207SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk207SegmentTree {
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
pub struct Xk207DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk207DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_207).
#[derive(Debug, Clone)]
pub struct Xl207Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl207Rope {
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

/// Suffix array for efficient string searching (xl_207).
#[derive(Debug, Clone)]
pub struct Xl207SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl207SuffixArray {
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
pub struct Xm207MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm207MatrixSparse {
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
pub struct Xm207Tokenizer {
    text: String,
}

impl Xm207Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 207.
pub struct Xn207Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn207Fenwick {
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

// ----- AVL tree map — crate 207 -----

#[derive(Debug, Clone)]
struct Xn207AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn207AvlNode<K, V>>>,
    right: Option<Box<Xn207AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 207.
#[derive(Debug, Clone)]
pub struct Xn207AVL<K, V> {
    root: Option<Box<Xn207AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn207AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn207AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn207AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn207AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn207AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn207AvlNode<K, V>>) -> Box<Xn207AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn207AvlNode<K, V>>) -> Box<Xn207AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn207AvlNode<K, V>>) -> Box<Xn207AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn207AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn207AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn207AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn207AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn207AvlNode<K, V>>) -> &Xn207AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn207AvlNode<K, V>>) -> (Box<Xn207AvlNode<K, V>>, Option<Box<Xn207AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn207AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn207AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn207AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn207AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn207AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn207AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn207AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo207RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo207Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo207RBNode<K, V> {
    key: K,
    value: V,
    color: Xo207Color,
    left: Option<Box<Xo207RBNode<K, V>>>,
    right: Option<Box<Xo207RBNode<K, V>>>,
}

/// A red-black tree map for crate 207.
#[derive(Debug, Clone)]
pub struct Xo207RedBlack<K, V> {
    root: Option<Box<Xo207RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo207RedBlack<K, V> {
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
            r.color = Xo207Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo207RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo207RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo207RBNode {
                    key, value, color: Xo207Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo207RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo207Color::Red)
    }

    fn xo_balance(mut h: Box<Xo207RBNode<K, V>>) -> Box<Xo207RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo207Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo207RBNode<K, V>>) -> Box<Xo207RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo207Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo207RBNode<K, V>>) -> Box<Xo207RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo207Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo207RBNode<K, V>>) {
        h.color = Xo207Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo207Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo207Color::Black; }
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
            r.color = Xo207Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo207RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo207RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo207RBNode<K, V>) -> (K, V, Option<Box<Xo207RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo207RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo207Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo207RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo207ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 207.
#[derive(Debug, Clone)]
pub struct Xo207ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo207ConsistentHash {
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
            let vkey = format!("{}#xo207#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo207#{}", node, i);
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


    // -- xg_115 graph tests ------------------------------------------------

    #[test]
    fn xg_115_graph_empty() {
        let g = super::Xg115Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_115_graph_add_node() {
        let mut g = super::Xg115Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_115_graph_add_edge() {
        let mut g = super::Xg115Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_115_graph_neighbors() {
        let mut g = super::Xg115Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_115_graph_has_path() {
        let mut g = super::Xg115Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_115_graph_self_path() {
        let g = super::Xg115Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_115_graph_topo_sort() {
        let mut g = super::Xg115Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_115_graph_cycle_detect_false() {
        let mut g = super::Xg115Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_115_graph_cycle_detect_true() {
        let mut g = super::Xg115Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_115 heap tests -------------------------------------------------

    #[test]
    fn xg_115_heap_empty() {
        let h: super::Xg115Heap<i32> = super::Xg115Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_115_heap_push_pop() {
        let mut h = super::Xg115Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_115_heap_peek() {
        let mut h = super::Xg115Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_115_heap_drain_sorted() {
        let mut h = super::Xg115Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_115_heap_merge() {
        let mut a = super::Xg115Heap::new();
        let mut b = super::Xg115Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_115_heap_default() {
        let h: super::Xg115Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_115_graph_default() {
        let g: super::Xg115Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh207_skip_insert_contains() {
        let mut sl = super::Xh207SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh207_skip_remove() {
        let mut sl = super::Xh207SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh207_skip_len() {
        let mut sl = super::Xh207SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh207_skip_range_query() {
        let mut sl = super::Xh207SkipList::xh_new(4);
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
    fn xh207_skip_floor_ceiling() {
        let mut sl = super::Xh207SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh207_skip_rank() {
        let mut sl = super::Xh207SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh207_skip_empty() {
        let sl = super::Xh207SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh207_skip_duplicates() {
        let mut sl = super::Xh207SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh207_bitset_set_test() {
        let mut bs = super::Xh207BitSet::xh_new(256);
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
    fn xh207_bitset_clear_count() {
        let mut bs = super::Xh207BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh207_bitset_and_or_xor() {
        let mut a = super::Xh207BitSet::xh_new(128);
        let mut b = super::Xh207BitSet::xh_new(128);
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
    fn xh207_bitset_iter_ones() {
        let mut bs = super::Xh207BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh207_bitset_first_last() {
        let mut bs = super::Xh207BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh207_bitset_empty() {
        let bs = super::Xh207BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi207_deque_push_pop_back() {
        let mut dq = super::Xi207Deque::xi_new(4);
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
    fn xi207_deque_push_pop_front() {
        let mut dq = super::Xi207Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi207_deque_mixed_ops() {
        let mut dq = super::Xi207Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi207_deque_get_and_split() {
        let mut dq = super::Xi207Deque::xi_new(8);
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
    fn xi207_deque_rotate_left() {
        let mut dq = super::Xi207Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi207_deque_rotate_right() {
        let mut dq = super::Xi207Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi207_deque_grow() {
        let mut dq = super::Xi207Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi207_deque_empty() {
        let dq = super::Xi207Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi207_interval_tree_insert_query() {
        let mut tree = super::Xi207IntervalTree::xi_new();
        tree.xi_insert(super::Xi207Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi207Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi207Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi207_interval_tree_overlap() {
        let mut tree = super::Xi207IntervalTree::xi_new();
        tree.xi_insert(super::Xi207Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi207Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi207Interval::xi_new(12, 20));
        let q = super::Xi207Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi207_interval_tree_remove() {
        let mut tree = super::Xi207IntervalTree::xi_new();
        tree.xi_insert(super::Xi207Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi207Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi207_interval_tree_gaps() {
        let mut tree = super::Xi207IntervalTree::xi_new();
        tree.xi_insert(super::Xi207Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi207Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi207Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi207Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi207Interval::xi_new(8, 10));
    }

    #[test]
    fn xi207_interval_tree_merge() {
        let mut tree = super::Xi207IntervalTree::xi_new();
        tree.xi_insert(super::Xi207Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi207Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi207Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi207Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi207Interval::xi_new(10, 15));
    }

    #[test]
    fn xi207_interval_tree_all() {
        let mut tree = super::Xi207IntervalTree::xi_new();
        tree.xi_insert(super::Xi207Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi207Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi207_interval_tree_empty() {
        let tree = super::Xi207IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi207_interval_tree_contains_point() {
        let iv = super::Xi207Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 207) ---

    #[test]
    fn xj_207_uf_make_and_find() {
        let mut uf = super::Xj207UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_207_uf_union_connected() {
        let mut uf = super::Xj207UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_207_uf_component_count() {
        let mut uf = super::Xj207UnionFind::xj_new();
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
    fn xj_207_uf_component_size() {
        let mut uf = super::Xj207UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_207_uf_largest_component() {
        let mut uf = super::Xj207UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_207_uf_many_elements() {
        let mut uf = super::Xj207UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_207_uf_separate_components() {
        let mut uf = super::Xj207UnionFind::xj_new();
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
    fn xj_207_uf_path_compression() {
        let mut uf = super::Xj207UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_207_bt_insert_get() {
        let mut bt = super::Xj207BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_207_bt_contains_len() {
        let mut bt = super::Xj207BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_207_bt_replace() {
        let mut bt = super::Xj207BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_207_bt_remove() {
        let mut bt = super::Xj207BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_207_bt_keys_values() {
        let mut bt = super::Xj207BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_207_bt_range() {
        let mut bt = super::Xj207BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_207_bt_min_max() {
        let mut bt = super::Xj207BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_207_bt_many_inserts() {
        let mut bt = super::Xj207BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_207 segment tree tests ---

    #[test]
    fn xk_207_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk207SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_207_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk207SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_207_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk207SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_207_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk207SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_207_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk207SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_207_st_single_element() {
        let data = vec![42];
        let st = super::Xk207SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_207_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk207SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_207_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk207SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_207 disjoint intervals tests ---

    #[test]
    fn xk_207_di_add_and_count() {
        let mut di = super::Xk207DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_207_di_merge_overlap() {
        let mut di = super::Xk207DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_207_di_contains() {
        let mut di = super::Xk207DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_207_di_remove() {
        let mut di = super::Xk207DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_207_di_covered_length() {
        let mut di = super::Xk207DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_207_di_gaps() {
        let mut di = super::Xk207DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_207_di_merge_adjacent() {
        let mut di = super::Xk207DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_207_di_empty() {
        let di = super::Xk207DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_207_rope_new_empty() {
        let rope = super::Xl207Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_207_rope_from_str() {
        let rope = super::Xl207Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_207_rope_insert_at() {
        let mut rope = super::Xl207Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_207_rope_delete_range() {
        let mut rope = super::Xl207Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_207_rope_char_at() {
        let rope = super::Xl207Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_207_rope_split_concat() {
        let rope = super::Xl207Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_207_rope_line_count() {
        let rope = super::Xl207Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_207_rope_line_at() {
        let rope = super::Xl207Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_207_sa_build_and_search() {
        let sa = super::Xl207SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_207_sa_count() {
        let sa = super::Xl207SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_207_sa_longest_repeated() {
        let sa = super::Xl207SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_207_sa_all_positions() {
        let sa = super::Xl207SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_207_sa_len() {
        let sa = super::Xl207SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_207_sa_empty() {
        let sa = super::Xl207SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_207_rope_slice() {
        let rope = super::Xl207Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_207_sa_search_start() {
        let sa = super::Xl207SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_207_sparse_set_get() {
        let mut m = super::Xm207MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_207_sparse_row_col() {
        let mut m = super::Xm207MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_207_sparse_transpose() {
        let mut m = super::Xm207MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_207_sparse_multiply_vec() {
        let mut m = super::Xm207MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_207_sparse_nnz_density() {
        let mut m = super::Xm207MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_207_sparse_clear() {
        let mut m = super::Xm207MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_207_sparse_overwrite_zero() {
        let mut m = super::Xm207MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_207_tokenizer_basic() {
        let t = super::Xm207Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_207_tokenizer_count() {
        let t = super::Xm207Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_207_tokenizer_unique() {
        let t = super::Xm207Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_207_tokenizer_frequency() {
        let t = super::Xm207Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_207_tokenizer_delimiter() {
        let t = super::Xm207Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_207_tokenizer_whitespace() {
        let t = super::Xm207Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_207_tokenizer_empty() {
        let t = super::Xm207Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 207 ----

    #[test]
    fn xn_207_fenwick_prefix_sum() {
        let mut ft = super::Xn207Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_207_fenwick_range_sum() {
        let mut ft = super::Xn207Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_207_fenwick_point_query() {
        let mut ft = super::Xn207Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_207_fenwick_len() {
        let ft = super::Xn207Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_207_fenwick_multiple_updates() {
        let mut ft = super::Xn207Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_207_fenwick_single_element() {
        let mut ft = super::Xn207Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_207_fenwick_find_kth() {
        let mut ft = super::Xn207Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_207_fenwick_negative_delta() {
        let mut ft = super::Xn207Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 207 ----

    #[test]
    fn xn_207_avl_insert_get() {
        let mut m = super::Xn207AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_207_avl_remove() {
        let mut m = super::Xn207AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_207_avl_in_order() {
        let mut m = super::Xn207AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_207_avl_min_max() {
        let mut m = super::Xn207AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_207_avl_floor_ceiling() {
        let mut m = super::Xn207AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_207_avl_height_balanced() {
        let mut m = super::Xn207AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_207_avl_overwrite() {
        let mut m = super::Xn207AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_207_avl_empty() {
        let m: super::Xn207AVL<i32, i32> = super::Xn207AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo207RedBlack tests ---

    #[test]
    fn xo_207_rb_insert_and_get() {
        let mut tree = super::Xo207RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_207_rb_len_and_empty() {
        let mut tree = super::Xo207RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_207_rb_min_max() {
        let mut tree = super::Xo207RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_207_rb_contains() {
        let mut tree = super::Xo207RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_207_rb_remove() {
        let mut tree = super::Xo207RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_207_rb_in_order() {
        let mut tree = super::Xo207RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_207_rb_black_height() {
        let mut tree = super::Xo207RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_207_rb_overwrite() {
        let mut tree = super::Xo207RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo207ConsistentHash tests ---

    #[test]
    fn xo_207_ch_add_and_count() {
        let mut ring = super::Xo207ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_207_ch_remove_node() {
        let mut ring = super::Xo207ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_207_ch_get_node() {
        let mut ring = super::Xo207ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_207_ch_empty_ring() {
        let ring = super::Xo207ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_207_ch_distribution() {
        let mut ring = super::Xo207ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_207_ch_rebalance() {
        let mut ring = super::Xo207ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_207_ch_virtual_nodes() {
        let mut ring = super::Xo207ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_207_ch_consistent_lookup() {
        let mut ring = super::Xo207ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}