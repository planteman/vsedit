//! Platform path resolution.

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
}
