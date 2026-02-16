//! Text file operations.

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
}
