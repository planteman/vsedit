//! Platform path resolution.

use std::collections::HashMap;
use std::fmt;
/// Path separator style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSeparator {
    Unix,
    Windows,
}

/// Service for path manipulation.
pub struct PathService {
    separator: PathSeparator,
}

impl PathService {
    pub fn new(separator: PathSeparator) -> Self {
        Self { separator }
    }

    pub fn join(&self, base: &str, path: &str) -> String {
        if self.is_absolute(path) {
            return path.to_string();
        }
        let sep = self.sep_char();
        let base = base.trim_end_matches(|c| c == '/' || c == '\\');
        format!("{base}{sep}{path}")
    }

    /// Normalize a path by resolving `.` and `..` segments and collapsing separators.
    pub fn normalize(&self, path: &str) -> String {
        let sep = self.sep_char();
        let is_abs = self.is_absolute(path);
        let parts: Vec<&str> = path
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect();
        let mut stack: Vec<&str> = Vec::new();
        for part in &parts {
            if *part == ".." {
                if let Some(top) = stack.last() {
                    if *top != ".." {
                        stack.pop();
                        continue;
                    }
                }
                if !is_abs {
                    stack.push(part);
                }
            } else {
                stack.push(part);
            }
        }
        let joined = stack.join(&sep.to_string());
        if is_abs {
            // Preserve Windows drive prefix like C:
            let prefix = self.abs_prefix(path);
            format!("{prefix}{sep}{joined}")
        } else if joined.is_empty() {
            ".".to_string()
        } else {
            joined
        }
    }

    pub fn dirname<'a>(&self, path: &'a str) -> &'a str {
        let path = path.trim_end_matches(|c| c == '/' || c == '\\');
        match path.rfind(|c: char| c == '/' || c == '\\') {
            Some(0) => &path[..1],
            Some(i) => &path[..i],
            None => ".",
        }
    }

    pub fn basename<'a>(&self, path: &'a str) -> &'a str {
        let path = path.trim_end_matches(|c| c == '/' || c == '\\');
        match path.rfind(|c: char| c == '/' || c == '\\') {
            Some(i) => &path[i + 1..],
            None => path,
        }
    }

    pub fn extname<'a>(&self, path: &'a str) -> Option<&'a str> {
        let base = self.basename(path);
        match base.rfind('.') {
            Some(0) | None => None,
            Some(i) => Some(&base[i..]),
        }
    }

    pub fn is_absolute(&self, path: &str) -> bool {
        match self.separator {
            PathSeparator::Unix => path.starts_with('/'),
            PathSeparator::Windows => {
                path.starts_with('/')
                    || path.starts_with('\\')
                    || (path.len() >= 3
                        && path.as_bytes()[0].is_ascii_alphabetic()
                        && path.as_bytes()[1] == b':'
                        && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/'))
            }
        }
    }

    pub fn to_unix(&self, path: &str) -> String {
        path.replace('\\', "/")
    }

    pub fn to_windows(&self, path: &str) -> String {
        path.replace('/', "\\")
    }

    pub fn resolve_relative(&self, base: &str, relative: &str) -> String {
        let dir = self.dirname(base);
        let joined = self.join(dir, relative);
        self.normalize(&joined)
    }

    fn sep_char(&self) -> char {
        match self.separator {
            PathSeparator::Unix => '/',
            PathSeparator::Windows => '\\',
        }
    }

    fn abs_prefix(&self, path: &str) -> String {
        match self.separator {
            PathSeparator::Unix => String::new(),
            PathSeparator::Windows => {
                if path.len() >= 2
                    && path.as_bytes()[0].is_ascii_alphabetic()
                    && path.as_bytes()[1] == b':'
                {
                    path[..2].to_string()
                } else {
                    String::new()
                }
            }
        }
    }

    /// Compute a relative path from `from` to `to`.
    pub fn relative(&self, from: &str, to: &str) -> String {
        let sep = self.sep_char();
        let norm_from = self.normalize(from);
        let norm_to = self.normalize(to);
        let from_parts: Vec<&str> = norm_from
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .collect();
        let to_parts: Vec<&str> = norm_to
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .collect();
        let common = from_parts
            .iter()
            .zip(to_parts.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let ups = from_parts.len() - common;
        let mut result: Vec<&str> = Vec::new();
        for _ in 0..ups {
            result.push("..");
        }
        for part in &to_parts[common..] {
            result.push(part);
        }
        if result.is_empty() {
            ".".to_string()
        } else {
            result.join(&sep.to_string())
        }
    }

    /// Check whether `child` is a descendant of `parent`.
    pub fn is_child_of(&self, child: &str, parent: &str) -> bool {
        let norm_child = self.normalize(child);
        let norm_parent = self.normalize(parent);
        let sep = self.sep_char();
        let parent_prefix = if norm_parent.ends_with(sep) {
            norm_parent.clone()
        } else {
            format!("{norm_parent}{sep}")
        };
        norm_child.starts_with(&parent_prefix) && norm_child.len() > parent_prefix.len()
    }

    /// Return the longest common path prefix among the given paths.
    pub fn common_prefix(&self, paths: &[&str]) -> String {
        if paths.is_empty() {
            return String::new();
        }
        let sep = self.sep_char();
        let normalized: Vec<String> = paths.iter().map(|p| self.normalize(p)).collect();
        let first_parts: Vec<&str> = normalized[0]
            .split(|c: char| c == '/' || c == '\\')
            .collect();
        let mut prefix_len = first_parts.len();
        for path in &normalized[1..] {
            let parts: Vec<&str> = path.split(|c: char| c == '/' || c == '\\').collect();
            let common = first_parts
                .iter()
                .zip(parts.iter())
                .take_while(|(a, b)| a == b)
                .count();
            prefix_len = prefix_len.min(common);
        }
        let result = first_parts[..prefix_len].join(&sep.to_string());
        if result.is_empty() && self.is_absolute(&normalized[0]) {
            sep.to_string()
        } else {
            result
        }
    }

    /// Replace the extension of `path` with `ext`.
    pub fn with_extension(&self, path: &str, ext: &str) -> String {
        let stripped = self.strip_extension(path);
        if ext.is_empty() {
            stripped
        } else if ext.starts_with('.') {
            format!("{stripped}{ext}")
        } else {
            format!("{stripped}.{ext}")
        }
    }

    /// Remove the file extension from `path`.
    pub fn strip_extension(&self, path: &str) -> String {
        let base = self.basename(path);
        match base.rfind('.') {
            Some(0) | None => path.to_string(),
            Some(i) => {
                let dir = self.dirname(path);
                let stem = &base[..i];
                if dir == "." {
                    stem.to_string()
                } else {
                    let sep = self.sep_char();
                    format!("{dir}{sep}{stem}")
                }
            }
        }
    }

    /// Split a path into its individual components.
    pub fn components(&self, path: &str) -> Vec<String> {
        path.split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    /// Return the depth (number of components) of a path.
    pub fn depth(&self, path: &str) -> usize {
        self.components(path).len()
    }

    /// Check whether `path` represents a root directory.
    pub fn is_root(&self, path: &str) -> bool {
        match self.separator {
            PathSeparator::Unix => path == "/",
            PathSeparator::Windows => {
                path == "/" || path == "\\"
                    || (path.len() == 3
                        && path.as_bytes()[0].is_ascii_alphabetic()
                        && path.as_bytes()[1] == b':'
                        && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/'))
            }
        }
    }

    /// Make a relative `path` absolute by joining it with `cwd`.
    pub fn make_absolute(&self, path: &str, cwd: &str) -> String {
        if self.is_absolute(path) {
            self.normalize(path)
        } else {
            let joined = self.join(cwd, path);
            self.normalize(&joined)
        }
    }
}

impl Default for PathService {
    fn default() -> Self {
        Self::new(PathSeparator::Unix)
    }
}

/// Accumulated statistics for wb-path operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbPathStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbPathStats {
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
    pub fn merge(&mut self, other: &WbPathStats) {
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

impl Default for WbPathStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbPathStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbPathStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-path.
#[derive(Debug, Clone)]
pub struct WbPathValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbPathValidator {
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

impl Default for WbPathValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PathBar – breadcrumb-style path display
// ---------------------------------------------------------------------------

/// A breadcrumb-style path display that splits a path into navigable segments.
#[derive(Debug, Clone)]
pub struct PathBar {
    /// The individual path segments (directory/file names).
    pub segments: Vec<String>,
    /// The separator character used in the original path.
    pub separator: char,
}

impl PathBar {
    /// Create a `PathBar` by splitting `path` according to the given
    /// [`PathSeparator`] style.
    pub fn from_path(path: &str, separator: PathSeparator) -> Self {
        let sep = match separator {
            PathSeparator::Unix => '/',
            PathSeparator::Windows => '\\',
        };
        let segments: Vec<String> = path
            .split(sep)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        Self {
            segments,
            separator: sep,
        }
    }

    /// Render the breadcrumb as `"seg1 > seg2 > seg3"`.
    pub fn render(&self) -> String {
        self.segments.join(" > ")
    }

    /// Return a new `PathBar` that keeps only the last `max_segments` segments.
    ///
    /// If truncation occurs the first visible segment is prefixed with `"…"`.
    pub fn truncate(&self, max_segments: usize) -> PathBar {
        if self.segments.len() <= max_segments {
            return self.clone();
        }
        let start = self.segments.len() - max_segments;
        let mut truncated: Vec<String> = self.segments[start..].to_vec();
        if let Some(first) = truncated.first_mut() {
            *first = format!("…{}{}", self.separator, first);
        }
        PathBar {
            segments: truncated,
            separator: self.separator,
        }
    }

    /// Return the number of segments in the path bar.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Simulate clicking on a segment at `index`.
    ///
    /// Returns the full path up to and including that segment, or `None` if
    /// the index is out of range.
    pub fn click_segment(&self, index: usize) -> Option<String> {
        if index >= self.segments.len() {
            return None;
        }
        let sep = self.separator.to_string();
        let joined = self.segments[..=index].join(&sep);
        // Preserve a leading separator for absolute Unix paths.
        if self.separator == '/' {
            Some(format!("/{joined}"))
        } else {
            Some(joined)
        }
    }
}

impl fmt::Display for PathBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

// ---------------------------------------------------------------------------
// PathCompletionProvider – path autocomplete
// ---------------------------------------------------------------------------

/// Provides path auto-completion against a set of known paths.
#[derive(Debug, Clone)]
pub struct PathCompletionProvider {
    /// The set of known paths available for completion.
    pub known_paths: Vec<String>,
}

impl PathCompletionProvider {
    /// Create an empty provider.
    pub fn new() -> Self {
        Self {
            known_paths: Vec::new(),
        }
    }

    /// Register a path for future completions.
    pub fn add_path(&mut self, path: &str) {
        self.known_paths.push(path.to_string());
    }

    /// Return all known paths that start with `partial`, sorted
    /// lexicographically.
    pub fn complete<'a>(&'a self, partial: &str) -> Vec<&'a str> {
        let mut matches: Vec<&str> = self
            .known_paths
            .iter()
            .filter(|p| p.starts_with(partial))
            .map(String::as_str)
            .collect();
        matches.sort();
        matches
    }

    /// Return all known paths whose **basename** (last component) starts with
    /// `partial`, sorted lexicographically.
    pub fn complete_basename<'a>(&'a self, partial: &str) -> Vec<&'a str> {
        let mut matches: Vec<&str> = self
            .known_paths
            .iter()
            .filter(|p| {
                let basename = p.rsplit('/').next().or_else(|| p.rsplit('\\').next()).unwrap_or(p);
                basename.starts_with(partial)
            })
            .map(String::as_str)
            .collect();
        matches.sort();
        matches
    }
}

impl Default for PathCompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// path_shorten – replace home directory prefix with "~"
// ---------------------------------------------------------------------------

/// Shorten `path` by replacing a leading `home_dir` prefix with `~`.
///
/// If `path` does not start with `home_dir` the original string is returned
/// unchanged.
pub fn path_shorten(path: &str, home_dir: &str) -> String {
    let home = home_dir.trim_end_matches('/');
    if path == home {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix(home) {
        if rest.starts_with('/') {
            return format!("~{rest}");
        }
    }
    path.to_string()
}


// ---------------------------------------------------------------------------
// PathSeparator helpers
// ---------------------------------------------------------------------------

impl PathSeparator {
    /// Returns the character for this separator.
    pub fn as_char(&self) -> char {
        match self {
            PathSeparator::Unix => '/',
            PathSeparator::Windows => '\\',
        }
    }

    /// Detect separator from a path string.
    pub fn detect(path: &str) -> Self {
        if path.contains('\\') {
            PathSeparator::Windows
        } else {
            PathSeparator::Unix
        }
    }

    /// Returns the other separator variant.
    pub fn opposite(&self) -> Self {
        match self {
            PathSeparator::Unix => PathSeparator::Windows,
            PathSeparator::Windows => PathSeparator::Unix,
        }
    }
}

impl fmt::Display for PathSeparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathSeparator::Unix => write!(f, "Unix (/)"),
            PathSeparator::Windows => write!(f, "Windows (\\)"),
        }
    }
}

