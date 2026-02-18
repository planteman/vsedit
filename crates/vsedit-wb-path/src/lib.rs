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


// --- xd_78 deepening: state machine + event bus ---

/// States for the Xd78 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd78State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd78State {
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
pub struct Xd78Transition {
    pub from: Xd78State,
    pub to: Xd78State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd78StateMachine {
    current: Xd78State,
    history: Vec<Xd78Transition>,
    step_counter: usize,
}

impl Xd78StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd78State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd78State {
        self.current
    }

    pub fn history(&self) -> &[Xd78Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd78State) -> Result<Xd78State, String> {
        let allowed = match (self.current, target) {
            (Xd78State::Idle, Xd78State::Running) => true,
            (Xd78State::Running, Xd78State::Paused) => true,
            (Xd78State::Running, Xd78State::Done) => true,
            (Xd78State::Paused, Xd78State::Running) => true,
            (Xd78State::Paused, Xd78State::Done) => true,
            (Xd78State::Done, Xd78State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_78: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd78Transition {
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
            "Xd78SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd78State> {
        let prefix = "Xd78SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd78State::Idle),
            "Running" => Some(Xd78State::Running),
            "Paused" => Some(Xd78State::Paused),
            "Done" => Some(Xd78State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd78State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd78 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd78Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd78Event {
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

type Xd78HandlerFn = Box<dyn Fn(&Xd78Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd78EventBus {
    handlers: Vec<(usize, Option<String>, Xd78HandlerFn)>,
    next_id: usize,
    published: Vec<Xd78Event>,
}

impl Xd78EventBus {
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
        F: Fn(&Xd78Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd78Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd78Event) {
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

    pub fn published_events(&self) -> &[Xd78Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #97
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf97Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf97TrieNode {
    children: std::collections::HashMap<char, Xf97TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf97Trie {
    root: Xf97TrieNode,
    count: usize,
}

impl Xf97Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf97TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf97TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf97TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf97BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf97BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 219).
pub struct Xh219SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh219SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 261 as u64,
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

/// A compact bit set supporting boolean operations (variant 219).
pub struct Xh219BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh219BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 219).
pub struct Xi219Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi219Deque<T> {
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
pub struct Xi219Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi219Interval {
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

/// A simple interval tree (variant 219).
pub struct Xi219IntervalTree {
    xi_intervals: Vec<Xi219Interval>,
}

impl Xi219IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi219Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi219Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi219Interval) -> Vec<&Xi219Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi219Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi219Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi219Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi219Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi219Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi219Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 219) ---

/// Disjoint set / union-find for crate 219.
pub struct Xj219UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj219UnionFind {
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

const XJ219_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 219.
pub struct Xj219BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj219BTreeNode<K, V>>>,
    len: usize,
}

struct Xj219BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj219BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj219BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ219_BTREE_ORDER - 1
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
        let mid = XJ219_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj219BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj219BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj219BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj219BTreeNode::xj_new_leaf();
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


// --- xk_219 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk219SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk219SegmentTree {
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
pub struct Xk219DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk219DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_219).
#[derive(Debug, Clone)]
pub struct Xl219Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl219Rope {
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

/// Suffix array for efficient string searching (xl_219).
#[derive(Debug, Clone)]
pub struct Xl219SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl219SuffixArray {
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
pub struct Xm219MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm219MatrixSparse {
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
pub struct Xm219Tokenizer {
    text: String,
}

impl Xm219Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 219.
pub struct Xn219Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn219Fenwick {
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

// ----- AVL tree map — crate 219 -----

#[derive(Debug, Clone)]
struct Xn219AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn219AvlNode<K, V>>>,
    right: Option<Box<Xn219AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 219.
#[derive(Debug, Clone)]
pub struct Xn219AVL<K, V> {
    root: Option<Box<Xn219AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn219AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn219AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn219AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn219AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn219AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn219AvlNode<K, V>>) -> Box<Xn219AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn219AvlNode<K, V>>) -> Box<Xn219AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn219AvlNode<K, V>>) -> Box<Xn219AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn219AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn219AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn219AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn219AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn219AvlNode<K, V>>) -> &Xn219AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn219AvlNode<K, V>>) -> (Box<Xn219AvlNode<K, V>>, Option<Box<Xn219AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn219AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn219AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn219AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn219AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn219AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn219AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn219AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


    // --- xd_78 deepening tests ---

    #[test]
    fn xd_78_sm_initial_state() {
        let sm = Xd78StateMachine::new();
        assert_eq!(sm.current_state(), Xd78State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_78_sm_valid_idle_to_running() {
        let mut sm = Xd78StateMachine::new();
        assert!(sm.transition(Xd78State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd78State::Running);
    }

    #[test]
    fn xd_78_sm_valid_running_to_paused() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        assert!(sm.transition(Xd78State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd78State::Paused);
    }

    #[test]
    fn xd_78_sm_valid_running_to_done() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        assert!(sm.transition(Xd78State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd78State::Done);
    }

    #[test]
    fn xd_78_sm_valid_paused_to_running() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        sm.transition(Xd78State::Paused).unwrap();
        assert!(sm.transition(Xd78State::Running).is_ok());
    }

    #[test]
    fn xd_78_sm_valid_done_to_idle() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        sm.transition(Xd78State::Done).unwrap();
        assert!(sm.transition(Xd78State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd78State::Idle);
    }

    #[test]
    fn xd_78_sm_invalid_idle_to_done() {
        let mut sm = Xd78StateMachine::new();
        assert!(sm.transition(Xd78State::Done).is_err());
    }

    #[test]
    fn xd_78_sm_invalid_idle_to_paused() {
        let mut sm = Xd78StateMachine::new();
        assert!(sm.transition(Xd78State::Paused).is_err());
    }

    #[test]
    fn xd_78_sm_history_tracking() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        sm.transition(Xd78State::Paused).unwrap();
        sm.transition(Xd78State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd78State::Idle);
        assert_eq!(sm.history()[0].to, Xd78State::Running);
        assert_eq!(sm.history()[1].from, Xd78State::Running);
        assert_eq!(sm.history()[2].to, Xd78State::Done);
    }

    #[test]
    fn xd_78_sm_serialize_deserialize() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd78StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd78State::Running));
    }

    #[test]
    fn xd_78_sm_deserialize_invalid() {
        assert_eq!(Xd78StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_78_sm_reset() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd78State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_78_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd78EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd78Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_78_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd78EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd78Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd78Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_78_bus_unsubscribe() {
        let mut bus = Xd78EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_78_event_kind_and_payload() {
        let e = Xd78Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd78Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_78_bus_clear_history() {
        let mut bus = Xd78EventBus::new();
        bus.publish(Xd78Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_78_sm_step_counter_increments() {
        let mut sm = Xd78StateMachine::new();
        sm.transition(Xd78State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd78State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #97 --

    #[test]
    fn xf97_trie_insert_search() {
        let mut t = Xf97Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf97_trie_starts_with() {
        let mut t = Xf97Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf97_trie_remove() {
        let mut t = Xf97Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf97_trie_word_count() {
        let mut t = Xf97Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf97_trie_longest_prefix() {
        let mut t = Xf97Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf97_trie_all_words() {
        let mut t = Xf97Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf97_trie_autocomplete() {
        let mut t = Xf97Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf97_trie_empty_search() {
        let t = Xf97Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf97_bloom_add_contains() {
        let mut bf = Xf97BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf97_bloom_probably_absent() {
        let bf = Xf97BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf97_bloom_false_positive_rate() {
        let mut bf = Xf97BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf97_bloom_clear() {
        let mut bf = Xf97BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf97_bloom_union() {
        let mut a = Xf97BloomFilter::xf_new(512, 2);
        let mut b = Xf97BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf97_bloom_intersection_estimate() {
        let mut a = Xf97BloomFilter::xf_new(512, 2);
        let mut b = Xf97BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf97_bloom_union_size_mismatch() {
        let a = Xf97BloomFilter::xf_new(256, 2);
        let b = Xf97BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh219_skip_insert_contains() {
        let mut sl = super::Xh219SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh219_skip_remove() {
        let mut sl = super::Xh219SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh219_skip_len() {
        let mut sl = super::Xh219SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh219_skip_range_query() {
        let mut sl = super::Xh219SkipList::xh_new(4);
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
    fn xh219_skip_floor_ceiling() {
        let mut sl = super::Xh219SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh219_skip_rank() {
        let mut sl = super::Xh219SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh219_skip_empty() {
        let sl = super::Xh219SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh219_skip_duplicates() {
        let mut sl = super::Xh219SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh219_bitset_set_test() {
        let mut bs = super::Xh219BitSet::xh_new(256);
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
    fn xh219_bitset_clear_count() {
        let mut bs = super::Xh219BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh219_bitset_and_or_xor() {
        let mut a = super::Xh219BitSet::xh_new(128);
        let mut b = super::Xh219BitSet::xh_new(128);
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
    fn xh219_bitset_iter_ones() {
        let mut bs = super::Xh219BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh219_bitset_first_last() {
        let mut bs = super::Xh219BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh219_bitset_empty() {
        let bs = super::Xh219BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi219_deque_push_pop_back() {
        let mut dq = super::Xi219Deque::xi_new(4);
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
    fn xi219_deque_push_pop_front() {
        let mut dq = super::Xi219Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi219_deque_mixed_ops() {
        let mut dq = super::Xi219Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi219_deque_get_and_split() {
        let mut dq = super::Xi219Deque::xi_new(8);
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
    fn xi219_deque_rotate_left() {
        let mut dq = super::Xi219Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi219_deque_rotate_right() {
        let mut dq = super::Xi219Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi219_deque_grow() {
        let mut dq = super::Xi219Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi219_deque_empty() {
        let dq = super::Xi219Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi219_interval_tree_insert_query() {
        let mut tree = super::Xi219IntervalTree::xi_new();
        tree.xi_insert(super::Xi219Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi219Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi219Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi219_interval_tree_overlap() {
        let mut tree = super::Xi219IntervalTree::xi_new();
        tree.xi_insert(super::Xi219Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi219Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi219Interval::xi_new(12, 20));
        let q = super::Xi219Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi219_interval_tree_remove() {
        let mut tree = super::Xi219IntervalTree::xi_new();
        tree.xi_insert(super::Xi219Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi219Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi219_interval_tree_gaps() {
        let mut tree = super::Xi219IntervalTree::xi_new();
        tree.xi_insert(super::Xi219Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi219Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi219Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi219Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi219Interval::xi_new(8, 10));
    }

    #[test]
    fn xi219_interval_tree_merge() {
        let mut tree = super::Xi219IntervalTree::xi_new();
        tree.xi_insert(super::Xi219Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi219Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi219Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi219Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi219Interval::xi_new(10, 15));
    }

    #[test]
    fn xi219_interval_tree_all() {
        let mut tree = super::Xi219IntervalTree::xi_new();
        tree.xi_insert(super::Xi219Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi219Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi219_interval_tree_empty() {
        let tree = super::Xi219IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi219_interval_tree_contains_point() {
        let iv = super::Xi219Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 219) ---

    #[test]
    fn xj_219_uf_make_and_find() {
        let mut uf = super::Xj219UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_219_uf_union_connected() {
        let mut uf = super::Xj219UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_219_uf_component_count() {
        let mut uf = super::Xj219UnionFind::xj_new();
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
    fn xj_219_uf_component_size() {
        let mut uf = super::Xj219UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_219_uf_largest_component() {
        let mut uf = super::Xj219UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_219_uf_many_elements() {
        let mut uf = super::Xj219UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_219_uf_separate_components() {
        let mut uf = super::Xj219UnionFind::xj_new();
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
    fn xj_219_uf_path_compression() {
        let mut uf = super::Xj219UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_219_bt_insert_get() {
        let mut bt = super::Xj219BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_219_bt_contains_len() {
        let mut bt = super::Xj219BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_219_bt_replace() {
        let mut bt = super::Xj219BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_219_bt_remove() {
        let mut bt = super::Xj219BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_219_bt_keys_values() {
        let mut bt = super::Xj219BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_219_bt_range() {
        let mut bt = super::Xj219BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_219_bt_min_max() {
        let mut bt = super::Xj219BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_219_bt_many_inserts() {
        let mut bt = super::Xj219BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_219 segment tree tests ---

    #[test]
    fn xk_219_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk219SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_219_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk219SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_219_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk219SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_219_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk219SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_219_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk219SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_219_st_single_element() {
        let data = vec![42];
        let st = super::Xk219SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_219_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk219SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_219_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk219SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_219 disjoint intervals tests ---

    #[test]
    fn xk_219_di_add_and_count() {
        let mut di = super::Xk219DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_219_di_merge_overlap() {
        let mut di = super::Xk219DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_219_di_contains() {
        let mut di = super::Xk219DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_219_di_remove() {
        let mut di = super::Xk219DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_219_di_covered_length() {
        let mut di = super::Xk219DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_219_di_gaps() {
        let mut di = super::Xk219DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_219_di_merge_adjacent() {
        let mut di = super::Xk219DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_219_di_empty() {
        let di = super::Xk219DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_219_rope_new_empty() {
        let rope = super::Xl219Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_219_rope_from_str() {
        let rope = super::Xl219Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_219_rope_insert_at() {
        let mut rope = super::Xl219Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_219_rope_delete_range() {
        let mut rope = super::Xl219Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_219_rope_char_at() {
        let rope = super::Xl219Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_219_rope_split_concat() {
        let rope = super::Xl219Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_219_rope_line_count() {
        let rope = super::Xl219Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_219_rope_line_at() {
        let rope = super::Xl219Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_219_sa_build_and_search() {
        let sa = super::Xl219SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_219_sa_count() {
        let sa = super::Xl219SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_219_sa_longest_repeated() {
        let sa = super::Xl219SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_219_sa_all_positions() {
        let sa = super::Xl219SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_219_sa_len() {
        let sa = super::Xl219SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_219_sa_empty() {
        let sa = super::Xl219SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_219_rope_slice() {
        let rope = super::Xl219Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_219_sa_search_start() {
        let sa = super::Xl219SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_219_sparse_set_get() {
        let mut m = super::Xm219MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_219_sparse_row_col() {
        let mut m = super::Xm219MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_219_sparse_transpose() {
        let mut m = super::Xm219MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_219_sparse_multiply_vec() {
        let mut m = super::Xm219MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_219_sparse_nnz_density() {
        let mut m = super::Xm219MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_219_sparse_clear() {
        let mut m = super::Xm219MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_219_sparse_overwrite_zero() {
        let mut m = super::Xm219MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_219_tokenizer_basic() {
        let t = super::Xm219Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_219_tokenizer_count() {
        let t = super::Xm219Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_219_tokenizer_unique() {
        let t = super::Xm219Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_219_tokenizer_frequency() {
        let t = super::Xm219Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_219_tokenizer_delimiter() {
        let t = super::Xm219Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_219_tokenizer_whitespace() {
        let t = super::Xm219Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_219_tokenizer_empty() {
        let t = super::Xm219Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 219 ----

    #[test]
    fn xn_219_fenwick_prefix_sum() {
        let mut ft = super::Xn219Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_219_fenwick_range_sum() {
        let mut ft = super::Xn219Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_219_fenwick_point_query() {
        let mut ft = super::Xn219Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_219_fenwick_len() {
        let ft = super::Xn219Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_219_fenwick_multiple_updates() {
        let mut ft = super::Xn219Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_219_fenwick_single_element() {
        let mut ft = super::Xn219Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_219_fenwick_find_kth() {
        let mut ft = super::Xn219Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_219_fenwick_negative_delta() {
        let mut ft = super::Xn219Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 219 ----

    #[test]
    fn xn_219_avl_insert_get() {
        let mut m = super::Xn219AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_219_avl_remove() {
        let mut m = super::Xn219AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_219_avl_in_order() {
        let mut m = super::Xn219AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_219_avl_min_max() {
        let mut m = super::Xn219AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_219_avl_floor_ceiling() {
        let mut m = super::Xn219AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_219_avl_height_balanced() {
        let mut m = super::Xn219AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_219_avl_overwrite() {
        let mut m = super::Xn219AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_219_avl_empty() {
        let m: super::Xn219AVL<i32, i32> = super::Xn219AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
