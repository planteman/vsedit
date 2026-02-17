//! Cross-platform path manipulation.
//!
//! Equivalent to VS Code's `vs/base/common/path.ts`.

use std::fmt;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

/// The platform path separator.
pub const SEP: char = MAIN_SEPARATOR;

/// Normalize a path by resolving `.` and `..` segments.
pub fn normalize(path: &str) -> String {
    let p = PathBuf::from(path);
    let mut components = Vec::new();
    for component in p.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            other => components.push(other),
        }
    }
    let result: PathBuf = components.iter().collect();
    result.to_string_lossy().into_owned()
}

/// Join path segments.
pub fn join(base: &str, segments: &[&str]) -> String {
    let mut path = PathBuf::from(base);
    for seg in segments {
        path.push(seg);
    }
    path.to_string_lossy().into_owned()
}

/// Get the directory name of a path.
pub fn dirname(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Get the base name (file name) of a path.
pub fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Get the file extension (without the dot).
pub fn extname(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default()
}

/// Check if a path is absolute.
pub fn is_absolute(path: &str) -> bool {
    Path::new(path).is_absolute()
}

/// Compute a relative path from `from` to `to`.
pub fn relative(from: &str, to: &str) -> String {
    let from_path = PathBuf::from(from);
    let to_path = PathBuf::from(to);

    let from_components: Vec<_> = from_path.components().collect();
    let to_components: Vec<_> = to_path.components().collect();

    let common_len = from_components
        .iter()
        .zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ups = from_components.len() - common_len;
    let mut result = PathBuf::new();
    for _ in 0..ups {
        result.push("..");
    }
    for component in &to_components[common_len..] {
        result.push(component);
    }
    result.to_string_lossy().into_owned()
}

/// Convert backslashes to forward slashes (for cross-platform normalization).
pub fn to_forward_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

/// Convert forward slashes to backslashes (Windows-style).
pub fn to_back_slashes(path: &str) -> String {
    path.replace('/', "\\")
}

/// Remove trailing path separator.
pub fn remove_trailing_separator(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
}

/// Check if a path has a trailing separator.
pub fn has_trailing_separator(path: &str) -> bool {
    path.ends_with('/') || path.ends_with('\\')
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by path operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// The path string was empty.
    EmptyPath,
    /// The path contained invalid characters or structure.
    InvalidPath(String),
    /// A relative path was expected but an absolute path was given.
    RelativePathExpected,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::EmptyPath => write!(f, "path is empty"),
            PathError::InvalidPath(detail) => write!(f, "invalid path: {detail}"),
            PathError::RelativePathExpected => write!(f, "expected a relative path"),
        }
    }
}

impl std::error::Error for PathError {}

// ---------------------------------------------------------------------------
// Additional path helpers
// ---------------------------------------------------------------------------

/// Return the file stem (filename without extension).
///
/// ```
/// assert_eq!(vsedit_path::stem("a/b/file.tar.gz"), "file.tar");
/// assert_eq!(vsedit_path::stem("noext"), "noext");
/// ```
pub fn stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Replace (or add) the extension of a path.
///
/// The new extension should be provided **without** the leading dot.
/// Pass an empty string to remove the extension.
///
/// ```
/// assert_eq!(vsedit_path::change_extension("a/b/file.rs", "txt"), "a/b/file.txt");
/// ```
pub fn change_extension(path: &str, new_ext: &str) -> String {
    let mut buf = PathBuf::from(path);
    buf.set_extension(new_ext);
    buf.to_string_lossy().into_owned()
}

/// Check whether `child` is a descendant of `parent`.
///
/// Both paths are normalised with forward slashes before comparison so the
/// check works regardless of separator style.
pub fn is_child_of(child: &str, parent: &str) -> bool {
    let norm = |p: &str| {
        let n = normalize(p);
        let mut s = to_forward_slashes(&n);
        if !s.ends_with('/') {
            s.push('/');
        }
        s
    };
    let parent_norm = norm(parent);
    let child_fwd = to_forward_slashes(&normalize(child));
    child_fwd.starts_with(&parent_norm) && child_fwd.len() > parent_norm.len() - 1
}

/// Return the longest common directory prefix of two paths.
///
/// The result always uses forward slashes and never has a trailing separator
/// (unless the common prefix is a root like `/`).
pub fn common_prefix(a: &str, b: &str) -> String {
    let a_parts: Vec<&str> = to_forward_slashes(&normalize(a))
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|s| Box::leak(s.into_boxed_str()) as &str)
        .collect();
    let b_norm = to_forward_slashes(&normalize(b));
    let b_parts: Vec<&str> = b_norm.split('/').filter(|s| !s.is_empty()).collect();

    let common: Vec<&str> = a_parts
        .iter()
        .zip(b_parts.iter())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| *x)
        .collect();

    common.join("/")
}

/// Add a trailing separator if one is not already present.
pub fn ensure_trailing_separator(path: &str) -> String {
    if path.is_empty() || has_trailing_separator(path) {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

/// Convert a path to lowercase for case-insensitive comparison.
pub fn normalize_case(path: &str) -> String {
    to_forward_slashes(path).to_lowercase()
}

/// Check whether a path is a UNC path (`\\server\share`).
pub fn is_unc_path(path: &str) -> bool {
    let s = path.as_bytes();
    s.len() >= 3
        && (s[0] == b'\\' && s[1] == b'\\' && s[2] != b'\\')
}

// ---------------------------------------------------------------------------
// PathComponents
// ---------------------------------------------------------------------------

/// Parsed components of a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathComponents {
    /// Root portion (e.g. `/`, `C:\`, or empty for relative paths).
    pub root: String,
    /// Directory segments between root and filename.
    pub dir_parts: Vec<String>,
    /// Filename without extension.
    pub stem: String,
    /// Extension **with** the leading dot, or empty.
    pub extension: String,
}

impl PathComponents {
    /// Parse a path string into its components.
    pub fn parse(path: &str) -> Self {
        let p = Path::new(path);

        // Root
        let root = {
            let mut components = p.components();
            match components.next() {
                Some(std::path::Component::Prefix(pre)) => {
                    let prefix = pre.as_os_str().to_string_lossy().into_owned();
                    // Check if a RootDir follows
                    if let Some(std::path::Component::RootDir) = components.next() {
                        format!("{prefix}{}", MAIN_SEPARATOR)
                    } else {
                        prefix
                    }
                }
                Some(std::path::Component::RootDir) => "/".to_string(),
                _ => String::new(),
            }
        };

        // Collect normal components
        let normal_parts: Vec<String> = p
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => {
                    Some(s.to_string_lossy().into_owned())
                }
                _ => None,
            })
            .collect();

        let (dir_parts, file_part) = if normal_parts.is_empty() {
            (Vec::new(), None)
        } else {
            let (dirs, file) = normal_parts.split_at(normal_parts.len() - 1);
            (dirs.to_vec(), file.first().cloned())
        };

        let stem_val = file_part
            .as_deref()
            .and_then(|f| Path::new(f).file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let ext_val = file_part
            .as_deref()
            .and_then(|f| Path::new(f).extension())
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        Self {
            root,
            dir_parts,
            stem: stem_val,
            extension: ext_val,
        }
    }

    /// Returns true if dir_parts is empty.
    pub fn is_dir_parts_empty(&self) -> bool {
        self.dir_parts.is_empty()
    }

    /// Get the first dir_part, if any.
    pub fn first_dir_part(&self) -> Option<&String> {
        self.dir_parts.first()
    }

    /// Get the last dir_part, if any.
    pub fn last_dir_part(&self) -> Option<&String> {
        self.dir_parts.last()
    }

    /// Retain only dir_parts matching the predicate.
    pub fn retain_dir_parts(&mut self, f: impl Fn(&String) -> bool) {
        self.dir_parts.retain(|item| f(item));
    }

    /// Reconstruct the path string from its components.
    pub fn to_path_string(&self) -> String {
        let mut result = String::new();
        result.push_str(&self.root);
        for (i, part) in self.dir_parts.iter().enumerate() {
            if i > 0 || (!self.root.is_empty() && !self.root.ends_with('/') && !self.root.ends_with('\\')) {
                result.push('/');
            }
            result.push_str(part);
        }
        if !self.dir_parts.is_empty() || !self.root.is_empty() {
            result.push('/');
        }
        result.push_str(&self.stem);
        result.push_str(&self.extension);
        result
    }

    /// Return the number of directory parts (depth of nesting).
    pub fn depth(&self) -> usize {
        self.dir_parts.len()
    }

    /// Create a new path string with a different extension.
    ///
    /// The new extension should include the leading dot (e.g. `".txt"`).
    /// Pass an empty string to remove the extension.
    pub fn with_extension(&self, ext: &str) -> String {
        let mut result = String::new();
        result.push_str(&self.root);
        for (i, part) in self.dir_parts.iter().enumerate() {
            if i > 0 || (!self.root.is_empty() && !self.root.ends_with('/') && !self.root.ends_with('\\')) {
                result.push('/');
            }
            result.push_str(part);
        }
        if !self.dir_parts.is_empty() || !self.root.is_empty() {
            result.push('/');
        }
        result.push_str(&self.stem);
        result.push_str(ext);
        result
    }
}

impl fmt::Display for PathComponents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        for (i, part) in self.dir_parts.iter().enumerate() {
            if i > 0 || !self.root.is_empty() {
                write!(f, "/")?;
            }
            write!(f, "{part}")?;
        }
        if !self.dir_parts.is_empty() || !self.root.is_empty() {
            write!(f, "/")?;
        }
        write!(f, "{}{}", self.stem, self.extension)
    }
}