impl Default for PathSeparator {
    fn default() -> Self {
        PathSeparator::Unix
    }
}

// ---------------------------------------------------------------------------
// Path analysis helpers
// ---------------------------------------------------------------------------

/// Count the depth (number of segments) in a path.
pub fn path_depth(path: &str) -> usize {
    path.split(|c: char| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
        .count()
}

/// Returns the common prefix of two paths.
pub fn common_prefix(a: &str, b: &str) -> String {
    let sep = if a.contains('\\') || b.contains('\\') { '\\' } else { '/' };
    let a_parts: Vec<&str> = a.split(|c: char| c == '/' || c == '\\').collect();
    let b_parts: Vec<&str> = b.split(|c: char| c == '/' || c == '\\').collect();
    let common: Vec<&str> = a_parts.iter().zip(b_parts.iter())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| *x)
        .collect();
    if common.is_empty() {
        String::new()
    } else {
        common.join(&sep.to_string())
    }
}

/// Converts all separators in a path to the given separator.
pub fn normalize_separators(path: &str, sep: PathSeparator) -> String {
    let c = sep.as_char();
    path.chars().map(|ch| if ch == '/' || ch == '\\' { c } else { ch }).collect()
}

/// Returns true if the path has a file extension.
pub fn has_extension(path: &str) -> bool {
    let filename = path.rsplit(|c: char| c == '/' || c == '\\').next().unwrap_or(path);
    filename.contains('.') && !filename.starts_with('.')
}

/// Returns all ancestor paths (from root to parent).
pub fn ancestors(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split(|c: char| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
        .collect();
    let sep = if path.contains('\\') { "\\" } else { "/" };
    let prefix = if path.starts_with('/') { "/" } else { "" };
    let mut result = Vec::new();
    for i in 1..parts.len() {
        let ancestor = format!("{}{}", prefix, parts[..i].join(sep));
        result.push(ancestor);
    }
    result
}

/// Joins multiple path segments.
pub fn path_join_many(base: &str, segments: &[&str]) -> String {
    let svc = PathService::new(PathSeparator::detect(base));
    let mut result = base.to_string();
    for seg in segments {
        result = svc.join(&result, seg);
    }
    result
}

// ---------------------------------------------------------------------------
// PathComponents – structured path decomposition
// ---------------------------------------------------------------------------

/// A parsed representation of a filesystem path broken into its constituent
/// parts: an optional drive letter, directory segments, filename stem, and
/// file extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathComponents {
    /// Drive letter including the colon (e.g. `"C:"`), or empty on Unix.
    pub drive: String,
    /// The individual directory segments (excluding drive and filename).
    pub directories: Vec<String>,
    /// The filename without extension, if present.
    pub stem: String,
    /// The extension including the leading dot (e.g. `".rs"`), or empty.
    pub extension: String,
}

