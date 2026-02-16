//! Environment and paths service.
//!
//! Equivalent to VS Code's `vs/platform/environment/common/environment.ts`.
//! Provides well-known paths and CLI arguments for vsedit.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Errors that can occur in the environment service.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentError {
    /// A required path was empty or invalid.
    InvalidPath(String),
    /// The goto specification is malformed.
    InvalidGoto(String),
    /// Conflicting CLI flags were specified.
    ConflictingFlags(String),
    /// A directory operation failed.
    IoError(String),
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(msg) => write!(f, "invalid path: {msg}"),
            Self::InvalidGoto(msg) => write!(f, "invalid goto: {msg}"),
            Self::ConflictingFlags(msg) => write!(f, "conflicting flags: {msg}"),
            Self::IoError(msg) => write!(f, "io error: {msg}"),
        }
    }
}

impl std::error::Error for EnvironmentError {}

impl From<std::io::Error> for EnvironmentError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e.to_string())
    }
}

/// Well-known directory paths for vsedit data.
#[derive(Debug, Clone, PartialEq)]
pub struct AppPaths {
    /// User data directory (~/.config/vsedit on Linux).
    pub user_data: PathBuf,
    /// Extensions directory.
    pub extensions: PathBuf,
    /// User settings file.
    pub settings_file: PathBuf,
    /// User keybindings file.
    pub keybindings_file: PathBuf,
    /// Snippets directory.
    pub snippets: PathBuf,
    /// Global storage directory.
    pub global_storage: PathBuf,
    /// Workspace storage directory.
    pub workspace_storage: PathBuf,
    /// Log directory.
    pub logs: PathBuf,
    /// Temp directory.
    pub tmp: PathBuf,
}

impl AppPaths {
    /// Create paths using platform-appropriate defaults.
    pub fn defaults() -> Self {
        let data_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vsedit");

        Self::from_user_data(data_dir)
    }

    /// Create paths rooted at a specific data directory.
    pub fn from_user_data(user_data: PathBuf) -> Self {
        let extensions = user_data.join("extensions");
        let settings_file = user_data.join("User").join("settings.json");
        let keybindings_file = user_data.join("User").join("keybindings.json");
        let snippets = user_data.join("User").join("snippets");
        let global_storage = user_data.join("globalStorage");
        let workspace_storage = user_data.join("workspaceStorage");
        let logs = user_data.join("logs");
        let tmp = user_data.join("tmp");

        Self {
            user_data,
            extensions,
            settings_file,
            keybindings_file,
            snippets,
            global_storage,
            workspace_storage,
            logs,
            tmp,
        }
    }

    /// Ensure all directories exist.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for dir in [
            &self.user_data,
            &self.extensions,
            &self.snippets,
            &self.global_storage,
            &self.workspace_storage,
            &self.logs,
            &self.tmp,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        // Ensure parent of settings file exists.
        if let Some(parent) = self.settings_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Return all directory paths managed by this instance.
    pub fn all_directories(&self) -> Vec<&Path> {
        vec![
            &self.user_data,
            &self.extensions,
            &self.snippets,
            &self.global_storage,
            &self.workspace_storage,
            &self.logs,
            &self.tmp,
        ]
    }

    /// Check whether every managed directory already exists on disk.
    pub fn all_dirs_exist(&self) -> bool {
        self.all_directories().iter().all(|p| p.is_dir())
    }

    /// Return the path for a named extension inside the extensions directory.
    pub fn extension_path(&self, extension_id: &str) -> PathBuf {
        self.extensions.join(extension_id)
    }

    /// Return the workspace-specific storage path for a given workspace id.
    pub fn workspace_storage_for(&self, workspace_id: &str) -> PathBuf {
        self.workspace_storage.join(workspace_id)
    }

    /// Return the log file path for a given session identifier.
    pub fn log_file_for(&self, session_id: &str) -> PathBuf {
        self.logs.join(format!("{session_id}.log"))
    }
}

impl fmt::Display for AppPaths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AppPaths({})", self.user_data.display())
    }
}

/// Parsed CLI arguments.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliArgs {
    /// Files or folders to open.
    pub paths: Vec<PathBuf>,
    /// Go to line:column in first file.
    pub goto: Option<(u32, u32)>,
    /// Start in diff mode.
    pub diff: bool,
    /// Wait for files to be closed.
    pub wait: bool,
    /// Create new window.
    pub new_window: bool,
    /// Reuse existing window.
    pub reuse_window: bool,
    /// Log level.
    pub log_level: Option<String>,
    /// Extensions dir override.
    pub extensions_dir: Option<PathBuf>,
    /// User data dir override.
    pub user_data_dir: Option<PathBuf>,
    /// Disable all extensions.
    pub disable_extensions: bool,
    /// Verbose output.
    pub verbose: bool,
    /// Start in 3-way merge mode.
    pub merge: bool,
    /// Override display locale (e.g. "en-US").
    pub locale: Option<String>,
}

impl CliArgs {
    /// Validate that flags do not conflict.
    pub fn validate(&self) -> Result<(), EnvironmentError> {
        if self.new_window && self.reuse_window {
            return Err(EnvironmentError::ConflictingFlags(
                "cannot specify both --new-window and --reuse-window".into(),
            ));
        }
        if self.diff && self.paths.len() != 2 {
            return Err(EnvironmentError::ConflictingFlags(
                "diff mode requires exactly two paths".into(),
            ));
        }
        if self.merge && self.paths.len() != 3 {
            return Err(EnvironmentError::ConflictingFlags(
                "merge mode requires exactly three paths (mine, base, theirs)".into(),
            ));
        }
        if self.diff && self.merge {
            return Err(EnvironmentError::ConflictingFlags(
                "cannot specify both --diff and --merge".into(),
            ));
        }
        if let Some((line, col)) = self.goto {
            if line == 0 || col == 0 {
                return Err(EnvironmentError::InvalidGoto(
                    "line and column must be >= 1".into(),
                ));
            }
        }
        Ok(())
    }

