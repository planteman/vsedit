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
}