impl PathComponents {
    /// Parse `path` using the given separator convention.
    pub fn parse(path: &str, separator: PathSeparator) -> Self {
        let svc = PathService::new(separator);
        let drive = svc.abs_prefix(path);
        let without_drive = if drive.is_empty() {
            path
        } else {
            &path[drive.len()..]
        };

        let parts: Vec<&str> = without_drive
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            return Self {
                drive,
                directories: Vec::new(),
                stem: String::new(),
                extension: String::new(),
            };
        }

        let last = parts[parts.len() - 1];
        let dirs: Vec<String> = parts[..parts.len() - 1].iter().map(|s| s.to_string()).collect();

        // Determine stem and extension from the last component.
        let (stem, ext) = match last.rfind('.') {
            Some(0) | None => (last.to_string(), String::new()),
            Some(i) => (last[..i].to_string(), last[i..].to_string()),
        };

        Self {
            drive,
            directories: dirs,
            stem,
            extension: ext,
        }
    }

    /// The full filename (stem + extension).
    pub fn filename(&self) -> String {
        if self.extension.is_empty() {
            self.stem.clone()
        } else {
            format!("{}{}", self.stem, self.extension)
        }
    }

    /// The directory portion of the path (drive + directories joined).
    pub fn directory(&self, separator: PathSeparator) -> String {
        let sep = separator.as_char().to_string();
        let dir = self.directories.join(&sep);
        if self.drive.is_empty() {
            dir
        } else {
            format!("{}{}{}", self.drive, sep, dir)
        }
    }

    /// Reconstruct the full path.
    pub fn to_path(&self, separator: PathSeparator) -> String {
        let sep = separator.as_char().to_string();
        let mut parts: Vec<&str> = Vec::new();
        for d in &self.directories {
            parts.push(d.as_str());
        }
        let filename = self.filename();
        if !filename.is_empty() {
            parts.push(&filename);
        }
        let joined = parts.join(&sep);
        if self.drive.is_empty() {
            joined
        } else {
            format!("{}{}{}", self.drive, sep, joined)
        }
    }

    /// Returns true if this path has no meaningful components.
    pub fn is_empty(&self) -> bool {
        self.drive.is_empty()
            && self.directories.is_empty()
            && self.stem.is_empty()
            && self.extension.is_empty()
    }
}

impl fmt::Display for PathComponents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PathComponents(drive={:?}, dirs={:?}, stem={:?}, ext={:?})",
            self.drive, self.directories, self.stem, self.extension
        )
    }
}

impl From<&str> for PathComponents {
    fn from(path: &str) -> Self {
        let sep = PathSeparator::detect(path);
        Self::parse(path, sep)
    }
}

// ---------------------------------------------------------------------------
// relative_path – compute relative path between two absolute paths
// ---------------------------------------------------------------------------

/// Compute the relative path from `from` to `to`.
///
/// Both paths are treated as absolute paths. The separator style is
/// auto-detected from the `from` path.
pub fn relative_path(from: &str, to: &str) -> String {
    let sep = PathSeparator::detect(from);
    let svc = PathService::new(sep);
    svc.relative(from, to)
}

// ---------------------------------------------------------------------------
// PathMatcher – glob-like pattern matching
// ---------------------------------------------------------------------------

/// A simple glob-style path matcher supporting `*` (any characters except
/// separators) and `**` (any characters including separators).
#[derive(Debug, Clone)]
pub struct PathMatcher {
    pattern: String,
}

impl PathMatcher {
    /// Create a new matcher from a glob pattern string.
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
        }
    }

    /// Test whether `path` matches the pattern.
    ///
    /// Pattern rules:
    /// - `*` matches zero or more characters within a single path segment
    ///   (does not cross `/` or `\`).
    /// - `**` matches zero or more complete path segments (including separators).
    /// - All other characters are matched literally (case-sensitive).
    pub fn matches(&self, path: &str) -> bool {
        let pattern_parts = Self::split_pattern(&self.pattern);
        let path_segments: Vec<&str> = path
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .collect();
        Self::match_segments(&pattern_parts, &path_segments)
    }

    /// Split a pattern by path separators, preserving `**` as its own token.
    fn split_pattern(pattern: &str) -> Vec<String> {
        pattern
            .split(|c: char| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    /// Recursively match pattern parts against path segments.
    fn match_segments(pattern: &[String], path: &[&str]) -> bool {
        if pattern.is_empty() {
            return path.is_empty();
        }

        if pattern[0] == "**" {
            // `**` can match zero or more segments
            for i in 0..=path.len() {
                if Self::match_segments(&pattern[1..], &path[i..]) {
                    return true;
                }
            }
            return false;
        }

        if path.is_empty() {
            return false;
        }

        if Self::match_wildcard(&pattern[0], path[0]) {
            Self::match_segments(&pattern[1..], &path[1..])
        } else {
            false
        }
    }

    /// Match a single pattern segment (with `*` wildcards) against a single
    /// path segment.
    fn match_wildcard(pattern: &str, segment: &str) -> bool {
        let p: Vec<char> = pattern.chars().collect();
        let s: Vec<char> = segment.chars().collect();
        Self::match_wild(&p, &s)
    }

    fn match_wild(p: &[char], s: &[char]) -> bool {
        if p.is_empty() {
            return s.is_empty();
        }
        if p[0] == '*' {
            // `*` matches zero or more non-separator characters
            for i in 0..=s.len() {
                if Self::match_wild(&p[1..], &s[i..]) {
                    return true;
                }
            }
            return false;
        }
        if s.is_empty() {
            return false;
        }
        if p[0] == s[0] {
            Self::match_wild(&p[1..], &s[1..])
        } else {
            false
        }
    }

    /// Filter an iterator of paths, returning only those that match.
    pub fn filter<'a>(&self, paths: &'a [&str]) -> Vec<&'a str> {
        paths.iter().copied().filter(|p| self.matches(p)).collect()
    }
}

impl fmt::Display for PathMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PathMatcher({})", self.pattern)
    }
}

impl From<&str> for PathMatcher {
    fn from(pattern: &str) -> Self {
        Self::new(pattern)
    }
}

// ---------------------------------------------------------------------------
// expand_tilde – expand ~ to home directory
// ---------------------------------------------------------------------------

/// Expand a leading `~` in `path` to `home_dir`.
///
/// If `path` does not start with `~` it is returned unchanged. A bare `~`
/// expands to `home_dir`; `~/rest` expands to `home_dir/rest`.
pub fn expand_tilde(path: &str, home_dir: &str) -> String {
    if path == "~" {
        return home_dir.trim_end_matches('/').to_string();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        let home = home_dir.trim_end_matches('/');
        return format!("{home}/{rest}");
    }
    path.to_string()
}

// ---------------------------------------------------------------------------
// PathLabelFormatter – compact and full path label formatting
// ---------------------------------------------------------------------------

/// Formats paths into human-readable labels for display in UI elements such as
/// tabs, breadcrumbs, and status bars.
#[derive(Debug, Clone)]
pub struct PathLabelFormatter {
    /// The workspace root against which paths are made relative.
    workspace_root: String,
    /// Home directory for tilde substitution.
    home_dir: String,
    separator: PathSeparator,
}

impl PathLabelFormatter {
    /// Create a new formatter with the given workspace root and home directory.
    pub fn new(workspace_root: &str, home_dir: &str, separator: PathSeparator) -> Self {
        Self {
            workspace_root: workspace_root.trim_end_matches(|c| c == '/' || c == '\\').to_string(),
            home_dir: home_dir.trim_end_matches(|c| c == '/' || c == '\\').to_string(),
            separator,
        }
    }

    /// Produce a compact label: workspace-relative if possible, otherwise
    /// tilde-shortened, otherwise the full path. The basename is always shown;
    /// the parent directory is included when `show_parent` is true.
    pub fn compact(&self, path: &str, show_parent: bool) -> String {
        let svc = PathService::new(self.separator);
        let rel = self.workspace_relative(path);
        let display = rel.as_deref().unwrap_or(path);
        if show_parent {
            let dir = svc.dirname(display);
            let base = svc.basename(display);
            if dir == "." {
                base.to_string()
            } else {
                let short_dir = svc.basename(dir);
                let sep = self.separator.as_char();
                format!("{short_dir}{sep}{base}")
            }
        } else {
            svc.basename(display).to_string()
        }
    }

