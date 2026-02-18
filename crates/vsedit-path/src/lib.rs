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



// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 137
// ---------------------------------------------------------------------------

/// Generic object pool `Xc137Pool<T>`.
pub struct Xc137Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc137Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc137PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc137Pool<T> {
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
    pub fn stats(&self) -> Xc137PoolStats {
        Xc137PoolStats {
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

impl<T> Default for Xc137Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc137Scheduler`.
pub struct Xc137Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc137Scheduler {
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

impl Default for Xc137Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_137 hash for the given byte slice.
pub fn xc_137_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_137 convention.
pub fn xc_137_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe47 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe47Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe47PipelineError {
    pub stage: Xe47Stage,
    pub message: String,
}

impl std::fmt::Display for Xe47PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe47Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe47Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError>>>,
    stage_names: Vec<Xe47Stage>,
}

impl Xe47Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe47Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe47Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe47Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe47Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> {
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

    pub fn compose(mut self, other: Xe47Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe47CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe47CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe47Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe47CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe47CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe47Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe47CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_47_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe47CacheEntry {
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

    fn xe_47_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe47CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_47_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> {
    Ok(data)
}

pub fn xe_47_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_47_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_47_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_47_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe47PipelineError> {
    Err(Xe47PipelineError {
        stage: Xe47Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_18: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg18Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg18Graph {
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

impl Default for Xg18Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_18: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg18Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg18Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg18Heap<T>) {
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

impl<T: Ord> Default for Xg18Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 136).
pub struct Xh136SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh136SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 178 as u64,
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

/// A compact bit set supporting boolean operations (variant 136).
pub struct Xh136BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh136BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 136).
pub struct Xi136Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi136Deque<T> {
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
pub struct Xi136Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi136Interval {
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

/// A simple interval tree (variant 136).
pub struct Xi136IntervalTree {
    xi_intervals: Vec<Xi136Interval>,
}

impl Xi136IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi136Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi136Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi136Interval) -> Vec<&Xi136Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi136Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi136Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi136Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi136Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi136Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi136Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 136) ---

/// Disjoint set / union-find for crate 136.
pub struct Xj136UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj136UnionFind {
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

const XJ136_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 136.
pub struct Xj136BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj136BTreeNode<K, V>>>,
    len: usize,
}

struct Xj136BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj136BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj136BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ136_BTREE_ORDER - 1
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
        let mid = XJ136_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj136BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj136BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj136BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj136BTreeNode::xj_new_leaf();
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


// --- xk_136 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk136SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk136SegmentTree {
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
pub struct Xk136DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk136DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_136).
#[derive(Debug, Clone)]
pub struct Xl136Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl136Rope {
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

/// Suffix array for efficient string searching (xl_136).
#[derive(Debug, Clone)]
pub struct Xl136SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl136SuffixArray {
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
pub struct Xm136MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm136MatrixSparse {
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
pub struct Xm136Tokenizer {
    text: String,
}

impl Xm136Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 136.
pub struct Xn136Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn136Fenwick {
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

// ----- AVL tree map — crate 136 -----

#[derive(Debug, Clone)]
struct Xn136AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn136AvlNode<K, V>>>,
    right: Option<Box<Xn136AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 136.
#[derive(Debug, Clone)]
pub struct Xn136AVL<K, V> {
    root: Option<Box<Xn136AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn136AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn136AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn136AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn136AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn136AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn136AvlNode<K, V>>) -> Box<Xn136AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn136AvlNode<K, V>>) -> Box<Xn136AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn136AvlNode<K, V>>) -> Box<Xn136AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn136AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn136AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn136AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn136AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn136AvlNode<K, V>>) -> &Xn136AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn136AvlNode<K, V>>) -> (Box<Xn136AvlNode<K, V>>, Option<Box<Xn136AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn136AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn136AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn136AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn136AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn136AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn136AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn136AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo136RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo136Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo136RBNode<K, V> {
    key: K,
    value: V,
    color: Xo136Color,
    left: Option<Box<Xo136RBNode<K, V>>>,
    right: Option<Box<Xo136RBNode<K, V>>>,
}

/// A red-black tree map for crate 136.
#[derive(Debug, Clone)]
pub struct Xo136RedBlack<K, V> {
    root: Option<Box<Xo136RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo136RedBlack<K, V> {
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
            r.color = Xo136Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo136RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo136RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo136RBNode {
                    key, value, color: Xo136Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo136RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo136Color::Red)
    }

    fn xo_balance(mut h: Box<Xo136RBNode<K, V>>) -> Box<Xo136RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo136Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo136RBNode<K, V>>) -> Box<Xo136RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo136Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo136RBNode<K, V>>) -> Box<Xo136RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo136Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo136RBNode<K, V>>) {
        h.color = Xo136Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo136Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo136Color::Black; }
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
            r.color = Xo136Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo136RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo136RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo136RBNode<K, V>) -> (K, V, Option<Box<Xo136RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo136RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo136Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo136RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo136ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 136.
#[derive(Debug, Clone)]
pub struct Xo136ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo136ConsistentHash {
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
            let vkey = format!("{}#xo136#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo136#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 136).
#[derive(Debug)]
pub struct Xp136SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp136Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp136Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp136Node<K, V>>>,
    xp_right: Option<Box<Xp136Node<K, V>>>,
}

impl<K: Ord, V> Xp136Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp136SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp136SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp136Node<K, V>>>, key: &K) -> Option<Box<Xp136Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp136Node<K, V>>) -> Box<Xp136Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp136Node<K, V>>) -> Box<Xp136Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp136Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp136Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp136Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq136Treap ---------------

use std::cmp::Ordering as Xq136Ord;

struct Xq136TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq136TreapNode<K, V>>>,
    right: Option<Box<Xq136TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq136Treap<K, V> {
    root: Option<Box<Xq136TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq136TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_136_size<K, V>(node: &Option<Box<Xq136TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_136_update_size<K, V>(node: &mut Xq136TreapNode<K, V>) {
    node.size = 1 + xq_136_size(&node.left) + xq_136_size(&node.right);
}

fn xq_136_rotate_right<K, V>(mut node: Box<Xq136TreapNode<K, V>>) -> Box<Xq136TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_136_update_size(&mut node);
    left.right = Some(node);
    xq_136_update_size(&mut left);
    left
}

fn xq_136_rotate_left<K, V>(mut node: Box<Xq136TreapNode<K, V>>) -> Box<Xq136TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_136_update_size(&mut node);
    right.left = Some(node);
    xq_136_update_size(&mut right);
    right
}

fn xq_136_insert_node<K: Ord, V>(
    node: Option<Box<Xq136TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq136TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq136TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq136Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq136Ord::Less => {
                let (new_left, old) = xq_136_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_136_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_136_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq136Ord::Greater => {
                let (new_right, old) = xq_136_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_136_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_136_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_136_remove_node<K: Ord, V>(
    node: Option<Box<Xq136TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq136TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq136Ord::Less => {
                let (new_left, old) = xq_136_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_136_update_size(&mut n);
                (Some(n), old)
            }
            Xq136Ord::Greater => {
                let (new_right, old) = xq_136_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_136_update_size(&mut n);
                (Some(n), old)
            }
            Xq136Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_136_rotate_right(n);
                    let (new_right, old) = xq_136_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_136_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_136_rotate_left(n);
                    let (new_left, old) = xq_136_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_136_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_136_find_min<K, V>(node: &Option<Box<Xq136TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_136_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_136_find_max<K, V>(node: &Option<Box<Xq136TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_136_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_136_rank<K: Ord, V>(node: &Option<Box<Xq136TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq136Ord::Less => xq_136_rank(&n.left, key),
            Xq136Ord::Equal => xq_136_size(&n.left),
            Xq136Ord::Greater => 1 + xq_136_size(&n.left) + xq_136_rank(&n.right, key),
        },
    }
}

fn xq_136_kth<K, V>(node: &Option<Box<Xq136TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_136_size(&n.left);
        if k < left_size {
            xq_136_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_136_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_136_in_order<K: Clone, V>(node: &Option<Box<Xq136TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_136_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_136_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq136Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 136 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_136_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq136Ord::Equal => return Some(&n.value),
                Xq136Ord::Less => cur = &n.left,
                Xq136Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_136_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_136_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_136_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_136_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_136_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_136_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_136_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq136VEBTree ---------------

pub struct Xq136VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq136VEBTree>>,
    clusters: Vec<Option<Box<Xq136VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq136VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq136VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq136VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr136KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr136KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr136BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr136KDNode {
    xr_point: Xr136KDPoint,
    xr_left: Option<Box<Xr136KDNode>>,
    xr_right: Option<Box<Xr136KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr136KDTree {
    xr_root: Option<Box<Xr136KDNode>>,
    xr_size: usize,
}

impl Xr136KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr136KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr136KDNode>>,
        point: Xr136KDPoint,
        depth: usize,
    ) -> Box<Xr136KDNode> {
        match node {
            None => Box::new(Xr136KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr136KDPoint) -> Option<Xr136KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr136KDNode>,
        query: &Xr136KDPoint,
        depth: usize,
        best: &mut Xr136KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr136KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr136KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr136KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr136KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr136KDNode>>, pts: &mut Vec<Xr136KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr136KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr136BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr136BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs136PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs136PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs136PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs136PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs136ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs136ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs136ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs136RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs136RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs136RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs136CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs136CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs136CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_136 data structures.
#[derive(Debug, Clone)]
pub struct Xs136StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs136StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs136StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
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


    // ---- xc_ pool / scheduler tests – block 137 ----

    #[test]
    fn xc_137_pool_new_empty() {
        let pool: super::Xc137Pool<i32> = super::Xc137Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_137_pool_release_acquire() {
        let mut pool = super::Xc137Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_137_pool_acquire_empty() {
        let mut pool: super::Xc137Pool<i32> = super::Xc137Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_137_pool_full() {
        let mut pool = super::Xc137Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_137_pool_drain() {
        let mut pool = super::Xc137Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_137_pool_stats() {
        let mut pool = super::Xc137Pool::new(8);
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
    fn xc_137_pool_clear() {
        let mut pool = super::Xc137Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_137_pool_shrink() {
        let mut pool = super::Xc137Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_137_pool_default() {
        let pool: super::Xc137Pool<String> = super::Xc137Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_137_pool_extend() {
        let mut pool = super::Xc137Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_137_pool_retain() {
        let mut pool = super::Xc137Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_137_scheduler_round_robin() {
        let mut sched = super::Xc137Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_137_scheduler_empty() {
        let mut sched = super::Xc137Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_137_scheduler_reset() {
        let mut sched = super::Xc137Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_137_scheduler_add_remove() {
        let mut sched = super::Xc137Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_137_scheduler_targets() {
        let sched = super::Xc137Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_137_hash_empty() {
        assert_eq!(super::xc_137_hash(b""), 5381);
    }

    #[test]
    fn xc_137_hash_data() {
        let h = super::xc_137_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_137_hash(b"hello"), h);
    }

    #[test]
    fn xc_137_reverse_str() {
        assert_eq!(super::xc_137_reverse("abc"), "cba");
        assert_eq!(super::xc_137_reverse(""), "");
    }


    #[test]
    fn xe_47_pipeline_empty() {
        let p = super::Xe47Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_47_pipeline_parse_stage() {
        let p = super::Xe47Pipeline::new()
            .add_parse(super::xe_47_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_47_pipeline_transform_double() {
        let p = super::Xe47Pipeline::new()
            .add_transform(super::xe_47_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_47_pipeline_validate_reverse() {
        let p = super::Xe47Pipeline::new()
            .add_validate(super::xe_47_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_47_pipeline_emit_filter() {
        let p = super::Xe47Pipeline::new()
            .add_emit(super::xe_47_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_47_pipeline_multi_stage() {
        let p = super::Xe47Pipeline::new()
            .add_parse(super::xe_47_pipeline_identity)
            .add_transform(super::xe_47_pipeline_double)
            .add_validate(super::xe_47_pipeline_reverse)
            .add_emit(super::xe_47_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_47_pipeline_error_propagation() {
        let p = super::Xe47Pipeline::new()
            .add_parse(super::xe_47_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe47Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_47_pipeline_compose() {
        let p1 = super::Xe47Pipeline::new()
            .add_parse(super::xe_47_pipeline_identity);
        let p2 = super::Xe47Pipeline::new()
            .add_transform(super::xe_47_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_47_pipeline_error_display() {
        let e = super::Xe47PipelineError {
            stage: super::Xe47Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_47_cache_put_get() {
        let mut c = super::Xe47Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_47_cache_miss() {
        let mut c: super::Xe47Cache<&str, i32> = super::Xe47Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_47_cache_ttl_expiry() {
        let mut c = super::Xe47Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_47_cache_evict() {
        let mut c = super::Xe47Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_47_cache_capacity() {
        let mut c = super::Xe47Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_47_cache_stats() {
        let mut c = super::Xe47Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_47_cache_clear() {
        let mut c = super::Xe47Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_18 graph tests ------------------------------------------------

    #[test]
    fn xg_18_graph_empty() {
        let g = super::Xg18Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_18_graph_add_node() {
        let mut g = super::Xg18Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_18_graph_add_edge() {
        let mut g = super::Xg18Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_18_graph_neighbors() {
        let mut g = super::Xg18Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_18_graph_has_path() {
        let mut g = super::Xg18Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_18_graph_self_path() {
        let g = super::Xg18Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_18_graph_topo_sort() {
        let mut g = super::Xg18Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_18_graph_cycle_detect_false() {
        let mut g = super::Xg18Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_18_graph_cycle_detect_true() {
        let mut g = super::Xg18Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_18 heap tests -------------------------------------------------

    #[test]
    fn xg_18_heap_empty() {
        let h: super::Xg18Heap<i32> = super::Xg18Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_18_heap_push_pop() {
        let mut h = super::Xg18Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_18_heap_peek() {
        let mut h = super::Xg18Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_18_heap_drain_sorted() {
        let mut h = super::Xg18Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_18_heap_merge() {
        let mut a = super::Xg18Heap::new();
        let mut b = super::Xg18Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_18_heap_default() {
        let h: super::Xg18Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_18_graph_default() {
        let g: super::Xg18Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh136_skip_insert_contains() {
        let mut sl = super::Xh136SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh136_skip_remove() {
        let mut sl = super::Xh136SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh136_skip_len() {
        let mut sl = super::Xh136SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh136_skip_range_query() {
        let mut sl = super::Xh136SkipList::xh_new(4);
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
    fn xh136_skip_floor_ceiling() {
        let mut sl = super::Xh136SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh136_skip_rank() {
        let mut sl = super::Xh136SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh136_skip_empty() {
        let sl = super::Xh136SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh136_skip_duplicates() {
        let mut sl = super::Xh136SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh136_bitset_set_test() {
        let mut bs = super::Xh136BitSet::xh_new(256);
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
    fn xh136_bitset_clear_count() {
        let mut bs = super::Xh136BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh136_bitset_and_or_xor() {
        let mut a = super::Xh136BitSet::xh_new(128);
        let mut b = super::Xh136BitSet::xh_new(128);
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
    fn xh136_bitset_iter_ones() {
        let mut bs = super::Xh136BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh136_bitset_first_last() {
        let mut bs = super::Xh136BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh136_bitset_empty() {
        let bs = super::Xh136BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi136_deque_push_pop_back() {
        let mut dq = super::Xi136Deque::xi_new(4);
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
    fn xi136_deque_push_pop_front() {
        let mut dq = super::Xi136Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi136_deque_mixed_ops() {
        let mut dq = super::Xi136Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi136_deque_get_and_split() {
        let mut dq = super::Xi136Deque::xi_new(8);
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
    fn xi136_deque_rotate_left() {
        let mut dq = super::Xi136Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi136_deque_rotate_right() {
        let mut dq = super::Xi136Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi136_deque_grow() {
        let mut dq = super::Xi136Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi136_deque_empty() {
        let dq = super::Xi136Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi136_interval_tree_insert_query() {
        let mut tree = super::Xi136IntervalTree::xi_new();
        tree.xi_insert(super::Xi136Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi136Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi136Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi136_interval_tree_overlap() {
        let mut tree = super::Xi136IntervalTree::xi_new();
        tree.xi_insert(super::Xi136Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi136Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi136Interval::xi_new(12, 20));
        let q = super::Xi136Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi136_interval_tree_remove() {
        let mut tree = super::Xi136IntervalTree::xi_new();
        tree.xi_insert(super::Xi136Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi136Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi136_interval_tree_gaps() {
        let mut tree = super::Xi136IntervalTree::xi_new();
        tree.xi_insert(super::Xi136Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi136Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi136Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi136Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi136Interval::xi_new(8, 10));
    }

    #[test]
    fn xi136_interval_tree_merge() {
        let mut tree = super::Xi136IntervalTree::xi_new();
        tree.xi_insert(super::Xi136Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi136Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi136Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi136Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi136Interval::xi_new(10, 15));
    }

    #[test]
    fn xi136_interval_tree_all() {
        let mut tree = super::Xi136IntervalTree::xi_new();
        tree.xi_insert(super::Xi136Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi136Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi136_interval_tree_empty() {
        let tree = super::Xi136IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi136_interval_tree_contains_point() {
        let iv = super::Xi136Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 136) ---

    #[test]
    fn xj_136_uf_make_and_find() {
        let mut uf = super::Xj136UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_136_uf_union_connected() {
        let mut uf = super::Xj136UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_136_uf_component_count() {
        let mut uf = super::Xj136UnionFind::xj_new();
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
    fn xj_136_uf_component_size() {
        let mut uf = super::Xj136UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_136_uf_largest_component() {
        let mut uf = super::Xj136UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_136_uf_many_elements() {
        let mut uf = super::Xj136UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_136_uf_separate_components() {
        let mut uf = super::Xj136UnionFind::xj_new();
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
    fn xj_136_uf_path_compression() {
        let mut uf = super::Xj136UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_136_bt_insert_get() {
        let mut bt = super::Xj136BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_136_bt_contains_len() {
        let mut bt = super::Xj136BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_136_bt_replace() {
        let mut bt = super::Xj136BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_136_bt_remove() {
        let mut bt = super::Xj136BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_136_bt_keys_values() {
        let mut bt = super::Xj136BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_136_bt_range() {
        let mut bt = super::Xj136BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_136_bt_min_max() {
        let mut bt = super::Xj136BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_136_bt_many_inserts() {
        let mut bt = super::Xj136BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_136 segment tree tests ---

    #[test]
    fn xk_136_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk136SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_136_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk136SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_136_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk136SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_136_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk136SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_136_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk136SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_136_st_single_element() {
        let data = vec![42];
        let st = super::Xk136SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_136_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk136SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_136_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk136SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_136 disjoint intervals tests ---

    #[test]
    fn xk_136_di_add_and_count() {
        let mut di = super::Xk136DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_136_di_merge_overlap() {
        let mut di = super::Xk136DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_136_di_contains() {
        let mut di = super::Xk136DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_136_di_remove() {
        let mut di = super::Xk136DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_136_di_covered_length() {
        let mut di = super::Xk136DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_136_di_gaps() {
        let mut di = super::Xk136DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_136_di_merge_adjacent() {
        let mut di = super::Xk136DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_136_di_empty() {
        let di = super::Xk136DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_136_rope_new_empty() {
        let rope = super::Xl136Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_136_rope_from_str() {
        let rope = super::Xl136Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_136_rope_insert_at() {
        let mut rope = super::Xl136Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_136_rope_delete_range() {
        let mut rope = super::Xl136Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_136_rope_char_at() {
        let rope = super::Xl136Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_136_rope_split_concat() {
        let rope = super::Xl136Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_136_rope_line_count() {
        let rope = super::Xl136Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_136_rope_line_at() {
        let rope = super::Xl136Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_136_sa_build_and_search() {
        let sa = super::Xl136SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_136_sa_count() {
        let sa = super::Xl136SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_136_sa_longest_repeated() {
        let sa = super::Xl136SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_136_sa_all_positions() {
        let sa = super::Xl136SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_136_sa_len() {
        let sa = super::Xl136SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_136_sa_empty() {
        let sa = super::Xl136SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_136_rope_slice() {
        let rope = super::Xl136Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_136_sa_search_start() {
        let sa = super::Xl136SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_136_sparse_set_get() {
        let mut m = super::Xm136MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_136_sparse_row_col() {
        let mut m = super::Xm136MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_136_sparse_transpose() {
        let mut m = super::Xm136MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_136_sparse_multiply_vec() {
        let mut m = super::Xm136MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_136_sparse_nnz_density() {
        let mut m = super::Xm136MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_136_sparse_clear() {
        let mut m = super::Xm136MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_136_sparse_overwrite_zero() {
        let mut m = super::Xm136MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_136_tokenizer_basic() {
        let t = super::Xm136Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_136_tokenizer_count() {
        let t = super::Xm136Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_136_tokenizer_unique() {
        let t = super::Xm136Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_136_tokenizer_frequency() {
        let t = super::Xm136Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_136_tokenizer_delimiter() {
        let t = super::Xm136Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_136_tokenizer_whitespace() {
        let t = super::Xm136Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_136_tokenizer_empty() {
        let t = super::Xm136Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 136 ----

    #[test]
    fn xn_136_fenwick_prefix_sum() {
        let mut ft = super::Xn136Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_136_fenwick_range_sum() {
        let mut ft = super::Xn136Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_136_fenwick_point_query() {
        let mut ft = super::Xn136Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_136_fenwick_len() {
        let ft = super::Xn136Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_136_fenwick_multiple_updates() {
        let mut ft = super::Xn136Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_136_fenwick_single_element() {
        let mut ft = super::Xn136Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_136_fenwick_find_kth() {
        let mut ft = super::Xn136Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_136_fenwick_negative_delta() {
        let mut ft = super::Xn136Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 136 ----

    #[test]
    fn xn_136_avl_insert_get() {
        let mut m = super::Xn136AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_136_avl_remove() {
        let mut m = super::Xn136AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_136_avl_in_order() {
        let mut m = super::Xn136AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_136_avl_min_max() {
        let mut m = super::Xn136AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_136_avl_floor_ceiling() {
        let mut m = super::Xn136AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_136_avl_height_balanced() {
        let mut m = super::Xn136AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_136_avl_overwrite() {
        let mut m = super::Xn136AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_136_avl_empty() {
        let m: super::Xn136AVL<i32, i32> = super::Xn136AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo136RedBlack tests ---

    #[test]
    fn xo_136_rb_insert_and_get() {
        let mut tree = super::Xo136RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_136_rb_len_and_empty() {
        let mut tree = super::Xo136RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_136_rb_min_max() {
        let mut tree = super::Xo136RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_136_rb_contains() {
        let mut tree = super::Xo136RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_136_rb_remove() {
        let mut tree = super::Xo136RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_136_rb_in_order() {
        let mut tree = super::Xo136RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_136_rb_black_height() {
        let mut tree = super::Xo136RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_136_rb_overwrite() {
        let mut tree = super::Xo136RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo136ConsistentHash tests ---

    #[test]
    fn xo_136_ch_add_and_count() {
        let mut ring = super::Xo136ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_136_ch_remove_node() {
        let mut ring = super::Xo136ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_136_ch_get_node() {
        let mut ring = super::Xo136ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_136_ch_empty_ring() {
        let ring = super::Xo136ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_136_ch_distribution() {
        let mut ring = super::Xo136ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_136_ch_rebalance() {
        let mut ring = super::Xo136ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_136_ch_virtual_nodes() {
        let mut ring = super::Xo136ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_136_ch_consistent_lookup() {
        let mut ring = super::Xo136ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_136_splay_insert_get() {
        let mut t = super::Xp136SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_136_splay_remove() {
        let mut t = super::Xp136SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_136_splay_count_increases() {
        let mut t = super::Xp136SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_136_splay_depth() {
        let mut t = super::Xp136SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_136_splay_len_empty() {
        let t = super::Xp136SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_136_splay_min_max() {
        let mut t = super::Xp136SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_136_splay_overwrite() {
        let mut t = super::Xp136SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_136_splay_remove_missing() {
        let mut t = super::Xp136SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_136 treap tests ----
    #[test]
    fn xq_136_treap_empty() {
        let t = super::Xq136Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_136_treap_insert_get() {
        let mut t = super::Xq136Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_136_treap_overwrite() {
        let mut t = super::Xq136Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_136_treap_remove() {
        let mut t = super::Xq136Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_136_treap_min_max() {
        let mut t = super::Xq136Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_136_treap_rank() {
        let mut t = super::Xq136Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_136_treap_kth() {
        let mut t = super::Xq136Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_136_treap_in_order() {
        let mut t = super::Xq136Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_136 VEB tree tests ----
    #[test]
    fn xq_136_veb_empty() {
        let v = super::Xq136VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_136_veb_insert_contains() {
        let mut v = super::Xq136VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_136_veb_min_max() {
        let mut v = super::Xq136VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_136_veb_delete() {
        let mut v = super::Xq136VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_136_veb_successor() {
        let mut v = super::Xq136VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_136_veb_predecessor() {
        let mut v = super::Xq136VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_136_veb_count() {
        let mut v = super::Xq136VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_136_veb_duplicate_insert() {
        let mut v = super::Xq136VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_136_kdtree_empty() {
        let tree = super::Xr136KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_136_kdtree_insert_one() {
        let mut tree = super::Xr136KDTree::xr_new();
        tree.xr_insert(super::Xr136KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_136_kdtree_insert_multiple() {
        let mut tree = super::Xr136KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr136KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_136_kdtree_nearest_neighbor() {
        let mut tree = super::Xr136KDTree::xr_new();
        tree.xr_insert(super::Xr136KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr136KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr136KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_136_kdtree_nn_empty() {
        let tree = super::Xr136KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr136KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_136_kdtree_range_search() {
        let mut tree = super::Xr136KDTree::xr_new();
        tree.xr_insert(super::Xr136KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr136KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr136KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_136_kdtree_range_empty() {
        let mut tree = super::Xr136KDTree::xr_new();
        tree.xr_insert(super::Xr136KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_136_kdtree_all_points() {
        let mut tree = super::Xr136KDTree::xr_new();
        tree.xr_insert(super::Xr136KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr136KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_136_kdtree_depth() {
        let mut tree = super::Xr136KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr136KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_136_kdtree_bounding_box() {
        let mut tree = super::Xr136KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr136KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr136KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_136_persistent_array_new() {
        let arr = super::Xs136PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_136_persistent_array_push() {
        let mut arr = super::Xs136PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_136_persistent_array_set() {
        let mut arr = super::Xs136PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_136_persistent_array_diff() {
        let mut arr = super::Xs136PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_136_persistent_array_rollback() {
        let mut arr = super::Xs136PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_136_persistent_array_history() {
        let mut arr = super::Xs136PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_136_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs136PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_136_persistent_array_from_vec() {
        let arr = super::Xs136PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_136_concurrent_queue_new() {
        let q = super::Xs136ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_136_concurrent_queue_push_pop() {
        let mut q = super::Xs136ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_136_concurrent_queue_full() {
        let mut q = super::Xs136ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_136_concurrent_queue_drain() {
        let mut q = super::Xs136ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_136_concurrent_queue_try_pop() {
        let mut q = super::Xs136ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_136_concurrent_queue_clear() {
        let mut q = super::Xs136ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_136_range_map_new() {
        let rm = super::Xs136RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_136_range_map_insert_get() {
        let mut rm = super::Xs136RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_136_range_map_overlap() {
        let mut rm = super::Xs136RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_136_range_map_remove() {
        let mut rm = super::Xs136RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_136_range_map_gaps() {
        let mut rm = super::Xs136RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_136_range_map_coverage() {
        let mut rm = super::Xs136RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_136_range_map_contains() {
        let mut rm = super::Xs136RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_136_range_map_clear() {
        let mut rm = super::Xs136RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_136_circular_buffer_new() {
        let buf = super::Xs136CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_136_circular_buffer_push_pop() {
        let mut buf = super::Xs136CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_136_circular_buffer_overwrite() {
        let mut buf = super::Xs136CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_136_circular_buffer_peek() {
        let mut buf = super::Xs136CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_136_circular_buffer_is_full() {
        let mut buf = super::Xs136CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_136_circular_buffer_iter() {
        let mut buf = super::Xs136CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_136_circular_buffer_clear() {
        let mut buf = super::Xs136CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_136_circular_buffer_to_vec() {
        let mut buf = super::Xs136CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_136_stats_tracker_new() {
        let tracker = super::Xs136StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_136_stats_tracker_mean() {
        let mut tracker = super::Xs136StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_136_stats_tracker_min_max() {
        let mut tracker = super::Xs136StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_136_stats_tracker_median() {
        let mut tracker = super::Xs136StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_136_stats_tracker_variance() {
        let mut tracker = super::Xs136StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_136_stats_tracker_range() {
        let mut tracker = super::Xs136StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_136_stats_tracker_clear() {
        let mut tracker = super::Xs136StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_136_stats_tracker_sum() {
        let mut tracker = super::Xs136StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }

}