    /// Parse a "line:column" string into a goto tuple.
    pub fn parse_goto(s: &str) -> Result<(u32, u32), EnvironmentError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(EnvironmentError::InvalidGoto(format!(
                "expected line:column, got '{s}'"
            )));
        }
        let line: u32 = parts[0]
            .parse()
            .map_err(|_| EnvironmentError::InvalidGoto(format!("invalid line number: '{}'", parts[0])))?;
        let col: u32 = parts[1]
            .parse()
            .map_err(|_| EnvironmentError::InvalidGoto(format!("invalid column number: '{}'", parts[1])))?;
        if line == 0 || col == 0 {
            return Err(EnvironmentError::InvalidGoto(
                "line and column must be >= 1".into(),
            ));
        }
        Ok((line, col))
    }

    /// Return the number of file/folder paths specified.
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Whether this invocation requests opening specific files or folders.
    pub fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }
}

/// Builder for constructing [`CliArgs`] incrementally.
#[derive(Debug, Default)]
pub struct CliArgsBuilder {
    inner: CliArgs,
}

impl CliArgsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path(mut self, p: impl Into<PathBuf>) -> Self {
        self.inner.paths.push(p.into());
        self
    }

    pub fn goto(mut self, line: u32, col: u32) -> Self {
        self.inner.goto = Some((line, col));
        self
    }

    pub fn diff(mut self, enabled: bool) -> Self {
        self.inner.diff = enabled;
        self
    }

    pub fn wait(mut self, enabled: bool) -> Self {
        self.inner.wait = enabled;
        self
    }

    pub fn new_window(mut self, enabled: bool) -> Self {
        self.inner.new_window = enabled;
        self
    }

    pub fn reuse_window(mut self, enabled: bool) -> Self {
        self.inner.reuse_window = enabled;
        self
    }

    pub fn verbose(mut self, enabled: bool) -> Self {
        self.inner.verbose = enabled;
        self
    }

    pub fn disable_extensions(mut self, enabled: bool) -> Self {
        self.inner.disable_extensions = enabled;
        self
    }

    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.inner.log_level = Some(level.into());
        self
    }

    pub fn extensions_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.inner.extensions_dir = Some(dir.into());
        self
    }

    pub fn user_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.inner.user_data_dir = Some(dir.into());
        self
    }

    pub fn merge(mut self, enabled: bool) -> Self {
        self.inner.merge = enabled;
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.inner.locale = Some(locale.into());
        self
    }

    /// Validate and build the [`CliArgs`].
    pub fn build(self) -> Result<CliArgs, EnvironmentError> {
        self.inner.validate()?;
        Ok(self.inner)
    }
}

/// The environment service providing runtime information.
pub struct EnvironmentService {
    pub paths: AppPaths,
    pub args: CliArgs,
    pub app_name: String,
    pub app_version: String,
}

impl EnvironmentService {
    pub fn new(args: CliArgs) -> Self {
        let user_data = args
            .user_data_dir
            .clone()
            .unwrap_or_else(|| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("vsedit")
            });

        let mut paths = AppPaths::from_user_data(user_data);
        if let Some(ext_dir) = &args.extensions_dir {
            paths.extensions = ext_dir.clone();
        }

        Self {
            paths,
            args,
            app_name: "vsedit".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn is_verbose(&self) -> bool {
        self.args.verbose
    }

    pub fn is_extensions_disabled(&self) -> bool {
        self.args.disable_extensions
    }

    /// Return the resolved log level, defaulting to `"info"`.
    pub fn log_level(&self) -> &str {
        self.args.log_level.as_deref().unwrap_or("info")
    }

    /// Return a summary string suitable for startup logging.
    pub fn startup_summary(&self) -> String {
        format!(
            "{} v{} | data={} | extensions={} | verbose={} | log_level={}",
            self.app_name,
            self.app_version,
            self.paths.user_data.display(),
            if self.is_extensions_disabled() { "disabled" } else { "enabled" },
            self.is_verbose(),
            self.log_level(),
        )
    }
}

impl fmt::Display for EnvironmentService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} v{}", self.app_name, self.app_version)
    }
}

impl fmt::Debug for EnvironmentService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvironmentService")
            .field("app_name", &self.app_name)
            .field("app_version", &self.app_version)
            .field("paths", &self.paths)
            .field("args", &self.args)
            .finish()
    }
}

/// Accumulated statistics for environment operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl EnvironmentStats {
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
    pub fn merge(&mut self, other: &EnvironmentStats) {
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

impl Default for EnvironmentStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EnvironmentStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EnvironmentStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for environment.
#[derive(Debug, Clone)]
pub struct EnvironmentValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl EnvironmentValidator {
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

impl Default for EnvironmentValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolves environment variable references in strings.
/// Replaces `${env:VAR_NAME}` with the value of the environment variable.
pub fn resolve_env_variables(input: &str, env_getter: &dyn Fn(&str) -> Option<String>) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut var_ref = String::new();
            while let Some(&c) = chars.peek() {
                if c == '}' {
                    chars.next();
                    break;
                }
                var_ref.push(c);
                chars.next();
            }
            if let Some(var_name) = var_ref.strip_prefix("env:") {
                if let Some(val) = env_getter(var_name) {
                    result.push_str(&val);
                } else {
                    result.push_str(&format!("${{env:{}}}", var_name));
                }
            } else {
                result.push_str(&format!("${{{}}}", var_ref));
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Resolve environment variables using the actual process environment.
pub fn resolve_env_variables_from_process(input: &str) -> String {
    resolve_env_variables(input, &|name| std::env::var(name).ok())
}

/// Captured snapshot of the shell environment.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellEnvironment {
    variables: HashMap<String, String>,
}

impl ShellEnvironment {
    /// Create an empty shell environment.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Capture the current process environment.
    pub fn capture() -> Self {
        let variables: HashMap<String, String> = std::env::vars().collect();
        Self { variables }
    }

    /// Create from a set of key-value pairs.
    pub fn from_pairs(pairs: Vec<(&str, &str)>) -> Self {
        let variables = pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Self { variables }
    }

    /// Get a variable value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }

    /// Set a variable.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    /// Remove a variable.
    pub fn remove(&mut self, key: &str) -> bool {
        self.variables.remove(key).is_some()
    }

    /// Number of variables.
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// Get all variable names.
    pub fn keys(&self) -> Vec<&str> {
        self.variables.keys().map(|s| s.as_str()).collect()
    }

    /// Merge another environment into this one (other takes precedence).
    pub fn merge(&mut self, other: &ShellEnvironment) {
        for (k, v) in &other.variables {
            self.variables.insert(k.clone(), v.clone());
        }
    }

    /// Create a getter function compatible with resolve_env_variables.
    pub fn as_getter(&self) -> impl Fn(&str) -> Option<String> + '_ {
        move |name: &str| self.variables.get(name).cloned()
    }
}

impl Default for ShellEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ShellEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ShellEnvironment({} vars)", self.variables.len())
    }
}