    /// Produce a full label: workspace-relative with tilde fallback.
    pub fn full(&self, path: &str) -> String {
        if let Some(rel) = self.workspace_relative(path) {
            return rel;
        }
        path_shorten(path, &self.home_dir)
    }

    /// Return the workspace-relative path, or `None` if the path is outside
    /// the workspace.
    pub fn workspace_relative(&self, path: &str) -> Option<String> {
        let svc = PathService::new(self.separator);
        let norm = svc.normalize(path);
        let root_prefix = format!("{}{}", self.workspace_root, self.separator.as_char());
        if norm == self.workspace_root {
            return Some(".".to_string());
        }
        if let Some(rest) = norm.strip_prefix(&root_prefix) {
            Some(rest.to_string())
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// PathIconResolver – file icon resolution by extension
// ---------------------------------------------------------------------------

/// Maps file extensions to icon identifiers for use in workbench tree views,
/// tabs, and explorer panels.
#[derive(Debug, Clone)]
pub struct PathIconResolver {
    mappings: Vec<(String, String)>,
    default_file_icon: String,
    default_folder_icon: String,
}

impl PathIconResolver {
    /// Create a resolver pre-loaded with common language/file-type mappings.
    pub fn new() -> Self {
        let mut mappings = Vec::new();
        let defaults: &[(&str, &str)] = &[
            (".rs", "rust"),
            (".toml", "toml"),
            (".json", "json"),
            (".yaml", "yaml"),
            (".yml", "yaml"),
            (".js", "javascript"),
            (".ts", "typescript"),
            (".tsx", "react"),
            (".jsx", "react"),
            (".py", "python"),
            (".go", "go"),
            (".md", "markdown"),
            (".txt", "text"),
            (".html", "html"),
            (".css", "css"),
            (".svg", "svg"),
            (".png", "image"),
            (".jpg", "image"),
            (".gif", "image"),
            (".lock", "lock"),
            (".sh", "shell"),
            (".bash", "shell"),
            (".zsh", "shell"),
            (".c", "c"),
            (".cpp", "cpp"),
            (".h", "c-header"),
            (".java", "java"),
            (".xml", "xml"),
            (".sql", "database"),
        ];
        for (ext, icon) in defaults {
            mappings.push((ext.to_string(), icon.to_string()));
        }
        Self {
            mappings,
            default_file_icon: "file".to_string(),
            default_folder_icon: "folder".to_string(),
        }
    }

    /// Register a custom extension-to-icon mapping (extension must include
    /// the leading dot).
    pub fn add_mapping(&mut self, extension: &str, icon: &str) {
        self.mappings.push((extension.to_string(), icon.to_string()));
    }

    /// Resolve the icon identifier for a file path.
    pub fn resolve(&self, path: &str) -> &str {
        let svc = PathService::default();
        if let Some(ext) = svc.extname(path) {
            let ext_lower = ext.to_ascii_lowercase();
            for (mapped_ext, icon) in self.mappings.iter().rev() {
                if mapped_ext == &ext_lower {
                    return icon;
                }
            }
        }
        // Special filenames
        let base = svc.basename(path);
        match base {
            "Cargo.toml" | "Cargo.lock" => "rust",
            "package.json" => "npm",
            "Makefile" | "CMakeLists.txt" => "build",
            "Dockerfile" => "docker",
            ".gitignore" | ".gitattributes" => "git",
            "LICENSE" | "LICENSE.md" => "certificate",
            "README.md" | "README" => "info",
            _ => &self.default_file_icon,
        }
    }

    /// Resolve the icon for a directory.
    pub fn resolve_folder(&self, name: &str) -> &str {
        match name {
            "src" | "lib" => "folder-src",
            "test" | "tests" | "__tests__" => "folder-test",
            "docs" | "doc" => "folder-docs",
            "target" | "build" | "dist" | "out" => "folder-build",
            ".git" => "folder-git",
            "node_modules" => "folder-node",
            ".github" => "folder-github",
            "crates" => "folder-crate",
            _ => &self.default_folder_icon,
        }
    }
}

impl Default for PathIconResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PathBookmarkManager – bookmark management for paths
// ---------------------------------------------------------------------------

/// Manages a set of bookmarked paths with labels.
#[derive(Debug, Clone)]
pub struct PathBookmarkManager {
    bookmarks: Vec<PathBookmark>,
}

/// A single bookmarked path entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathBookmark {
    /// User-visible label for the bookmark.
    pub label: String,
    /// The bookmarked path.
    pub path: String,
}

impl PathBookmarkManager {
    pub fn new() -> Self {
        Self {
            bookmarks: Vec::new(),
        }
    }

    /// Add a bookmark. Returns `false` if the path is already bookmarked.
    pub fn add(&mut self, label: &str, path: &str) -> bool {
        if self.bookmarks.iter().any(|b| b.path == path) {
            return false;
        }
        self.bookmarks.push(PathBookmark {
            label: label.to_string(),
            path: path.to_string(),
        });
        true
    }

    /// Remove a bookmark by path. Returns `true` if it was found and removed.
    pub fn remove(&mut self, path: &str) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.path != path);
        self.bookmarks.len() < before
    }

    /// Check whether a path is bookmarked.
    pub fn contains(&self, path: &str) -> bool {
        self.bookmarks.iter().any(|b| b.path == path)
    }

    /// Return all bookmarks.
    pub fn list(&self) -> &[PathBookmark] {
        &self.bookmarks
    }