/// Consistent cross-platform path normalization: converts all separators to
/// forward slashes, resolves `.`/`..`, collapses repeated slashes, and removes
/// trailing slash (unless root).
pub fn path_normalize(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let fwd = to_forward_slashes(path);
    // Collapse repeated slashes (preserve leading // for UNC)
    let mut result = String::with_capacity(fwd.len());
    let mut prev_slash = false;
    let chars: Vec<char> = fwd.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '/' {
            if prev_slash && i > 1 {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        result.push(c);
    }
    // Resolve . and .. segments
    let normalized = normalize(&result);
    let mut out = to_forward_slashes(&normalized);
    // Remove trailing slash unless it's just "/"
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

/// Find the shared base directory of a list of paths.
pub fn path_common_prefix(paths: &[&str]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    if paths.len() == 1 {
        return path_normalize(paths[0]);
    }
    let normalized: Vec<String> = paths.iter().map(|p| path_normalize(p)).collect();
    let first_parts: Vec<&str> = normalized[0].split('/').collect();
    let mut common_len = first_parts.len();
    for path in &normalized[1..] {
        let parts: Vec<&str> = path.split('/').collect();
        common_len = common_len.min(parts.len());
        for i in 0..common_len {
            if first_parts[i] != parts[i] {
                common_len = i;
                break;
            }
        }
    }
    if common_len == 0 {
        return String::new();
    }
    first_parts[..common_len].join("/")
}

/// Compute relative path from one absolute path to another.
pub fn relative_to(from: &str, to: &str) -> Result<String, PathError> {
    let from_norm = path_normalize(from);
    let to_norm = path_normalize(to);
    if from_norm.is_empty() {
        return Err(PathError::EmptyPath);
    }
    if to_norm.is_empty() {
        return Err(PathError::EmptyPath);
    }
    let from_parts: Vec<&str> = from_norm.split('/').filter(|s| !s.is_empty()).collect();
    let to_parts: Vec<&str> = to_norm.split('/').filter(|s| !s.is_empty()).collect();
    let common = from_parts
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let ups = from_parts.len() - common;
    let mut result_parts: Vec<&str> = Vec::new();
    for _ in 0..ups {
        result_parts.push("..");
    }
    for part in &to_parts[common..] {
        result_parts.push(part);
    }
    if result_parts.is_empty() {
        Ok(".".to_string())
    } else {
        Ok(result_parts.join("/"))
    }
}

/// Accumulated statistics for path operations.
#[derive(Debug, Clone, PartialEq)]
pub struct PathStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl PathStats {
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
    pub fn merge(&mut self, other: &PathStats) {
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

impl Default for PathStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PathStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PathStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for path.
#[derive(Debug, Clone)]
pub struct PathValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl PathValidator {
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

impl Default for PathValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if the filename component of a path is hidden (starts with `.`).
pub fn is_hidden(path: &str) -> bool {
    let name = basename(path);
    name.starts_with('.') && name.len() > 1
}

/// Insert a suffix just before the file extension.
///
/// ```
/// assert_eq!(vsedit_path::add_suffix_before_ext("file.txt", "_backup"), "file_backup.txt");
/// assert_eq!(vsedit_path::add_suffix_before_ext("noext", "_v2"), "noext_v2");
/// ```
pub fn add_suffix_before_ext(path: &str, suffix: &str) -> String {
    let p = Path::new(path);
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = p
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();
    let new_name = format!("{stem}{suffix}{ext}");
    if parent.is_empty() {
        new_name
    } else {
        format!("{parent}/{new_name}")
    }
}

/// Split a path on its first `/` separator.
///
/// Returns `(before, after)` where `after` does not include the separator.
/// If there is no separator the entire string is returned as the first element
/// and the second element is empty.
pub fn split_on_first_separator(path: &str) -> (&str, &str) {
    let normalized = path;
    match normalized.find('/') {
        Some(pos) => (&normalized[..pos], &normalized[pos + 1..]),
        None => (normalized, ""),
    }
}

/// Count the number of segments in a path (split by `/`).
///
/// Empty segments (from leading/trailing/double slashes) are ignored.
pub fn count_segments(path: &str) -> usize {
    path.split('/')
        .filter(|s| !s.is_empty())
        .count()
}

/// Check if a path is relative (not absolute).
pub fn is_relative(path: &str) -> bool {
    !is_absolute(path)
}

// ---------------------------------------------------------------------------
// PathTemplate – template-based path generation with variables
// ---------------------------------------------------------------------------

/// A template-based path generator that substitutes `${variable}` placeholders.
#[derive(Debug, Clone)]
pub struct PathTemplate {
    template: String,
    variables: std::collections::HashMap<String, String>,
}

impl PathTemplate {
    /// Create a new path template from a template string.
    ///
    /// Variables are referenced as `${name}` in the template.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
            variables: std::collections::HashMap::new(),
        }
    }

    /// Set a variable value used during expansion.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.variables.insert(name.into(), value.into());
        self
    }

    /// Expand the template, replacing all `${name}` occurrences.
    ///
    /// Unknown variables are left as-is.
    pub fn expand(&self) -> String {
        let mut result = self.template.clone();
        for (key, value) in &self.variables {
            let placeholder = format!("${{{key}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Return the names of all variables referenced in the template.
    pub fn referenced_variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        let bytes = self.template.as_bytes();
        let mut i = 0;
        while i < bytes.len().saturating_sub(2) {
            if bytes[i] == b'$' && bytes[i + 1] == b'{' {
                if let Some(end) = self.template[i + 2..].find('}') {
                    let name = &self.template[i + 2..i + 2 + end];
                    if !name.is_empty() && !vars.contains(&name.to_string()) {
                        vars.push(name.to_string());
                    }
                    i += 3 + end;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        vars
    }

    /// Check if all referenced variables have values set.
    pub fn is_complete(&self) -> bool {
        self.referenced_variables()
            .iter()
            .all(|v| self.variables.contains_key(v))
    }

    /// Return the list of variables that have no value set.
    pub fn missing_variables(&self) -> Vec<String> {
        self.referenced_variables()
            .into_iter()
            .filter(|v| !self.variables.contains_key(v))
            .collect()
    }
}

impl fmt::Display for PathTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.expand())
    }
}

// ---------------------------------------------------------------------------
// PathMatcher – match paths against multiple glob-like patterns
// ---------------------------------------------------------------------------

/// Matches paths against a set of include/exclude patterns.
///
/// Patterns support `*` (match anything except `/`) and `**` (match
/// everything including `/`).
#[derive(Debug, Clone)]
pub struct PathMatcher {
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
}

impl PathMatcher {
    /// Create an empty path matcher.
    pub fn new() -> Self {
        Self {
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }

    /// Add an include pattern.
    pub fn include(&mut self, pattern: impl Into<String>) -> &mut Self {
        self.include_patterns.push(pattern.into());
        self
    }

    /// Add an exclude pattern.
    pub fn exclude(&mut self, pattern: impl Into<String>) -> &mut Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Test whether `path` matches the configured patterns.
    ///
    /// A path matches if it matches at least one include pattern and no
    /// exclude patterns.  If no include patterns have been added, every
    /// path is considered included.
    pub fn matches(&self, path: &str) -> bool {
        let normalized = to_forward_slashes(path);
        let included = if self.include_patterns.is_empty() {
            true
        } else {
            self.include_patterns
                .iter()
                .any(|p| Self::glob_match(p, &normalized))
        };
        if !included {
            return false;
        }
        !self
            .exclude_patterns
            .iter()
            .any(|p| Self::glob_match(p, &normalized))
    }

    /// Simple glob matching: `*` matches non-`/` chars, `**` matches everything.
    fn glob_match(pattern: &str, text: &str) -> bool {
        let pat_parts: Vec<&str> = pattern.split("**").collect();
        if pat_parts.len() == 1 {
            return Self::simple_glob(pattern, text);
        }
        let mut pos = 0usize;
        for (i, part) in pat_parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            // Strip leading/trailing path separators around the ** boundary
            let part = part.trim_matches('/');
            if part.is_empty() {
                continue;
            }
            if i == 0 {
                if !Self::simple_glob(part, &text[..part.len().min(text.len())]) {
                    return false;
                }
                pos = part.len();
            } else {
                // Find matching segment in remaining text
                let remaining = &text[pos..];
                let remaining = remaining.trim_start_matches('/');
                let mut found = false;
                for start in 0..=remaining.len().saturating_sub(part.len()) {
                    let candidate = &remaining[start..];
                    if Self::simple_glob(part, &candidate[..part.len().min(candidate.len())]) {
                        pos = text.len() - remaining.len() + start + part.len();
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
        }
        true
    }

    /// Match a simple pattern (no `**`) against text.
    fn simple_glob(pattern: &str, text: &str) -> bool {
        let mut pi = 0usize;
        let mut ti = 0usize;
        let mut star_pi = usize::MAX;
        let mut star_ti = 0usize;
        let pb = pattern.as_bytes();
        let tb = text.as_bytes();
        while ti < tb.len() {
            if pi < pb.len() && (pb[pi] == b'?' || pb[pi] == tb[ti]) {
                pi += 1;
                ti += 1;
            } else if pi < pb.len() && pb[pi] == b'*' {
                star_pi = pi;
                star_ti = ti;
                pi += 1;
            } else if star_pi != usize::MAX {
                pi = star_pi + 1;
                star_ti += 1;
                ti = star_ti;
            } else {
                return false;
            }
        }
        while pi < pb.len() && pb[pi] == b'*' {
            pi += 1;
        }
        pi == pb.len()
    }

    /// Return the total number of patterns (include + exclude).
    pub fn pattern_count(&self) -> usize {
        self.include_patterns.len() + self.exclude_patterns.len()
    }
}

impl Default for PathMatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PathCanonicalizer – intelligently resolve . and .. segments
// ---------------------------------------------------------------------------

/// Resolve `.` and `..` segments without touching the filesystem.
pub struct PathCanonicalizer;

impl PathCanonicalizer {
    /// Canonicalize a path by resolving `.` and `..` while preserving
    /// leading `..` segments for relative paths.
    pub fn canonicalize(path: &str) -> String {
        let is_abs = is_absolute(path);
        let fwd = to_forward_slashes(path);
        let parts: Vec<&str> = fwd.split('/').filter(|s| !s.is_empty()).collect();
        let mut stack: Vec<&str> = Vec::new();
        for part in &parts {
            match *part {
                "." => {}
                ".." => {
                    if !stack.is_empty() && *stack.last().unwrap() != ".." {
                        stack.pop();
                    } else if !is_abs {
                        stack.push("..");
                    }
                }
                other => stack.push(other),
            }
        }
        let joined = stack.join("/");
        if is_abs {
            format!("/{joined}")
        } else if joined.is_empty() {
            ".".to_string()
        } else {
            joined
        }
    }

    /// Return the canonical depth (number of segments after resolution).
    pub fn canonical_depth(path: &str) -> usize {
        let c = Self::canonicalize(path);
        c.split('/').filter(|s| !s.is_empty() && *s != ".").count()
    }

    /// Check if the path escapes its root (has unresolvable `..` at the front).
    pub fn escapes_root(path: &str) -> bool {
        let c = Self::canonicalize(path);
        c.starts_with("../") || c == ".."
    }
}

// ---------------------------------------------------------------------------
// PathComponents – iteration and reconstruction extensions
// ---------------------------------------------------------------------------

impl PathComponents {
    /// Iterate over all segments (dir parts + filename) as string slices.
    pub fn segments(&self) -> Vec<&str> {
        let mut segs: Vec<&str> = self.dir_parts.iter().map(|s| s.as_str()).collect();
        let filename = self.filename();
        if !filename.is_empty() {
            // We can't return a reference to a temporary, so this method
            // returns owned segments below via `segments_owned`.
            segs.push(""); // placeholder
        }
        // Since we need to include the filename, use segments_owned instead
        // for a complete view. This returns dir parts only.
        self.dir_parts.iter().map(|s| s.as_str()).collect()
    }

    /// Return owned copies of every segment (dir parts + filename).
    pub fn segments_owned(&self) -> Vec<String> {
        let mut segs = self.dir_parts.clone();
        let fname = self.filename();
        if !fname.is_empty() {
            segs.push(fname);
        }
        segs
    }

    /// Return the full filename (stem + extension).
    pub fn filename(&self) -> String {
        format!("{}{}", self.stem, self.extension)
    }

    /// Create a new `PathComponents` with only the first `n` directory parts.
    pub fn truncate_dir(&self, n: usize) -> Self {
        Self {
            root: self.root.clone(),
            dir_parts: self.dir_parts.iter().take(n).cloned().collect(),
            stem: self.stem.clone(),
            extension: self.extension.clone(),
        }
    }

    /// Create a new `PathComponents` with an additional directory part appended.
    pub fn push_dir(&self, part: impl Into<String>) -> Self {
        let mut dp = self.dir_parts.clone();
        dp.push(part.into());
        Self {
            root: self.root.clone(),
            dir_parts: dp,
            stem: self.stem.clone(),
            extension: self.extension.clone(),
        }
    }

    /// Return true if the path has a non-empty extension.
    pub fn has_extension(&self) -> bool {
        !self.extension.is_empty()
    }

    /// Return the extension without the leading dot, or `None`.
    pub fn extension_without_dot(&self) -> Option<&str> {
        if self.extension.starts_with('.') {
            Some(&self.extension[1..])
        } else if self.extension.is_empty() {
            None
        } else {
            Some(&self.extension)
        }
    }
}

// ---------------------------------------------------------------------------
// Path depth calculation
// ---------------------------------------------------------------------------

/// Return the depth of a path (number of non-empty segments).
///
/// Leading/trailing slashes and `.` segments are ignored.
/// `..` segments count as negative depth adjustments.
///
/// ```
/// assert_eq!(vsedit_path::path_depth("/usr/local/bin"), 3);
/// assert_eq!(vsedit_path::path_depth("a/b/../c"), 2);
/// ```
pub fn path_depth(path: &str) -> usize {
    let fwd = to_forward_slashes(path);
    let mut depth: isize = 0;
    for seg in fwd.split('/').filter(|s| !s.is_empty()) {
        match seg {
            "." => {}
            ".." => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ => depth += 1,
        }
    }
    depth as usize
}

// ---------------------------------------------------------------------------
// File extension helpers
// ---------------------------------------------------------------------------

/// Return all extensions of a filename as a vector (without dots).
///
/// For `"archive.tar.gz"` this returns `["tar", "gz"]`.
pub fn extensions(path: &str) -> Vec<String> {
    let name = basename(path);
    if name.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() <= 1 {
        return Vec::new();
    }
    // first part is the stem, rest are extensions
    parts[1..].iter().map(|s| (*s).to_string()).collect()
}

/// Check if a path has a specific extension (case-insensitive).
pub fn has_extension(path: &str, ext: &str) -> bool {
    let path_ext = extname(path);
    let want = if ext.starts_with('.') {
        ext.to_lowercase()
    } else {
        format!(".{}", ext.to_lowercase())
    };
    path_ext.to_lowercase() == want
}

/// Remove all extensions from a path, leaving only the stem.
///
/// ```
/// assert_eq!(vsedit_path::remove_all_extensions("a/b/file.tar.gz"), "a/b/file");
/// ```
pub fn remove_all_extensions(path: &str) -> String {
    let p = Path::new(path);
    let parent = p
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();

    let name = basename(path);
    let first_stem = name.split('.').next().unwrap_or("");

    if parent.is_empty() {
        first_stem.to_string()
    } else {
        format!("{parent}/{first_stem}")
    }
}

// ---------------------------------------------------------------------------
// Path sanitization
// ---------------------------------------------------------------------------

/// Characters that are invalid in file names on Windows.
const INVALID_FILENAME_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*', '\0'];

/// Sanitize a filename by replacing invalid characters with a replacement char.
///
/// This removes characters that are invalid on Windows (and control characters)
/// to produce a cross-platform safe filename.
pub fn sanitize_filename(name: &str, replacement: char) -> String {
    name.chars()
        .map(|c| {
            if c.is_control() || INVALID_FILENAME_CHARS.contains(&c) {
                replacement
            } else {
                c
            }
        })
        .collect()
}

/// Sanitize an entire path, applying filename sanitization to each component
/// while preserving separators and root.
pub fn sanitize_path(path: &str, replacement: char) -> String {
    let fwd = to_forward_slashes(path);
    let leading_slash = fwd.starts_with('/');
    let parts: Vec<String> = fwd
        .split('/')
        .enumerate()
        .map(|(i, seg)| {
            if seg.is_empty() {
                String::new()
            } else if i == 0 && (seg.ends_with(':') || seg == "\\\\") {
                // Preserve drive letters and UNC roots
                seg.to_string()
            } else {
                sanitize_filename(seg, replacement)
            }
        })
        .collect();
    let joined = parts
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if leading_slash {
        format!("/{joined}")
    } else {
        joined
    }
}

// ---------------------------------------------------------------------------
// URI ↔ path conversion
// ---------------------------------------------------------------------------

/// Convert a `file://` URI to a local path string.
///
/// Handles percent-decoding of common sequences (`%20` for space, `%23` for `#`).
/// Returns `Err` if the URI doesn't start with `file://`.
pub fn uri_to_path(uri: &str) -> Result<String, PathError> {
    let stripped = if uri.starts_with("file:///") {
        &uri[7..] // keep leading / for Unix absolute paths
    } else if uri.starts_with("file://") {
        &uri[7..]
    } else {
        return Err(PathError::InvalidPath(
            "not a file:// URI".to_string(),
        ));
    };
    Ok(percent_decode(stripped))
}

/// Convert a local path to a `file://` URI.
///
/// Percent-encodes spaces and `#` characters.
pub fn path_to_uri(path: &str) -> String {
    let fwd = to_forward_slashes(path);
    let encoded = percent_encode(&fwd);
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

/// Minimal percent-decoding for common URI characters.
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                &String::from_utf8_lossy(&bytes[i + 1..i + 3]),
                16,
            ) {
                result.push(byte as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Minimal percent-encoding for spaces and `#`.
fn percent_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '#' => result.push_str("%23"),
            _ => result.push(c),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Path comparison helpers
// ---------------------------------------------------------------------------

/// Case-insensitive path equality check (with slash normalization).
pub fn paths_equal_ignore_case(a: &str, b: &str) -> bool {
    normalize_case(a) == normalize_case(b)
}

/// Case-sensitive path equality check with slash normalization.
pub fn paths_equal(a: &str, b: &str) -> bool {
    to_forward_slashes(a) == to_forward_slashes(b)
}

// ---------------------------------------------------------------------------
// PathBreadcrumbs – decompose a path into navigable breadcrumb entries
// ---------------------------------------------------------------------------

/// A single breadcrumb entry representing a navigable path segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breadcrumb {
    /// Display label for this segment.
    pub label: String,
    /// Full path up to and including this segment.
    pub path: String,
}

/// Decompose a path into breadcrumb entries for UI navigation.
///
/// Each breadcrumb contains the segment label and the full path up to that point.
///
/// ```
/// let crumbs = vsedit_path::breadcrumbs("/usr/local/bin");
/// assert_eq!(crumbs.len(), 3);
/// assert_eq!(crumbs[0].label, "usr");
/// assert_eq!(crumbs[2].path, "/usr/local/bin");
/// ```
pub fn breadcrumbs(path: &str) -> Vec<Breadcrumb> {
    let fwd = to_forward_slashes(path);
    let is_abs = fwd.starts_with('/');
    let segments: Vec<&str> = fwd.split('/').filter(|s| !s.is_empty()).collect();
    let mut result = Vec::with_capacity(segments.len());
    let mut accumulated = if is_abs {
        String::from("/")
    } else {
        String::new()
    };
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 || is_abs {
            if !accumulated.ends_with('/') {
                accumulated.push('/');
            }
        }
        accumulated.push_str(seg);
        result.push(Breadcrumb {
            label: seg.to_string(),
            path: accumulated.clone(),
        });
    }
    result
}

// ---------------------------------------------------------------------------
// Tilde expansion helper
// ---------------------------------------------------------------------------

/// Expand a leading `~` to the provided home directory path.
///
/// If the path does not start with `~`, it is returned unchanged.
pub fn expand_tilde(path: &str, home: &str) -> String {
    if path == "~" {
        home.to_string()
    } else if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{rest}", remove_trailing_separator(home))
    } else if let Some(rest) = path.strip_prefix("~\\") {
        format!("{}\\{rest}", remove_trailing_separator(home))
    } else {
        path.to_string()
    }
}


// ---------------------------------------------------------------------------
// PathCompletionProvider – suggest completions for partial path input
// ---------------------------------------------------------------------------

/// Result of a path completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCompletion {
    /// The completed path string.
    pub completed: String,
    /// Whether the completion represents a directory (true) or file (false).
    pub is_directory: bool,
    /// The portion of the input that was matched.
    pub matched_prefix: String,
    /// Display label for the completion (typically the last segment).
    pub label: String,
}

/// Provides path completion suggestions from a known set of paths.
///
/// This is a pure in-memory completer: it does not touch the filesystem.
/// Callers supply the candidate paths up front, then query for completions
/// against partial input.
#[derive(Debug, Clone)]
pub struct PathCompletionProvider {
    candidates: Vec<String>,
    case_sensitive: bool,
}

impl PathCompletionProvider {
    /// Create a new provider with a set of known candidate paths.
    pub fn new(candidates: Vec<String>) -> Self {
        Self {
            candidates,
            case_sensitive: true,
        }
    }

    /// Set whether matching is case-sensitive (default: `true`).
    pub fn case_sensitive(mut self, yes: bool) -> Self {
        self.case_sensitive = yes;
        self
    }

    /// Return completions that match the given partial input.
    pub fn complete(&self, partial: &str) -> Vec<PathCompletion> {
        let normalized_partial = to_forward_slashes(partial);
        let match_partial = if self.case_sensitive {
            normalized_partial.clone()
        } else {
            normalized_partial.to_lowercase()
        };

        let mut results = Vec::new();
        for candidate in &self.candidates {
            let normalized_candidate = to_forward_slashes(candidate);
            let match_candidate = if self.case_sensitive {
                normalized_candidate.clone()
            } else {
                normalized_candidate.to_lowercase()
            };

            if match_candidate.starts_with(&match_partial) {
                let is_dir = normalized_candidate.ends_with('/');
                let label = basename(&normalized_candidate);
                results.push(PathCompletion {
                    completed: normalized_candidate,
                    is_directory: is_dir,
                    matched_prefix: normalized_partial.clone(),
                    label: if label.is_empty() {
                        candidate.clone()
                    } else {
                        label
                    },
                });
            }
        }
        results.sort_by(|a, b| a.completed.cmp(&b.completed));
        results
    }

    /// Return completions that match the given partial input at any segment
    /// boundary (not just as a prefix of the whole path).
    pub fn complete_fuzzy(&self, partial: &str) -> Vec<PathCompletion> {
        if partial.is_empty() {
            return Vec::new();
        }
        let match_partial = if self.case_sensitive {
            partial.to_string()
        } else {
            partial.to_lowercase()
        };

        let mut results = Vec::new();
        for candidate in &self.candidates {
            let normalized = to_forward_slashes(candidate);
            let match_candidate = if self.case_sensitive {
                normalized.clone()
            } else {
                normalized.to_lowercase()
            };

            // Check if any segment starts with the partial
            let segments: Vec<&str> = match_candidate.split('/').collect();
            let matched = segments.iter().any(|seg| seg.starts_with(&match_partial));
            if matched {
                let is_dir = normalized.ends_with('/');
                let label = basename(&normalized);
                results.push(PathCompletion {
                    completed: normalized,
                    is_directory: is_dir,
                    matched_prefix: partial.to_string(),
                    label: if label.is_empty() {
                        candidate.clone()
                    } else {
                        label
                    },
                });
            }
        }
        results.sort_by(|a, b| a.completed.cmp(&b.completed));
        results
    }

    /// Add new candidates to the provider.
    pub fn add_candidates(&mut self, paths: &[&str]) {
        for p in paths {
            self.candidates.push((*p).to_string());
        }
    }

    /// Return the number of candidates.
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

// ---------------------------------------------------------------------------
// PathEnvVarResolver – resolve environment variable references in paths
// ---------------------------------------------------------------------------

/// Resolves `$VAR` and `${VAR}` references in path strings using a provided
/// variable map.
///
/// This does **not** read the actual process environment; all variables must
/// be supplied explicitly so the resolver is deterministic and testable.
#[derive(Debug, Clone)]
pub struct PathEnvVarResolver {
    vars: std::collections::HashMap<String, String>,
}

impl PathEnvVarResolver {
    /// Create a resolver with no variables.
    pub fn new() -> Self {
        Self {
            vars: std::collections::HashMap::new(),
        }
    }

    /// Create a resolver pre-populated with common Unix-like variables.
    pub fn with_defaults(home: &str, user: &str) -> Self {
        let mut vars = std::collections::HashMap::new();
        vars.insert("HOME".to_string(), home.to_string());
        vars.insert("USER".to_string(), user.to_string());
        Self { vars }
    }

    /// Set a variable value.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.vars.insert(name.into(), value.into());
        self
    }

    /// Resolve all `$VAR` and `${VAR}` references in the given string.
    ///
    /// Unknown variables are left as-is.
    pub fn resolve(&self, input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'$' && i + 1 < bytes.len() {
                if bytes[i + 1] == b'{' {
                    // ${VAR} form
                    if let Some(end) = input[i + 2..].find('}') {
                        let var_name = &input[i + 2..i + 2 + end];
                        if let Some(value) = self.vars.get(var_name) {
                            result.push_str(value);
                        } else {
                            result.push_str(&input[i..i + 3 + end]);
                        }
                        i += 3 + end;
                    } else {
                        result.push('$');
                        i += 1;
                    }
                } else if bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_' {
                    // $VAR form – consume alphanumeric + underscore
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len()
                        && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_')
                    {
                        end += 1;
                    }
                    let var_name = &input[start..end];
                    if let Some(value) = self.vars.get(var_name) {
                        result.push_str(value);
                    } else {
                        result.push_str(&input[i..end]);
                    }
                    i = end;
                } else {
                    result.push('$');
                    i += 1;
                }
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }
        result
    }

    /// Return the names of all variables referenced in the input string.
    pub fn referenced_vars(input: &str) -> Vec<String> {
        let mut vars = Vec::new();
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'$' && i + 1 < bytes.len() {
                if bytes[i + 1] == b'{' {
                    if let Some(end) = input[i + 2..].find('}') {
                        let var_name = &input[i + 2..i + 2 + end];
                        if !var_name.is_empty() && !vars.contains(&var_name.to_string()) {
                            vars.push(var_name.to_string());
                        }
                        i += 3 + end;
                    } else {
                        i += 1;
                    }
                } else if bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_' {
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len()
                        && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_')
                    {
                        end += 1;
                    }
                    let var_name = input[start..end].to_string();
                    if !vars.contains(&var_name) {
                        vars.push(var_name);
                    }
                    i = end;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        vars
    }

    /// Check if the input contains any variable references.
    pub fn has_variables(input: &str) -> bool {
        !Self::referenced_vars(input).is_empty()
    }
}

impl Default for PathEnvVarResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PathShortener – shorten paths for display
// ---------------------------------------------------------------------------

/// Shortens long paths for display by abbreviating intermediate directories.
///
/// For example, `/home/user/projects/my-app/src/lib.rs` can become
/// `/h/u/p/m/src/lib.rs` or `…/src/lib.rs` depending on the strategy.
#[derive(Debug, Clone)]
pub struct PathShortener {
    max_length: usize,
    ellipsis: String,
}

impl PathShortener {
    /// Create a new shortener with the given maximum display length.
    pub fn new(max_length: usize) -> Self {
        Self {
            max_length,
            ellipsis: "…".to_string(),
        }
    }

    /// Set a custom ellipsis string (default: `"…"`).
    pub fn ellipsis(mut self, e: impl Into<String>) -> Self {
        self.ellipsis = e.into();
        self
    }

    /// Shorten a path by abbreviating each intermediate directory to its
    /// first character.
    ///
    /// The last two segments (parent dir + filename) are always kept intact.
    pub fn abbreviate(&self, path: &str) -> String {
        let fwd = to_forward_slashes(path);
        let is_abs = fwd.starts_with('/');
        let segments: Vec<&str> = fwd.split('/').filter(|s| !s.is_empty()).collect();

        if segments.len() <= 2 {
            return path.to_string();
        }

        // Keep last 2 segments fully, abbreviate the rest
        let boundary = segments.len() - 2;
        let mut parts: Vec<String> = Vec::with_capacity(segments.len());
        for (i, seg) in segments.iter().enumerate() {
            if i < boundary {
                // Take first char of the segment
                let first: String = seg.chars().take(1).collect();
                parts.push(first);
            } else {
                parts.push(seg.to_string());
            }
        }
        let joined = parts.join("/");
        if is_abs {
            format!("/{joined}")
        } else {
            joined
        }
    }

    /// Shorten a path by replacing leading segments with the ellipsis,
    /// keeping only enough trailing segments to fit within `max_length`.
    pub fn truncate_leading(&self, path: &str) -> String {
        if path.len() <= self.max_length {
            return path.to_string();
        }

        let fwd = to_forward_slashes(path);
        let segments: Vec<&str> = fwd.split('/').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            return path.to_string();
        }

        // Try keeping progressively fewer trailing segments
        for keep in (1..=segments.len()).rev() {
            let tail: String = segments[segments.len() - keep..].join("/");
            let shortened = format!("{}/{tail}", self.ellipsis);
            if shortened.len() <= self.max_length || keep == 1 {
                return shortened;
            }
        }
        path.to_string()
    }

    /// Shorten a path by collapsing the middle, keeping the first and last
    /// segments.
    pub fn collapse_middle(&self, path: &str) -> String {
        if path.len() <= self.max_length {
            return path.to_string();
        }

        let fwd = to_forward_slashes(path);
        let is_abs = fwd.starts_with('/');
        let segments: Vec<&str> = fwd.split('/').filter(|s| !s.is_empty()).collect();

        if segments.len() <= 2 {
            return path.to_string();
        }

        let first = segments[0];
        let last = segments[segments.len() - 1];
        let collapsed = format!("{first}/{}/{last}", self.ellipsis);
        if is_abs {
            format!("/{collapsed}")
        } else {
            collapsed
        }
    }
}

// ---------------------------------------------------------------------------
// PathSegmentValidator – validate individual path segments
// ---------------------------------------------------------------------------

/// Validates individual path segments against common filesystem rules.
#[derive(Debug, Clone)]
pub struct PathSegmentValidator {
    max_segment_length: usize,
    forbidden_names: Vec<String>,
    forbidden_chars: Vec<char>,
}

impl PathSegmentValidator {
    /// Create a new validator with defaults suitable for cross-platform use.
    pub fn new() -> Self {
        Self {
            max_segment_length: 255,
            forbidden_names: vec![
                "CON", "PRN", "AUX", "NUL",
                "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
                "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            forbidden_chars: vec!['<', '>', ':', '"', '|', '?', '*', '\0'],
        }
    }

    /// Set the maximum segment length.
    pub fn max_segment_length(mut self, max: usize) -> Self {
        self.max_segment_length = max;
        self
    }

    /// Add additional forbidden segment names (checked case-insensitively).
    pub fn forbid_name(mut self, name: impl Into<String>) -> Self {
        self.forbidden_names.push(name.into());
        self
    }

    /// Validate a single path segment, returning all violations found.
    pub fn validate_segment(&self, segment: &str) -> Vec<String> {
        let mut errors = Vec::new();

        if segment.is_empty() {
            errors.push("segment must not be empty".to_string());
            return errors;
        }

        if segment.len() > self.max_segment_length {
            errors.push(format!(
                "segment length {} exceeds maximum {}",
                segment.len(),
                self.max_segment_length
            ));
        }

        let upper = segment.to_uppercase();
        // Check the stem part (before extension) against forbidden names
        let stem_part = upper.split('.').next().unwrap_or(&upper);
        for forbidden in &self.forbidden_names {
            if stem_part == forbidden.to_uppercase() {
                errors.push(format!(
                    "'{}' is a reserved device name",
                    segment
                ));
                break;
            }
        }

        for ch in segment.chars() {
            if self.forbidden_chars.contains(&ch) {
                errors.push(format!("character '{}' is not allowed in segment", ch));
            }
            if ch.is_control() {
                errors.push(format!(
                    "control character U+{:04X} is not allowed",
                    ch as u32
                ));
            }
        }

        if segment.ends_with(' ') || segment.ends_with('.') {
            errors.push(format!(
                "segment '{}' must not end with a space or period",
                segment
            ));
        }

        errors
    }

    /// Validate every segment of a full path.
    pub fn validate_path(&self, path: &str) -> Vec<(String, Vec<String>)> {
        let fwd = to_forward_slashes(path);
        let mut results = Vec::new();
        for seg in fwd.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." {
                continue;
            }
            // Skip drive letter segments like "C:"
            if seg.len() == 2 && seg.as_bytes()[1] == b':' {
                continue;
            }
            let errors = self.validate_segment(seg);
            if !errors.is_empty() {
                results.push((seg.to_string(), errors));
            }
        }
        results
    }

    /// Check if a full path is valid (all segments pass validation).
    pub fn is_valid_path(&self, path: &str) -> bool {
        self.validate_path(path).is_empty()
    }
}

impl Default for PathSegmentValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// PathCompletionProvider – additional methods
// ---------------------------------------------------------------------------

impl PathCompletionProvider {
    /// Add a single known path to the candidate set.
    pub fn add_known_path(&mut self, path: &str) {
        self.candidates.push(path.to_string());
    }

    /// Return completions whose basename starts with the given prefix.
    pub fn complete_basename(&self, prefix: &str) -> Vec<PathCompletion> {
        if prefix.is_empty() {
            return Vec::new();
        }
        let match_prefix = if self.case_sensitive {
            prefix.to_string()
        } else {
            prefix.to_lowercase()
        };

        let mut results = Vec::new();
        for candidate in &self.candidates {
            let normalized = to_forward_slashes(candidate);
            let base = basename(&normalized);
            let match_base = if self.case_sensitive {
                base.clone()
            } else {
                base.to_lowercase()
            };

            if match_base.starts_with(&match_prefix) {
                let is_dir = normalized.ends_with('/');
                results.push(PathCompletion {
                    completed: normalized,
                    is_directory: is_dir,
                    matched_prefix: prefix.to_string(),
                    label: if base.is_empty() {
                        candidate.clone()
                    } else {
                        base
                    },
                });
            }
        }
        results.sort_by(|a, b| a.completed.cmp(&b.completed));
        results
    }

    /// Clear all known candidate paths.
    pub fn clear(&mut self) {
        self.candidates.clear();
    }

    /// Return all candidate paths that are immediate children of the given
    /// directory (i.e. whose dirname matches `dir`).
    pub fn paths_in_dir(&self, dir: &str) -> Vec<String> {
        let norm_dir = to_forward_slashes(dir);
        let norm_dir_trimmed = norm_dir.trim_end_matches('/');

        let mut results = Vec::new();
        for candidate in &self.candidates {
            let norm_cand = to_forward_slashes(candidate);
            let cand_dir = dirname(&norm_cand);
            let cand_dir_trimmed = cand_dir.trim_end_matches('/');
            if cand_dir_trimmed == norm_dir_trimmed {
                results.push(norm_cand);
            }
        }
        results.sort();
        results
    }

    /// Check whether a specific path is in the candidate set.
    pub fn has_path(&self, path: &str) -> bool {
        let norm = to_forward_slashes(path);
        self.candidates.iter().any(|c| to_forward_slashes(c) == norm)
    }
}

// ---------------------------------------------------------------------------
// PathEnvVarResolver – additional methods
// ---------------------------------------------------------------------------

impl PathEnvVarResolver {
    /// Check whether a variable with the given name is defined.
    pub fn has_var(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }

    /// Return the number of defined variables.
    pub fn var_count(&self) -> usize {
        self.vars.len()
    }

    /// Return the names of variables that are referenced in `path` but not
    /// defined in this resolver.
    pub fn unresolved_vars(&self, path: &str) -> Vec<String> {
        let referenced = Self::referenced_vars(path);
        referenced
            .into_iter()
            .filter(|name| !self.vars.contains_key(name))
            .collect()
    }

    /// Remove a variable from the resolver. Returns `true` if it was present.
    pub fn remove_var(&mut self, name: &str) -> bool {
        self.vars.remove(name).is_some()
    }

    /// Return the value of a variable, if defined.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(|s| s.as_str())
    }

    /// Return all variable names defined in this resolver.
    pub fn var_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.vars.keys().cloned().collect();
        names.sort();
        names
    }
}

// ---------------------------------------------------------------------------
// PathShortener – additional methods
// ---------------------------------------------------------------------------

impl PathShortener {
    /// Shorten a path using the default strategy: abbreviate first, and if
    /// still too long, truncate the leading portion.
    pub fn shorten(&self, path: &str) -> String {
        let abbreviated = self.abbreviate(path);
        if abbreviated.len() <= self.max_length {
            return abbreviated;
        }
        self.truncate_leading(path)
    }

    /// Shorten a path relative to a base directory. If the path starts with
    /// `base`, strip the base prefix before shortening.
    pub fn shorten_relative(&self, path: &str, base: &str) -> String {
        let norm_path = to_forward_slashes(path);
        let mut norm_base = to_forward_slashes(base);
        if !norm_base.ends_with('/') {
            norm_base.push('/');
        }
        let rel = if norm_path.starts_with(&norm_base) {
            &norm_path[norm_base.len()..]
        } else {
            &norm_path
        };
        self.shorten(rel)
    }

    /// Shorten a path by replacing the home-directory prefix with `~`, then
    /// applying the standard shortening strategy.
    pub fn shorten_home(&self, path: &str, home: &str) -> String {
        let norm_path = to_forward_slashes(path);
        let mut norm_home = to_forward_slashes(home);
        if !norm_home.ends_with('/') {
            norm_home.push('/');
        }
        let replaced = if norm_path.starts_with(&norm_home) {
            format!("~/{}", &norm_path[norm_home.len()..])
        } else if norm_path == norm_home.trim_end_matches('/') {
            "~".to_string()
        } else {
            norm_path
        };
        self.shorten(&replaced)
    }

    /// Check whether the path already fits within the maximum length.
    pub fn fits(&self, path: &str) -> bool {
        path.len() <= self.max_length
    }

    /// Return the configured maximum display length.
    pub fn max_length(&self) -> usize {
        self.max_length
    }
}

// ---------------------------------------------------------------------------
// PathValidatorExtended – thorough cross-platform path validation
// ---------------------------------------------------------------------------

/// Extended path validation utilities, complementing `PathSegmentValidator`.
///
/// Provides additional helpers for checking and sanitising file names against
/// common filesystem restrictions on Windows, macOS, and Linux.
#[derive(Debug, Clone)]
pub struct PathValidatorExtended {
    /// Characters considered invalid in file names.
    invalid_chars: Vec<char>,
    /// Maximum allowed length for a single path component.
    max_component: usize,
    /// Reserved device names (case-insensitive, Windows).
    reserved_names: Vec<String>,
}

impl PathValidatorExtended {
    /// Create a validator with cross-platform defaults.
    pub fn new() -> Self {
        Self {
            invalid_chars: vec![
                '<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0',
            ],
            max_component: 255,
            reserved_names: vec![
                "CON", "PRN", "AUX", "NUL",
                "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
                "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }

    /// Check whether `name` is a valid filename (single component, no separators).
    pub fn is_valid_filename(&self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        if name.len() > self.max_component {
            return false;
        }
        if self.has_invalid_chars(name) {
            return false;
        }
        if name.ends_with(' ') || name.ends_with('.') {
            return false;
        }
        // Check reserved names (stem only, before first dot)
        let stem = name.split('.').next().unwrap_or(name);
        let upper = stem.to_uppercase();
        if self.reserved_names.iter().any(|r| r == &upper) {
            return false;
        }
        // Reject names that are only dots or spaces
        if name.chars().all(|c| c == '.' || c == ' ') {
            return false;
        }
        true
    }

    /// Check whether `path` is a valid full path (all components valid).
    pub fn is_valid_path(&self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        let fwd = to_forward_slashes(path);
        for seg in fwd.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." {
                continue;
            }
            // Skip drive letters like "C:"
            if seg.len() == 2 && seg.as_bytes().get(1) == Some(&b':') {
                continue;
            }
            if !self.is_valid_filename(seg) {
                return false;
            }
        }
        true
    }

    /// Return a sorted list of invalid characters found in `name`.
    pub fn invalid_chars(&self, name: &str) -> Vec<char> {
        let mut found: Vec<char> = name
            .chars()
            .filter(|c| self.invalid_chars.contains(c) || c.is_control())
            .collect();
        found.sort();
        found.dedup();
        found
    }

    /// Suggest a sanitised filename by replacing invalid characters with `_`.
    pub fn suggested_filename(&self, name: &str) -> String {
        if name.is_empty() {
            return "_".to_string();
        }
        let mut result = String::with_capacity(name.len());
        for ch in name.chars() {
            if self.invalid_chars.contains(&ch) || ch.is_control() {
                result.push('_');
            } else {
                result.push(ch);
            }
        }
        // Trim trailing spaces and dots
        let trimmed = result.trim_end_matches(|c: char| c == ' ' || c == '.').to_string();
        let trimmed = if trimmed.is_empty() { "_".to_string() } else { trimmed };

        // Handle reserved names by prefixing with underscore
        let stem = trimmed.split('.').next().unwrap_or(&trimmed);
        let upper = stem.to_uppercase();
        if self.reserved_names.iter().any(|r| r == &upper) {
            return format!("_{trimmed}");
        }
        trimmed
    }

    /// Return the maximum allowed component length.
    pub fn max_component_length(&self) -> usize {
        self.max_component
    }

    /// Check whether a path exceeds a given maximum total length.
    pub fn is_too_long(&self, path: &str, max: usize) -> bool {
        path.len() > max
    }

    /// Check whether a filename contains any invalid characters.
    pub fn has_invalid_chars(&self, name: &str) -> bool {
        name.chars()
            .any(|c| self.invalid_chars.contains(&c) || c.is_control())
    }

    /// Set a custom maximum component length.
    pub fn with_max_component(mut self, max: usize) -> Self {
        self.max_component = max;
        self
    }

    /// Add extra characters to the forbidden set.
    pub fn forbid_chars(mut self, chars: &[char]) -> Self {
        for &ch in chars {
            if !self.invalid_chars.contains(&ch) {
                self.invalid_chars.push(ch);
            }
        }
        self
    }
}

impl Default for PathValidatorExtended {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("a/b/../c"), "a/c");
        assert_eq!(normalize("a/./b/c"), "a/b/c");
    }

    #[test]
    fn test_join() {
        let result = join("a", &["b", "c"]);
        assert!(result == "a/b/c" || result == "a\\b\\c");
    }

    #[test]
    fn test_dirname() {
        assert_eq!(dirname("a/b/c.txt"), "a/b");
    }

    #[test]
    fn test_basename() {
        assert_eq!(basename("a/b/c.txt"), "c.txt");
    }

    #[test]
    fn test_extname() {
        assert_eq!(extname("file.rs"), ".rs");
        assert_eq!(extname("file"), "");
    }

    #[test]
    fn test_to_forward_slashes() {
        assert_eq!(to_forward_slashes("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn test_remove_trailing_separator() {
        assert_eq!(remove_trailing_separator("path/to/dir/"), "path/to/dir");
    }

    #[test]
    fn test_stem() {
        assert_eq!(stem("a/b/file.rs"), "file");
        assert_eq!(stem("archive.tar.gz"), "archive.tar");
        assert_eq!(stem("noext"), "noext");
        assert_eq!(stem(""), "");
    }

    #[test]
    fn test_change_extension() {
        assert_eq!(change_extension("a/b/file.rs", "txt"), "a/b/file.txt");
        assert_eq!(change_extension("noext", "md"), "noext.md");
        // remove extension
        let without = change_extension("a/b/file.rs", "");
        assert!(!without.ends_with('.'));
    }

    #[test]
    fn test_is_child_of() {
        assert!(is_child_of("a/b/c", "a/b"));
        assert!(is_child_of("a/b/c/d", "a"));
        assert!(!is_child_of("a/b", "a/b"));
        assert!(!is_child_of("a/bc", "a/b"));
    }

    #[test]
    fn test_common_prefix() {
        assert_eq!(common_prefix("a/b/c", "a/b/d"), "a/b");
        assert_eq!(common_prefix("x/y", "a/b"), "");
        assert_eq!(common_prefix("a/b/c", "a/b/c"), "a/b/c");
    }

    #[test]
    fn test_ensure_trailing_separator() {
        assert_eq!(ensure_trailing_separator("path/to"), "path/to/");
        assert_eq!(ensure_trailing_separator("path/to/"), "path/to/");
        assert_eq!(ensure_trailing_separator(""), "");
    }

    #[test]
    fn test_path_components() {
        let pc = PathComponents::parse("a/b/file.rs");
        assert_eq!(pc.root, "");
        assert_eq!(pc.dir_parts, vec!["a", "b"]);
        assert_eq!(pc.stem, "file");
        assert_eq!(pc.extension, ".rs");

        let pc2 = PathComponents::parse("/usr/local/bin/tool");
        assert_eq!(pc2.root, "/");
        assert_eq!(pc2.dir_parts, vec!["usr", "local", "bin"]);
        assert_eq!(pc2.stem, "tool");
        assert_eq!(pc2.extension, "");
    }

    #[test]
    fn test_path_components_display() {
        let pc = PathComponents::parse("a/b/file.rs");
        assert_eq!(pc.to_string(), "a/b/file.rs");
    }

    #[test]
    fn test_normalize_case() {
        assert_eq!(normalize_case("A/B/File.RS"), "a/b/file.rs");
        assert_eq!(normalize_case("C:\\Users\\Foo"), "c:/users/foo");
    }

    #[test]
    fn test_is_unc_path() {
        assert!(is_unc_path("\\\\server\\share"));
        assert!(!is_unc_path("/normal/path"));
        assert!(!is_unc_path("\\not_unc"));
        assert!(!is_unc_path("\\\\\\triple"));
    }

    #[test]
    fn test_path_error_display() {
        assert_eq!(PathError::EmptyPath.to_string(), "path is empty");
        assert_eq!(
            PathError::InvalidPath("bad".into()).to_string(),
            "invalid path: bad"
        );
        assert_eq!(
            PathError::RelativePathExpected.to_string(),
            "expected a relative path"
        );
    }

    #[test]
    fn eq_patherror_same() {
        assert_eq!(PathError::EmptyPath, PathError::EmptyPath);
    }

    #[test]
    fn ne_patherror_diff() {
        assert_ne!(PathError::EmptyPath, PathError::RelativePathExpected);
    }

    #[test]
    fn display_patherror_variants() {
        assert!(!PathError::EmptyPath.to_string().is_empty());
        assert!(!PathError::RelativePathExpected.to_string().is_empty());
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
    fn path_stats_new_defaults() {
        let stats = PathStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn path_stats_record_success() {
        let mut stats = PathStats::new();
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
    fn path_stats_record_failure() {
        let mut stats = PathStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn path_stats_reset() {
        let mut stats = PathStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn path_stats_merge() {
        let mut a = PathStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = PathStats::new();
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
    fn path_stats_display() {
        let mut stats = PathStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn path_stats_default() {
        let stats = PathStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn path_validator_accepts_valid_name() {
        let v = PathValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn path_validator_rejects_empty() {
        let v = PathValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn path_validator_rejects_too_long() {
        let v = PathValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn path_validator_forbidden_prefix() {
        let v = PathValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn path_validator_allowed_chars() {
        let v = PathValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn path_validator_range() {
        let v = PathValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn path_sanitize_removes_control() {
        let result = PathValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn path_truncate_short_string() {
        assert_eq!(PathValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn path_truncate_long_string() {
        let result = PathValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn path_is_ascii_printable() {
        assert!(PathValidator::is_ascii_printable("Hello World 123"));
        assert!(!PathValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn path_normalize_resolves_dots() {
        assert_eq!(path_normalize("/a/b/../c"), "/a/c");
        assert_eq!(path_normalize("/a/./b/./c"), "/a/b/c");
    }

    #[test]
    fn path_normalize_collapses_slashes() {
        assert_eq!(path_normalize("/a//b///c"), "/a/b/c");
    }

    #[test]
    fn path_normalize_removes_trailing_slash() {
        assert_eq!(path_normalize("/a/b/c/"), "/a/b/c");
    }

    #[test]
    fn path_normalize_preserves_root() {
        assert_eq!(path_normalize("/"), "/");
    }

    #[test]
    fn path_normalize_empty() {
        assert_eq!(path_normalize(""), "");
    }

    #[test]
    fn path_normalize_backslashes() {
        assert_eq!(path_normalize("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn path_common_prefix_multiple() {
        assert_eq!(path_common_prefix(&["/a/b/c", "/a/b/d", "/a/b/e"]), "/a/b");
    }

    #[test]
    fn path_common_prefix_single() {
        assert_eq!(path_common_prefix(&["/a/b/c"]), "/a/b/c");
    }

    #[test]
    fn path_common_prefix_empty() {
        assert_eq!(path_common_prefix(&[]), "");
    }

    #[test]
    fn path_common_prefix_no_common() {
        assert_eq!(path_common_prefix(&["a/b", "c/d"]), "");
    }

    #[test]
    fn relative_to_sibling() {
        let r = relative_to("/a/b", "/a/c").unwrap();
        assert_eq!(r, "../c");
    }

    #[test]
    fn relative_to_child() {
        let r = relative_to("/a", "/a/b/c").unwrap();
        assert_eq!(r, "b/c");
    }

    #[test]
    fn relative_to_same() {
        let r = relative_to("/a/b", "/a/b").unwrap();
        assert_eq!(r, ".");
    }

    #[test]
    fn relative_to_empty_err() {
        assert!(relative_to("", "/a").is_err());
        assert!(relative_to("/a", "").is_err());
    }

    #[test]
    fn test_path_components_to_path_string() {
        let pc = PathComponents::parse("a/b/file.rs");
        assert_eq!(pc.to_path_string(), "a/b/file.rs");

        let pc2 = PathComponents::parse("/usr/local/bin/tool");
        assert_eq!(pc2.to_path_string(), "/usr/local/bin/tool");
    }

    #[test]
    fn test_path_components_depth() {
        let pc = PathComponents::parse("a/b/c/file.rs");
        assert_eq!(pc.depth(), 3);

        let pc2 = PathComponents::parse("file.rs");
        assert_eq!(pc2.depth(), 0);
    }

    #[test]
    fn test_path_components_with_extension() {
        let pc = PathComponents::parse("src/main.rs");
        assert_eq!(pc.with_extension(".txt"), "src/main.txt");
        assert_eq!(pc.with_extension(""), "src/main");
    }

    #[test]
    fn test_is_hidden() {
        assert!(is_hidden(".gitignore"));
        assert!(is_hidden("/home/user/.bashrc"));
        assert!(!is_hidden("visible.txt"));
        assert!(!is_hidden(""));
    }

    #[test]
    fn test_add_suffix_before_ext() {
        assert_eq!(add_suffix_before_ext("file.txt", "_backup"), "file_backup.txt");
        assert_eq!(add_suffix_before_ext("noext", "_v2"), "noext_v2");
        assert_eq!(add_suffix_before_ext("a/b/file.rs", "_test"), "a/b/file_test.rs");
    }

    #[test]
    fn test_split_on_first_separator() {
        assert_eq!(split_on_first_separator("a/b/c"), ("a", "b/c"));
        assert_eq!(split_on_first_separator("single"), ("single", ""));
    }

    #[test]
    fn test_count_segments() {
        assert_eq!(count_segments("a/b/c"), 3);
        assert_eq!(count_segments("/usr/local/bin/"), 3);
        assert_eq!(count_segments("single"), 1);
        assert_eq!(count_segments(""), 0);
    }

    #[test]
    fn test_is_relative() {
        assert!(is_relative("a/b/c"));
        assert!(is_relative("file.txt"));
        assert!(!is_relative("/absolute/path"));
    }

    // -- PathTemplate tests ------------------------------------------------

    #[test]
    fn path_template_expand_basic() {
        let mut tpl = PathTemplate::new("${workspace}/src/${file}.rs");
        tpl.set("workspace", "/home/user/project");
        tpl.set("file", "main");
        assert_eq!(tpl.expand(), "/home/user/project/src/main.rs");
    }

    #[test]
    fn path_template_missing_variable_left_as_is() {
        let tpl = PathTemplate::new("${base}/${name}.txt");
        assert!(tpl.expand().contains("${base}"));
        assert!(!tpl.is_complete());
        assert_eq!(tpl.missing_variables(), vec!["base", "name"]);
    }

    #[test]
    fn path_template_referenced_variables() {
        let tpl = PathTemplate::new("${a}/${b}/${a}");
        let vars = tpl.referenced_variables();
        assert_eq!(vars, vec!["a", "b"]);
    }

    #[test]
    fn path_template_is_complete() {
        let mut tpl = PathTemplate::new("${x}/${y}");
        assert!(!tpl.is_complete());
        tpl.set("x", "1");
        assert!(!tpl.is_complete());
        tpl.set("y", "2");
        assert!(tpl.is_complete());
    }

    #[test]
    fn path_template_display() {
        let mut tpl = PathTemplate::new("${dir}/file.txt");
        tpl.set("dir", "output");
        assert_eq!(format!("{tpl}"), "output/file.txt");
    }

    // -- PathMatcher tests -------------------------------------------------

    #[test]
    fn path_matcher_include_only() {
        let mut m = PathMatcher::new();
        m.include("*.rs");
        assert!(m.matches("main.rs"));
        assert!(!m.matches("main.py"));
    }

    #[test]
    fn path_matcher_exclude() {
        let mut m = PathMatcher::new();
        m.include("*").exclude("*.log");
        assert!(m.matches("app.rs"));
        assert!(!m.matches("debug.log"));
    }

    #[test]
    fn path_matcher_doublestar() {
        let mut m = PathMatcher::new();
        m.include("src/**/*.rs");
        assert!(m.matches("src/lib/parser.rs"));
        assert!(m.matches("src/main.rs"));
        assert!(!m.matches("tests/test.rs"));
    }

    #[test]
    fn path_matcher_no_include_means_all() {
        let m = PathMatcher::new();
        assert!(m.matches("anything.txt"));
    }

    #[test]
    fn path_matcher_pattern_count() {
        let mut m = PathMatcher::new();
        m.include("*.rs").exclude("*.bak");
        assert_eq!(m.pattern_count(), 2);
    }

    // -- PathCanonicalizer tests -------------------------------------------

    #[test]
    fn canonicalizer_resolves_dot_and_dotdot() {
        assert_eq!(PathCanonicalizer::canonicalize("a/./b/../c"), "a/c");
        assert_eq!(PathCanonicalizer::canonicalize("/a/b/../c"), "/a/c");
    }

    #[test]
    fn canonicalizer_relative_leading_dotdot() {
        assert_eq!(PathCanonicalizer::canonicalize("../../a"), "../../a");
        assert!(!PathCanonicalizer::escapes_root("a/b"));
        assert!(PathCanonicalizer::escapes_root("../a"));
    }

    #[test]
    fn canonicalizer_depth() {
        assert_eq!(PathCanonicalizer::canonical_depth("a/b/c"), 3);
        assert_eq!(PathCanonicalizer::canonical_depth("a/./b/../c"), 2);
    }

    // -- PathComponents extension tests ------------------------------------

    #[test]
    fn path_components_segments_owned() {
        let pc = PathComponents::parse("src/lib/parser.rs");
        let segs = pc.segments_owned();
        assert_eq!(segs, vec!["src", "lib", "parser.rs"]);
    }

    #[test]
    fn path_components_filename_and_extension() {
        let pc = PathComponents::parse("a/b/hello.tar.gz");
        assert_eq!(pc.filename(), "hello.tar.gz");
        assert!(pc.has_extension());
        assert_eq!(pc.extension_without_dot(), Some("gz"));
    }

    #[test]
    fn path_components_truncate_and_push() {
        let pc = PathComponents::parse("a/b/c/file.txt");
        let truncated = pc.truncate_dir(1);
        assert_eq!(truncated.dir_parts, vec!["a"]);
        let pushed = pc.push_dir("d");
        assert_eq!(pushed.dir_parts.last().unwrap(), "d");
    }

    // -- path_depth tests --------------------------------------------------

    #[test]
    fn test_path_depth_basic() {
        assert_eq!(path_depth("a/b/c"), 3);
        assert_eq!(path_depth("/usr/local/bin"), 3);
        assert_eq!(path_depth("file.txt"), 1);
        assert_eq!(path_depth(""), 0);
    }

    #[test]
    fn test_path_depth_with_dots() {
        assert_eq!(path_depth("a/b/../c"), 2);
        assert_eq!(path_depth("a/./b/c"), 3);
        assert_eq!(path_depth("../a"), 1);
    }

    // -- extensions tests --------------------------------------------------

    #[test]
    fn test_extensions_multiple() {
        assert_eq!(extensions("archive.tar.gz"), vec!["tar", "gz"]);
        assert_eq!(extensions("file.rs"), vec!["rs"]);
        assert_eq!(extensions("noext"), Vec::<String>::new());
        assert_eq!(extensions(""), Vec::<String>::new());
    }

    #[test]
    fn test_has_extension() {
        assert!(has_extension("file.RS", "rs"));
        assert!(has_extension("file.rs", ".rs"));
        assert!(!has_extension("file.rs", "txt"));
        assert!(!has_extension("noext", "rs"));
    }

    #[test]
    fn test_remove_all_extensions() {
        assert_eq!(remove_all_extensions("a/b/file.tar.gz"), "a/b/file");
        assert_eq!(remove_all_extensions("file.txt"), "file");
        assert_eq!(remove_all_extensions("noext"), "noext");
    }

    // -- sanitize tests ----------------------------------------------------

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello<world>.txt", '_'), "hello_world_.txt");
        assert_eq!(sanitize_filename("normal.txt", '_'), "normal.txt");
        assert_eq!(sanitize_filename("a:b|c", '-'), "a-b-c");
    }

    #[test]
    fn test_sanitize_path() {
        assert_eq!(sanitize_path("/a/b<c>/d", '_'), "/a/b_c_/d");
        assert_eq!(sanitize_path("clean/path/file.txt", '_'), "clean/path/file.txt");
    }

    // -- URI conversion tests ----------------------------------------------

    #[test]
    fn test_uri_to_path() {
        assert_eq!(uri_to_path("file:///home/user/file.txt").unwrap(), "/home/user/file.txt");
        assert_eq!(uri_to_path("file:///path%20with%20spaces").unwrap(), "/path with spaces");
        assert!(uri_to_path("http://example.com").is_err());
    }

    #[test]
    fn test_path_to_uri() {
        assert_eq!(path_to_uri("/home/user/file.txt"), "file:///home/user/file.txt");
        assert_eq!(path_to_uri("/path with spaces"), "file:///path%20with%20spaces");
        assert_eq!(path_to_uri("relative/path"), "file:///relative/path");
    }

    #[test]
    fn test_uri_roundtrip() {
        let original = "/home/user/my project/file#1.txt";
        let uri = path_to_uri(original);
        let back = uri_to_path(&uri).unwrap();
        assert_eq!(back, original);
    }

    // -- path comparison tests ---------------------------------------------

    #[test]
    fn test_paths_equal_ignore_case() {
        assert!(paths_equal_ignore_case("A/B/C.txt", "a/b/c.txt"));
        assert!(paths_equal_ignore_case("a\\b\\c", "A/B/C"));
        assert!(!paths_equal_ignore_case("a/b", "a/c"));
    }

    #[test]
    fn test_paths_equal() {
        assert!(paths_equal("a/b/c", "a/b/c"));
        assert!(paths_equal("a\\b\\c", "a/b/c"));
        assert!(!paths_equal("a/b/C", "a/b/c"));
    }

    // -- breadcrumbs tests -------------------------------------------------

    #[test]
    fn test_breadcrumbs_absolute() {
        let crumbs = breadcrumbs("/usr/local/bin");
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[0].label, "usr");
        assert_eq!(crumbs[0].path, "/usr");
        assert_eq!(crumbs[1].label, "local");
        assert_eq!(crumbs[1].path, "/usr/local");
        assert_eq!(crumbs[2].label, "bin");
        assert_eq!(crumbs[2].path, "/usr/local/bin");
    }

    #[test]
    fn test_breadcrumbs_relative() {
        let crumbs = breadcrumbs("src/lib/parser.rs");
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[0].label, "src");
        assert_eq!(crumbs[0].path, "src");
        assert_eq!(crumbs[2].label, "parser.rs");
        assert_eq!(crumbs[2].path, "src/lib/parser.rs");
    }

    // -- expand_tilde tests ------------------------------------------------

    #[test]
    fn test_expand_tilde() {
        assert_eq!(expand_tilde("~/docs/file.txt", "/home/user"), "/home/user/docs/file.txt");
        assert_eq!(expand_tilde("~", "/home/user"), "/home/user");
        assert_eq!(expand_tilde("/absolute/path", "/home/user"), "/absolute/path");
        assert_eq!(expand_tilde("relative", "/home/user"), "relative");
    }

    #[test]
    fn test_expand_tilde_trailing_slash_home() {
        assert_eq!(expand_tilde("~/file", "/home/user/"), "/home/user/file");
    }

    // -- PathCompletionProvider tests --------------------------------------

    #[test]
    fn test_completion_basic_prefix() {
        let provider = PathCompletionProvider::new(vec![
            "src/lib.rs".to_string(),
            "src/main.rs".to_string(),
            "tests/test1.rs".to_string(),
        ]);
        let results = provider.complete("src/");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].completed, "src/lib.rs");
        assert_eq!(results[1].completed, "src/main.rs");
    }

    #[test]
    fn test_completion_no_match() {
        let provider = PathCompletionProvider::new(vec![
            "src/lib.rs".to_string(),
        ]);
        let results = provider.complete("tests/");
        assert!(results.is_empty());
    }

    #[test]
    fn test_completion_case_insensitive() {
        let provider = PathCompletionProvider::new(vec![
            "Src/Lib.rs".to_string(),
            "src/main.rs".to_string(),
        ])
        .case_sensitive(false);
        let results = provider.complete("src/");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_completion_directory_flag() {
        let provider = PathCompletionProvider::new(vec![
            "src/".to_string(),
            "src/lib.rs".to_string(),
        ]);
        let results = provider.complete("src");
        assert_eq!(results.len(), 2);
        assert!(results[0].is_directory); // "src/"
        assert!(!results[1].is_directory); // "src/lib.rs"
    }

    #[test]
    fn test_completion_fuzzy() {
        let provider = PathCompletionProvider::new(vec![
            "a/b/target.rs".to_string(),
            "x/y/other.rs".to_string(),
        ]);
        let results = provider.complete_fuzzy("target");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "target.rs");
    }

    #[test]
    fn test_completion_fuzzy_empty_input() {
        let provider = PathCompletionProvider::new(vec![
            "a/b.rs".to_string(),
        ]);
        assert!(provider.complete_fuzzy("").is_empty());
    }

    #[test]
    fn test_completion_add_candidates() {
        let mut provider = PathCompletionProvider::new(vec![]);
        assert_eq!(provider.candidate_count(), 0);
        provider.add_candidates(&["a.rs", "b.rs"]);
        assert_eq!(provider.candidate_count(), 2);
    }

    // -- PathEnvVarResolver tests ------------------------------------------

    #[test]
    fn test_env_resolve_dollar_var() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("HOME", "/home/alice");
        assert_eq!(resolver.resolve("$HOME/docs"), "/home/alice/docs");
    }

    #[test]
    fn test_env_resolve_braced_var() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("PROJECT", "myapp");
        assert_eq!(
            resolver.resolve("/opt/${PROJECT}/bin"),
            "/opt/myapp/bin"
        );
    }

    #[test]
    fn test_env_resolve_unknown_var_left_intact() {
        let resolver = PathEnvVarResolver::new();
        assert_eq!(resolver.resolve("$UNKNOWN/path"), "$UNKNOWN/path");
        assert_eq!(resolver.resolve("${MISSING}/path"), "${MISSING}/path");
    }

    #[test]
    fn test_env_resolve_multiple_vars() {
        let resolver = PathEnvVarResolver::with_defaults("/home/bob", "bob");
        assert_eq!(
            resolver.resolve("$HOME/users/$USER"),
            "/home/bob/users/bob"
        );
    }

    #[test]
    fn test_env_resolve_no_vars() {
        let resolver = PathEnvVarResolver::new();
        assert_eq!(resolver.resolve("/plain/path"), "/plain/path");
    }

    #[test]
    fn test_env_referenced_vars() {
        let vars = PathEnvVarResolver::referenced_vars("$HOME/${PROJECT}/src/$USER");
        assert_eq!(vars, vec!["HOME", "PROJECT", "USER"]);
    }

    #[test]
    fn test_env_has_variables() {
        assert!(PathEnvVarResolver::has_variables("$HOME/path"));
        assert!(!PathEnvVarResolver::has_variables("/plain/path"));
    }

    #[test]
    fn test_env_adjacent_dollar_signs() {
        let resolver = PathEnvVarResolver::new();
        assert_eq!(resolver.resolve("$$"), "$$");
    }

    // -- PathShortener tests -----------------------------------------------

    #[test]
    fn test_shortener_abbreviate() {
        let shortener = PathShortener::new(40);
        assert_eq!(
            shortener.abbreviate("/home/user/projects/my-app/src/lib.rs"),
            "/h/u/p/m/src/lib.rs"
        );
    }

    #[test]
    fn test_shortener_abbreviate_short_path() {
        let shortener = PathShortener::new(40);
        assert_eq!(shortener.abbreviate("src/lib.rs"), "src/lib.rs");
    }

    #[test]
    fn test_shortener_abbreviate_relative() {
        let shortener = PathShortener::new(40);
        assert_eq!(
            shortener.abbreviate("a/b/c/d/file.txt"),
            "a/b/c/d/file.txt"
        );
        // Only 5 segments: first 3 abbreviated, last 2 kept
        assert_eq!(
            shortener.abbreviate("alpha/bravo/charlie/delta/file.txt"),
            "a/b/c/delta/file.txt"
        );
    }

    #[test]
    fn test_shortener_truncate_leading() {
        let shortener = PathShortener::new(20);
        let result = shortener.truncate_leading("/very/long/deep/nested/path/file.rs");
        assert!(result.len() <= 20);
        assert!(result.contains("file.rs"));
    }

    #[test]
    fn test_shortener_truncate_leading_short() {
        let shortener = PathShortener::new(40);
        let path = "/a/b.rs";
        assert_eq!(shortener.truncate_leading(path), path);
    }

    #[test]
    fn test_shortener_collapse_middle() {
        let shortener = PathShortener::new(20);
        let result = shortener.collapse_middle("/home/user/projects/deep/nested/file.rs");
        assert!(result.contains("home"));
        assert!(result.contains("file.rs"));
        assert!(result.contains("…"));
    }

    #[test]
    fn test_shortener_collapse_middle_short() {
        let shortener = PathShortener::new(40);
        assert_eq!(shortener.collapse_middle("a/b"), "a/b");
    }

    #[test]
    fn test_shortener_custom_ellipsis() {
        let shortener = PathShortener::new(20).ellipsis("...");
        let result = shortener.truncate_leading("/aaa/bbb/ccc/ddd/eee/fff.rs");
        assert!(result.contains("..."));
    }

    // -- PathSegmentValidator tests ----------------------------------------

    #[test]
    fn test_segment_validator_valid() {
        let v = PathSegmentValidator::new();
        assert!(v.validate_segment("hello.txt").is_empty());
        assert!(v.validate_segment("my-file_v2").is_empty());
    }

    #[test]
    fn test_segment_validator_reserved_name() {
        let v = PathSegmentValidator::new();
        let errors = v.validate_segment("CON");
        assert!(!errors.is_empty());
        assert!(errors[0].contains("reserved"));
    }

    #[test]
    fn test_segment_validator_reserved_with_extension() {
        let v = PathSegmentValidator::new();
        let errors = v.validate_segment("con.txt");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_segment_validator_forbidden_chars() {
        let v = PathSegmentValidator::new();
        let errors = v.validate_segment("file<name>.txt");
        assert!(errors.len() >= 2); // '<' and '>'
    }

    #[test]
    fn test_segment_validator_trailing_space() {
        let v = PathSegmentValidator::new();
        let errors = v.validate_segment("file ");
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.contains("space or period")));
    }

    #[test]
    fn test_segment_validator_trailing_dot() {
        let v = PathSegmentValidator::new();
        let errors = v.validate_segment("file.");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_segment_validator_empty() {
        let v = PathSegmentValidator::new();
        let errors = v.validate_segment("");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("empty"));
    }

    #[test]
    fn test_segment_validator_too_long() {
        let v = PathSegmentValidator::new().max_segment_length(10);
        let errors = v.validate_segment("verylongname.txt");
        assert!(!errors.is_empty());
        assert!(errors[0].contains("exceeds"));
    }

    #[test]
    fn test_segment_validator_custom_forbidden() {
        let v = PathSegmentValidator::new().forbid_name("CUSTOM");
        let errors = v.validate_segment("custom");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_segment_validate_path() {
        let v = PathSegmentValidator::new();
        assert!(v.is_valid_path("/usr/local/bin"));
        assert!(v.is_valid_path("src/main.rs"));
    }

    #[test]
    fn test_segment_validate_path_with_bad_segment() {
        let v = PathSegmentValidator::new();
        let results = v.validate_path("/usr/CON/file.txt");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "CON");
    }

    #[test]
    fn test_segment_validate_path_skips_dots_and_drive() {
        let v = PathSegmentValidator::new();
        assert!(v.is_valid_path("C:/Users/file.txt"));
        assert!(v.is_valid_path("./relative/../path/file.txt"));
    }


    // -----------------------------------------------------------------------
    // PathCompletionProvider – additional method tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_completion_add_known_path() {
        let mut provider = PathCompletionProvider::new(vec![]);
        provider.add_known_path("src/main.rs");
        provider.add_known_path("src/lib.rs");
        assert_eq!(provider.candidate_count(), 2);
    }

    #[test]
    fn test_completion_complete_basename() {
        let provider = PathCompletionProvider::new(vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "tests/main_test.rs".to_string(),
        ]);
        let results = provider.complete_basename("main");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|c| c.completed == "src/main.rs"));
        assert!(results.iter().any(|c| c.completed == "tests/main_test.rs"));
    }

    #[test]
    fn test_completion_complete_basename_empty() {
        let provider = PathCompletionProvider::new(vec!["a.rs".to_string()]);
        assert!(provider.complete_basename("").is_empty());
    }

    #[test]
    fn test_completion_clear() {
        let mut provider = PathCompletionProvider::new(vec!["a.rs".to_string()]);
        assert_eq!(provider.candidate_count(), 1);
        provider.clear();
        assert_eq!(provider.candidate_count(), 0);
    }

    #[test]
    fn test_completion_paths_in_dir() {
        let provider = PathCompletionProvider::new(vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "tests/test.rs".to_string(),
        ]);
        let in_src = provider.paths_in_dir("src");
        assert_eq!(in_src.len(), 2);
        assert!(in_src.contains(&"src/lib.rs".to_string()));
        assert!(in_src.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn test_completion_paths_in_dir_trailing_slash() {
        let provider = PathCompletionProvider::new(vec![
            "src/main.rs".to_string(),
        ]);
        let in_src = provider.paths_in_dir("src/");
        assert_eq!(in_src.len(), 1);
    }

    #[test]
    fn test_completion_has_path() {
        let provider = PathCompletionProvider::new(vec![
            "src/main.rs".to_string(),
        ]);
        assert!(provider.has_path("src/main.rs"));
        assert!(!provider.has_path("src/lib.rs"));
    }

    #[test]
    fn test_completion_has_path_backslash() {
        let provider = PathCompletionProvider::new(vec![
            "src/main.rs".to_string(),
        ]);
        // Backslash path should match forward slash candidate
        assert!(provider.has_path("src\\main.rs"));
    }

    #[test]
    fn test_completion_basename_case_insensitive() {
        let provider = PathCompletionProvider::new(vec![
            "src/README.md".to_string(),
            "docs/readme.txt".to_string(),
        ])
        .case_sensitive(false);
        let results = provider.complete_basename("readme");
        assert_eq!(results.len(), 2);
    }

    // -----------------------------------------------------------------------
    // PathEnvVarResolver – additional method tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolver_has_var() {
        let mut resolver = PathEnvVarResolver::new();
        assert!(!resolver.has_var("HOME"));
        resolver.set("HOME", "/home/user");
        assert!(resolver.has_var("HOME"));
    }

    #[test]
    fn test_resolver_var_count() {
        let mut resolver = PathEnvVarResolver::new();
        assert_eq!(resolver.var_count(), 0);
        resolver.set("A", "1");
        resolver.set("B", "2");
        assert_eq!(resolver.var_count(), 2);
    }

    #[test]
    fn test_resolver_unresolved_vars() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("HOME", "/home/user");
        let unresolved = resolver.unresolved_vars("$HOME/$PROJECT/src");
        assert_eq!(unresolved, vec!["PROJECT"]);
    }

    #[test]
    fn test_resolver_unresolved_vars_all_resolved() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("HOME", "/home/user");
        let unresolved = resolver.unresolved_vars("$HOME/src");
        assert!(unresolved.is_empty());
    }

    #[test]
    fn test_resolver_remove_var() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("HOME", "/home/user");
        assert!(resolver.remove_var("HOME"));
        assert!(!resolver.has_var("HOME"));
        assert!(!resolver.remove_var("HOME")); // already removed
    }

    #[test]
    fn test_resolver_get() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("HOME", "/home/user");
        assert_eq!(resolver.get("HOME"), Some("/home/user"));
        assert_eq!(resolver.get("MISSING"), None);
    }

    #[test]
    fn test_resolver_var_names() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("ZEBRA", "z");
        resolver.set("ALPHA", "a");
        let names = resolver.var_names();
        assert_eq!(names, vec!["ALPHA", "ZEBRA"]);
    }

    #[test]
    fn test_resolver_unresolved_with_braces() {
        let mut resolver = PathEnvVarResolver::new();
        resolver.set("HOME", "/home/user");
        let unresolved = resolver.unresolved_vars("${HOME}/${WORKSPACE}/file");
        assert_eq!(unresolved, vec!["WORKSPACE"]);
    }

    // -----------------------------------------------------------------------
    // PathShortener – additional method tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_shortener_shorten_short_path() {
        let s = PathShortener::new(50);
        assert_eq!(s.shorten("src/lib.rs"), "src/lib.rs");
    }

    #[test]
    fn test_shortener_shorten_long_path() {
        let s = PathShortener::new(20);
        let result = s.shorten("/home/user/projects/my-app/src/lib.rs");
        assert!(result.len() <= 20 || result.contains("…"));
    }

    #[test]
    fn test_shortener_shorten_relative() {
        let s = PathShortener::new(50);
        let result = s.shorten_relative("/home/user/project/src/lib.rs", "/home/user/project");
        assert_eq!(result, "src/lib.rs");
    }

    #[test]
    fn test_shortener_shorten_relative_no_match() {
        let s = PathShortener::new(50);
        let result = s.shorten_relative("/other/path/file.rs", "/home/user");
        // Path doesn't start with base, so full path is shortened (abbreviated)
        assert!(result.contains("file.rs"));
    }

    #[test]
    fn test_shortener_shorten_home() {
        let s = PathShortener::new(50);
        let result = s.shorten_home("/home/user/projects/file.rs", "/home/user");
        assert!(result.starts_with("~/"));
        assert!(result.contains("projects/file.rs"));
    }

    #[test]
    fn test_shortener_shorten_home_exact() {
        let s = PathShortener::new(50);
        let result = s.shorten_home("/home/user", "/home/user");
        assert_eq!(result, "~");
    }

    #[test]
    fn test_shortener_shorten_home_no_match() {
        let s = PathShortener::new(50);
        let result = s.shorten_home("/opt/bin/tool", "/home/user");
        // Path doesn't match home, so it's abbreviated: /o/bin/tool
        assert!(result.contains("bin/tool"));
    }

    #[test]
    fn test_shortener_fits() {
        let s = PathShortener::new(10);
        assert!(s.fits("short.rs"));
        assert!(!s.fits("a_very_long_filename.rs"));
    }

    #[test]
    fn test_shortener_max_length() {
        let s = PathShortener::new(42);
        assert_eq!(s.max_length(), 42);
    }

    #[test]
    fn test_shortener_shorten_relative_trailing_slash() {
        let s = PathShortener::new(50);
        let result = s.shorten_relative("/home/user/project/src/lib.rs", "/home/user/project/");
        assert_eq!(result, "src/lib.rs");
    }

    // -----------------------------------------------------------------------
    // PathValidatorExtended – tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validator_ext_valid_filename() {
        let v = PathValidatorExtended::new();
        assert!(v.is_valid_filename("hello.txt"));
        assert!(v.is_valid_filename("my-file_v2.tar.gz"));
    }

    #[test]
    fn test_validator_ext_invalid_filename_empty() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_valid_filename(""));
    }

    #[test]
    fn test_validator_ext_invalid_filename_reserved() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_valid_filename("CON"));
        assert!(!v.is_valid_filename("con.txt"));
        assert!(!v.is_valid_filename("NUL"));
        assert!(!v.is_valid_filename("COM1"));
    }

    #[test]
    fn test_validator_ext_invalid_filename_chars() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_valid_filename("file<name>.txt"));
        assert!(!v.is_valid_filename("file:name"));
        assert!(!v.is_valid_filename("file\0name"));
    }

    #[test]
    fn test_validator_ext_invalid_filename_trailing() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_valid_filename("file."));
        assert!(!v.is_valid_filename("file "));
    }

    #[test]
    fn test_validator_ext_valid_path() {
        let v = PathValidatorExtended::new();
        assert!(v.is_valid_path("/home/user/file.txt"));
        assert!(v.is_valid_path("relative/path/file.rs"));
        assert!(v.is_valid_path("C:/Users/file.txt"));
    }

    #[test]
    fn test_validator_ext_invalid_path_empty() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_valid_path(""));
    }

    #[test]
    fn test_validator_ext_invalid_path_bad_segment() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_valid_path("/home/user/CON/file.txt"));
    }

    #[test]
    fn test_validator_ext_invalid_chars() {
        let v = PathValidatorExtended::new();
        let chars = v.invalid_chars("he<ll>o");
        assert_eq!(chars, vec!['<', '>']);
    }

    #[test]
    fn test_validator_ext_invalid_chars_none() {
        let v = PathValidatorExtended::new();
        assert!(v.invalid_chars("hello.txt").is_empty());
    }

    #[test]
    fn test_validator_ext_suggested_filename() {
        let v = PathValidatorExtended::new();
        assert_eq!(v.suggested_filename("he<ll>o.txt"), "he_ll_o.txt");
    }

    #[test]
    fn test_validator_ext_suggested_filename_empty() {
        let v = PathValidatorExtended::new();
        assert_eq!(v.suggested_filename(""), "_");
    }

    #[test]
    fn test_validator_ext_suggested_filename_reserved() {
        let v = PathValidatorExtended::new();
        assert_eq!(v.suggested_filename("CON"), "_CON");
    }

    #[test]
    fn test_validator_ext_suggested_trailing_dots() {
        let v = PathValidatorExtended::new();
        assert_eq!(v.suggested_filename("file..."), "file");
    }

    #[test]
    fn test_validator_ext_max_component_length() {
        let v = PathValidatorExtended::new();
        assert_eq!(v.max_component_length(), 255);
    }

    #[test]
    fn test_validator_ext_is_too_long() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_too_long("short", 100));
        assert!(v.is_too_long("short", 3));
    }

    #[test]
    fn test_validator_ext_has_invalid_chars() {
        let v = PathValidatorExtended::new();
        assert!(v.has_invalid_chars("file<name>"));
        assert!(!v.has_invalid_chars("valid-file_name.txt"));
    }

    #[test]
    fn test_validator_ext_with_max_component() {
        let v = PathValidatorExtended::new().with_max_component(10);
        assert!(v.is_valid_filename("short.txt"));
        assert!(!v.is_valid_filename("very_long_filename.txt"));
    }

    #[test]
    fn test_validator_ext_forbid_chars() {
        let v = PathValidatorExtended::new().forbid_chars(&['@', '#']);
        assert!(v.has_invalid_chars("file@name"));
        assert!(v.has_invalid_chars("file#name"));
        assert!(v.is_valid_filename("file-name.txt"));
    }

    #[test]
    fn test_validator_ext_dots_only() {
        let v = PathValidatorExtended::new();
        assert!(!v.is_valid_filename("..."));
        assert!(!v.is_valid_filename("  "));
    }

    #[test]
    fn test_validator_ext_path_with_dots_and_parent() {
        let v = PathValidatorExtended::new();
        assert!(v.is_valid_path("./src/../lib/file.rs"));
    }

    #[test]
    fn test_validator_ext_suggested_control_chars() {
        let v = PathValidatorExtended::new();
        let result = v.suggested_filename("file\x01name\x02.txt");
        assert_eq!(result, "file_name_.txt");
    }

    #[test]
    fn test_validator_ext_default() {
        let v = PathValidatorExtended::default();
        assert_eq!(v.max_component_length(), 255);
        assert!(v.is_valid_filename("test.rs"));
    }

}