/// Parse a PATH-style environment variable into a list of paths.
/// Uses `:` as separator on Unix, `;` on Windows.
pub fn env_path_list(value: &str) -> Vec<PathBuf> {
    let separator = if cfg!(windows) { ';' } else { ':' };
    value
        .split(separator)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Join a list of paths into a PATH-style string.
pub fn env_path_join(paths: &[PathBuf]) -> String {
    let separator = if cfg!(windows) { ";" } else { ":" };
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Prepend a path to a PATH-style value, deduplicating.
pub fn env_path_prepend(path: &Path, existing: &str) -> String {
    let mut paths = env_path_list(existing);
    paths.retain(|p| p != path);
    paths.insert(0, path.to_path_buf());
    env_path_join(&paths)
}

/// Check if a path is in a PATH-style variable.
pub fn env_path_contains(value: &str, path: &Path) -> bool {
    env_path_list(value).iter().any(|p| p == path)
}

/// A frozen snapshot of a [`ShellEnvironment`] that can be compared or restored.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvSnapshot {
    variables: HashMap<String, String>,
    timestamp_epoch_ms: u64,
    label: Option<String>,
}

impl EnvSnapshot {
    /// Take a snapshot of the given shell environment.
    pub fn take(env: &ShellEnvironment, label: Option<&str>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            variables: env.variables.clone(),
            timestamp_epoch_ms: now,
            label: label.map(|s| s.to_string()),
        }
    }

    /// Create a snapshot directly from a set of key-value pairs (useful for tests).
    pub fn from_pairs(pairs: Vec<(&str, &str)>, label: Option<&str>) -> Self {
        let variables = pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Self {
            variables,
            timestamp_epoch_ms: 0,
            label: label.map(|s| s.to_string()),
        }
    }

    /// Restore this snapshot into a [`ShellEnvironment`], replacing all variables.
    pub fn restore(&self) -> ShellEnvironment {
        ShellEnvironment {
            variables: self.variables.clone(),
        }
    }

    /// Return the optional label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Return the epoch-millisecond timestamp when the snapshot was taken.
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_epoch_ms
    }

    /// Return the number of variables captured.
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// Get a variable value from the snapshot.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }
}

impl fmt::Display for EnvSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EnvSnapshot({} vars, label={:?})",
            self.variables.len(),
            self.label
        )
    }
}

/// Represents the difference between two environment states.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvDiff {
    /// Variables added (present in `after` but not `before`).
    pub added: HashMap<String, String>,
    /// Variables removed (present in `before` but not `after`).
    pub removed: HashMap<String, String>,
    /// Variables whose values changed: key → (old_value, new_value).
    pub changed: HashMap<String, (String, String)>,
}

impl EnvDiff {
    /// Compute the diff between two snapshots.
    pub fn between(before: &EnvSnapshot, after: &EnvSnapshot) -> Self {
        let mut added = HashMap::new();
        let mut removed = HashMap::new();
        let mut changed = HashMap::new();

        for (k, v) in &after.variables {
            match before.variables.get(k) {
                None => {
                    added.insert(k.clone(), v.clone());
                }
                Some(old_v) if old_v != v => {
                    changed.insert(k.clone(), (old_v.clone(), v.clone()));
                }
                _ => {}
            }
        }
        for (k, v) in &before.variables {
            if !after.variables.contains_key(k) {
                removed.insert(k.clone(), v.clone());
            }
        }

        Self { added, removed, changed }
    }

    /// Whether no differences exist.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Total number of individual changes (additions + removals + modifications).
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }

    /// Return all affected variable names.
    pub fn affected_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .added
            .keys()
            .chain(self.removed.keys())
            .chain(self.changed.keys())
            .map(|s| s.as_str())
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

impl fmt::Display for EnvDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EnvDiff(+{} -{} ~{})",
            self.added.len(),
            self.removed.len(),
            self.changed.len()
        )
    }
}

/// Manages a PATH-style environment variable, providing typed operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvPathManager {
    entries: Vec<PathBuf>,
}

impl EnvPathManager {
    /// Create an empty path manager.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Parse from a PATH-style string.
    pub fn from_path_string(value: &str) -> Self {
        Self {
            entries: env_path_list(value),
        }
    }

    /// Prepend a path, removing any existing duplicate.
    pub fn prepend(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        self.entries.retain(|p| p != &path);
        self.entries.insert(0, path);
    }

    /// Append a path, removing any existing duplicate.
    pub fn append(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        self.entries.retain(|p| p != &path);
        self.entries.push(path);
    }

    /// Remove a path. Returns true if it was present.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.entries.len();
        self.entries.retain(|p| p != path);
        self.entries.len() < before
    }

    /// Check whether a path is contained in the list.
    pub fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|p| p == path)
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the path list is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return the entries as a slice.
    pub fn entries(&self) -> &[PathBuf] {
        &self.entries
    }

    /// Serialize back to a PATH-style string.
    pub fn to_path_string(&self) -> String {
        env_path_join(&self.entries)
    }

    /// Remove entries that do not exist on disk.
    pub fn remove_nonexistent(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|p| p.exists());
        before - self.entries.len()
    }

    /// Deduplicate entries, keeping the first occurrence of each path.
    pub fn dedup(&mut self) {
        let mut seen = Vec::new();
        self.entries.retain(|p| {
            if seen.contains(p) {
                false
            } else {
                seen.push(p.clone());
                true
            }
        });
    }
}

impl Default for EnvPathManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EnvPathManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnvPathManager({} entries)", self.entries.len())
    }
}

/// A named collection of environment variable overrides that can be applied together.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvProfile {
    name: String,
    overrides: HashMap<String, Option<String>>,
}