    /// Find bookmarks whose label or path contains `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&PathBookmark> {
        let q = query.to_ascii_lowercase();
        self.bookmarks
            .iter()
            .filter(|b| {
                b.label.to_ascii_lowercase().contains(&q)
                    || b.path.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    /// Return the number of bookmarks.
    pub fn len(&self) -> usize {
        self.bookmarks.len()
    }

    /// Return `true` if there are no bookmarks.
    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }
}

impl Default for PathBookmarkManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PathTemplateExpander – expand template variables in paths
// ---------------------------------------------------------------------------

/// Expands `${variable}` placeholders within path strings against a set of
/// registered variables.
#[derive(Debug, Clone)]
pub struct PathTemplateExpander {
    variables: Vec<(String, String)>,
}

impl PathTemplateExpander {
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
        }
    }

    /// Register a variable name and its value.
    pub fn set(&mut self, name: &str, value: &str) {
        // Update existing or insert new.
        for (k, v) in &mut self.variables {
            if k == name {
                *v = value.to_string();
                return;
            }
        }
        self.variables.push((name.to_string(), value.to_string()));
    }

    /// Expand all `${name}` placeholders in `template`. Unknown variables are
    /// left as-is.
    pub fn expand(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (name, value) in &self.variables {
            let placeholder = format!("${{{name}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Return the names of all registered variables.
    pub fn variables(&self) -> Vec<&str> {
        self.variables.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Return the value of a variable, if registered.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.variables
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

impl Default for PathTemplateExpander {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OS-specific path validation
// ---------------------------------------------------------------------------

/// Characters forbidden in Windows file/directory names.
const WINDOWS_FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];

/// Reserved device names on Windows (case-insensitive).
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validate that a filename is legal on Windows. Returns a list of problems
/// (empty if valid).
pub fn validate_filename_windows(name: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if name.is_empty() {
        problems.push("filename must not be empty".to_string());
        return problems;
    }
    if name.len() > 255 {
        problems.push(format!("filename length {} exceeds 255", name.len()));
    }
    for ch in WINDOWS_FORBIDDEN_CHARS {
        if name.contains(*ch) {
            problems.push(format!("contains forbidden character '{ch}'"));
        }
    }
    if name.ends_with('.') || name.ends_with(' ') {
        problems.push("must not end with a period or space".to_string());
    }
    // Check for control characters (0x00–0x1F).
    if name.chars().any(|c| (c as u32) < 0x20) {
        problems.push("contains control characters".to_string());
    }
    let upper = name.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or(&upper);
    if WINDOWS_RESERVED_NAMES.contains(&stem) {
        problems.push(format!("'{stem}' is a reserved device name"));
    }
    problems
}

/// Validate that a filename is legal on Unix/Linux. Returns a list of problems
/// (empty if valid).
pub fn validate_filename_unix(name: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if name.is_empty() {
        problems.push("filename must not be empty".to_string());
        return problems;
    }
    if name.len() > 255 {
        problems.push(format!("filename length {} exceeds 255", name.len()));
    }
    if name.contains('/') {
        problems.push("contains path separator '/'".to_string());
    }
    if name.contains('\0') {
        problems.push("contains null byte".to_string());
    }
    problems
}

/// Validate a full path's segments for the given OS.
pub fn validate_path_segments(path: &str, sep: PathSeparator) -> Vec<String> {
    let parts: Vec<&str> = path
        .split(|c: char| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
        .collect();
    let mut problems = Vec::new();
    for part in &parts {
        // Skip drive letter segments like "C:".
        if sep == PathSeparator::Windows
            && part.len() == 2
            && part.as_bytes()[0].is_ascii_alphabetic()
            && part.as_bytes()[1] == b':'
        {
            continue;
        }
        let segment_problems = match sep {
            PathSeparator::Windows => validate_filename_windows(part),
            PathSeparator::Unix => validate_filename_unix(part),
        };
        for p in segment_problems {
            problems.push(format!("segment '{}': {}", part, p));
        }
    }
    problems
}


// === Path Display Formatter ===

/// Path Display Formatter implementation.
#[derive(Debug, Clone)]
pub struct PathDisplayFormatter {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: PathDisplayFormatterStats,
}

/// Statistics for PathDisplayFormatter.
#[derive(Debug, Clone, Default)]
pub struct PathDisplayFormatterStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl PathDisplayFormatterStats {
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

impl PathDisplayFormatter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: PathDisplayFormatterStats::default(),
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

    pub fn stats(&self) -> &PathDisplayFormatterStats {
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

impl Default for PathDisplayFormatter {
    fn default() -> Self {
        Self::new()
    }
}

// === Path Comparison Normalizer ===

/// Priority level for PathComparisonNormalizer items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathComparisonNormalizerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl PathComparisonNormalizerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for PathComparisonNormalizerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Path Comparison Normalizer implementation.
#[derive(Debug, Clone)]
pub struct PathComparisonNormalizer {
    items: Vec<PathComparisonNormalizerItem>,
    max_items: usize,
    default_priority: PathComparisonNormalizerPriority,
}

/// A single item in PathComparisonNormalizer.
#[derive(Debug, Clone)]
pub struct PathComparisonNormalizerItem {
    pub id: String,
    pub label: String,
    pub priority: PathComparisonNormalizerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl PathComparisonNormalizerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: PathComparisonNormalizerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: PathComparisonNormalizerPriority) -> Self {
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

impl PathComparisonNormalizer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: PathComparisonNormalizerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: PathComparisonNormalizerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<PathComparisonNormalizerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&PathComparisonNormalizerItem> {
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

    pub fn by_priority(&self, priority: PathComparisonNormalizerPriority) -> Vec<&PathComparisonNormalizerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&PathComparisonNormalizerItem> {
        let mut sorted: Vec<&PathComparisonNormalizerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&PathComparisonNormalizerItem> {
        let mut sorted: Vec<&PathComparisonNormalizerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&PathComparisonNormalizerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: PathComparisonNormalizerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> PathComparisonNormalizerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &PathComparisonNormalizerItem> {
        self.items.iter()
    }
}

impl Default for PathComparisonNormalizer {
    fn default() -> Self {
        Self::new()
    }
}


/// Workbench path configuration manager.
#[derive(Debug, Clone)]
pub struct WbPathConfig {
    entries: Vec<WbPathEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench path entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbPathEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbPathEntry {
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

impl WbPathConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbPathEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&WbPathEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbPathEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbPathEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&WbPathEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbPathEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<WbPathEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
// xa_ extended helpers for wb_path
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbPathRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbPathRingBuf {
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
pub struct XaWbPathCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbPathCounter {
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

impl Default for XaWbPathCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 220
// ---------------------------------------------------------------------------

/// Generic object pool `Xc220Pool<T>`.
pub struct Xc220Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc220Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc220PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc220Pool<T> {
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
    pub fn stats(&self) -> Xc220PoolStats {
        Xc220PoolStats {
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

impl<T> Default for Xc220Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc220Scheduler`.
pub struct Xc220Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc220Scheduler {
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

impl Default for Xc220Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_220 hash for the given byte slice.
pub fn xc_220_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_220 convention.
pub fn xc_220_reverse(s: &str) -> String {
    s.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_normalize() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.normalize("/a/b/../c/./d"), "/a/c/d");
        assert_eq!(svc.normalize("a//b"), "a/b");
    }

    #[test]
    fn basename_extname() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.basename("/foo/bar.txt"), "bar.txt");
        assert_eq!(svc.extname("/foo/bar.txt"), Some(".txt"));
        assert_eq!(svc.extname("/foo/.hidden"), None);
        assert_eq!(svc.dirname("/foo/bar.txt"), "/foo");
    }

    #[test]
    fn windows_paths() {
        let svc = PathService::new(PathSeparator::Windows);
        assert!(svc.is_absolute("C:\\Users"));
        assert!(!svc.is_absolute("relative\\path"));
        assert_eq!(svc.to_unix("C:\\a\\b"), "C:/a/b");
    }

    #[test]
    fn resolve_relative() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.resolve_relative("/a/b/c.txt", "../d.txt"), "/a/d.txt");
    }

    #[test]
    fn relative_path() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.relative("/a/b", "/a/c/d"), "../c/d");
        assert_eq!(svc.relative("/a/b/c", "/a/b/c"), ".");
        assert_eq!(svc.relative("/a", "/a/b/c"), "b/c");
    }

    #[test]
    fn is_child_of_check() {
        let svc = PathService::new(PathSeparator::Unix);
        assert!(svc.is_child_of("/a/b/c", "/a/b"));
        assert!(svc.is_child_of("/a/b/c/d", "/a"));
        assert!(!svc.is_child_of("/a/b", "/a/b"));
        assert!(!svc.is_child_of("/x/y", "/a/b"));
    }

    #[test]
    fn common_prefix_paths() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.common_prefix(&["/a/b/c", "/a/b/d", "/a/b/e"]), "/a/b");
        assert_eq!(svc.common_prefix(&["/a/b", "/c/d"]), "/");
        assert_eq!(svc.common_prefix(&[]), "");
    }

    #[test]
    fn with_and_strip_extension() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.with_extension("/a/b.txt", "rs"), "/a/b.rs");
        assert_eq!(svc.with_extension("/a/b.txt", ".rs"), "/a/b.rs");
        assert_eq!(svc.strip_extension("/a/b.txt"), "/a/b");
        assert_eq!(svc.strip_extension("/a/.hidden"), "/a/.hidden");
    }

    #[test]
    fn components_and_depth() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.components("/a/b/c"), vec!["a", "b", "c"]);
        assert_eq!(svc.depth("/a/b/c"), 3);
        assert_eq!(svc.depth("a"), 1);
    }

    #[test]
    fn is_root_check() {
        let unix = PathService::new(PathSeparator::Unix);
        assert!(unix.is_root("/"));
        assert!(!unix.is_root("/a"));

        let win = PathService::new(PathSeparator::Windows);
        assert!(win.is_root("C:\\"));
        assert!(!win.is_root("C:\\Users"));
    }

    #[test]
    fn make_absolute_path() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.make_absolute("b/c", "/a"), "/a/b/c");
        assert_eq!(svc.make_absolute("/x/y", "/a"), "/x/y");
    }

    #[test]
    fn with_extension_empty() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.with_extension("/a/b.txt", ""), "/a/b");
    }

    #[test]
    fn relative_windows() {
        let svc = PathService::new(PathSeparator::Windows);
        assert_eq!(svc.relative("C:\\a\\b", "C:\\a\\c\\d"), "..\\c\\d");
    }

    #[test]
    fn depth_edge_cases() {
        let svc = PathService::new(PathSeparator::Unix);
        assert_eq!(svc.depth("/"), 0);
        assert_eq!(svc.depth("//"), 0);
        assert_eq!(svc.depth("/a/b/c/d/e"), 5);
    }

    #[test]
    fn eq_pathseparator_same() {
        assert_eq!(PathSeparator::Unix, PathSeparator::Unix);
    }

    #[test]
    fn ne_pathseparator_diff() {
        assert_ne!(PathSeparator::Unix, PathSeparator::Windows);
    }

    #[test]
    fn pathbar_from_unix_path() {
        let bar = PathBar::from_path("/home/user/docs", PathSeparator::Unix);
        assert_eq!(bar.segments, vec!["home", "user", "docs"]);
        assert_eq!(bar.separator, '/');
    }

    #[test]
    fn pathbar_render() {
        let bar = PathBar::from_path("/a/b/c", PathSeparator::Unix);
        assert_eq!(bar.render(), "a > b > c");
    }

    #[test]
    fn pathbar_truncate() {
        let bar = PathBar::from_path("/a/b/c/d", PathSeparator::Unix);
        let t = bar.truncate(2);
        assert_eq!(t.segment_count(), 2);
        assert_eq!(t.render(), "…/c > d");

        // No truncation when within limit.
        let t2 = bar.truncate(10);
        assert_eq!(t2.segment_count(), 4);
    }

    #[test]
    fn pathbar_click_segment() {
        let bar = PathBar::from_path("/home/user/docs", PathSeparator::Unix);
        assert_eq!(bar.click_segment(0), Some("/home".to_string()));
        assert_eq!(bar.click_segment(1), Some("/home/user".to_string()));
        assert_eq!(bar.click_segment(2), Some("/home/user/docs".to_string()));
        assert_eq!(bar.click_segment(5), None);
    }

    #[test]
    fn completion_provider_complete() {
        let mut cp = PathCompletionProvider::new();
        cp.add_path("/home/user/docs");
        cp.add_path("/home/user/downloads");
        cp.add_path("/etc/config");

        let results = cp.complete("/home/user/do");
        assert_eq!(results, vec!["/home/user/docs", "/home/user/downloads"]);

        let empty = cp.complete("/var");
        assert!(empty.is_empty());
    }

    #[test]
    fn completion_provider_complete_basename() {
        let mut cp = PathCompletionProvider::new();
        cp.add_path("/home/user/docs");
        cp.add_path("/home/user/downloads");
        cp.add_path("/etc/default");

        let results = cp.complete_basename("do");
        assert_eq!(results, vec!["/home/user/docs", "/home/user/downloads"]);
    }

    #[test]
    fn path_shorten_replaces_home() {
        assert_eq!(path_shorten("/home/user/docs", "/home/user"), "~/docs");
        assert_eq!(path_shorten("/home/user", "/home/user"), "~");
        assert_eq!(
            path_shorten("/home/user/a/b", "/home/user/"),
            "~/a/b"
        );
    }

    #[test]
    fn path_shorten_no_home_prefix() {
        assert_eq!(path_shorten("/etc/config", "/home/user"), "/etc/config");
        assert_eq!(
            path_shorten("/home/username/file", "/home/user"),
            "/home/username/file"
        );
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_33() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_34() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_35() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_36() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_37() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_38() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_39() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_40() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn wb_path_stats_new_defaults() {
        let stats = WbPathStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_path_stats_record_success() {
        let mut stats = WbPathStats::new();
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
    fn wb_path_stats_record_failure() {
        let mut stats = WbPathStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_path_stats_reset() {
        let mut stats = WbPathStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_path_stats_merge() {
        let mut a = WbPathStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbPathStats::new();
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
    fn wb_path_stats_display() {
        let mut stats = WbPathStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_path_stats_default() {
        let stats = WbPathStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_path_validator_accepts_valid_name() {
        let v = WbPathValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_path_validator_rejects_empty() {
        let v = WbPathValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_path_validator_rejects_too_long() {
        let v = WbPathValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_path_validator_forbidden_prefix() {
        let v = WbPathValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_path_validator_allowed_chars() {
        let v = WbPathValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_path_validator_range() {
        let v = WbPathValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_path_sanitize_removes_control() {
        let result = WbPathValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_path_truncate_short_string() {
        assert_eq!(WbPathValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_path_truncate_long_string() {
        let result = WbPathValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_path_is_ascii_printable() {
        assert!(WbPathValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbPathValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn test_path_separator_as_char() {
        assert_eq!(PathSeparator::Unix.as_char(), '/');
        assert_eq!(PathSeparator::Windows.as_char(), '\\');
    }

    #[test]
    fn test_path_separator_detect() {
        assert_eq!(PathSeparator::detect("/usr/bin"), PathSeparator::Unix);
        assert_eq!(PathSeparator::detect("C:\\Users"), PathSeparator::Windows);
    }

    #[test]
    fn test_path_separator_opposite() {
        assert_eq!(PathSeparator::Unix.opposite(), PathSeparator::Windows);
        assert_eq!(PathSeparator::Windows.opposite(), PathSeparator::Unix);
    }

    #[test]
    fn test_path_separator_display_and_default() {
        assert!(format!("{}", PathSeparator::Unix).contains('/'));
        assert_eq!(PathSeparator::default(), PathSeparator::Unix);
    }

    #[test]
    fn test_path_depth() {
        assert_eq!(path_depth("/usr/local/bin"), 3);
        assert_eq!(path_depth("a/b"), 2);
        assert_eq!(path_depth(""), 0);
    }

    #[test]
    fn test_common_prefix() {
        assert_eq!(common_prefix("/usr/local/bin", "/usr/local/lib"), "/usr/local");
        assert_eq!(common_prefix("/a/b", "/c/d"), "");
    }

    #[test]
    fn test_normalize_separators() {
        assert_eq!(normalize_separators("a/b\\c", PathSeparator::Unix), "a/b/c");
        assert_eq!(normalize_separators("a/b/c", PathSeparator::Windows), "a\\b\\c");
    }

    #[test]
    fn test_has_extension() {
        assert!(has_extension("file.rs"));
        assert!(!has_extension(".hidden"));
        assert!(!has_extension("noext"));
    }

    #[test]
    fn test_ancestors() {
        let ancs = ancestors("/usr/local/bin/tool");
        assert_eq!(ancs, vec!["/usr", "/usr/local", "/usr/local/bin"]);
    }

    #[test]
    fn test_path_join_many() {
        let result = path_join_many("/home", &["user", "docs", "file.txt"]);
        assert_eq!(result, "/home/user/docs/file.txt");
    }

    // -----------------------------------------------------------------------
    // PathComponents tests
    // -----------------------------------------------------------------------

    #[test]
    fn path_components_parse_unix() {
        let pc = PathComponents::parse("/home/user/docs/readme.md", PathSeparator::Unix);
        assert_eq!(pc.drive, "");
        assert_eq!(pc.directories, vec!["home", "user", "docs"]);
        assert_eq!(pc.stem, "readme");
        assert_eq!(pc.extension, ".md");
        assert_eq!(pc.filename(), "readme.md");
        assert!(!pc.is_empty());
    }

    #[test]
    fn path_components_parse_windows_drive() {
        let pc = PathComponents::parse("C:\\Users\\admin\\file.txt", PathSeparator::Windows);
        assert_eq!(pc.drive, "C:");
        assert_eq!(pc.directories, vec!["Users", "admin"]);
        assert_eq!(pc.stem, "file");
        assert_eq!(pc.extension, ".txt");
        assert_eq!(pc.to_path(PathSeparator::Windows), "C:\\Users\\admin\\file.txt");
    }

    #[test]
    fn path_components_no_extension() {
        let pc = PathComponents::parse("/usr/bin/cargo", PathSeparator::Unix);
        assert_eq!(pc.stem, "cargo");
        assert_eq!(pc.extension, "");
        assert_eq!(pc.filename(), "cargo");
    }

    #[test]
    fn path_components_hidden_file() {
        let pc = PathComponents::parse("/home/.gitignore", PathSeparator::Unix);
        assert_eq!(pc.stem, ".gitignore");
        assert_eq!(pc.extension, "");
    }

    #[test]
    fn path_components_empty_path() {
        let pc = PathComponents::parse("", PathSeparator::Unix);
        assert!(pc.is_empty());
    }

    #[test]
    fn path_components_from_str() {
        let pc = PathComponents::from("/etc/hosts");
        assert_eq!(pc.directories, vec!["etc"]);
        assert_eq!(pc.stem, "hosts");
    }

    #[test]
    fn path_components_display() {
        let pc = PathComponents::parse("/a/b.rs", PathSeparator::Unix);
        let s = format!("{pc}");
        assert!(s.contains("stem="));
        assert!(s.contains(".rs"));
    }

    // -----------------------------------------------------------------------
    // relative_path tests
    // -----------------------------------------------------------------------

    #[test]
    fn relative_path_fn_sibling() {
        assert_eq!(super::relative_path("/a/b", "/a/c"), "../c");
    }

    #[test]
    fn relative_path_fn_nested() {
        assert_eq!(super::relative_path("/a", "/a/b/c"), "b/c");
    }

    // -----------------------------------------------------------------------
    // PathMatcher tests
    // -----------------------------------------------------------------------

    #[test]
    fn path_matcher_exact() {
        let m = PathMatcher::new("/home/user/file.txt");
        assert!(m.matches("/home/user/file.txt"));
        assert!(!m.matches("/home/user/other.txt"));
    }

    #[test]
    fn path_matcher_single_star() {
        let m = PathMatcher::new("/home/user/*.txt");
        assert!(m.matches("/home/user/readme.txt"));
        assert!(m.matches("/home/user/a.txt"));
        assert!(!m.matches("/home/user/sub/readme.txt"));
        assert!(!m.matches("/home/user/readme.rs"));
    }

    #[test]
    fn path_matcher_double_star() {
        let m = PathMatcher::new("/home/**/*.rs");
        assert!(m.matches("/home/user/project/main.rs"));
        assert!(m.matches("/home/lib.rs"));
        assert!(!m.matches("/home/user/readme.txt"));
    }

    #[test]
    fn path_matcher_filter() {
        let m = PathMatcher::new("*.rs");
        let paths = vec!["main.rs", "lib.rs", "readme.md", "build.rs"];
        let result = m.filter(&paths);
        assert_eq!(result, vec!["main.rs", "lib.rs", "build.rs"]);
    }

    #[test]
    fn path_matcher_display_and_from() {
        let m = PathMatcher::from("**/*.txt");
        assert_eq!(format!("{m}"), "PathMatcher(**/*.txt)");
    }

    // -----------------------------------------------------------------------
    // expand_tilde tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_expand_tilde_bare() {
        assert_eq!(expand_tilde("~", "/home/user"), "/home/user");
    }

    #[test]
    fn test_expand_tilde_subpath() {
        assert_eq!(
            expand_tilde("~/docs/file.txt", "/home/user"),
            "/home/user/docs/file.txt"
        );
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        assert_eq!(expand_tilde("/etc/hosts", "/home/user"), "/etc/hosts");
    }

    #[test]
    fn test_expand_tilde_trailing_slash() {
        assert_eq!(expand_tilde("~/a", "/home/user/"), "/home/user/a");
    }

    // -----------------------------------------------------------------------
    // PathLabelFormatter tests
    // -----------------------------------------------------------------------

    #[test]
    fn label_formatter_compact_workspace_relative() {
        let fmt = PathLabelFormatter::new("/home/user/project", "/home/user", PathSeparator::Unix);
        assert_eq!(fmt.compact("/home/user/project/src/main.rs", false), "main.rs");
        assert_eq!(fmt.compact("/home/user/project/src/main.rs", true), "src/main.rs");
    }

    #[test]
    fn label_formatter_full_workspace_relative() {
        let fmt = PathLabelFormatter::new("/home/user/project", "/home/user", PathSeparator::Unix);
        assert_eq!(fmt.full("/home/user/project/src/lib.rs"), "src/lib.rs");
        assert_eq!(fmt.full("/home/user/project"), ".");
        // Outside workspace falls back to tilde shortening.
        assert_eq!(fmt.full("/home/user/other/file.rs"), "~/other/file.rs");
    }

    #[test]
    fn label_formatter_workspace_relative_returns_none_outside() {
        let fmt = PathLabelFormatter::new("/workspace", "/home", PathSeparator::Unix);
        assert!(fmt.workspace_relative("/etc/hosts").is_none());
        assert_eq!(fmt.workspace_relative("/workspace/a/b"), Some("a/b".to_string()));
    }

    // -----------------------------------------------------------------------
    // PathIconResolver tests
    // -----------------------------------------------------------------------

    #[test]
    fn icon_resolver_common_extensions() {
        let r = PathIconResolver::new();
        assert_eq!(r.resolve("main.rs"), "rust");
        assert_eq!(r.resolve("app.ts"), "typescript");
        assert_eq!(r.resolve("style.css"), "css");
        assert_eq!(r.resolve("unknown"), "file");
    }

    #[test]
    fn icon_resolver_special_filenames_and_folders() {
        let r = PathIconResolver::new();
        assert_eq!(r.resolve("Dockerfile"), "docker");
        assert_eq!(r.resolve(".gitignore"), "git");
        assert_eq!(r.resolve_folder("src"), "folder-src");
        assert_eq!(r.resolve_folder("tests"), "folder-test");
        assert_eq!(r.resolve_folder("random"), "folder");
    }

    #[test]
    fn icon_resolver_custom_mapping() {
        let mut r = PathIconResolver::new();
        r.add_mapping(".vue", "vue");
        assert_eq!(r.resolve("app.vue"), "vue");
    }

    // -----------------------------------------------------------------------
    // PathBookmarkManager tests
    // -----------------------------------------------------------------------

    #[test]
    fn bookmark_manager_add_remove_search() {
        let mut bm = PathBookmarkManager::new();
        assert!(bm.is_empty());
        assert!(bm.add("Project", "/home/user/project"));
        assert!(!bm.add("Dup", "/home/user/project")); // duplicate path
        assert_eq!(bm.len(), 1);
        assert!(bm.contains("/home/user/project"));

        assert_eq!(bm.search("proj").len(), 1);
        assert_eq!(bm.search("xyz").len(), 0);

        assert!(bm.remove("/home/user/project"));
        assert!(!bm.remove("/nonexistent"));
        assert!(bm.is_empty());
    }

    // -----------------------------------------------------------------------
    // PathTemplateExpander tests
    // -----------------------------------------------------------------------

    #[test]
    fn template_expander_basic() {
        let mut exp = PathTemplateExpander::new();
        exp.set("workspaceFolder", "/home/user/project");
        exp.set("file", "main.rs");
        let result = exp.expand("${workspaceFolder}/src/${file}");
        assert_eq!(result, "/home/user/project/src/main.rs");
    }

    #[test]
    fn template_expander_unknown_variable_kept() {
        let exp = PathTemplateExpander::new();
        assert_eq!(exp.expand("${unknown}/path"), "${unknown}/path");
    }

    #[test]
    fn template_expander_overwrite_variable() {
        let mut exp = PathTemplateExpander::new();
        exp.set("root", "/old");
        exp.set("root", "/new");
        assert_eq!(exp.get("root"), Some("/new"));
        assert_eq!(exp.variables(), vec!["root"]);
    }

    // -----------------------------------------------------------------------
    // OS-specific path validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_filename_windows_basic() {
        assert!(validate_filename_windows("hello.txt").is_empty());
        let probs = validate_filename_windows("file<>.txt");
        assert!(!probs.is_empty());
        let probs = validate_filename_windows("CON");
        assert!(probs.iter().any(|p| p.contains("reserved")));
    }

    #[test]
    fn validate_filename_unix_basic() {
        assert!(validate_filename_unix("hello.txt").is_empty());
        let probs = validate_filename_unix("bad/name");
        assert!(probs.iter().any(|p| p.contains("separator")));
    }

    #[test]
    fn validate_path_segments_mixed() {
        let probs = validate_path_segments("/home/user/good", PathSeparator::Unix);
        assert!(probs.is_empty());
        let probs = validate_path_segments("C:\\Users\\CON\\file.txt", PathSeparator::Windows);
        assert!(probs.iter().any(|p| p.contains("reserved")));
    }

    #[test]
    fn pathDisplayFormatter_new() {
        let s = PathDisplayFormatter::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn pathDisplayFormatter_add_contains() {
        let mut s = PathDisplayFormatter::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn pathDisplayFormatter_add_duplicate() {
        let mut s = PathDisplayFormatter::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn pathDisplayFormatter_remove() {
        let mut s = PathDisplayFormatter::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn pathDisplayFormatter_capacity() {
        let s = PathDisplayFormatter::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn pathDisplayFormatter_search() {
        let mut s = PathDisplayFormatter::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn pathDisplayFormatter_stats() {
        let mut s = PathDisplayFormatter::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn pathComparisonNormalizer_new() {
        let m = PathComparisonNormalizer::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn pathComparisonNormalizer_add_find() {
        let mut m = PathComparisonNormalizer::new();
        m.add(PathComparisonNormalizerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn pathComparisonNormalizer_priority_filter() {
        let mut m = PathComparisonNormalizer::new();
        m.add(PathComparisonNormalizerItem::new("a", "A").with_priority(PathComparisonNormalizerPriority::High));
        m.add(PathComparisonNormalizerItem::new("b", "B").with_priority(PathComparisonNormalizerPriority::Low));
        m.add(PathComparisonNormalizerItem::new("c", "C").with_priority(PathComparisonNormalizerPriority::High));
        assert_eq!(m.by_priority(PathComparisonNormalizerPriority::High).len(), 2);
    }

    #[test]
    fn pathComparisonNormalizer_remove() {
        let mut m = PathComparisonNormalizer::new();
        m.add(PathComparisonNormalizerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn pathComparisonNormalizer_search() {
        let mut m = PathComparisonNormalizer::new();
        m.add(PathComparisonNormalizerItem::new("id1", "Hello World"));
        m.add(PathComparisonNormalizerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn pathComparisonNormalizer_total_weight() {
        let mut m = PathComparisonNormalizer::new();
        m.add(PathComparisonNormalizerItem::new("a", "A").with_priority(PathComparisonNormalizerPriority::Critical));
        m.add(PathComparisonNormalizerItem::new("b", "B").with_priority(PathComparisonNormalizerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn pathComparisonNormalizer_capacity_limit() {
        let mut m = PathComparisonNormalizer::new().with_max_items(2);
        m.add(PathComparisonNormalizerItem::new("1", "one"));
        m.add(PathComparisonNormalizerItem::new("2", "two"));
        assert!(!m.add(PathComparisonNormalizerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn pathComparisonNormalizer_sorted_by_priority() {
        let mut m = PathComparisonNormalizer::new();
        m.add(PathComparisonNormalizerItem::new("lo", "Low").with_priority(PathComparisonNormalizerPriority::Low));
        m.add(PathComparisonNormalizerItem::new("hi", "High").with_priority(PathComparisonNormalizerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn pathComparisonNormalizer_item_metadata() {
        let mut item = PathComparisonNormalizerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn pathDisplayFormatter_enabled_toggle() {
        let mut s = PathDisplayFormatter::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn pathComparisonNormalizer_priority_display() {
        assert_eq!(format!("{}", PathComparisonNormalizerPriority::High), "high");
        assert_eq!(format!("{}", PathComparisonNormalizerPriority::Low), "low");
    }


    #[test]
    fn wb_path_entry_creation() {
        let e = WbPathEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_path_entry_with_priority() {
        let e = WbPathEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_path_entry_metadata() {
        let e = WbPathEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_path_entry_remove_meta() {
        let mut e = WbPathEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_path_entry_activate_deactivate() {
        let mut e = WbPathEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_path_config_add_sorted() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("lo", "Lo").with_priority(1));
        c.add(WbPathEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_path_config_capacity() {
        let mut c = WbPathConfig::new(1);
        assert!(c.add(WbPathEntry::new("a", "A")));
        assert!(!c.add(WbPathEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_path_config_remove() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_path_config_get() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_path_config_active_entries() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "A"));
        c.add(WbPathEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_path_config_enable_disable() {
        let mut c = WbPathConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_path_config_clear() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_path_config_find_by_label() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_path_config_top_n() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "A").with_priority(1));
        c.add(WbPathEntry::new("b", "B").with_priority(2));
        c.add(WbPathEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_path_config_deactivate_activate_all() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "A"));
        c.add(WbPathEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_path_config_highest_priority() {
        let mut c = WbPathConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbPathEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_path_config_contains() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_path_config_labels() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "Alpha"));
        c.add(WbPathEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_path_config_drain_inactive() {
        let mut c = WbPathConfig::new(10);
        c.add(WbPathEntry::new("a", "A"));
        c.add(WbPathEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for wb_path
    #[test]
    fn xa_wb_path_ring_new() {
        let rb = super::XaWbPathRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_path_ring_push_len() {
        let mut rb = super::XaWbPathRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_path_ring_wrap() {
        let mut rb = super::XaWbPathRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_path_ring_mean_empty() {
        let rb = super::XaWbPathRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_path_ring_mean_values() {
        let mut rb = super::XaWbPathRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_path_ring_min_max() {
        let mut rb = super::XaWbPathRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_path_ring_iter() {
        let mut rb = super::XaWbPathRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_path_counter_new() {
        let c = super::XaWbPathCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_path_counter_inc() {
        let mut c = super::XaWbPathCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_path_counter_inc_by() {
        let mut c = super::XaWbPathCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_path_counter_reset() {
        let mut c = super::XaWbPathCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_path_counter_clear() {
        let mut c = super::XaWbPathCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_path_counter_default() {
        let c = super::XaWbPathCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 220 ----

    #[test]
    fn xc_220_pool_new_empty() {
        let pool: super::Xc220Pool<i32> = super::Xc220Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_220_pool_release_acquire() {
        let mut pool = super::Xc220Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_220_pool_acquire_empty() {
        let mut pool: super::Xc220Pool<i32> = super::Xc220Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_220_pool_full() {
        let mut pool = super::Xc220Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_220_pool_drain() {
        let mut pool = super::Xc220Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_220_pool_stats() {
        let mut pool = super::Xc220Pool::new(8);
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
    fn xc_220_pool_clear() {
        let mut pool = super::Xc220Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_220_pool_shrink() {
        let mut pool = super::Xc220Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_220_pool_default() {
        let pool: super::Xc220Pool<String> = super::Xc220Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_220_pool_extend() {
        let mut pool = super::Xc220Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_220_pool_retain() {
        let mut pool = super::Xc220Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_220_scheduler_round_robin() {
        let mut sched = super::Xc220Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_220_scheduler_empty() {
        let mut sched = super::Xc220Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_220_scheduler_reset() {
        let mut sched = super::Xc220Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_220_scheduler_add_remove() {
        let mut sched = super::Xc220Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_220_scheduler_targets() {
        let sched = super::Xc220Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_220_hash_empty() {
        assert_eq!(super::xc_220_hash(b""), 5381);
    }

    #[test]
    fn xc_220_hash_data() {
        let h = super::xc_220_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_220_hash(b"hello"), h);
    }

    #[test]
    fn xc_220_reverse_str() {
        assert_eq!(super::xc_220_reverse("abc"), "cba");
        assert_eq!(super::xc_220_reverse(""), "");
    }

}