impl EnvProfile {
    /// Create a new empty profile with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            overrides: HashMap::new(),
        }
    }

    /// Set a variable in this profile.
    pub fn set_var(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.overrides.insert(key.into(), Some(value.into()));
    }

    /// Mark a variable for removal in this profile.
    pub fn unset_var(&mut self, key: impl Into<String>) {
        self.overrides.insert(key.into(), None);
    }

    /// Return the profile name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the number of overrides.
    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    /// Whether the profile has no overrides.
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Apply this profile to a [`ShellEnvironment`].
    pub fn apply_to(&self, env: &mut ShellEnvironment) {
        for (key, value) in &self.overrides {
            match value {
                Some(v) => env.set(key.clone(), v.clone()),
                None => {
                    env.remove(key);
                }
            }
        }
    }

    /// Return all keys that this profile would modify.
    pub fn affected_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.overrides.keys().map(|s| s.as_str()).collect();
        keys.sort();
        keys
    }
}

impl fmt::Display for EnvProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnvProfile({}, {} overrides)", self.name, self.overrides.len())
    }
}

// ---------------------------------------------------------------------------
// Additional impl blocks for existing types
// ---------------------------------------------------------------------------

impl AppPaths {
    /// Return the path for a session-specific temporary directory.
    pub fn tmp_for_session(&self, session_id: &str) -> PathBuf {
        self.tmp.join(session_id)
    }

    /// Return the path for a named global storage bucket.
    pub fn global_storage_for(&self, bucket: &str) -> PathBuf {
        self.global_storage.join(bucket)
    }

    /// Return the User directory (parent of settings / keybindings).
    pub fn user_dir(&self) -> PathBuf {
        self.user_data.join("User")
    }

    /// Check whether the settings file exists on disk.
    pub fn has_settings_file(&self) -> bool {
        self.settings_file.is_file()
    }

    /// Check whether the keybindings file exists on disk.
    pub fn has_keybindings_file(&self) -> bool {
        self.keybindings_file.is_file()
    }

    /// Return the number of managed directory paths.
    pub fn directory_count(&self) -> usize {
        self.all_directories().len()
    }
}

impl CliArgs {
    /// Whether this invocation is in any special mode (diff, merge).
    pub fn is_special_mode(&self) -> bool {
        self.diff || self.merge
    }

    /// Return the first path, if any.
    pub fn first_path(&self) -> Option<&Path> {
        self.paths.first().map(|p| p.as_path())
    }

    /// Return a human-readable summary of the invocation.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.diff {
            parts.push("diff".to_string());
        }
        if self.merge {
            parts.push("merge".to_string());
        }
        if self.wait {
            parts.push("wait".to_string());
        }
        if self.new_window {
            parts.push("new-window".to_string());
        }
        if self.reuse_window {
            parts.push("reuse-window".to_string());
        }
        if self.verbose {
            parts.push("verbose".to_string());
        }
        if self.disable_extensions {
            parts.push("no-extensions".to_string());
        }
        if let Some((l, c)) = self.goto {
            parts.push(format!("goto={l}:{c}"));
        }
        parts.push(format!("{} path(s)", self.paths.len()));
        parts.join(", ")
    }

    /// Return the effective locale, defaulting to `"en-US"`.
    pub fn effective_locale(&self) -> &str {
        self.locale.as_deref().unwrap_or("en-US")
    }
}

impl fmt::Display for CliArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CliArgs({})", self.summary())
    }
}

impl ShellEnvironment {
    /// Return all key-value pairs as a sorted vector of tuples.
    pub fn sorted_pairs(&self) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = self
            .variables
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        pairs.sort_by_key(|(k, _)| *k);
        pairs
    }

    /// Return only the variables whose keys start with the given prefix.
    pub fn filter_by_prefix(&self, prefix: &str) -> HashMap<String, String> {
        self.variables
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Return true if a variable is set and non-empty.
    pub fn has_nonempty(&self, key: &str) -> bool {
        self.variables
            .get(key)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    /// Resolve a template string using this environment as the variable source.
    pub fn resolve(&self, template: &str) -> String {
        resolve_env_variables(template, &self.as_getter())
    }
}

impl EnvSnapshot {
    /// Compute the diff from this snapshot to another.
    pub fn diff_to(&self, other: &EnvSnapshot) -> EnvDiff {
        EnvDiff::between(self, other)
    }

    /// Return all keys in the snapshot, sorted.
    pub fn sorted_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.variables.keys().map(|s| s.as_str()).collect();
        keys.sort();
        keys
    }

    /// Check whether a specific key is present.
    pub fn contains_key(&self, key: &str) -> bool {
        self.variables.contains_key(key)
    }
}

impl EnvDiff {
    /// Apply this diff to a `ShellEnvironment`, producing the "after" state.
    pub fn apply_to(&self, env: &mut ShellEnvironment) {
        for key in self.removed.keys() {
            env.remove(key);
        }
        for (key, value) in &self.added {
            env.set(key.clone(), value.clone());
        }
        for (key, (_old, new)) in &self.changed {
            env.set(key.clone(), new.clone());
        }
    }

    /// Return true if only additions exist (no removals or modifications).
    pub fn is_additive_only(&self) -> bool {
        self.removed.is_empty() && self.changed.is_empty()
    }
}

impl EnvPathManager {
    /// Return the entry at the given index, if it exists.
    pub fn get(&self, index: usize) -> Option<&Path> {
        self.entries.get(index).map(|p| p.as_path())
    }

    /// Swap two entries by index. Returns false if either index is out of bounds.
    pub fn swap(&mut self, a: usize, b: usize) -> bool {
        if a >= self.entries.len() || b >= self.entries.len() {
            return false;
        }
        self.entries.swap(a, b);
        true
    }

    /// Return the position of a path, if present.
    pub fn position(&self, path: &Path) -> Option<usize> {
        self.entries.iter().position(|p| p == path)
    }

    /// Create from an iterator of paths.
    pub fn from_iter(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            entries: paths.into_iter().collect(),
        }
    }
}

impl EnvProfile {
    /// Merge another profile into this one (other's overrides take precedence).
    pub fn merge(&mut self, other: &EnvProfile) {
        for (k, v) in &other.overrides {
            self.overrides.insert(k.clone(), v.clone());
        }
    }

    /// Check whether a specific key is affected by this profile.
    pub fn affects(&self, key: &str) -> bool {
        self.overrides.contains_key(key)
    }

    /// Return the override value for a key, if any (None means "unset").
    pub fn get_override(&self, key: &str) -> Option<Option<&str>> {
        self.overrides.get(key).map(|v| v.as_deref())
    }
}

impl EnvironmentStats {
    /// Return true if no failures have been recorded.
    pub fn is_all_success(&self) -> bool {
        self.failed_operations == 0
    }

    /// Return the total time spent in operations, in milliseconds.
    pub fn total_time_ms(&self) -> f64 {
        self.total_time_ns as f64 / 1_000_000.0
    }

    /// Return a human-readable summary string.
    pub fn summary(&self) -> String {
        format!(
            "{} ops ({} ok, {} err) avg={:.2}ms",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.total_time_ms() / self.total_operations.max(1) as f64,
        )
    }
}

impl EnvironmentValidator {
    /// Validate a path string, ensuring it is non-empty and does not contain null bytes.
    pub fn validate_path(path: &str) -> Result<(), EnvironmentError> {
        if path.is_empty() {
            return Err(EnvironmentError::InvalidPath(
                "path must not be empty".into(),
            ));
        }
        if path.contains('\0') {
            return Err(EnvironmentError::InvalidPath(
                "path must not contain null bytes".into(),
            ));
        }
        Ok(())
    }

    /// Normalize a path string by collapsing consecutive separators.
    pub fn normalize_path(path: &str) -> String {
        let sep = std::path::MAIN_SEPARATOR;
        let mut result = String::with_capacity(path.len());
        let mut prev_sep = false;
        for ch in path.chars() {
            let is_sep = ch == '/' || ch == '\\';
            if is_sep {
                if !prev_sep {
                    result.push(sep);
                }
                prev_sep = true;
            } else {
                result.push(ch);
                prev_sep = false;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_structure() {
        let paths = AppPaths::from_user_data(PathBuf::from("/tmp/test-vsedit"));
        assert_eq!(paths.extensions, PathBuf::from("/tmp/test-vsedit/extensions"));
        assert_eq!(
            paths.settings_file,
            PathBuf::from("/tmp/test-vsedit/User/settings.json")
        );
        assert_eq!(
            paths.keybindings_file,
            PathBuf::from("/tmp/test-vsedit/User/keybindings.json")
        );
        assert_eq!(
            paths.global_storage,
            PathBuf::from("/tmp/test-vsedit/globalStorage")
        );
    }

    #[test]
    fn ensure_dirs_creates_directories() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_user_data(dir.path().join("vsedit"));
        paths.ensure_dirs().unwrap();

        assert!(paths.user_data.is_dir());
        assert!(paths.extensions.is_dir());
        assert!(paths.global_storage.is_dir());
        assert!(paths.logs.is_dir());
    }

    #[test]
    fn environment_service_overrides() {
        let args = CliArgs {
            user_data_dir: Some(PathBuf::from("/custom/data")),
            extensions_dir: Some(PathBuf::from("/custom/ext")),
            verbose: true,
            ..Default::default()
        };
        let env_svc = EnvironmentService::new(args);
        assert_eq!(env_svc.paths.user_data, PathBuf::from("/custom/data"));
        assert_eq!(env_svc.paths.extensions, PathBuf::from("/custom/ext"));
        assert!(env_svc.is_verbose());
    }

    #[test]
    fn cli_args_defaults() {
        let args = CliArgs::default();
        assert!(args.paths.is_empty());
        assert!(!args.diff);
        assert!(!args.wait);
        assert!(!args.disable_extensions);
    }

    #[test]
    fn cli_args_validate_conflicting_windows() {
        let args = CliArgs {
            new_window: true,
            reuse_window: true,
            ..Default::default()
        };
        let err = args.validate().unwrap_err();
        assert_eq!(
            err,
            EnvironmentError::ConflictingFlags(
                "cannot specify both --new-window and --reuse-window".into()
            )
        );
    }

    #[test]
    fn cli_args_validate_diff_requires_two_paths() {
        let args = CliArgs {
            diff: true,
            paths: vec![PathBuf::from("a.txt")],
            ..Default::default()
        };
        assert!(args.validate().is_err());

        let args_ok = CliArgs {
            diff: true,
            paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
            ..Default::default()
        };
        assert!(args_ok.validate().is_ok());
    }

    #[test]
    fn cli_args_validate_goto_zero_rejected() {
        let args = CliArgs {
            goto: Some((0, 5)),
            ..Default::default()
        };
        assert!(args.validate().is_err());
    }

    #[test]
    fn parse_goto_valid() {
        assert_eq!(CliArgs::parse_goto("10:5").unwrap(), (10, 5));
        assert_eq!(CliArgs::parse_goto("1:1").unwrap(), (1, 1));
    }

    #[test]
    fn parse_goto_invalid_format() {
        assert!(CliArgs::parse_goto("10").is_err());
        assert!(CliArgs::parse_goto("abc:def").is_err());
        assert!(CliArgs::parse_goto("0:1").is_err());
    }

    #[test]
    fn builder_produces_valid_args() {
        let args = CliArgsBuilder::new()
            .path("/tmp/file.rs")
            .verbose(true)
            .log_level("debug")
            .build()
            .unwrap();
        assert_eq!(args.paths, vec![PathBuf::from("/tmp/file.rs")]);
        assert!(args.verbose);
        assert_eq!(args.log_level.as_deref(), Some("debug"));
    }

    #[test]
    fn builder_rejects_conflicting_flags() {
        let result = CliArgsBuilder::new()
            .new_window(true)
            .reuse_window(true)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn app_paths_extension_path() {
        let paths = AppPaths::from_user_data(PathBuf::from("/data"));
        assert_eq!(
            paths.extension_path("my-ext"),
            PathBuf::from("/data/extensions/my-ext")
        );
    }

    #[test]
    fn app_paths_workspace_storage_for() {
        let paths = AppPaths::from_user_data(PathBuf::from("/data"));
        assert_eq!(
            paths.workspace_storage_for("abc123"),
            PathBuf::from("/data/workspaceStorage/abc123")
        );
    }

    #[test]
    fn app_paths_log_file_for() {
        let paths = AppPaths::from_user_data(PathBuf::from("/data"));
        assert_eq!(
            paths.log_file_for("session-42"),
            PathBuf::from("/data/logs/session-42.log")
        );
    }

    #[test]
    fn app_paths_display() {
        let paths = AppPaths::from_user_data(PathBuf::from("/home/user/.config/vsedit"));
        let display = format!("{paths}");
        assert!(display.contains("/home/user/.config/vsedit"));
    }

    #[test]
    fn environment_service_log_level_default() {
        let env_svc = EnvironmentService::new(CliArgs::default());
        assert_eq!(env_svc.log_level(), "info");
    }

    #[test]
    fn environment_service_startup_summary() {
        let args = CliArgs {
            verbose: true,
            log_level: Some("debug".into()),
            ..Default::default()
        };
        let env_svc = EnvironmentService::new(args);
        let summary = env_svc.startup_summary();
        assert!(summary.contains("vsedit"));
        assert!(summary.contains("verbose=true"));
        assert!(summary.contains("log_level=debug"));
    }

    #[test]
    fn environment_service_display_and_debug() {
        let env_svc = EnvironmentService::new(CliArgs::default());
        let display = format!("{env_svc}");
        assert!(display.starts_with("vsedit v"));
        let debug = format!("{env_svc:?}");
        assert!(debug.contains("EnvironmentService"));
    }

    #[test]
    fn error_display_messages() {
        let e = EnvironmentError::InvalidPath("empty".into());
        assert_eq!(format!("{e}"), "invalid path: empty");

        let e2 = EnvironmentError::ConflictingFlags("oops".into());
        assert_eq!(format!("{e2}"), "conflicting flags: oops");
    }

    #[test]
    fn app_paths_clone_equality() {
        let a = AppPaths::from_user_data(PathBuf::from("/x"));
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn environment_stats_new_defaults() {
        let stats = EnvironmentStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn environment_stats_record_success() {
        let mut stats = EnvironmentStats::new();
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
    fn environment_stats_record_failure() {
        let mut stats = EnvironmentStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn environment_stats_reset() {
        let mut stats = EnvironmentStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn environment_stats_merge() {
        let mut a = EnvironmentStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = EnvironmentStats::new();
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
    fn environment_stats_display() {
        let mut stats = EnvironmentStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn environment_stats_default() {
        let stats = EnvironmentStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn environment_validator_accepts_valid_name() {
        let v = EnvironmentValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn environment_validator_rejects_empty() {
        let v = EnvironmentValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn environment_validator_rejects_too_long() {
        let v = EnvironmentValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn environment_validator_forbidden_prefix() {
        let v = EnvironmentValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn environment_validator_allowed_chars() {
        let v = EnvironmentValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn environment_validator_range() {
        let v = EnvironmentValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn environment_sanitize_removes_control() {
        let result = EnvironmentValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn environment_truncate_short_string() {
        assert_eq!(EnvironmentValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn environment_truncate_long_string() {
        let result = EnvironmentValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn environment_is_ascii_printable() {
        assert!(EnvironmentValidator::is_ascii_printable("Hello World 123"));
        assert!(!EnvironmentValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn resolve_env_single_variable() {
        let result = resolve_env_variables("Hello ${env:USER}!", &|name| {
            if name == "USER" {
                Some("alice".into())
            } else {
                None
            }
        });
        assert_eq!(result, "Hello alice!");
    }

    #[test]
    fn resolve_env_missing_variable() {
        let result = resolve_env_variables("${env:MISSING}", &|_| None);
        assert_eq!(result, "${env:MISSING}");
    }

    #[test]
    fn resolve_env_multiple_variables() {
        let result = resolve_env_variables("${env:HOME}/.config/${env:APP}", &|name| match name {
            "HOME" => Some("/home/user".into()),
            "APP" => Some("vsedit".into()),
            _ => None,
        });
        assert_eq!(result, "/home/user/.config/vsedit");
    }

    #[test]
    fn resolve_env_no_variables() {
        let result = resolve_env_variables("no vars here", &|_| None);
        assert_eq!(result, "no vars here");
    }

    #[test]
    fn resolve_env_non_env_braces() {
        let result = resolve_env_variables("${workspaceFolder}/src", &|_| None);
        assert_eq!(result, "${workspaceFolder}/src");
    }

    #[test]
    fn shell_environment_basic() {
        let mut env = ShellEnvironment::new();
        assert!(env.is_empty());
        env.set("FOO", "bar");
        assert_eq!(env.get("FOO"), Some("bar"));
        assert_eq!(env.len(), 1);
        env.remove("FOO");
        assert!(env.is_empty());
    }

    #[test]
    fn shell_environment_merge() {
        let mut base = ShellEnvironment::from_pairs(vec![("A", "1"), ("B", "2")]);
        let overlay = ShellEnvironment::from_pairs(vec![("B", "override"), ("C", "3")]);
        base.merge(&overlay);
        assert_eq!(base.get("A"), Some("1"));
        assert_eq!(base.get("B"), Some("override"));
        assert_eq!(base.get("C"), Some("3"));
    }

    #[test]
    fn shell_environment_as_getter() {
        let env = ShellEnvironment::from_pairs(vec![("HOME", "/home/test")]);
        let result = resolve_env_variables("${env:HOME}/docs", &env.as_getter());
        assert_eq!(result, "/home/test/docs");
    }

    #[test]
    fn env_path_list_parsing() {
        let paths = env_path_list("/usr/bin:/usr/local/bin:/home/user/bin");
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], PathBuf::from("/usr/bin"));
        assert_eq!(paths[2], PathBuf::from("/home/user/bin"));
    }

    #[test]
    fn env_path_join_roundtrip() {
        let paths = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        let joined = env_path_join(&paths);
        let parsed = env_path_list(&joined);
        assert_eq!(parsed, paths);
    }

    #[test]
    fn env_path_prepend_deduplicates() {
        let result =
            env_path_prepend(Path::new("/usr/local/bin"), "/usr/bin:/usr/local/bin:/bin");
        let paths = env_path_list(&result);
        assert_eq!(paths[0], PathBuf::from("/usr/local/bin"));
        assert_eq!(
            paths
                .iter()
                .filter(|p| p.as_path() == Path::new("/usr/local/bin"))
                .count(),
            1
        );
    }

    #[test]
    fn env_path_contains_check() {
        assert!(env_path_contains(
            "/usr/bin:/usr/local/bin",
            Path::new("/usr/bin")
        ));
        assert!(!env_path_contains(
            "/usr/bin:/usr/local/bin",
            Path::new("/home/user/bin")
        ));
    }

    #[test]
    fn env_path_list_empty() {
        let paths = env_path_list("");
        assert!(paths.is_empty());
    }

    #[test]
    fn shell_environment_capture() {
        let env = ShellEnvironment::capture();
        assert!(!env.is_empty());
    }

    #[test]
    fn env_snapshot_take_and_restore() {
        let mut env = ShellEnvironment::new();
        env.set("X", "1");
        env.set("Y", "2");
        let snap = EnvSnapshot::take(&env, Some("before-change"));
        assert_eq!(snap.label(), Some("before-change"));
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.get("X"), Some("1"));

        env.set("X", "changed");
        env.remove("Y");
        env.set("Z", "3");

        let restored = snap.restore();
        assert_eq!(restored.get("X"), Some("1"));
        assert_eq!(restored.get("Y"), Some("2"));
        assert_eq!(restored.get("Z"), None);
    }

    #[test]
    fn env_diff_detects_all_change_types() {
        let before = EnvSnapshot::from_pairs(
            vec![("A", "1"), ("B", "2"), ("C", "3")],
            None,
        );
        let after = EnvSnapshot::from_pairs(
            vec![("A", "1"), ("B", "changed"), ("D", "4")],
            None,
        );
        let diff = EnvDiff::between(&before, &after);

        assert!(diff.added.contains_key("D"));
        assert_eq!(diff.added["D"], "4");
        assert!(diff.removed.contains_key("C"));
        assert_eq!(diff.removed["C"], "3");
        assert!(diff.changed.contains_key("B"));
        assert_eq!(diff.changed["B"], ("2".to_string(), "changed".to_string()));
        assert!(!diff.is_empty());
        assert_eq!(diff.total_changes(), 3);

        let keys = diff.affected_keys();
        assert!(keys.contains(&"B"));
        assert!(keys.contains(&"C"));
        assert!(keys.contains(&"D"));
    }

    #[test]
    fn env_diff_identical_snapshots_is_empty() {
        let snap = EnvSnapshot::from_pairs(vec![("A", "1")], None);
        let diff = EnvDiff::between(&snap, &snap);
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
        assert_eq!(format!("{diff}"), "EnvDiff(+0 -0 ~0)");
    }

    #[test]
    fn env_path_manager_prepend_append_remove() {
        let mut mgr = EnvPathManager::new();
        assert!(mgr.is_empty());

        mgr.append("/usr/bin");
        mgr.append("/bin");
        mgr.prepend("/usr/local/bin");
        assert_eq!(mgr.len(), 3);
        assert_eq!(mgr.entries()[0], PathBuf::from("/usr/local/bin"));
        assert_eq!(mgr.entries()[2], PathBuf::from("/bin"));

        // Prepending an existing entry moves it to front
        mgr.prepend("/bin");
        assert_eq!(mgr.len(), 3);
        assert_eq!(mgr.entries()[0], PathBuf::from("/bin"));

        assert!(mgr.contains(Path::new("/usr/bin")));
        assert!(mgr.remove(Path::new("/usr/bin")));
        assert!(!mgr.contains(Path::new("/usr/bin")));
        assert_eq!(mgr.len(), 2);

        let s = mgr.to_path_string();
        assert!(s.contains("/bin"));
    }

    #[test]
    fn env_profile_apply_sets_and_unsets() {
        let mut profile = EnvProfile::new("test-profile");
        assert_eq!(profile.name(), "test-profile");
        assert!(profile.is_empty());

        profile.set_var("NEW_VAR", "value");
        profile.set_var("OVERRIDE", "new");
        profile.unset_var("REMOVE_ME");
        assert_eq!(profile.len(), 3);

        let mut env = ShellEnvironment::from_pairs(vec![
            ("OVERRIDE", "old"),
            ("REMOVE_ME", "gone"),
            ("KEEP", "yes"),
        ]);
        profile.apply_to(&mut env);

        assert_eq!(env.get("NEW_VAR"), Some("value"));
        assert_eq!(env.get("OVERRIDE"), Some("new"));
        assert_eq!(env.get("REMOVE_ME"), None);
        assert_eq!(env.get("KEEP"), Some("yes"));

        let keys = profile.affected_keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"NEW_VAR"));
    }

    // -----------------------------------------------------------------------
    // New tests for deepened functionality
    // -----------------------------------------------------------------------

    #[test]
    fn app_paths_tmp_for_session() {
        let paths = AppPaths::from_user_data(PathBuf::from("/data"));
        assert_eq!(
            paths.tmp_for_session("sess-1"),
            PathBuf::from("/data/tmp/sess-1")
        );
    }

    #[test]
    fn app_paths_global_storage_for() {
        let paths = AppPaths::from_user_data(PathBuf::from("/data"));
        assert_eq!(
            paths.global_storage_for("bucket-a"),
            PathBuf::from("/data/globalStorage/bucket-a")
        );
    }

    #[test]
    fn app_paths_user_dir_and_counts() {
        let paths = AppPaths::from_user_data(PathBuf::from("/data"));
        assert_eq!(paths.user_dir(), PathBuf::from("/data/User"));
        assert!(paths.directory_count() >= 7);
        // Non-existent paths should report false
        assert!(!paths.has_settings_file());
        assert!(!paths.has_keybindings_file());
    }

    #[test]
    fn cli_args_special_mode_and_first_path() {
        let args = CliArgs {
            diff: true,
            paths: vec![PathBuf::from("a"), PathBuf::from("b")],
            ..Default::default()
        };
        assert!(args.is_special_mode());
        assert_eq!(args.first_path(), Some(Path::new("a")));

        let empty = CliArgs::default();
        assert!(!empty.is_special_mode());
        assert_eq!(empty.first_path(), None);
    }

    #[test]
    fn cli_args_summary_includes_flags() {
        let args = CliArgs {
            verbose: true,
            wait: true,
            goto: Some((10, 3)),
            paths: vec![PathBuf::from("file.rs")],
            ..Default::default()
        };
        let s = args.summary();
        assert!(s.contains("verbose"));
        assert!(s.contains("wait"));
        assert!(s.contains("goto=10:3"));
        assert!(s.contains("1 path(s)"));
    }

    #[test]
    fn cli_args_effective_locale_default_and_override() {
        let args = CliArgs::default();
        assert_eq!(args.effective_locale(), "en-US");

        let args2 = CliArgs {
            locale: Some("fr-FR".into()),
            ..Default::default()
        };
        assert_eq!(args2.effective_locale(), "fr-FR");
    }

    #[test]
    fn cli_args_display_trait() {
        let args = CliArgs {
            diff: true,
            paths: vec![PathBuf::from("a"), PathBuf::from("b")],
            ..Default::default()
        };
        let s = format!("{args}");
        assert!(s.starts_with("CliArgs("));
        assert!(s.contains("diff"));
    }

    #[test]
    fn shell_environment_sorted_pairs() {
        let env = ShellEnvironment::from_pairs(vec![("Z", "3"), ("A", "1"), ("M", "2")]);
        let pairs = env.sorted_pairs();
        assert_eq!(pairs[0].0, "A");
        assert_eq!(pairs[1].0, "M");
        assert_eq!(pairs[2].0, "Z");
    }

    #[test]
    fn shell_environment_filter_by_prefix() {
        let env = ShellEnvironment::from_pairs(vec![
            ("APP_NAME", "vsedit"),
            ("APP_VERSION", "1.0"),
            ("HOME", "/home/user"),
        ]);
        let filtered = env.filter_by_prefix("APP_");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("APP_NAME"));
        assert!(filtered.contains_key("APP_VERSION"));
        assert!(!filtered.contains_key("HOME"));
    }

    #[test]
    fn shell_environment_has_nonempty() {
        let mut env = ShellEnvironment::new();
        env.set("FILLED", "value");
        env.set("EMPTY", "");
        assert!(env.has_nonempty("FILLED"));
        assert!(!env.has_nonempty("EMPTY"));
        assert!(!env.has_nonempty("MISSING"));
    }

    #[test]
    fn shell_environment_resolve_template() {
        let env = ShellEnvironment::from_pairs(vec![("USER", "bob"), ("DIR", "/opt")]);
        let result = env.resolve("Hello ${env:USER}, dir=${env:DIR}");
        assert_eq!(result, "Hello bob, dir=/opt");
    }

    #[test]
    fn env_snapshot_diff_to() {
        let a = EnvSnapshot::from_pairs(vec![("X", "1")], Some("a"));
        let b = EnvSnapshot::from_pairs(vec![("X", "2"), ("Y", "3")], Some("b"));
        let diff = a.diff_to(&b);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.added.len(), 1);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn env_snapshot_sorted_keys_and_contains() {
        let snap = EnvSnapshot::from_pairs(vec![("C", "3"), ("A", "1"), ("B", "2")], None);
        let keys = snap.sorted_keys();
        assert_eq!(keys, vec!["A", "B", "C"]);
        assert!(snap.contains_key("B"));
        assert!(!snap.contains_key("D"));
    }

    #[test]
    fn env_diff_apply_to() {
        let before = EnvSnapshot::from_pairs(vec![("A", "1"), ("B", "2"), ("C", "3")], None);
        let after = EnvSnapshot::from_pairs(vec![("A", "1"), ("B", "changed"), ("D", "4")], None);
        let diff = EnvDiff::between(&before, &after);

        let mut env = ShellEnvironment::from_pairs(vec![("A", "1"), ("B", "2"), ("C", "3")]);
        diff.apply_to(&mut env);
        assert_eq!(env.get("A"), Some("1"));
        assert_eq!(env.get("B"), Some("changed"));
        assert_eq!(env.get("C"), None);
        assert_eq!(env.get("D"), Some("4"));
    }

    #[test]
    fn env_diff_is_additive_only() {
        let before = EnvSnapshot::from_pairs(vec![("A", "1")], None);
        let after = EnvSnapshot::from_pairs(vec![("A", "1"), ("B", "2")], None);
        let diff = EnvDiff::between(&before, &after);
        assert!(diff.is_additive_only());

        let after2 = EnvSnapshot::from_pairs(vec![("A", "changed"), ("B", "2")], None);
        let diff2 = EnvDiff::between(&before, &after2);
        assert!(!diff2.is_additive_only());
    }

    #[test]
    fn env_path_manager_get_swap_position() {
        let mut mgr = EnvPathManager::from_iter(vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ]);
        assert_eq!(mgr.get(0), Some(Path::new("/a")));
        assert_eq!(mgr.get(5), None);
        assert_eq!(mgr.position(Path::new("/b")), Some(1));
        assert_eq!(mgr.position(Path::new("/z")), None);

        assert!(mgr.swap(0, 2));
        assert_eq!(mgr.get(0), Some(Path::new("/c")));
        assert_eq!(mgr.get(2), Some(Path::new("/a")));
        assert!(!mgr.swap(0, 99));
    }

    #[test]
    fn env_profile_merge_and_affects() {
        let mut base = EnvProfile::new("base");
        base.set_var("A", "1");
        base.set_var("B", "2");

        let mut overlay = EnvProfile::new("overlay");
        overlay.set_var("B", "override");
        overlay.set_var("C", "3");

        base.merge(&overlay);
        assert_eq!(base.len(), 3);
        assert!(base.affects("C"));
        assert!(!base.affects("Z"));
        assert_eq!(base.get_override("B"), Some(Some("override")));
        assert_eq!(base.get_override("MISSING"), None);
    }

    #[test]
    fn env_profile_get_override_unset() {
        let mut p = EnvProfile::new("p");
        p.unset_var("GONE");
        // An unset entry yields Some(None)
        assert_eq!(p.get_override("GONE"), Some(None));
    }

    #[test]
    fn environment_stats_is_all_success_and_summary() {
        let mut stats = EnvironmentStats::new();
        assert!(stats.is_all_success());
        stats.record_success(1_000_000);
        assert!(stats.is_all_success());
        let s = stats.summary();
        assert!(s.contains("1 ops"));
        assert!(s.contains("1 ok"));
        assert!(s.contains("0 err"));

        stats.record_failure(2_000_000);
        assert!(!stats.is_all_success());
    }

    #[test]
    fn environment_stats_total_time_ms() {
        let mut stats = EnvironmentStats::new();
        stats.record_success(5_000_000); // 5ms
        stats.record_success(3_000_000); // 3ms
        let ms = stats.total_time_ms();
        assert!((ms - 8.0).abs() < 0.001);
    }

    #[test]
    fn validator_validate_path_ok_and_errors() {
        assert!(EnvironmentValidator::validate_path("/usr/bin").is_ok());
        assert!(EnvironmentValidator::validate_path("").is_err());
        assert!(EnvironmentValidator::validate_path("/bad\0path").is_err());
    }

    #[test]
    fn validator_normalize_path_collapses_separators() {
        let norm = EnvironmentValidator::normalize_path("/usr///local//bin");
        assert_eq!(norm, "/usr/local/bin");
    }
}
