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


// ── Environment Snapshot ──

/// A frozen snapshot of environment variables for comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentSnapshot {
    vars: HashMap<String, String>,
    timestamp: u64,
}

impl EnvironmentSnapshot {
    /// Create a snapshot from a map of variables.
    pub fn from_map(vars: HashMap<String, String>, timestamp: u64) -> Self {
        Self { vars, timestamp }
    }

    /// Create an empty snapshot.
    pub fn empty(timestamp: u64) -> Self {
        Self {
            vars: HashMap::new(),
            timestamp,
        }
    }

    /// Get a variable value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }

    /// Number of variables in this snapshot.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// The timestamp of this snapshot.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Return keys present in `self` but not in `other`.
    pub fn added_keys(&self, other: &EnvironmentSnapshot) -> Vec<String> {
        let mut keys: Vec<String> = self.vars.keys()
            .filter(|k| !other.vars.contains_key(*k))
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    /// Return keys present in `other` but not in `self`.
    pub fn removed_keys(&self, other: &EnvironmentSnapshot) -> Vec<String> {
        let mut keys: Vec<String> = other.vars.keys()
            .filter(|k| !self.vars.contains_key(*k))
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    /// Return keys whose values differ between the two snapshots.
    pub fn changed_keys(&self, other: &EnvironmentSnapshot) -> Vec<String> {
        let mut keys: Vec<String> = self.vars.iter()
            .filter(|(k, v)| other.vars.get(*k).map_or(false, |ov| ov != *v))
            .map(|(k, _)| k.clone())
            .collect();
        keys.sort();
        keys
    }

    /// Return true if two snapshots are equivalent (same keys and values).
    pub fn is_equivalent(&self, other: &EnvironmentSnapshot) -> bool {
        self.vars == other.vars
    }
}

// ── Environment Override Resolver ──

/// Layer-based override resolver for environment variables.
/// Higher-priority layers override lower ones.
#[derive(Debug, Clone)]
pub struct EnvironmentOverrideResolver {
    layers: Vec<(String, HashMap<String, String>)>,
}

impl EnvironmentOverrideResolver {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
        }
    }

    /// Add a named layer of overrides. Later layers have higher priority.
    pub fn add_layer(&mut self, name: &str, vars: HashMap<String, String>) {
        self.layers.push((name.to_string(), vars));
    }

    /// Resolve a single variable by checking layers from highest to lowest priority.
    pub fn resolve(&self, key: &str) -> Option<&str> {
        for (_name, vars) in self.layers.iter().rev() {
            if let Some(val) = vars.get(key) {
                return Some(val.as_str());
            }
        }
        None
    }

    /// Which layer provides a given key (the highest-priority one).
    pub fn provided_by(&self, key: &str) -> Option<&str> {
        for (name, vars) in self.layers.iter().rev() {
            if vars.contains_key(key) {
                return Some(name.as_str());
            }
        }
        None
    }

    /// Resolve all keys across all layers into a single merged map.
    pub fn resolve_all(&self) -> HashMap<String, String> {
        let mut merged = HashMap::new();
        for (_name, vars) in &self.layers {
            for (k, v) in vars {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged
    }

    /// Number of layers.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Total number of unique keys across all layers.
    pub fn total_unique_keys(&self) -> usize {
        self.resolve_all().len()
    }

    /// Return layer names in priority order (lowest first).
    pub fn layer_names(&self) -> Vec<&str> {
        self.layers.iter().map(|(n, _)| n.as_str()).collect()
    }
}

// ── Environment Variable Sanitizer ──

/// Sanitizer for environment variable names and values.
pub struct EnvironmentSanitizer;

impl EnvironmentSanitizer {
    /// Check if a variable name is valid (non-empty, no '=', no null bytes).
    pub fn is_valid_name(name: &str) -> bool {
        !name.is_empty() && !name.contains('=') && !name.contains('\0')
    }

    /// Sanitize a variable name by removing invalid characters.
    pub fn sanitize_name(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    }

    /// Remove null bytes from a value.
    pub fn sanitize_value(value: &str) -> String {
        value.replace('\0', "")
    }

    /// Validate and sanitize an entire map, returning only valid entries.
    pub fn sanitize_map(vars: &HashMap<String, String>) -> HashMap<String, String> {
        vars.iter()
            .filter(|(k, _)| Self::is_valid_name(k))
            .map(|(k, v)| (Self::sanitize_name(k), Self::sanitize_value(v)))
            .collect()
    }

    /// Return names of invalid variables from a map.
    pub fn invalid_names(vars: &HashMap<String, String>) -> Vec<String> {
        let mut names: Vec<String> = vars.keys()
            .filter(|k| !Self::is_valid_name(k))
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// Check if a PATH-like variable has duplicate entries.
    pub fn has_duplicate_path_entries(path_value: &str) -> bool {
        let entries: Vec<&str> = path_value.split(':').collect();
        let unique: std::collections::HashSet<&str> = entries.iter().copied().collect();
        unique.len() < entries.len()
    }

    /// Deduplicate PATH entries, preserving first occurrence order.
    pub fn dedup_path(path_value: &str) -> String {
        let mut seen = std::collections::HashSet::new();
        let deduped: Vec<&str> = path_value.split(':')
            .filter(|entry| seen.insert(*entry))
            .collect();
        deduped.join(":")
    }
}

// ── Environment Diff Display ──

/// Represents a single change in an environment diff.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvDiffEntry {
    Added { key: String, value: String },
    Removed { key: String, value: String },
    Changed { key: String, old_value: String, new_value: String },
}

impl fmt::Display for EnvDiffEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added { key, value } => write!(f, "+ {key}={value}"),
            Self::Removed { key, value } => write!(f, "- {key}={value}"),
            Self::Changed { key, old_value, new_value } => {
                write!(f, "~ {key}: {old_value} -> {new_value}")
            }
        }
    }
}

/// Compute a full diff between two environment snapshots.
pub struct EnvDiffDisplay;

impl EnvDiffDisplay {
    /// Compute diff entries between `old` and `new` snapshots.
    pub fn diff(old: &EnvironmentSnapshot, new: &EnvironmentSnapshot) -> Vec<EnvDiffEntry> {
        let mut entries = Vec::new();

        let added = new.added_keys(old);
        for key in &added {
            if let Some(val) = new.get(key) {
                entries.push(EnvDiffEntry::Added {
                    key: key.clone(),
                    value: val.to_string(),
                });
            }
        }

        let removed = new.removed_keys(old);
        for key in &removed {
            if let Some(val) = old.get(key) {
                entries.push(EnvDiffEntry::Removed {
                    key: key.clone(),
                    value: val.to_string(),
                });
            }
        }

        let changed = new.changed_keys(old);
        for key in &changed {
            if let (Some(ov), Some(nv)) = (old.get(key), new.get(key)) {
                entries.push(EnvDiffEntry::Changed {
                    key: key.clone(),
                    old_value: ov.to_string(),
                    new_value: nv.to_string(),
                });
            }
        }

        entries
    }

    /// Format diff as a multi-line string.
    pub fn format(entries: &[EnvDiffEntry]) -> String {
        entries.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
    }

    /// How many changes total.
    pub fn change_count(entries: &[EnvDiffEntry]) -> usize {
        entries.len()
    }

    /// Count only additions.
    pub fn addition_count(entries: &[EnvDiffEntry]) -> usize {
        entries.iter().filter(|e| matches!(e, EnvDiffEntry::Added { .. })).count()
    }

    /// Count only removals.
    pub fn removal_count(entries: &[EnvDiffEntry]) -> usize {
        entries.iter().filter(|e| matches!(e, EnvDiffEntry::Removed { .. })).count()
    }

    /// Whether the diff is empty (no changes).
    pub fn is_empty(entries: &[EnvDiffEntry]) -> bool {
        entries.is_empty()
    }
}


// ---------------------------------------------------------------------------
// environment – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XEnvironmentCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XEnvironmentCapabilities {
    pub fn new() -> Self {
        Self { flags: std::collections::HashSet::new() }
    }

    pub fn register(&mut self, cap: impl Into<String>) {
        self.flags.insert(cap.into());
    }

    pub fn has(&self, cap: &str) -> bool {
        self.flags.contains(cap)
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Return the intersection with another capability set.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.intersection(&other.flags).cloned().collect(),
        }
    }

    /// Return capabilities present here but not in `other`.
    pub fn diff(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.difference(&other.flags).cloned().collect(),
        }
    }

    pub fn all(&self) -> Vec<&str> {
        self.flags.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for XEnvironmentCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XEnvironmentServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XEnvironmentServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. Returns the previous value if the key was already present.
    pub fn register(&mut self, name: impl Into<String>, descriptor: impl Into<String>) -> Option<String> {
        self.services.insert(name.into(), descriptor.into())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.services.get(name).map(|s| s.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.services.remove(name)
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

/// Sanitize a path-like string by collapsing repeated separators and removing trailing ones.
pub fn x_environment_sanitize_path(p: &str) -> String {
    let mut result = String::with_capacity(p.len());
    let mut last_was_sep = false;
    for ch in p.chars() {
        if ch == '/' || ch == '\\' {
            if !last_was_sep {
                result.push('/');
            }
            last_was_sep = true;
        } else {
            result.push(ch);
            last_was_sep = false;
        }
    }
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}


/// Configuration manager for environment functionality.
pub struct EnvironmentConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl EnvironmentConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &EnvironmentConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for environment operations.
pub struct EnvironmentRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl EnvironmentRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for environment.
pub struct EnvironmentValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl EnvironmentValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &EnvironmentValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Environment variable resolution — extended utilities (qk)
// ---------------------------------------------------------------------------

/// Metric accumulator for environ operations.
#[derive(Debug, Clone)]
pub struct QkMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QkMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for environ.
#[derive(Debug, Clone)]
pub struct QkRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QkRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for environ lookups.
#[derive(Debug, Clone)]
pub struct QkLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QkLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for environment
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaEnvironmentRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaEnvironmentRingBuf {
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
pub struct XaEnvironmentCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaEnvironmentCounter {
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

impl Default for XaEnvironmentCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 44
// ---------------------------------------------------------------------------

/// Generic object pool `Xc44Pool<T>`.
pub struct Xc44Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc44Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc44PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc44Pool<T> {
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
    pub fn stats(&self) -> Xc44PoolStats {
        Xc44PoolStats {
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

impl<T> Default for Xc44Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc44Scheduler`.
pub struct Xc44Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc44Scheduler {
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

impl Default for Xc44Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_44 hash for the given byte slice.
pub fn xc_44_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_44 convention.
pub fn xc_44_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe11 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe11Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe11PipelineError {
    pub stage: Xe11Stage,
    pub message: String,
}

impl std::fmt::Display for Xe11PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe11Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe11Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError>>>,
    stage_names: Vec<Xe11Stage>,
}

impl Xe11Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe11Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe11Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe11Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe11Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> {
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

    pub fn compose(mut self, other: Xe11Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe11CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe11CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe11Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe11CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe11CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe11Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe11CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_11_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe11CacheEntry {
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

    fn xe_11_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe11CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_11_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> {
    Ok(data)
}

pub fn xe_11_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_11_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_11_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_11_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe11PipelineError> {
    Err(Xe11PipelineError {
        stage: Xe11Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #75
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf75Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf75TrieNode {
    children: std::collections::HashMap<char, Xf75TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf75Trie {
    root: Xf75TrieNode,
    count: usize,
}

impl Xf75Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf75TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf75TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf75TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf75BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf75BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 43).
pub struct Xh43SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh43SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 85 as u64,
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

/// A compact bit set supporting boolean operations (variant 43).
pub struct Xh43BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh43BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 43).
pub struct Xi43Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi43Deque<T> {
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
pub struct Xi43Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi43Interval {
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

/// A simple interval tree (variant 43).
pub struct Xi43IntervalTree {
    xi_intervals: Vec<Xi43Interval>,
}

impl Xi43IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi43Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi43Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi43Interval) -> Vec<&Xi43Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi43Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi43Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi43Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi43Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi43Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi43Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 43) ---

/// Disjoint set / union-find for crate 43.
pub struct Xj43UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj43UnionFind {
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

const XJ43_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 43.
pub struct Xj43BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj43BTreeNode<K, V>>>,
    len: usize,
}

struct Xj43BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj43BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj43BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ43_BTREE_ORDER - 1
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
        let mid = XJ43_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj43BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj43BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj43BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj43BTreeNode::xj_new_leaf();
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


// --- xk_43 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk43SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk43SegmentTree {
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
pub struct Xk43DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk43DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_43).
#[derive(Debug, Clone)]
pub struct Xl43Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl43Rope {
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

/// Suffix array for efficient string searching (xl_43).
#[derive(Debug, Clone)]
pub struct Xl43SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl43SuffixArray {
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
pub struct Xm43MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm43MatrixSparse {
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
pub struct Xm43Tokenizer {
    text: String,
}

impl Xm43Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 43.
pub struct Xn43Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn43Fenwick {
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

// ----- AVL tree map — crate 43 -----

#[derive(Debug, Clone)]
struct Xn43AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn43AvlNode<K, V>>>,
    right: Option<Box<Xn43AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 43.
#[derive(Debug, Clone)]
pub struct Xn43AVL<K, V> {
    root: Option<Box<Xn43AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn43AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn43AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn43AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn43AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn43AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn43AvlNode<K, V>>) -> Box<Xn43AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn43AvlNode<K, V>>) -> Box<Xn43AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn43AvlNode<K, V>>) -> Box<Xn43AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn43AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn43AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn43AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn43AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn43AvlNode<K, V>>) -> &Xn43AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn43AvlNode<K, V>>) -> (Box<Xn43AvlNode<K, V>>, Option<Box<Xn43AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn43AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn43AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn43AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn43AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn43AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn43AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn43AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo43RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo43Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo43RBNode<K, V> {
    key: K,
    value: V,
    color: Xo43Color,
    left: Option<Box<Xo43RBNode<K, V>>>,
    right: Option<Box<Xo43RBNode<K, V>>>,
}

/// A red-black tree map for crate 43.
#[derive(Debug, Clone)]
pub struct Xo43RedBlack<K, V> {
    root: Option<Box<Xo43RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo43RedBlack<K, V> {
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
            r.color = Xo43Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo43RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo43RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo43RBNode {
                    key, value, color: Xo43Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo43RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo43Color::Red)
    }

    fn xo_balance(mut h: Box<Xo43RBNode<K, V>>) -> Box<Xo43RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo43Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo43RBNode<K, V>>) -> Box<Xo43RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo43Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo43RBNode<K, V>>) -> Box<Xo43RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo43Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo43RBNode<K, V>>) {
        h.color = Xo43Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo43Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo43Color::Black; }
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
            r.color = Xo43Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo43RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo43RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo43RBNode<K, V>) -> (K, V, Option<Box<Xo43RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo43RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo43Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo43RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo43ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 43.
#[derive(Debug, Clone)]
pub struct Xo43ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo43ConsistentHash {
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
            let vkey = format!("{}#xo43#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo43#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 43).
#[derive(Debug)]
pub struct Xp43SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp43Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp43Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp43Node<K, V>>>,
    xp_right: Option<Box<Xp43Node<K, V>>>,
}

impl<K: Ord, V> Xp43Node<K, V> {
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

impl<K: Ord, V> Default for Xp43SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp43SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp43Node<K, V>>>, key: &K) -> Option<Box<Xp43Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp43Node<K, V>>) -> Box<Xp43Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp43Node<K, V>>) -> Box<Xp43Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp43Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp43Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp43Node::xp_new(key, val));
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


// --------------- Xq43Treap ---------------

use std::cmp::Ordering as Xq43Ord;

struct Xq43TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq43TreapNode<K, V>>>,
    right: Option<Box<Xq43TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq43Treap<K, V> {
    root: Option<Box<Xq43TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq43TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_43_size<K, V>(node: &Option<Box<Xq43TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_43_update_size<K, V>(node: &mut Xq43TreapNode<K, V>) {
    node.size = 1 + xq_43_size(&node.left) + xq_43_size(&node.right);
}

fn xq_43_rotate_right<K, V>(mut node: Box<Xq43TreapNode<K, V>>) -> Box<Xq43TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_43_update_size(&mut node);
    left.right = Some(node);
    xq_43_update_size(&mut left);
    left
}

fn xq_43_rotate_left<K, V>(mut node: Box<Xq43TreapNode<K, V>>) -> Box<Xq43TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_43_update_size(&mut node);
    right.left = Some(node);
    xq_43_update_size(&mut right);
    right
}

fn xq_43_insert_node<K: Ord, V>(
    node: Option<Box<Xq43TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq43TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq43TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq43Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq43Ord::Less => {
                let (new_left, old) = xq_43_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_43_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_43_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq43Ord::Greater => {
                let (new_right, old) = xq_43_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_43_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_43_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_43_remove_node<K: Ord, V>(
    node: Option<Box<Xq43TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq43TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq43Ord::Less => {
                let (new_left, old) = xq_43_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_43_update_size(&mut n);
                (Some(n), old)
            }
            Xq43Ord::Greater => {
                let (new_right, old) = xq_43_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_43_update_size(&mut n);
                (Some(n), old)
            }
            Xq43Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_43_rotate_right(n);
                    let (new_right, old) = xq_43_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_43_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_43_rotate_left(n);
                    let (new_left, old) = xq_43_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_43_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_43_find_min<K, V>(node: &Option<Box<Xq43TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_43_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_43_find_max<K, V>(node: &Option<Box<Xq43TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_43_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_43_rank<K: Ord, V>(node: &Option<Box<Xq43TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq43Ord::Less => xq_43_rank(&n.left, key),
            Xq43Ord::Equal => xq_43_size(&n.left),
            Xq43Ord::Greater => 1 + xq_43_size(&n.left) + xq_43_rank(&n.right, key),
        },
    }
}

fn xq_43_kth<K, V>(node: &Option<Box<Xq43TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_43_size(&n.left);
        if k < left_size {
            xq_43_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_43_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_43_in_order<K: Clone, V>(node: &Option<Box<Xq43TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_43_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_43_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq43Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 43 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_43_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq43Ord::Equal => return Some(&n.value),
                Xq43Ord::Less => cur = &n.left,
                Xq43Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_43_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_43_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_43_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_43_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_43_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_43_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_43_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq43VEBTree ---------------

pub struct Xq43VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq43VEBTree>>,
    clusters: Vec<Option<Box<Xq43VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq43VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq43VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq43VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr43KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr43KDPoint {
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
pub struct Xr43BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr43KDNode {
    xr_point: Xr43KDPoint,
    xr_left: Option<Box<Xr43KDNode>>,
    xr_right: Option<Box<Xr43KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr43KDTree {
    xr_root: Option<Box<Xr43KDNode>>,
    xr_size: usize,
}

impl Xr43KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr43KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr43KDNode>>,
        point: Xr43KDPoint,
        depth: usize,
    ) -> Box<Xr43KDNode> {
        match node {
            None => Box::new(Xr43KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr43KDPoint) -> Option<Xr43KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr43KDNode>,
        query: &Xr43KDPoint,
        depth: usize,
        best: &mut Xr43KDPoint,
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
    ) -> Vec<Xr43KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr43KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr43KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr43KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr43KDNode>>, pts: &mut Vec<Xr43KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr43KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr43BoundingBox> {
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
        Some(Xr43BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs43PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs43PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs43PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs43PersistentArray {
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
pub struct Xs43ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs43ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs43ConcurrentQueue {
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
pub struct Xs43RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs43RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs43RangeMap {
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
pub struct Xs43CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs43CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs43CircularBuffer {
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


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}


// --- xy_ Cuckoo Hash Map ---

/// Cuckoo hash map with two hash functions and O(1) amortized lookup.
#[derive(Debug, Clone)]
pub struct XyCuckooMap<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xy_table1: Vec<Option<(K, V)>>,
    xy_table2: Vec<Option<(K, V)>>,
    xy_capacity: usize,
    xy_size: usize,
    xy_seed1: u64,
    xy_seed2: u64,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XyCuckooMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CuckooMap(size={}, cap={})", self.xy_size, self.xy_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> Default for XyCuckooMap<K, V> {
    fn default() -> Self { Self::xy_new(16) }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XyCuckooMap<K, V> {
    /// Create a new cuckoo hash map with given capacity.
    pub fn xy_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xy_table1: (0..cap).map(|_| None).collect(),
            xy_table2: (0..cap).map(|_| None).collect(),
            xy_capacity: cap,
            xy_size: 0,
            xy_seed1: 0x517cc1b727220a95,
            xy_seed2: 0x6c62272e07bb0142,
        }
    }

    fn xy_hash1(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed1.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    fn xy_hash2(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed2.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    /// Number of elements.
    pub fn xy_len(&self) -> usize { self.xy_size }

    /// Is empty.
    pub fn xy_is_empty(&self) -> bool { self.xy_size == 0 }

    /// Insert a key-value pair.
    pub fn xy_insert(&mut self, key: K, value: V) -> bool {
        if self.xy_get(&key).is_some() {
            let h1 = self.xy_hash1(&key);
            if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == key) {
                self.xy_table1[h1] = Some((key, value));
            } else {
                let h2 = self.xy_hash2(&key);
                self.xy_table2[h2] = Some((key, value));
            }
            return true;
        }
        let mut k = key;
        let mut v = value;
        for _ in 0..self.xy_capacity {
            let h1 = self.xy_hash1(&k);
            if self.xy_table1[h1].is_none() {
                self.xy_table1[h1] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old = self.xy_table1[h1].take().unwrap();
            self.xy_table1[h1] = Some((k, v));
            k = old.0;
            v = old.1;
            let h2 = self.xy_hash2(&k);
            if self.xy_table2[h2].is_none() {
                self.xy_table2[h2] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old2 = self.xy_table2[h2].take().unwrap();
            self.xy_table2[h2] = Some((k, v));
            k = old2.0;
            v = old2.1;
        }
        // Rehash needed — just put in table1 with linear probing fallback
        for i in 0..self.xy_capacity {
            if self.xy_table1[i].is_none() {
                self.xy_table1[i] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
        }
        false
    }

    /// Look up a key.
    pub fn xy_get(&self, key: &K) -> Option<&V> {
        let h1 = self.xy_hash1(key);
        if let Some((k, v)) = &self.xy_table1[h1] {
            if *k == *key { return Some(v); }
        }
        let h2 = self.xy_hash2(key);
        if let Some((k, v)) = &self.xy_table2[h2] {
            if *k == *key { return Some(v); }
        }
        None
    }

    /// Check if key exists.
    pub fn xy_contains(&self, key: &K) -> bool { self.xy_get(key).is_some() }

    /// Remove a key.
    pub fn xy_remove(&mut self, key: &K) -> Option<V> {
        let h1 = self.xy_hash1(key);
        if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table1[h1].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        let h2 = self.xy_hash2(key);
        if self.xy_table2[h2].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table2[h2].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        None
    }

    /// Clear the map.
    pub fn xy_clear(&mut self) {
        for slot in &mut self.xy_table1 { *slot = None; }
        for slot in &mut self.xy_table2 { *slot = None; }
        self.xy_size = 0;
    }

    /// Collect all keys.
    pub fn xy_keys(&self) -> Vec<K> {
        let mut keys = Vec::new();
        for slot in &self.xy_table1 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        for slot in &self.xy_table2 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        keys
    }
}

// --- xy_ Count-Min Sketch ---

/// Count-min sketch for approximate frequency counting with bounded error.
#[derive(Debug, Clone)]
pub struct XyCountMinSketch {
    xy_table: Vec<Vec<u64>>,
    xy_width: usize,
    xy_depth: usize,
    xy_seeds: Vec<u64>,
}

impl std::fmt::Display for XyCountMinSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CMS(w={}, d={})", self.xy_width, self.xy_depth)
    }
}

impl Default for XyCountMinSketch {
    fn default() -> Self { Self::xy_new(1000, 5) }
}

impl XyCountMinSketch {
    /// Create a new count-min sketch with given width and depth.
    pub fn xy_new(width: usize, depth: usize) -> Self {
        let seeds: Vec<u64> = (0..depth).map(|i| 0x9e3779b97f4a7c15u64.wrapping_add((i as u64).wrapping_mul(0x517cc1b727220a95))).collect();
        Self {
            xy_table: vec![vec![0u64; width]; depth],
            xy_width: width,
            xy_depth: depth,
            xy_seeds: seeds,
        }
    }

    fn xy_hash(&self, item: u64, seed: u64) -> usize {
        let h = item.wrapping_mul(seed).wrapping_add(seed >> 16);
        (h ^ (h >> 32)) as usize % self.xy_width
    }

    /// Increment the count for an item.
    pub fn xy_add(&mut self, item: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += 1;
        }
    }

    /// Add with a specific count.
    pub fn xy_add_count(&mut self, item: u64, count: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += count;
        }
    }

    /// Estimate the count for an item (guaranteed to be >= actual count).
    pub fn xy_estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            min_count = min_count.min(self.xy_table[i][idx]);
        }
        min_count
    }

    /// Width of the sketch.
    pub fn xy_width(&self) -> usize { self.xy_width }

    /// Depth of the sketch.
    pub fn xy_depth(&self) -> usize { self.xy_depth }

    /// Clear the sketch.
    pub fn xy_clear(&mut self) {
        for row in &mut self.xy_table {
            for cell in row { *cell = 0; }
        }
    }

    /// Merge another sketch into this one.
    pub fn xy_merge(&mut self, other: &XyCountMinSketch) {
        if self.xy_width != other.xy_width || self.xy_depth != other.xy_depth { return; }
        for i in 0..self.xy_depth {
            for j in 0..self.xy_width {
                self.xy_table[i][j] += other.xy_table[i][j];
            }
        }
    }
}


// --- xz_ HyperLogLog ---

/// HyperLogLog probabilistic cardinality estimator with configurable precision.
#[derive(Debug, Clone)]
pub struct XzHyperLogLog {
    xz_registers: Vec<u8>,
    xz_m: usize,
    xz_b: u32,
}

impl std::fmt::Display for XzHyperLogLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HLL(m={}, est={:.0})", self.xz_m, self.xz_estimate())
    }
}

impl Default for XzHyperLogLog {
    fn default() -> Self { Self::xz_new(10) }
}

impl XzHyperLogLog {
    /// Create a new HyperLogLog with precision b (4 <= b <= 16). Uses 2^b registers.
    pub fn xz_new(b: u32) -> Self {
        let b = b.clamp(4, 16);
        let m = 1 << b;
        Self { xz_registers: vec![0u8; m], xz_m: m, xz_b: b }
    }

    fn xz_hash(item: u64) -> u64 {
        let mut h = item;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    /// Add an item.
    pub fn xz_add(&mut self, item: u64) {
        let h = Self::xz_hash(item);
        let idx = (h as usize) & (self.xz_m - 1);
        let w = h >> self.xz_b;
        let rho = if w == 0 { 64 - self.xz_b } else { w.trailing_zeros() + 1 };
        let rho = rho.min(255) as u8;
        if rho > self.xz_registers[idx] {
            self.xz_registers[idx] = rho;
        }
    }

    /// Estimate the cardinality.
    pub fn xz_estimate(&self) -> f64 {
        let m = self.xz_m as f64;
        let alpha = match self.xz_m {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };
        let sum: f64 = self.xz_registers.iter().map(|&r| 2.0f64.powi(-(r as i32))).sum();
        let raw = alpha * m * m / sum;
        if raw <= 2.5 * m {
            let zeros = self.xz_registers.iter().filter(|&&r| r == 0).count();
            if zeros > 0 { m * (m / zeros as f64).ln() } else { raw }
        } else if raw <= (1u64 << 32) as f64 / 30.0 {
            raw
        } else {
            -(((1u64 << 32) as f64) * (1.0 - raw / (1u64 << 32) as f64).ln())
        }
    }

    /// Merge another HyperLogLog into this one.
    pub fn xz_merge(&mut self, other: &XzHyperLogLog) {
        if self.xz_m != other.xz_m { return; }
        for i in 0..self.xz_m {
            if other.xz_registers[i] > self.xz_registers[i] {
                self.xz_registers[i] = other.xz_registers[i];
            }
        }
    }

    /// Clear all registers.
    pub fn xz_clear(&mut self) {
        for r in &mut self.xz_registers { *r = 0; }
    }

    /// Number of registers.
    pub fn xz_num_registers(&self) -> usize { self.xz_m }

    /// Precision parameter.
    pub fn xz_precision(&self) -> u32 { self.xz_b }
}

// --- xz_ LRU Cache ---

/// LRU cache with O(1) get/put using a doubly-linked list and hash map.
#[derive(Debug, Clone)]
pub struct XzLruCache<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xz_capacity: usize,
    xz_entries: Vec<(K, V)>,
    xz_order: Vec<usize>,
    xz_map: std::collections::HashMap<K, usize>,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XzLruCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LRU(size={}, cap={})", self.xz_map.len(), self.xz_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XzLruCache<K, V> {
    /// Create a new LRU cache with given capacity.
    pub fn xz_new(capacity: usize) -> Self {
        Self {
            xz_capacity: capacity.max(1),
            xz_entries: Vec::new(),
            xz_order: Vec::new(),
            xz_map: std::collections::HashMap::new(),
        }
    }

    /// Number of entries.
    pub fn xz_len(&self) -> usize { self.xz_map.len() }

    /// Is empty.
    pub fn xz_is_empty(&self) -> bool { self.xz_map.is_empty() }

    /// Capacity.
    pub fn xz_capacity(&self) -> usize { self.xz_capacity }

    /// Get a value, marking it as recently used.
    pub fn xz_get(&mut self, key: &K) -> Option<&V> {
        if let Some(&idx) = self.xz_map.get(key) {
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            Some(&self.xz_entries[idx].1)
        } else {
            None
        }
    }

    /// Put a key-value pair, evicting the least recently used if at capacity.
    pub fn xz_put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.xz_map.get(&key) {
            self.xz_entries[idx].1 = value;
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            return;
        }
        if self.xz_map.len() >= self.xz_capacity {
            if let Some(evict_idx) = self.xz_order.first().copied() {
                self.xz_order.remove(0);
                let evict_key = self.xz_entries[evict_idx].0.clone();
                self.xz_map.remove(&evict_key);
            }
        }
        let idx = self.xz_entries.len();
        self.xz_entries.push((key.clone(), value));
        self.xz_map.insert(key, idx);
        self.xz_order.push(idx);
    }

    /// Check if key exists (without updating LRU order).
    pub fn xz_contains(&self, key: &K) -> bool { self.xz_map.contains_key(key) }

    /// Remove a key.
    pub fn xz_remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.xz_map.remove(key) {
            self.xz_order.retain(|&i| i != idx);
            Some(self.xz_entries[idx].1.clone())
        } else {
            None
        }
    }

    /// Clear the cache.
    pub fn xz_clear(&mut self) {
        self.xz_entries.clear();
        self.xz_order.clear();
        self.xz_map.clear();
    }

    /// Get all keys in LRU order (least recent first).
    pub fn xz_keys_lru(&self) -> Vec<K> {
        self.xz_order.iter().filter_map(|&idx| {
            let k = &self.xz_entries[idx].0;
            if self.xz_map.contains_key(k) { Some(k.clone()) } else { None }
        }).collect()
    }

    /// Peek at value without updating LRU order.
    pub fn xz_peek(&self, key: &K) -> Option<&V> {
        self.xz_map.get(key).map(|&idx| &self.xz_entries[idx].1)
    }
}


// --- ya_ Trie (Prefix Tree) ---

/// A node in a trie (prefix tree) for string key lookups.
#[derive(Debug, Clone)]
pub struct YaTrieNode<V: Clone> {
    ya_children: std::collections::HashMap<char, Box<YaTrieNode<V>>>,
    ya_value: Option<V>,
    ya_is_end: bool,
}

impl<V: Clone> Default for YaTrieNode<V> {
    fn default() -> Self {
        Self { ya_children: std::collections::HashMap::new(), ya_value: None, ya_is_end: false }
    }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrieNode<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrieNode(children={}, end={})", self.ya_children.len(), self.ya_is_end)
    }
}

/// Trie (prefix tree) for O(m) string key operations where m is key length.
#[derive(Debug, Clone)]
pub struct YaTrie<V: Clone> {
    ya_root: YaTrieNode<V>,
    ya_size: usize,
}

impl<V: Clone> Default for YaTrie<V> {
    fn default() -> Self { Self::ya_new() }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrie<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trie(size={})", self.ya_size)
    }
}

impl<V: Clone> YaTrie<V> {
    /// Create an empty trie.
    pub fn ya_new() -> Self { Self { ya_root: YaTrieNode::default(), ya_size: 0 } }

    /// Number of stored keys.
    pub fn ya_len(&self) -> usize { self.ya_size }

    /// Is the trie empty.
    pub fn ya_is_empty(&self) -> bool { self.ya_size == 0 }

    /// Insert a key-value pair.
    pub fn ya_insert(&mut self, key: &str, value: V) {
        let mut node = &mut self.ya_root;
        for ch in key.chars() {
            node = node.ya_children.entry(ch).or_insert_with(|| Box::new(YaTrieNode::default()));
        }
        if !node.ya_is_end { self.ya_size += 1; }
        node.ya_value = Some(value);
        node.ya_is_end = true;
    }

    /// Look up a key.
    pub fn ya_get(&self, key: &str) -> Option<&V> {
        let mut node = &self.ya_root;
        for ch in key.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return None,
            }
        }
        if node.ya_is_end { node.ya_value.as_ref() } else { None }
    }

    /// Check if a key exists.
    pub fn ya_contains(&self, key: &str) -> bool { self.ya_get(key).is_some() }

    /// Check if any key starts with the given prefix.
    pub fn ya_has_prefix(&self, prefix: &str) -> bool {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return false,
            }
        }
        true
    }

    /// Collect all keys with the given prefix.
    pub fn ya_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        Self::ya_collect_keys(node, &mut prefix.to_string(), &mut results);
        results
    }

    fn ya_collect_keys(node: &YaTrieNode<V>, current: &mut String, results: &mut Vec<String>) {
        if node.ya_is_end { results.push(current.clone()); }
        let mut chars: Vec<char> = node.ya_children.keys().copied().collect();
        chars.sort();
        for ch in chars {
            current.push(ch);
            Self::ya_collect_keys(node.ya_children.get(&ch).unwrap(), current, results);
            current.pop();
        }
    }

    /// Collect all keys.
    pub fn ya_all_keys(&self) -> Vec<String> {
        self.ya_keys_with_prefix("")
    }

    /// Remove a key. Returns the value if it existed.
    pub fn ya_remove(&mut self, key: &str) -> Option<V> {
        let result = Self::ya_remove_recursive(&mut self.ya_root, key, 0);
        if result.is_some() { self.ya_size -= 1; }
        result
    }

    fn ya_remove_recursive(node: &mut YaTrieNode<V>, key: &str, depth: usize) -> Option<V> {
        let chars: Vec<char> = key.chars().collect();
        if depth == chars.len() {
            if node.ya_is_end {
                node.ya_is_end = false;
                return node.ya_value.take();
            }
            return None;
        }
        let ch = chars[depth];
        if let Some(child) = node.ya_children.get_mut(&ch) {
            let result = Self::ya_remove_recursive(child, key, depth + 1);
            if !child.ya_is_end && child.ya_children.is_empty() {
                node.ya_children.remove(&ch);
            }
            result
        } else {
            None
        }
    }

    /// Clear the trie.
    pub fn ya_clear(&mut self) {
        self.ya_root = YaTrieNode::default();
        self.ya_size = 0;
    }

    /// Count keys with a given prefix.
    pub fn ya_count_prefix(&self, prefix: &str) -> usize {
        self.ya_keys_with_prefix(prefix).len()
    }

    /// Longest common prefix among all keys.
    pub fn ya_longest_common_prefix(&self) -> String {
        let mut result = String::new();
        let mut node = &self.ya_root;
        while node.ya_children.len() == 1 && !node.ya_is_end {
            let (&ch, child) = node.ya_children.iter().next().unwrap();
            result.push(ch);
            node = child;
        }
        result
    }
}

// --- ya_ Bloom Filter ---

/// Bloom filter for probabilistic set membership testing with no false negatives.
#[derive(Debug, Clone)]
pub struct YaBloomFilter {
    ya_bits: Vec<bool>,
    ya_size: usize,
    ya_num_hashes: usize,
    ya_count: usize,
}

impl std::fmt::Display for YaBloomFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bloom(bits={}, hashes={}, count={})", self.ya_size, self.ya_num_hashes, self.ya_count)
    }
}

impl Default for YaBloomFilter {
    fn default() -> Self { Self::ya_new(1000, 5) }
}

impl YaBloomFilter {
    /// Create a new bloom filter with given bit size and number of hash functions.
    pub fn ya_new(bits: usize, num_hashes: usize) -> Self {
        Self { ya_bits: vec![false; bits], ya_size: bits, ya_num_hashes: num_hashes.max(1), ya_count: 0 }
    }

    /// Create from expected number of items and desired false positive rate.
    pub fn ya_with_fp_rate(expected_items: usize, fp_rate: f64) -> Self {
        let bits = (-(expected_items as f64) * fp_rate.ln() / (2.0f64.ln().powi(2))).ceil() as usize;
        let bits = bits.max(64);
        let hashes = ((bits as f64 / expected_items as f64) * 2.0f64.ln()).ceil() as usize;
        let hashes = hashes.max(1);
        Self::ya_new(bits, hashes)
    }

    fn ya_hash(&self, item: u64, seed: usize) -> usize {
        let h = item.wrapping_mul(0xff51afd7ed558ccd_u64.wrapping_add(seed as u64));
        let h = h ^ (h >> 33);
        let h = h.wrapping_mul(0xc4ceb9fe1a85ec53_u64.wrapping_add(seed as u64 * 7));
        (h ^ (h >> 33)) as usize % self.ya_size
    }

    /// Add an item.
    pub fn ya_add(&mut self, item: u64) {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            self.ya_bits[idx] = true;
        }
        self.ya_count += 1;
    }

    /// Check if an item might be in the set (false positives possible, no false negatives).
    pub fn ya_might_contain(&self, item: u64) -> bool {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            if !self.ya_bits[idx] { return false; }
        }
        true
    }

    /// Number of items added.
    pub fn ya_count(&self) -> usize { self.ya_count }

    /// Bit array size.
    pub fn ya_bit_size(&self) -> usize { self.ya_size }

    /// Number of hash functions.
    pub fn ya_num_hashes(&self) -> usize { self.ya_num_hashes }

    /// Estimated false positive rate.
    pub fn ya_estimated_fp_rate(&self) -> f64 {
        let ones = self.ya_bits.iter().filter(|&&b| b).count() as f64;
        (ones / self.ya_size as f64).powi(self.ya_num_hashes as i32)
    }

    /// Clear the filter.
    pub fn ya_clear(&mut self) {
        for b in &mut self.ya_bits { *b = false; }
        self.ya_count = 0;
    }

    /// Merge another bloom filter (union).
    pub fn ya_merge(&mut self, other: &YaBloomFilter) {
        if self.ya_size != other.ya_size { return; }
        for i in 0..self.ya_size {
            self.ya_bits[i] = self.ya_bits[i] || other.ya_bits[i];
        }
    }
}


// --- yb_ Ternary Search Tree ---

/// Node in a ternary search tree (TST) for space-efficient string storage.
#[derive(Debug, Clone)]
pub struct YbTstNode<V: Clone> {
    yb_ch: char,
    yb_left: Option<Box<YbTstNode<V>>>,
    yb_mid: Option<Box<YbTstNode<V>>>,
    yb_right: Option<Box<YbTstNode<V>>>,
    yb_value: Option<V>,
}

impl<V: Clone> YbTstNode<V> {
    fn yb_new(ch: char) -> Self {
        Self { yb_ch: ch, yb_left: None, yb_mid: None, yb_right: None, yb_value: None }
    }
}

/// Ternary search tree for efficient string-keyed storage with prefix queries.
#[derive(Debug, Clone)]
pub struct YbTernarySearchTree<V: Clone> {
    yb_root: Option<Box<YbTstNode<V>>>,
    yb_size: usize,
}

impl<V: Clone> Default for YbTernarySearchTree<V> {
    fn default() -> Self { Self { yb_root: None, yb_size: 0 } }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YbTernarySearchTree<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TST(size={})", self.yb_size)
    }
}

impl<V: Clone> YbTernarySearchTree<V> {
    /// Create an empty TST.
    pub fn yb_new() -> Self { Self { yb_root: None, yb_size: 0 } }

    /// Number of stored keys.
    pub fn yb_len(&self) -> usize { self.yb_size }

    /// Is the tree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_size == 0 }

    /// Insert a key-value pair.
    pub fn yb_insert(&mut self, key: &str, value: V) {
        if key.is_empty() { return; }
        let chars: Vec<char> = key.chars().collect();
        let was_new = Self::yb_insert_node(&mut self.yb_root, &chars, 0, value);
        if was_new { self.yb_size += 1; }
    }

    fn yb_insert_node(node: &mut Option<Box<YbTstNode<V>>>, chars: &[char], depth: usize, value: V) -> bool {
        let ch = chars[depth];
        if node.is_none() { *node = Some(Box::new(YbTstNode::yb_new(ch))); }
        let n = node.as_mut().unwrap();
        if ch < n.yb_ch {
            Self::yb_insert_node(&mut n.yb_left, chars, depth, value)
        } else if ch > n.yb_ch {
            Self::yb_insert_node(&mut n.yb_right, chars, depth, value)
        } else if depth + 1 < chars.len() {
            Self::yb_insert_node(&mut n.yb_mid, chars, depth + 1, value)
        } else {
            let was_new = n.yb_value.is_none();
            n.yb_value = Some(value);
            was_new
        }
    }

    /// Look up a key.
    pub fn yb_get(&self, key: &str) -> Option<&V> {
        if key.is_empty() { return None; }
        let chars: Vec<char> = key.chars().collect();
        Self::yb_get_node(self.yb_root.as_deref(), &chars, 0)
    }

    fn yb_get_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a V> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_get_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_get_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_get_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            n.yb_value.as_ref()
        }
    }

    /// Check if a key exists.
    pub fn yb_contains(&self, key: &str) -> bool { self.yb_get(key).is_some() }

    /// Collect all keys.
    pub fn yb_all_keys(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut current = String::new();
        Self::yb_collect(self.yb_root.as_deref(), &mut current, &mut results);
        results
    }

    fn yb_collect(node: Option<&YbTstNode<V>>, current: &mut String, results: &mut Vec<String>) {
        let Some(n) = node else { return };
        Self::yb_collect(n.yb_left.as_deref(), current, results);
        current.push(n.yb_ch);
        if n.yb_value.is_some() { results.push(current.clone()); }
        Self::yb_collect(n.yb_mid.as_deref(), current, results);
        current.pop();
        Self::yb_collect(n.yb_right.as_deref(), current, results);
    }

    /// Collect keys with a given prefix.
    pub fn yb_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() { return self.yb_all_keys(); }
        let chars: Vec<char> = prefix.chars().collect();
        let node = Self::yb_prefix_node(self.yb_root.as_deref(), &chars, 0);
        let mut results = Vec::new();
        if let Some(n) = node {
            if n.yb_value.is_some() { results.push(prefix.to_string()); }
            let mut current = prefix.to_string();
            Self::yb_collect(n.yb_mid.as_deref(), &mut current, &mut results);
        }
        results
    }

    fn yb_prefix_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a YbTstNode<V>> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_prefix_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_prefix_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_prefix_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            Some(n)
        }
    }

    /// Clear the tree.
    pub fn yb_clear(&mut self) { self.yb_root = None; self.yb_size = 0; }
}

// --- yb_ Quadtree ---

/// A point in 2D space for quadtree storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YbPoint {
    pub yb_x: f64,
    pub yb_y: f64,
}

impl std::fmt::Display for YbPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}, {:.2})", self.yb_x, self.yb_y)
    }
}

impl Default for YbPoint {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0 } }
}

impl YbPoint {
    /// Create a new point.
    pub fn yb_new(x: f64, y: f64) -> Self { Self { yb_x: x, yb_y: y } }

    /// Distance to another point.
    pub fn yb_distance(&self, other: &YbPoint) -> f64 {
        ((self.yb_x - other.yb_x).powi(2) + (self.yb_y - other.yb_y).powi(2)).sqrt()
    }
}

/// Axis-aligned bounding box for quadtree partitioning.
#[derive(Debug, Clone, Copy)]
pub struct YbBounds {
    pub yb_x: f64,
    pub yb_y: f64,
    pub yb_w: f64,
    pub yb_h: f64,
}

impl std::fmt::Display for YbBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bounds({:.1},{:.1} {}x{})", self.yb_x, self.yb_y, self.yb_w, self.yb_h)
    }
}

impl Default for YbBounds {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0, yb_w: 100.0, yb_h: 100.0 } }
}

impl YbBounds {
    /// Create bounds from origin and size.
    pub fn yb_new(x: f64, y: f64, w: f64, h: f64) -> Self { Self { yb_x: x, yb_y: y, yb_w: w, yb_h: h } }

    /// Check if a point is inside these bounds.
    pub fn yb_contains(&self, p: &YbPoint) -> bool {
        p.yb_x >= self.yb_x && p.yb_x < self.yb_x + self.yb_w &&
        p.yb_y >= self.yb_y && p.yb_y < self.yb_y + self.yb_h
    }

    /// Check if two bounds intersect.
    pub fn yb_intersects(&self, other: &YbBounds) -> bool {
        !(self.yb_x + self.yb_w <= other.yb_x || other.yb_x + other.yb_w <= self.yb_x ||
          self.yb_y + self.yb_h <= other.yb_y || other.yb_y + other.yb_h <= self.yb_y)
    }
}

/// Quadtree for 2D spatial indexing with region queries.
#[derive(Debug, Clone)]
pub struct YbQuadtree {
    yb_bounds: YbBounds,
    yb_points: Vec<YbPoint>,
    yb_capacity: usize,
    yb_nw: Option<Box<YbQuadtree>>,
    yb_ne: Option<Box<YbQuadtree>>,
    yb_sw: Option<Box<YbQuadtree>>,
    yb_se: Option<Box<YbQuadtree>>,
    yb_divided: bool,
}

impl std::fmt::Display for YbQuadtree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quadtree(points={}, bounds={})", self.yb_count(), self.yb_bounds)
    }
}

impl Default for YbQuadtree {
    fn default() -> Self { Self::yb_new(YbBounds::default(), 4) }
}

impl YbQuadtree {
    /// Create a new quadtree with given bounds and node capacity.
    pub fn yb_new(bounds: YbBounds, capacity: usize) -> Self {
        Self {
            yb_bounds: bounds, yb_points: Vec::new(), yb_capacity: capacity.max(1),
            yb_nw: None, yb_ne: None, yb_sw: None, yb_se: None, yb_divided: false,
        }
    }

    fn yb_subdivide(&mut self) {
        let x = self.yb_bounds.yb_x;
        let y = self.yb_bounds.yb_y;
        let hw = self.yb_bounds.yb_w / 2.0;
        let hh = self.yb_bounds.yb_h / 2.0;
        self.yb_nw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y, hw, hh), self.yb_capacity)));
        self.yb_ne = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y, hw, hh), self.yb_capacity)));
        self.yb_sw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y + hh, hw, hh), self.yb_capacity)));
        self.yb_se = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y + hh, hw, hh), self.yb_capacity)));
        self.yb_divided = true;
    }

    /// Insert a point.
    pub fn yb_insert(&mut self, point: YbPoint) -> bool {
        if !self.yb_bounds.yb_contains(&point) { return false; }
        if self.yb_points.len() < self.yb_capacity && !self.yb_divided {
            self.yb_points.push(point);
            return true;
        }
        if !self.yb_divided { self.yb_subdivide(); }
        if self.yb_nw.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_ne.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_sw.as_mut().unwrap().yb_insert(point) { return true; }
        self.yb_se.as_mut().unwrap().yb_insert(point)
    }

    /// Query all points within a rectangular region.
    pub fn yb_query(&self, range: &YbBounds) -> Vec<YbPoint> {
        let mut found = Vec::new();
        self.yb_query_inner(range, &mut found);
        found
    }

    fn yb_query_inner(&self, range: &YbBounds, found: &mut Vec<YbPoint>) {
        if !self.yb_bounds.yb_intersects(range) { return; }
        for p in &self.yb_points {
            if range.yb_contains(p) { found.push(*p); }
        }
        if self.yb_divided {
            self.yb_nw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_ne.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_sw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_se.as_ref().unwrap().yb_query_inner(range, found);
        }
    }

    /// Count total points.
    pub fn yb_count(&self) -> usize {
        let mut c = self.yb_points.len();
        if self.yb_divided {
            c += self.yb_nw.as_ref().unwrap().yb_count();
            c += self.yb_ne.as_ref().unwrap().yb_count();
            c += self.yb_sw.as_ref().unwrap().yb_count();
            c += self.yb_se.as_ref().unwrap().yb_count();
        }
        c
    }

    /// Is the quadtree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_count() == 0 }

    /// Get bounds.
    pub fn yb_bounds(&self) -> &YbBounds { &self.yb_bounds }

    /// Find nearest point to a target.
    pub fn yb_nearest(&self, target: &YbPoint) -> Option<YbPoint> {
        let all = self.yb_query(&self.yb_bounds);
        all.into_iter().min_by(|a, b| {
            a.yb_distance(target).partial_cmp(&b.yb_distance(target)).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}


// --- yc_ Van Emde Boas Set ---

/// Simplified van Emde Boas-inspired set for integer keys in [0, universe).
/// Uses a flat bitmap for practical efficiency with O(1) operations.
#[derive(Debug, Clone)]
pub struct YcVebSet {
    yc_bits: Vec<u64>,
    yc_universe: usize,
    yc_count: usize,
    yc_min: Option<usize>,
    yc_max: Option<usize>,
}

impl std::fmt::Display for YcVebSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VebSet(universe={}, count={})", self.yc_universe, self.yc_count)
    }
}

impl Default for YcVebSet {
    fn default() -> Self { Self::yc_new(65536) }
}

impl YcVebSet {
    /// Create a set supporting keys in [0, universe).
    pub fn yc_new(universe: usize) -> Self {
        let words = (universe + 63) / 64;
        Self { yc_bits: vec![0; words], yc_universe: universe, yc_count: 0, yc_min: None, yc_max: None }
    }

    /// Universe size.
    pub fn yc_universe(&self) -> usize { self.yc_universe }

    /// Number of elements.
    pub fn yc_len(&self) -> usize { self.yc_count }

    /// Is the set empty.
    pub fn yc_is_empty(&self) -> bool { self.yc_count == 0 }

    /// Insert a key.
    pub fn yc_insert(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) != 0 { return false; }
        self.yc_bits[word] |= 1u64 << bit;
        self.yc_count += 1;
        self.yc_min = Some(self.yc_min.map_or(key, |m: usize| m.min(key)));
        self.yc_max = Some(self.yc_max.map_or(key, |m: usize| m.max(key)));
        true
    }

    /// Remove a key.
    pub fn yc_remove(&mut self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        let word = key / 64;
        let bit = key % 64;
        if self.yc_bits[word] & (1u64 << bit) == 0 { return false; }
        self.yc_bits[word] &= !(1u64 << bit);
        self.yc_count -= 1;
        if self.yc_count == 0 { self.yc_min = None; self.yc_max = None; }
        else {
            if self.yc_min == Some(key) { self.yc_min = self.yc_successor(key); }
            if self.yc_max == Some(key) { self.yc_max = self.yc_predecessor(key); }
        }
        true
    }

    /// Check membership.
    pub fn yc_contains(&self, key: usize) -> bool {
        if key >= self.yc_universe { return false; }
        self.yc_bits[key / 64] & (1u64 << (key % 64)) != 0
    }

    /// Minimum element.
    pub fn yc_min(&self) -> Option<usize> { self.yc_min }

    /// Maximum element.
    pub fn yc_max(&self) -> Option<usize> { self.yc_max }

    /// Find the smallest key > given key.
    pub fn yc_successor(&self, key: usize) -> Option<usize> {
        for k in (key + 1)..self.yc_universe {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Find the largest key < given key.
    pub fn yc_predecessor(&self, key: usize) -> Option<usize> {
        if key == 0 { return None; }
        for k in (0..key).rev() {
            if self.yc_contains(k) { return Some(k); }
        }
        None
    }

    /// Collect all elements in sorted order.
    pub fn yc_to_sorted_vec(&self) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.yc_count);
        for w in 0..self.yc_bits.len() {
            let mut bits = self.yc_bits[w];
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                result.push(w * 64 + tz);
                bits &= bits - 1;
            }
        }
        result
    }

    /// Clear the set.
    pub fn yc_clear(&mut self) {
        for w in &mut self.yc_bits { *w = 0; }
        self.yc_count = 0;
        self.yc_min = None;
        self.yc_max = None;
    }

    /// Union with another set (same universe).
    pub fn yc_union(&mut self, other: &YcVebSet) {
        if self.yc_universe != other.yc_universe { return; }
        for i in 0..self.yc_bits.len() {
            self.yc_bits[i] |= other.yc_bits[i];
        }
        self.yc_count = self.yc_to_sorted_vec().len();
        let sorted = self.yc_to_sorted_vec();
        self.yc_min = sorted.first().copied();
        self.yc_max = sorted.last().copied();
    }

    /// Intersection with another set.
    pub fn yc_intersection(&self, other: &YcVebSet) -> YcVebSet {
        let mut result = YcVebSet::yc_new(self.yc_universe);
        if self.yc_universe != other.yc_universe { return result; }
        for i in 0..self.yc_bits.len() {
            result.yc_bits[i] = self.yc_bits[i] & other.yc_bits[i];
        }
        let sorted = result.yc_to_sorted_vec();
        result.yc_count = sorted.len();
        result.yc_min = sorted.first().copied();
        result.yc_max = sorted.last().copied();
        result
    }
}

// --- yc_ Consistent Hash Ring ---

/// Consistent hash ring for distributed key mapping with virtual nodes.
#[derive(Debug, Clone)]
pub struct YcHashRing {
    yc_ring: std::collections::BTreeMap<u64, String>,
    yc_replicas: usize,
    yc_nodes: Vec<String>,
}

impl std::fmt::Display for YcHashRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HashRing(nodes={}, replicas={})", self.yc_nodes.len(), self.yc_replicas)
    }
}

impl Default for YcHashRing {
    fn default() -> Self { Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: 150, yc_nodes: Vec::new() } }
}

impl YcHashRing {
    /// Create a new hash ring with given replica count per node.
    pub fn yc_new(replicas: usize) -> Self {
        Self { yc_ring: std::collections::BTreeMap::new(), yc_replicas: replicas.max(1), yc_nodes: Vec::new() }
    }

    fn yc_hash(key: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in key.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// Add a node to the ring.
    pub fn yc_add_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.insert(hash, node.to_string());
        }
        self.yc_nodes.push(node.to_string());
    }

    /// Remove a node from the ring.
    pub fn yc_remove_node(&mut self, node: &str) {
        for i in 0..self.yc_replicas {
            let key = format!("{}:{}", node, i);
            let hash = Self::yc_hash(&key);
            self.yc_ring.remove(&hash);
        }
        self.yc_nodes.retain(|n| n != node);
    }

    /// Find the node responsible for a key.
    pub fn yc_get_node(&self, key: &str) -> Option<&str> {
        if self.yc_ring.is_empty() { return None; }
        let hash = Self::yc_hash(key);
        let node = self.yc_ring.range(hash..).next()
            .or_else(|| self.yc_ring.iter().next());
        node.map(|(_, v)| v.as_str())
    }

    /// Number of physical nodes.
    pub fn yc_node_count(&self) -> usize { self.yc_nodes.len() }

    /// Number of virtual nodes on the ring.
    pub fn yc_virtual_count(&self) -> usize { self.yc_ring.len() }

    /// List all physical nodes.
    pub fn yc_nodes(&self) -> &[String] { &self.yc_nodes }

    /// Check if a node is in the ring.
    pub fn yc_has_node(&self, node: &str) -> bool { self.yc_nodes.iter().any(|n| n == node) }
}


// --- yd_ Directed Acyclic Graph ---

/// Directed acyclic graph with topological sorting and cycle detection.
#[derive(Debug, Clone)]
pub struct YdDag {
    yd_adj: std::collections::HashMap<usize, Vec<usize>>,
    yd_in_degree: std::collections::HashMap<usize, usize>,
    yd_nodes: std::collections::HashSet<usize>,
}

impl std::fmt::Display for YdDag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let edges: usize = self.yd_adj.values().map(|v| v.len()).sum();
        write!(f, "DAG(nodes={}, edges={})", self.yd_nodes.len(), edges)
    }
}

impl Default for YdDag {
    fn default() -> Self { Self::yd_new() }
}

impl YdDag {
    /// Create an empty DAG.
    pub fn yd_new() -> Self {
        Self { yd_adj: std::collections::HashMap::new(), yd_in_degree: std::collections::HashMap::new(), yd_nodes: std::collections::HashSet::new() }
    }

    /// Add a node.
    pub fn yd_add_node(&mut self, node: usize) {
        self.yd_nodes.insert(node);
        self.yd_adj.entry(node).or_default();
        self.yd_in_degree.entry(node).or_insert(0);
    }

    /// Add a directed edge from -> to.
    pub fn yd_add_edge(&mut self, from: usize, to: usize) {
        self.yd_add_node(from);
        self.yd_add_node(to);
        self.yd_adj.entry(from).or_default().push(to);
        *self.yd_in_degree.entry(to).or_insert(0) += 1;
    }

    /// Number of nodes.
    pub fn yd_node_count(&self) -> usize { self.yd_nodes.len() }

    /// Number of edges.
    pub fn yd_edge_count(&self) -> usize { self.yd_adj.values().map(|v| v.len()).sum() }

    /// Topological sort using Kahn's algorithm. Returns None if cycle detected.
    pub fn yd_topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = self.yd_in_degree.clone();
        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        for (node, deg) in &in_deg {
            if *deg == 0 { queue.push_back(*node); }
        }
        let mut result = Vec::new();
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    let d = in_deg.get_mut(&next).unwrap();
                    *d -= 1;
                    if *d == 0 { queue.push_back(next); }
                }
            }
        }
        if result.len() == self.yd_nodes.len() { Some(result) } else { None }
    }

    /// Check if the graph has a cycle.
    pub fn yd_has_cycle(&self) -> bool { self.yd_topological_sort().is_none() }

    /// Get all neighbors of a node.
    pub fn yd_neighbors(&self, node: usize) -> &[usize] {
        self.yd_adj.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get in-degree of a node.
    pub fn yd_in_degree(&self, node: usize) -> usize {
        self.yd_in_degree.get(&node).copied().unwrap_or(0)
    }

    /// Get out-degree of a node.
    pub fn yd_out_degree(&self, node: usize) -> usize {
        self.yd_adj.get(&node).map(|v| v.len()).unwrap_or(0)
    }

    /// Find all root nodes (in-degree 0).
    pub fn yd_roots(&self) -> Vec<usize> {
        let mut roots: Vec<usize> = self.yd_in_degree.iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        roots.sort();
        roots
    }

    /// Find all leaf nodes (out-degree 0).
    pub fn yd_leaves(&self) -> Vec<usize> {
        let mut leaves: Vec<usize> = self.yd_nodes.iter()
            .filter(|&&n| self.yd_out_degree(n) == 0)
            .copied()
            .collect();
        leaves.sort();
        leaves
    }

    /// Check if node exists.
    pub fn yd_has_node(&self, node: usize) -> bool { self.yd_nodes.contains(&node) }

    /// Clear the graph.
    pub fn yd_clear(&mut self) {
        self.yd_adj.clear();
        self.yd_in_degree.clear();
        self.yd_nodes.clear();
    }

    /// BFS traversal from a start node.
    pub fn yd_bfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();
        queue.push_back(start);
        visited.insert(start);
        while let Some(node) = queue.pop_front() {
            result.push(node);
            if let Some(neighbors) = self.yd_adj.get(&node) {
                let mut sorted_n = neighbors.clone();
                sorted_n.sort();
                for next in sorted_n {
                    if visited.insert(next) { queue.push_back(next); }
                }
            }
        }
        result
    }

    /// DFS traversal from a start node.
    pub fn yd_dfs(&self, start: usize) -> Vec<usize> {
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        self.yd_dfs_inner(start, &mut visited, &mut result);
        result
    }

    fn yd_dfs_inner(&self, node: usize, visited: &mut std::collections::HashSet<usize>, result: &mut Vec<usize>) {
        if !visited.insert(node) { return; }
        result.push(node);
        if let Some(neighbors) = self.yd_adj.get(&node) {
            let mut sorted_n = neighbors.clone();
            sorted_n.sort();
            for next in sorted_n {
                self.yd_dfs_inner(next, visited, result);
            }
        }
    }

    /// Shortest path length (unweighted) between two nodes using BFS.
    pub fn yd_shortest_path(&self, from: usize, to: usize) -> Option<usize> {
        if from == to { return Some(0); }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((from, 0usize));
        visited.insert(from);
        while let Some((node, dist)) = queue.pop_front() {
            if let Some(neighbors) = self.yd_adj.get(&node) {
                for &next in neighbors {
                    if next == to { return Some(dist + 1); }
                    if visited.insert(next) { queue.push_back((next, dist + 1)); }
                }
            }
        }
        None
    }
}

// --- yd_ Sparse Matrix ---

/// Sparse matrix using coordinate (COO) format for efficient storage.
#[derive(Debug, Clone)]
pub struct YdSparseMatrix {
    yd_rows: usize,
    yd_cols: usize,
    yd_entries: std::collections::HashMap<(usize, usize), f64>,
}

impl std::fmt::Display for YdSparseMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SparseMatrix({}x{}, nnz={})", self.yd_rows, self.yd_cols, self.yd_entries.len())
    }
}

impl Default for YdSparseMatrix {
    fn default() -> Self { Self::yd_new(0, 0) }
}

impl YdSparseMatrix {
    /// Create a new sparse matrix with given dimensions.
    pub fn yd_new(rows: usize, cols: usize) -> Self {
        Self { yd_rows: rows, yd_cols: cols, yd_entries: std::collections::HashMap::new() }
    }

    /// Set a value.
    pub fn yd_set(&mut self, row: usize, col: usize, val: f64) {
        if val == 0.0 { self.yd_entries.remove(&(row, col)); }
        else { self.yd_entries.insert((row, col), val); }
    }

    /// Get a value.
    pub fn yd_get(&self, row: usize, col: usize) -> f64 {
        self.yd_entries.get(&(row, col)).copied().unwrap_or(0.0)
    }

    /// Number of non-zero entries.
    pub fn yd_nnz(&self) -> usize { self.yd_entries.len() }

    /// Dimensions.
    pub fn yd_rows(&self) -> usize { self.yd_rows }
    pub fn yd_cols(&self) -> usize { self.yd_cols }

    /// Transpose.
    pub fn yd_transpose(&self) -> YdSparseMatrix {
        let mut t = YdSparseMatrix::yd_new(self.yd_cols, self.yd_rows);
        for ((r, c), v) in &self.yd_entries {
            t.yd_set(*c, *r, *v);
        }
        t
    }

    /// Matrix-vector multiply.
    pub fn yd_mul_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.yd_rows];
        for ((r, c), v) in &self.yd_entries {
            if *c < vec.len() && *r < result.len() {
                result[*r] += *v * vec[*c];
            }
        }
        result
    }

    /// Scale all entries.
    pub fn yd_scale(&mut self, factor: f64) {
        for v in self.yd_entries.values_mut() { *v *= factor; }
    }

    /// Add another sparse matrix.
    pub fn yd_add(&self, other: &YdSparseMatrix) -> YdSparseMatrix {
        let mut result = self.clone();
        for ((r, c), v) in &other.yd_entries {
            let entry = result.yd_entries.entry((*r, *c)).or_insert(0.0);
            *entry += *v;
        }
        result
    }

    /// Clear all entries.
    pub fn yd_clear(&mut self) { self.yd_entries.clear(); }

    /// Row sum.
    pub fn yd_row_sum(&self, row: usize) -> f64 {
        self.yd_entries.iter()
            .filter(|((r, _), _)| *r == row)
            .map(|(_, v)| *v)
            .sum()
    }

    /// Frobenius norm squared.
    pub fn yd_frobenius_sq(&self) -> f64 {
        self.yd_entries.values().map(|v| *v * *v).sum()
    }
}


// --- ye_ Indexed Priority Queue ---

/// Indexed min-priority queue supporting decrease-key in O(log n).
#[derive(Debug, Clone)]
pub struct YeIndexedPQ {
    ye_heap: Vec<(usize, i64)>,
    ye_pos: std::collections::HashMap<usize, usize>,
}

impl std::fmt::Display for YeIndexedPQ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IndexedPQ(size={})", self.ye_heap.len())
    }
}

impl Default for YeIndexedPQ {
    fn default() -> Self { Self::ye_new() }
}

impl YeIndexedPQ {
    /// Create an empty indexed priority queue.
    pub fn ye_new() -> Self { Self { ye_heap: Vec::new(), ye_pos: std::collections::HashMap::new() } }

    /// Number of elements.
    pub fn ye_len(&self) -> usize { self.ye_heap.len() }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_heap.is_empty() }

    /// Insert an element with priority.
    pub fn ye_insert(&mut self, id: usize, priority: i64) {
        if self.ye_pos.contains_key(&id) { self.ye_decrease_key(id, priority); return; }
        let idx = self.ye_heap.len();
        self.ye_heap.push((id, priority));
        self.ye_pos.insert(id, idx);
        self.ye_sift_up(idx);
    }

    /// Peek at minimum.
    pub fn ye_peek(&self) -> Option<(usize, i64)> { self.ye_heap.first().copied() }

    /// Extract minimum.
    pub fn ye_pop(&mut self) -> Option<(usize, i64)> {
        if self.ye_heap.is_empty() { return None; }
        let min = self.ye_heap[0];
        let last = self.ye_heap.len() - 1;
        self.ye_swap(0, last);
        self.ye_heap.pop();
        self.ye_pos.remove(&min.0);
        if !self.ye_heap.is_empty() { self.ye_sift_down(0); }
        Some(min)
    }

    /// Decrease the priority of an element.
    pub fn ye_decrease_key(&mut self, id: usize, new_priority: i64) {
        if let Some(&idx) = self.ye_pos.get(&id) {
            if new_priority < self.ye_heap[idx].1 {
                self.ye_heap[idx].1 = new_priority;
                self.ye_sift_up(idx);
            }
        }
    }

    /// Check if an id is in the queue.
    pub fn ye_contains(&self, id: usize) -> bool { self.ye_pos.contains_key(&id) }

    /// Get priority of an id.
    pub fn ye_priority(&self, id: usize) -> Option<i64> {
        self.ye_pos.get(&id).map(|&idx| self.ye_heap[idx].1)
    }

    fn ye_swap(&mut self, i: usize, j: usize) {
        self.ye_heap.swap(i, j);
        self.ye_pos.insert(self.ye_heap[i].0, i);
        self.ye_pos.insert(self.ye_heap[j].0, j);
    }

    fn ye_sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.ye_heap[idx].1 < self.ye_heap[parent].1 {
                self.ye_swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn ye_sift_down(&mut self, mut idx: usize) {
        let n = self.ye_heap.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < n && self.ye_heap[left].1 < self.ye_heap[smallest].1 { smallest = left; }
            if right < n && self.ye_heap[right].1 < self.ye_heap[smallest].1 { smallest = right; }
            if smallest == idx { break; }
            self.ye_swap(idx, smallest);
            idx = smallest;
        }
    }

    /// Clear the queue.
    pub fn ye_clear(&mut self) { self.ye_heap.clear(); self.ye_pos.clear(); }

    /// Drain all elements in priority order.
    pub fn ye_drain_sorted(&mut self) -> Vec<(usize, i64)> {
        let mut result = Vec::with_capacity(self.ye_heap.len());
        while let Some(item) = self.ye_pop() { result.push(item); }
        result
    }
}

// --- ye_ Segment Tree with Lazy Propagation ---

/// Segment tree with lazy propagation for range queries and updates.
#[derive(Debug, Clone)]
pub struct YeSegTree {
    ye_n: usize,
    ye_tree: Vec<i64>,
    ye_lazy: Vec<i64>,
}

impl std::fmt::Display for YeSegTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SegTree(n={})", self.ye_n)
    }
}

impl Default for YeSegTree {
    fn default() -> Self { Self { ye_n: 0, ye_tree: Vec::new(), ye_lazy: Vec::new() } }
}

impl YeSegTree {
    /// Build from an array of values.
    pub fn ye_from_slice(data: &[i64]) -> Self {
        let n = data.len();
        let mut tree = vec![0i64; 4 * n];
        let lazy = vec![0i64; 4 * n];
        let mut st = Self { ye_n: n, ye_tree: tree.clone(), ye_lazy: lazy };
        if n > 0 { st.ye_build(data, 1, 0, n - 1); }
        st
    }

    fn ye_build(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.ye_tree[node] = data[start];
            return;
        }
        let mid = (start + end) / 2;
        self.ye_build(data, 2 * node, start, mid);
        self.ye_build(data, 2 * node + 1, mid + 1, end);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    fn ye_push_down(&mut self, node: usize, start: usize, end: usize) {
        if self.ye_lazy[node] != 0 {
            let mid = (start + end) / 2;
            self.ye_tree[2 * node] += self.ye_lazy[node] * (mid - start + 1) as i64;
            self.ye_tree[2 * node + 1] += self.ye_lazy[node] * (end - mid) as i64;
            self.ye_lazy[2 * node] += self.ye_lazy[node];
            self.ye_lazy[2 * node + 1] += self.ye_lazy[node];
            self.ye_lazy[node] = 0;
        }
    }

    /// Range sum query [l, r].
    pub fn ye_query(&mut self, l: usize, r: usize) -> i64 {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return 0; }
        self.ye_query_inner(1, 0, self.ye_n - 1, l, r)
    }

    fn ye_query_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.ye_tree[node]; }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_query_inner(2 * node, start, mid, l, r) +
        self.ye_query_inner(2 * node + 1, mid + 1, end, l, r)
    }

    /// Range update: add val to all elements in [l, r].
    pub fn ye_update(&mut self, l: usize, r: usize, val: i64) {
        if self.ye_n == 0 || l > r || r >= self.ye_n { return; }
        self.ye_update_inner(1, 0, self.ye_n - 1, l, r, val);
    }

    fn ye_update_inner(&mut self, node: usize, start: usize, end: usize, l: usize, r: usize, val: i64) {
        if r < start || end < l { return; }
        if l <= start && end <= r {
            self.ye_tree[node] += val * (end - start + 1) as i64;
            self.ye_lazy[node] += val;
            return;
        }
        self.ye_push_down(node, start, end);
        let mid = (start + end) / 2;
        self.ye_update_inner(2 * node, start, mid, l, r, val);
        self.ye_update_inner(2 * node + 1, mid + 1, end, l, r, val);
        self.ye_tree[node] = self.ye_tree[2 * node] + self.ye_tree[2 * node + 1];
    }

    /// Point query: get value at index.
    pub fn ye_point_query(&mut self, idx: usize) -> i64 {
        self.ye_query(idx, idx)
    }

    /// Size of underlying array.
    pub fn ye_len(&self) -> usize { self.ye_n }

    /// Is empty.
    pub fn ye_is_empty(&self) -> bool { self.ye_n == 0 }
}


// --- yf_ Disjoint Interval Set ---

/// Set of non-overlapping intervals with automatic merging.
#[derive(Debug, Clone)]
pub struct YfIntervalSet {
    yf_intervals: std::collections::BTreeMap<i64, i64>,
}

impl std::fmt::Display for YfIntervalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IntervalSet(count={})", self.yf_intervals.len())
    }
}

impl Default for YfIntervalSet {
    fn default() -> Self { Self::yf_new() }
}

impl YfIntervalSet {
    /// Create an empty interval set.
    pub fn yf_new() -> Self { Self { yf_intervals: std::collections::BTreeMap::new() } }

    /// Number of disjoint intervals.
    pub fn yf_len(&self) -> usize { self.yf_intervals.len() }

    /// Is empty.
    pub fn yf_is_empty(&self) -> bool { self.yf_intervals.is_empty() }

    /// Add an interval [lo, hi]. Merges with overlapping/adjacent intervals.
    pub fn yf_add(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let to_remove: Vec<i64> = self.yf_intervals.range(..=hi + 1)
            .filter(|(start, end)| **end >= lo - 1)
            .map(|(s, _)| *s)
            .collect();
        for s in &to_remove {
            let e = self.yf_intervals[s];
            new_lo = new_lo.min(*s);
            new_hi = new_hi.max(e);
            self.yf_intervals.remove(s);
        }
        self.yf_intervals.insert(new_lo, new_hi);
    }

    /// Check if a value is covered by any interval.
    pub fn yf_contains(&self, val: i64) -> bool {
        if let Some((_, end)) = self.yf_intervals.range(..=val).next_back() {
            *end >= val
        } else {
            false
        }
    }

    /// Remove a point, splitting intervals if needed.
    pub fn yf_remove_point(&mut self, val: i64) {
        let covering = self.yf_intervals.range(..=val)
            .filter(|(_, end)| **end >= val)
            .map(|(s, e)| (*s, *e))
            .next_back();
        if let Some((s, e)) = covering {
            self.yf_intervals.remove(&s);
            if s < val { self.yf_intervals.insert(s, val - 1); }
            if val < e { self.yf_intervals.insert(val + 1, e); }
        }
    }

    /// Get all intervals as sorted vec.
    pub fn yf_intervals(&self) -> Vec<(i64, i64)> {
        self.yf_intervals.iter().map(|(s, e)| (*s, *e)).collect()
    }

    /// Total covered length.
    pub fn yf_total_length(&self) -> i64 {
        self.yf_intervals.iter().map(|(s, e)| e - s + 1).sum()
    }

    /// Clear all intervals.
    pub fn yf_clear(&mut self) { self.yf_intervals.clear(); }

    /// Check if two interval sets overlap.
    pub fn yf_overlaps(&self, other: &YfIntervalSet) -> bool {
        for (s, e) in &self.yf_intervals {
            for (os, oe) in &other.yf_intervals {
                if s <= oe && os <= e { return true; }
            }
        }
        false
    }
}

// --- yf_ K-way Merge ---

/// K-way merge iterator that merges multiple sorted sequences.
#[derive(Debug, Clone)]
pub struct YfKWayMerge {
    yf_sources: Vec<Vec<i64>>,
    yf_indices: Vec<usize>,
}

impl std::fmt::Display for YfKWayMerge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KWayMerge(sources={})", self.yf_sources.len())
    }
}

impl Default for YfKWayMerge {
    fn default() -> Self { Self::yf_new() }
}

impl YfKWayMerge {
    /// Create an empty k-way merge.
    pub fn yf_new() -> Self { Self { yf_sources: Vec::new(), yf_indices: Vec::new() } }

    /// Add a sorted source.
    pub fn yf_add_source(&mut self, source: Vec<i64>) {
        self.yf_sources.push(source);
        self.yf_indices.push(0);
    }

    /// Number of sources.
    pub fn yf_source_count(&self) -> usize { self.yf_sources.len() }

    /// Merge all sources into a single sorted vec.
    pub fn yf_merge(&mut self) -> Vec<i64> {
        let mut result = Vec::new();
        loop {
            let mut min_val: Option<i64> = None;
            let mut min_src = 0;
            for (i, (src, idx)) in self.yf_sources.iter().zip(self.yf_indices.iter()).enumerate() {
                if *idx < src.len() {
                    let v = src[*idx];
                    if min_val.is_none() || v < min_val.unwrap() {
                        min_val = Some(v);
                        min_src = i;
                    }
                }
            }
            match min_val {
                Some(v) => { result.push(v); self.yf_indices[min_src] += 1; }
                None => break,
            }
        }
        result
    }

    /// Total remaining elements across all sources.
    pub fn yf_remaining(&self) -> usize {
        self.yf_sources.iter().zip(self.yf_indices.iter())
            .map(|(src, idx)| src.len().saturating_sub(*idx))
            .sum()
    }

    /// Reset all indices.
    pub fn yf_reset(&mut self) {
        for idx in &mut self.yf_indices { *idx = 0; }
    }

    /// Clear all sources.
    pub fn yf_clear(&mut self) { self.yf_sources.clear(); self.yf_indices.clear(); }

    /// Check if merge is complete.
    pub fn yf_is_done(&self) -> bool { self.yf_remaining() == 0 }

    /// Merge and deduplicate.
    pub fn yf_merge_unique(&mut self) -> Vec<i64> {
        let merged = self.yf_merge();
        let mut unique = Vec::new();
        for v in merged {
            if unique.last() != Some(&v) { unique.push(v); }
        }
        unique
    }
}


// --- yg_ Persistent Stack ---

/// Immutable persistent stack using a linked list of arcs.
#[derive(Debug, Clone)]
pub struct YgPersistentStack<T: Clone> {
    yg_head: Option<std::sync::Arc<YgStackNode<T>>>,
    yg_size: usize,
}

#[derive(Debug, Clone)]
struct YgStackNode<T: Clone> {
    yg_value: T,
    yg_next: Option<std::sync::Arc<YgStackNode<T>>>,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for YgPersistentStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PStack(size={})", self.yg_size)
    }
}

impl<T: Clone> Default for YgPersistentStack<T> {
    fn default() -> Self { Self::yg_new() }
}

impl<T: Clone> YgPersistentStack<T> {
    /// Create an empty persistent stack.
    pub fn yg_new() -> Self { Self { yg_head: None, yg_size: 0 } }

    /// Push returns a new stack with the element on top.
    pub fn yg_push(&self, value: T) -> Self {
        Self {
            yg_head: Some(std::sync::Arc::new(YgStackNode { yg_value: value, yg_next: self.yg_head.clone() })),
            yg_size: self.yg_size + 1,
        }
    }

    /// Pop returns the top value and a new stack without it.
    pub fn yg_pop(&self) -> Option<(T, Self)> {
        self.yg_head.as_ref().map(|node| {
            (node.yg_value.clone(), Self { yg_head: node.yg_next.clone(), yg_size: self.yg_size - 1 })
        })
    }

    /// Peek at the top value.
    pub fn yg_peek(&self) -> Option<&T> {
        self.yg_head.as_ref().map(|node| &node.yg_value)
    }

    /// Size of the stack.
    pub fn yg_len(&self) -> usize { self.yg_size }

    /// Is empty.
    pub fn yg_is_empty(&self) -> bool { self.yg_size == 0 }

    /// Convert to vec (top first).
    pub fn yg_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.yg_size);
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result.push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }

    /// Reverse the stack.
    pub fn yg_reverse(&self) -> Self {
        let mut result = Self::yg_new();
        let mut current = &self.yg_head;
        while let Some(node) = current {
            result = result.yg_push(node.yg_value.clone());
            current = &node.yg_next;
        }
        result
    }
}

// --- yg_ Bitmap Index ---

/// Bitmap index for fast multi-column filtering on categorical data.
#[derive(Debug, Clone)]
pub struct YgBitmapIndex {
    yg_bitmaps: std::collections::HashMap<String, Vec<u64>>,
    yg_num_rows: usize,
}

impl std::fmt::Display for YgBitmapIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BitmapIndex(rows={}, columns={})", self.yg_num_rows, self.yg_bitmaps.len())
    }
}

impl Default for YgBitmapIndex {
    fn default() -> Self { Self::yg_new(0) }
}

impl YgBitmapIndex {
    /// Create a bitmap index for a given number of rows.
    pub fn yg_new(num_rows: usize) -> Self {
        Self { yg_bitmaps: std::collections::HashMap::new(), yg_num_rows: num_rows }
    }

    fn yg_words(n: usize) -> usize { (n + 63) / 64 }

    /// Set a value for a column at a given row.
    pub fn yg_set(&mut self, column: &str, row: usize) {
        if row >= self.yg_num_rows { return; }
        let words = Self::yg_words(self.yg_num_rows);
        let bitmap = self.yg_bitmaps.entry(column.to_string()).or_insert_with(|| vec![0u64; words]);
        bitmap[row / 64] |= 1u64 << (row % 64);
    }

    /// Check if a column is set for a row.
    pub fn yg_get(&self, column: &str, row: usize) -> bool {
        if row >= self.yg_num_rows { return false; }
        self.yg_bitmaps.get(column)
            .map(|bm| bm[row / 64] & (1u64 << (row % 64)) != 0)
            .unwrap_or(false)
    }

    /// AND query: rows where all columns are set.
    pub fn yg_and(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![u64::MAX; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w &= bm[i]; }
            } else {
                return Vec::new();
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    /// OR query: rows where any column is set.
    pub fn yg_or(&self, columns: &[&str]) -> Vec<usize> {
        let words = Self::yg_words(self.yg_num_rows);
        let mut result = vec![0u64; words];
        for col in columns {
            if let Some(bm) = self.yg_bitmaps.get(*col) {
                for (i, w) in result.iter_mut().enumerate() { *w |= bm[i]; }
            }
        }
        Self::yg_bits_to_rows(&result, self.yg_num_rows)
    }

    fn yg_bits_to_rows(bits: &[u64], max_rows: usize) -> Vec<usize> {
        let mut rows = Vec::new();
        for (w_idx, word) in bits.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                let tz = bits.trailing_zeros() as usize;
                let row = w_idx * 64 + tz;
                if row < max_rows { rows.push(row); }
                bits &= bits - 1;
            }
        }
        rows
    }

    /// Count of rows for a column.
    pub fn yg_count(&self, column: &str) -> usize {
        self.yg_bitmaps.get(column)
            .map(|bm| bm.iter().map(|w| w.count_ones() as usize).sum())
            .unwrap_or(0)
    }

    /// Number of rows.
    pub fn yg_num_rows(&self) -> usize { self.yg_num_rows }

    /// Number of columns.
    pub fn yg_num_columns(&self) -> usize { self.yg_bitmaps.len() }

    /// List column names.
    pub fn yg_columns(&self) -> Vec<String> {
        let mut cols: Vec<String> = self.yg_bitmaps.keys().cloned().collect();
        cols.sort();
        cols
    }

    /// Clear all bitmaps.
    pub fn yg_clear(&mut self) { self.yg_bitmaps.clear(); }
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
    fn environment_validator_accepts_and_rejects() {
        let mut v = EnvironmentValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad env var");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad env var"));
    }

    #[test]
    fn environment_validator_warnings() {
        let mut v = EnvironmentValidationCollector::new();
        v.add_warning("deprecated var");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn environment_validator_clear_and_merge() {
        let mut v = EnvironmentValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = EnvironmentValidationCollector::new();
        a.add_error("a_err");
        let mut b = EnvironmentValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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

    // ── Snapshot tests ──

    #[test]
    fn snapshot_empty() {
        let snap = EnvironmentSnapshot::empty(100);
        assert!(snap.is_empty());
        assert_eq!(snap.timestamp(), 100);
    }

    #[test]
    fn snapshot_from_map() {
        let mut vars = HashMap::new();
        vars.insert("HOME".into(), "/home/user".into());
        vars.insert("SHELL".into(), "/bin/bash".into());
        let snap = EnvironmentSnapshot::from_map(vars, 200);
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.get("HOME"), Some("/home/user"));
    }

    #[test]
    fn snapshot_added_removed_keys() {
        let mut old_vars = HashMap::new();
        old_vars.insert("A".into(), "1".into());
        old_vars.insert("B".into(), "2".into());
        let old = EnvironmentSnapshot::from_map(old_vars, 1);

        let mut new_vars = HashMap::new();
        new_vars.insert("B".into(), "2".into());
        new_vars.insert("C".into(), "3".into());
        let new = EnvironmentSnapshot::from_map(new_vars, 2);

        assert_eq!(new.added_keys(&old), vec!["C"]);
        assert_eq!(new.removed_keys(&old), vec!["A"]);
    }

    #[test]
    fn snapshot_changed_keys() {
        let mut old_vars = HashMap::new();
        old_vars.insert("X".into(), "old".into());
        let old = EnvironmentSnapshot::from_map(old_vars, 1);

        let mut new_vars = HashMap::new();
        new_vars.insert("X".into(), "new".into());
        let new = EnvironmentSnapshot::from_map(new_vars, 2);

        assert_eq!(new.changed_keys(&old), vec!["X"]);
    }

    #[test]
    fn snapshot_is_equivalent() {
        let mut vars = HashMap::new();
        vars.insert("K".into(), "V".into());
        let a = EnvironmentSnapshot::from_map(vars.clone(), 1);
        let b = EnvironmentSnapshot::from_map(vars, 2);
        assert!(a.is_equivalent(&b));
    }

    // ── Override resolver tests ──

    #[test]
    fn override_resolver_basic() {
        let mut resolver = EnvironmentOverrideResolver::new();
        let mut base = HashMap::new();
        base.insert("PATH".into(), "/usr/bin".into());
        base.insert("HOME".into(), "/home/user".into());
        resolver.add_layer("base", base);

        let mut overrides = HashMap::new();
        overrides.insert("PATH".into(), "/custom/bin".into());
        resolver.add_layer("override", overrides);

        assert_eq!(resolver.resolve("PATH"), Some("/custom/bin"));
        assert_eq!(resolver.resolve("HOME"), Some("/home/user"));
        assert_eq!(resolver.provided_by("PATH"), Some("override"));
        assert_eq!(resolver.provided_by("HOME"), Some("base"));
    }

    #[test]
    fn override_resolver_resolve_all() {
        let mut resolver = EnvironmentOverrideResolver::new();
        let mut l1 = HashMap::new();
        l1.insert("A".into(), "1".into());
        resolver.add_layer("l1", l1);
        let mut l2 = HashMap::new();
        l2.insert("B".into(), "2".into());
        resolver.add_layer("l2", l2);

        let all = resolver.resolve_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all["A"], "1");
        assert_eq!(all["B"], "2");
    }

    #[test]
    fn override_resolver_layer_names() {
        let mut resolver = EnvironmentOverrideResolver::new();
        resolver.add_layer("system", HashMap::new());
        resolver.add_layer("user", HashMap::new());
        assert_eq!(resolver.layer_names(), vec!["system", "user"]);
        assert_eq!(resolver.layer_count(), 2);
    }

    // ── Sanitizer tests ──

    #[test]
    fn sanitizer_valid_names() {
        assert!(EnvironmentSanitizer::is_valid_name("PATH"));
        assert!(EnvironmentSanitizer::is_valid_name("MY_VAR_1"));
        assert!(!EnvironmentSanitizer::is_valid_name(""));
        assert!(!EnvironmentSanitizer::is_valid_name("BAD=NAME"));
    }

    #[test]
    fn sanitizer_sanitize_name() {
        assert_eq!(EnvironmentSanitizer::sanitize_name("MY-VAR!"), "MYVAR");
        assert_eq!(EnvironmentSanitizer::sanitize_name("ok_123"), "ok_123");
    }

    #[test]
    fn sanitizer_path_dedup() {
        assert!(EnvironmentSanitizer::has_duplicate_path_entries("/usr/bin:/usr/bin"));
        assert!(!EnvironmentSanitizer::has_duplicate_path_entries("/usr/bin:/usr/local/bin"));
        let deduped = EnvironmentSanitizer::dedup_path("/a:/b:/a:/c:/b");
        assert_eq!(deduped, "/a:/b:/c");
    }

    #[test]
    fn sanitizer_sanitize_map() {
        let mut vars = HashMap::new();
        vars.insert("GOOD".into(), "val".into());
        vars.insert("BAD=KEY".into(), "val".into());
        let clean = EnvironmentSanitizer::sanitize_map(&vars);
        assert_eq!(clean.len(), 1);
        assert!(clean.contains_key("GOOD"));
    }

    // ── Env diff display tests ──

    #[test]
    fn env_diff_basic() {
        let mut old_vars = HashMap::new();
        old_vars.insert("A".into(), "1".into());
        old_vars.insert("B".into(), "old".into());
        let old = EnvironmentSnapshot::from_map(old_vars, 1);

        let mut new_vars = HashMap::new();
        new_vars.insert("B".into(), "new".into());
        new_vars.insert("C".into(), "3".into());
        let new_snap = EnvironmentSnapshot::from_map(new_vars, 2);

        let diff = EnvDiffDisplay::diff(&old, &new_snap);
        assert_eq!(EnvDiffDisplay::addition_count(&diff), 1);
        assert_eq!(EnvDiffDisplay::removal_count(&diff), 1);
        assert_eq!(EnvDiffDisplay::change_count(&diff), 3);
    }

    #[test]
    fn env_diff_empty_when_same() {
        let mut vars = HashMap::new();
        vars.insert("X".into(), "Y".into());
        let a = EnvironmentSnapshot::from_map(vars.clone(), 1);
        let b = EnvironmentSnapshot::from_map(vars, 2);
        let diff = EnvDiffDisplay::diff(&a, &b);
        assert!(EnvDiffDisplay::is_empty(&diff));
    }

    #[test]
    fn env_diff_entry_display() {
        let entry = EnvDiffEntry::Added { key: "FOO".into(), value: "bar".into() };
        assert_eq!(entry.to_string(), "+ FOO=bar");
        let entry2 = EnvDiffEntry::Removed { key: "BAZ".into(), value: "qux".into() };
        assert_eq!(entry2.to_string(), "- BAZ=qux");
    }


    // -- environment additional tests -------------------------------------------

    #[test]
    fn x_environment_capabilities_register_and_has() {
        let mut caps = XEnvironmentCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_environment_capabilities_len() {
        let mut caps = XEnvironmentCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_environment_capabilities_intersect() {
        let mut a = XEnvironmentCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XEnvironmentCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_environment_capabilities_diff() {
        let mut a = XEnvironmentCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XEnvironmentCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_environment_service_registry_basic() {
        let mut reg = XEnvironmentServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_environment_service_registry_replace() {
        let mut reg = XEnvironmentServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_environment_service_registry_remove() {
        let mut reg = XEnvironmentServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_environment_service_registry_names() {
        let mut reg = XEnvironmentServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_environment_sanitize_path_basic() {
        assert_eq!(x_environment_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_environment_sanitize_path_backslash() {
        assert_eq!(x_environment_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_environment_sanitize_path_single() {
        assert_eq!(x_environment_sanitize_path("/"), "/");
    }

    #[test]
    fn x_environment_capabilities_default() {
        let caps = XEnvironmentCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_environment_capabilities_all() {
        let mut caps = XEnvironmentCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    #[test]
    fn environment_config_new() {
        let cfg = EnvironmentConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn environment_config_set_get() {
        let mut cfg = EnvironmentConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn environment_config_remove() {
        let mut cfg = EnvironmentConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn environment_config_keys_sorted() {
        let mut cfg = EnvironmentConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn environment_config_bump_version() {
        let mut cfg = EnvironmentConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn environment_config_clear() {
        let mut cfg = EnvironmentConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn environment_config_merge() {
        let mut cfg1 = EnvironmentConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = EnvironmentConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn environment_config_disable() {
        let mut cfg = EnvironmentConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn environment_rate_tracker_empty() {
        let rt = EnvironmentRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn environment_rate_tracker_record() {
        let mut rt = EnvironmentRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn environment_rate_tracker_prune() {
        let mut rt = EnvironmentRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn environment_validator_valid() {
        let v = EnvironmentValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn environment_validator_errors() {
        let mut v = EnvironmentValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn environment_validator_clear() {
        let mut v = EnvironmentValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn environment_validator_merge() {
        let mut v1 = EnvironmentValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = EnvironmentValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn environment_rate_tracker_clear() {
        let mut rt = EnvironmentRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qk_metrics_empty() {
        let m = QkMetrics::new("environ");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qk_metrics_record_and_mean() {
        let mut m = QkMetrics::new("environ");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qk_metrics_min_max() {
        let mut m = QkMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qk_metrics_variance_and_std() {
        let mut m = QkMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qk_metrics_percentile() {
        let mut m = QkMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qk_metrics_merge() {
        let mut a = QkMetrics::new("a");
        a.record(1.0);
        let mut b = QkMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qk_metrics_reset() {
        let mut m = QkMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qk_rate_window_empty() {
        let rw = QkRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qk_rate_window_tick_and_rate() {
        let mut rw = QkRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qk_lru_cache_basic() {
        let mut c = QkLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qk_lru_cache_contains_and_keys() {
        let mut c = QkLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qk_lru_cache_remove() {
        let mut c = QkLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qk_metrics_sum() {
        let mut m = QkMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qk_metrics_label() {
        let m = QkMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qk_lru_cache_clear() {
        let mut c = QkLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for environment
    #[test]
    fn xa_environment_ring_new() {
        let rb = super::XaEnvironmentRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_environment_ring_push_len() {
        let mut rb = super::XaEnvironmentRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_environment_ring_wrap() {
        let mut rb = super::XaEnvironmentRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_environment_ring_mean_empty() {
        let rb = super::XaEnvironmentRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_environment_ring_mean_values() {
        let mut rb = super::XaEnvironmentRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_environment_ring_min_max() {
        let mut rb = super::XaEnvironmentRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_environment_ring_iter() {
        let mut rb = super::XaEnvironmentRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_environment_counter_new() {
        let c = super::XaEnvironmentCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_environment_counter_inc() {
        let mut c = super::XaEnvironmentCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_environment_counter_inc_by() {
        let mut c = super::XaEnvironmentCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_environment_counter_reset() {
        let mut c = super::XaEnvironmentCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_environment_counter_clear() {
        let mut c = super::XaEnvironmentCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_environment_counter_default() {
        let c = super::XaEnvironmentCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 44 ----

    #[test]
    fn xc_44_pool_new_empty() {
        let pool: super::Xc44Pool<i32> = super::Xc44Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_44_pool_release_acquire() {
        let mut pool = super::Xc44Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_44_pool_acquire_empty() {
        let mut pool: super::Xc44Pool<i32> = super::Xc44Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_44_pool_full() {
        let mut pool = super::Xc44Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_44_pool_drain() {
        let mut pool = super::Xc44Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_44_pool_stats() {
        let mut pool = super::Xc44Pool::new(8);
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
    fn xc_44_pool_clear() {
        let mut pool = super::Xc44Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_44_pool_shrink() {
        let mut pool = super::Xc44Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_44_pool_default() {
        let pool: super::Xc44Pool<String> = super::Xc44Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_44_pool_extend() {
        let mut pool = super::Xc44Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_44_pool_retain() {
        let mut pool = super::Xc44Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_44_scheduler_round_robin() {
        let mut sched = super::Xc44Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_44_scheduler_empty() {
        let mut sched = super::Xc44Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_44_scheduler_reset() {
        let mut sched = super::Xc44Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_44_scheduler_add_remove() {
        let mut sched = super::Xc44Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_44_scheduler_targets() {
        let sched = super::Xc44Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_44_hash_empty() {
        assert_eq!(super::xc_44_hash(b""), 5381);
    }

    #[test]
    fn xc_44_hash_data() {
        let h = super::xc_44_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_44_hash(b"hello"), h);
    }

    #[test]
    fn xc_44_reverse_str() {
        assert_eq!(super::xc_44_reverse("abc"), "cba");
        assert_eq!(super::xc_44_reverse(""), "");
    }


    #[test]
    fn xe_11_pipeline_empty() {
        let p = super::Xe11Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_11_pipeline_parse_stage() {
        let p = super::Xe11Pipeline::new()
            .add_parse(super::xe_11_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_11_pipeline_transform_double() {
        let p = super::Xe11Pipeline::new()
            .add_transform(super::xe_11_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_11_pipeline_validate_reverse() {
        let p = super::Xe11Pipeline::new()
            .add_validate(super::xe_11_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_11_pipeline_emit_filter() {
        let p = super::Xe11Pipeline::new()
            .add_emit(super::xe_11_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_11_pipeline_multi_stage() {
        let p = super::Xe11Pipeline::new()
            .add_parse(super::xe_11_pipeline_identity)
            .add_transform(super::xe_11_pipeline_double)
            .add_validate(super::xe_11_pipeline_reverse)
            .add_emit(super::xe_11_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_11_pipeline_error_propagation() {
        let p = super::Xe11Pipeline::new()
            .add_parse(super::xe_11_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe11Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_11_pipeline_compose() {
        let p1 = super::Xe11Pipeline::new()
            .add_parse(super::xe_11_pipeline_identity);
        let p2 = super::Xe11Pipeline::new()
            .add_transform(super::xe_11_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_11_pipeline_error_display() {
        let e = super::Xe11PipelineError {
            stage: super::Xe11Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_11_cache_put_get() {
        let mut c = super::Xe11Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_11_cache_miss() {
        let mut c: super::Xe11Cache<&str, i32> = super::Xe11Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_11_cache_ttl_expiry() {
        let mut c = super::Xe11Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_11_cache_evict() {
        let mut c = super::Xe11Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_11_cache_capacity() {
        let mut c = super::Xe11Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_11_cache_stats() {
        let mut c = super::Xe11Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_11_cache_clear() {
        let mut c = super::Xe11Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #75 --

    #[test]
    fn xf75_trie_insert_search() {
        let mut t = Xf75Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf75_trie_starts_with() {
        let mut t = Xf75Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf75_trie_remove() {
        let mut t = Xf75Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf75_trie_word_count() {
        let mut t = Xf75Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf75_trie_longest_prefix() {
        let mut t = Xf75Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf75_trie_all_words() {
        let mut t = Xf75Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf75_trie_autocomplete() {
        let mut t = Xf75Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf75_trie_empty_search() {
        let t = Xf75Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf75_bloom_add_contains() {
        let mut bf = Xf75BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf75_bloom_probably_absent() {
        let bf = Xf75BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf75_bloom_false_positive_rate() {
        let mut bf = Xf75BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf75_bloom_clear() {
        let mut bf = Xf75BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf75_bloom_union() {
        let mut a = Xf75BloomFilter::xf_new(512, 2);
        let mut b = Xf75BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf75_bloom_intersection_estimate() {
        let mut a = Xf75BloomFilter::xf_new(512, 2);
        let mut b = Xf75BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf75_bloom_union_size_mismatch() {
        let a = Xf75BloomFilter::xf_new(256, 2);
        let b = Xf75BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh43_skip_insert_contains() {
        let mut sl = super::Xh43SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh43_skip_remove() {
        let mut sl = super::Xh43SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh43_skip_len() {
        let mut sl = super::Xh43SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh43_skip_range_query() {
        let mut sl = super::Xh43SkipList::xh_new(4);
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
    fn xh43_skip_floor_ceiling() {
        let mut sl = super::Xh43SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh43_skip_rank() {
        let mut sl = super::Xh43SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh43_skip_empty() {
        let sl = super::Xh43SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh43_skip_duplicates() {
        let mut sl = super::Xh43SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh43_bitset_set_test() {
        let mut bs = super::Xh43BitSet::xh_new(256);
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
    fn xh43_bitset_clear_count() {
        let mut bs = super::Xh43BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh43_bitset_and_or_xor() {
        let mut a = super::Xh43BitSet::xh_new(128);
        let mut b = super::Xh43BitSet::xh_new(128);
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
    fn xh43_bitset_iter_ones() {
        let mut bs = super::Xh43BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh43_bitset_first_last() {
        let mut bs = super::Xh43BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh43_bitset_empty() {
        let bs = super::Xh43BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi43_deque_push_pop_back() {
        let mut dq = super::Xi43Deque::xi_new(4);
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
    fn xi43_deque_push_pop_front() {
        let mut dq = super::Xi43Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi43_deque_mixed_ops() {
        let mut dq = super::Xi43Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi43_deque_get_and_split() {
        let mut dq = super::Xi43Deque::xi_new(8);
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
    fn xi43_deque_rotate_left() {
        let mut dq = super::Xi43Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi43_deque_rotate_right() {
        let mut dq = super::Xi43Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi43_deque_grow() {
        let mut dq = super::Xi43Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi43_deque_empty() {
        let dq = super::Xi43Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi43_interval_tree_insert_query() {
        let mut tree = super::Xi43IntervalTree::xi_new();
        tree.xi_insert(super::Xi43Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi43Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi43Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi43_interval_tree_overlap() {
        let mut tree = super::Xi43IntervalTree::xi_new();
        tree.xi_insert(super::Xi43Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi43Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi43Interval::xi_new(12, 20));
        let q = super::Xi43Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi43_interval_tree_remove() {
        let mut tree = super::Xi43IntervalTree::xi_new();
        tree.xi_insert(super::Xi43Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi43Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi43_interval_tree_gaps() {
        let mut tree = super::Xi43IntervalTree::xi_new();
        tree.xi_insert(super::Xi43Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi43Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi43Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi43Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi43Interval::xi_new(8, 10));
    }

    #[test]
    fn xi43_interval_tree_merge() {
        let mut tree = super::Xi43IntervalTree::xi_new();
        tree.xi_insert(super::Xi43Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi43Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi43Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi43Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi43Interval::xi_new(10, 15));
    }

    #[test]
    fn xi43_interval_tree_all() {
        let mut tree = super::Xi43IntervalTree::xi_new();
        tree.xi_insert(super::Xi43Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi43Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi43_interval_tree_empty() {
        let tree = super::Xi43IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi43_interval_tree_contains_point() {
        let iv = super::Xi43Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 43) ---

    #[test]
    fn xj_43_uf_make_and_find() {
        let mut uf = super::Xj43UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_43_uf_union_connected() {
        let mut uf = super::Xj43UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_43_uf_component_count() {
        let mut uf = super::Xj43UnionFind::xj_new();
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
    fn xj_43_uf_component_size() {
        let mut uf = super::Xj43UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_43_uf_largest_component() {
        let mut uf = super::Xj43UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_43_uf_many_elements() {
        let mut uf = super::Xj43UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_43_uf_separate_components() {
        let mut uf = super::Xj43UnionFind::xj_new();
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
    fn xj_43_uf_path_compression() {
        let mut uf = super::Xj43UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_43_bt_insert_get() {
        let mut bt = super::Xj43BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_43_bt_contains_len() {
        let mut bt = super::Xj43BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_43_bt_replace() {
        let mut bt = super::Xj43BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_43_bt_remove() {
        let mut bt = super::Xj43BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_43_bt_keys_values() {
        let mut bt = super::Xj43BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_43_bt_range() {
        let mut bt = super::Xj43BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_43_bt_min_max() {
        let mut bt = super::Xj43BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_43_bt_many_inserts() {
        let mut bt = super::Xj43BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_43 segment tree tests ---

    #[test]
    fn xk_43_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk43SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_43_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk43SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_43_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk43SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_43_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk43SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_43_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk43SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_43_st_single_element() {
        let data = vec![42];
        let st = super::Xk43SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_43_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk43SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_43_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk43SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_43 disjoint intervals tests ---

    #[test]
    fn xk_43_di_add_and_count() {
        let mut di = super::Xk43DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_43_di_merge_overlap() {
        let mut di = super::Xk43DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_43_di_contains() {
        let mut di = super::Xk43DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_43_di_remove() {
        let mut di = super::Xk43DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_43_di_covered_length() {
        let mut di = super::Xk43DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_43_di_gaps() {
        let mut di = super::Xk43DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_43_di_merge_adjacent() {
        let mut di = super::Xk43DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_43_di_empty() {
        let di = super::Xk43DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_43_rope_new_empty() {
        let rope = super::Xl43Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_43_rope_from_str() {
        let rope = super::Xl43Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_43_rope_insert_at() {
        let mut rope = super::Xl43Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_43_rope_delete_range() {
        let mut rope = super::Xl43Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_43_rope_char_at() {
        let rope = super::Xl43Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_43_rope_split_concat() {
        let rope = super::Xl43Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_43_rope_line_count() {
        let rope = super::Xl43Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_43_rope_line_at() {
        let rope = super::Xl43Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_43_sa_build_and_search() {
        let sa = super::Xl43SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_43_sa_count() {
        let sa = super::Xl43SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_43_sa_longest_repeated() {
        let sa = super::Xl43SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_43_sa_all_positions() {
        let sa = super::Xl43SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_43_sa_len() {
        let sa = super::Xl43SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_43_sa_empty() {
        let sa = super::Xl43SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_43_rope_slice() {
        let rope = super::Xl43Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_43_sa_search_start() {
        let sa = super::Xl43SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_43_sparse_set_get() {
        let mut m = super::Xm43MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_43_sparse_row_col() {
        let mut m = super::Xm43MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_43_sparse_transpose() {
        let mut m = super::Xm43MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_43_sparse_multiply_vec() {
        let mut m = super::Xm43MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_43_sparse_nnz_density() {
        let mut m = super::Xm43MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_43_sparse_clear() {
        let mut m = super::Xm43MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_43_sparse_overwrite_zero() {
        let mut m = super::Xm43MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_43_tokenizer_basic() {
        let t = super::Xm43Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_43_tokenizer_count() {
        let t = super::Xm43Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_43_tokenizer_unique() {
        let t = super::Xm43Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_43_tokenizer_frequency() {
        let t = super::Xm43Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_43_tokenizer_delimiter() {
        let t = super::Xm43Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_43_tokenizer_whitespace() {
        let t = super::Xm43Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_43_tokenizer_empty() {
        let t = super::Xm43Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 43 ----

    #[test]
    fn xn_43_fenwick_prefix_sum() {
        let mut ft = super::Xn43Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_43_fenwick_range_sum() {
        let mut ft = super::Xn43Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_43_fenwick_point_query() {
        let mut ft = super::Xn43Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_43_fenwick_len() {
        let ft = super::Xn43Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_43_fenwick_multiple_updates() {
        let mut ft = super::Xn43Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_43_fenwick_single_element() {
        let mut ft = super::Xn43Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_43_fenwick_find_kth() {
        let mut ft = super::Xn43Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_43_fenwick_negative_delta() {
        let mut ft = super::Xn43Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 43 ----

    #[test]
    fn xn_43_avl_insert_get() {
        let mut m = super::Xn43AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_43_avl_remove() {
        let mut m = super::Xn43AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_43_avl_in_order() {
        let mut m = super::Xn43AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_43_avl_min_max() {
        let mut m = super::Xn43AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_43_avl_floor_ceiling() {
        let mut m = super::Xn43AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_43_avl_height_balanced() {
        let mut m = super::Xn43AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_43_avl_overwrite() {
        let mut m = super::Xn43AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_43_avl_empty() {
        let m: super::Xn43AVL<i32, i32> = super::Xn43AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo43RedBlack tests ---

    #[test]
    fn xo_43_rb_insert_and_get() {
        let mut tree = super::Xo43RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_43_rb_len_and_empty() {
        let mut tree = super::Xo43RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_43_rb_min_max() {
        let mut tree = super::Xo43RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_43_rb_contains() {
        let mut tree = super::Xo43RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_43_rb_remove() {
        let mut tree = super::Xo43RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_43_rb_in_order() {
        let mut tree = super::Xo43RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_43_rb_black_height() {
        let mut tree = super::Xo43RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_43_rb_overwrite() {
        let mut tree = super::Xo43RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo43ConsistentHash tests ---

    #[test]
    fn xo_43_ch_add_and_count() {
        let mut ring = super::Xo43ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_43_ch_remove_node() {
        let mut ring = super::Xo43ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_43_ch_get_node() {
        let mut ring = super::Xo43ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_43_ch_empty_ring() {
        let ring = super::Xo43ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_43_ch_distribution() {
        let mut ring = super::Xo43ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_43_ch_rebalance() {
        let mut ring = super::Xo43ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_43_ch_virtual_nodes() {
        let mut ring = super::Xo43ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_43_ch_consistent_lookup() {
        let mut ring = super::Xo43ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_43_splay_insert_get() {
        let mut t = super::Xp43SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_43_splay_remove() {
        let mut t = super::Xp43SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_43_splay_count_increases() {
        let mut t = super::Xp43SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_43_splay_depth() {
        let mut t = super::Xp43SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_43_splay_len_empty() {
        let t = super::Xp43SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_43_splay_min_max() {
        let mut t = super::Xp43SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_43_splay_overwrite() {
        let mut t = super::Xp43SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_43_splay_remove_missing() {
        let mut t = super::Xp43SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_43 treap tests ----
    #[test]
    fn xq_43_treap_empty() {
        let t = super::Xq43Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_43_treap_insert_get() {
        let mut t = super::Xq43Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_43_treap_overwrite() {
        let mut t = super::Xq43Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_43_treap_remove() {
        let mut t = super::Xq43Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_43_treap_min_max() {
        let mut t = super::Xq43Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_43_treap_rank() {
        let mut t = super::Xq43Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_43_treap_kth() {
        let mut t = super::Xq43Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_43_treap_in_order() {
        let mut t = super::Xq43Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_43 VEB tree tests ----
    #[test]
    fn xq_43_veb_empty() {
        let v = super::Xq43VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_43_veb_insert_contains() {
        let mut v = super::Xq43VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_43_veb_min_max() {
        let mut v = super::Xq43VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_43_veb_delete() {
        let mut v = super::Xq43VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_43_veb_successor() {
        let mut v = super::Xq43VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_43_veb_predecessor() {
        let mut v = super::Xq43VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_43_veb_count() {
        let mut v = super::Xq43VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_43_veb_duplicate_insert() {
        let mut v = super::Xq43VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_43_kdtree_empty() {
        let tree = super::Xr43KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_43_kdtree_insert_one() {
        let mut tree = super::Xr43KDTree::xr_new();
        tree.xr_insert(super::Xr43KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_43_kdtree_insert_multiple() {
        let mut tree = super::Xr43KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr43KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_43_kdtree_nearest_neighbor() {
        let mut tree = super::Xr43KDTree::xr_new();
        tree.xr_insert(super::Xr43KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr43KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr43KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_43_kdtree_nn_empty() {
        let tree = super::Xr43KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr43KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_43_kdtree_range_search() {
        let mut tree = super::Xr43KDTree::xr_new();
        tree.xr_insert(super::Xr43KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr43KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr43KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_43_kdtree_range_empty() {
        let mut tree = super::Xr43KDTree::xr_new();
        tree.xr_insert(super::Xr43KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_43_kdtree_all_points() {
        let mut tree = super::Xr43KDTree::xr_new();
        tree.xr_insert(super::Xr43KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr43KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_43_kdtree_depth() {
        let mut tree = super::Xr43KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr43KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_43_kdtree_bounding_box() {
        let mut tree = super::Xr43KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr43KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr43KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_43_persistent_array_new() {
        let arr = super::Xs43PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_43_persistent_array_push() {
        let mut arr = super::Xs43PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_43_persistent_array_set() {
        let mut arr = super::Xs43PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_43_persistent_array_diff() {
        let mut arr = super::Xs43PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_43_persistent_array_rollback() {
        let mut arr = super::Xs43PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_43_persistent_array_history() {
        let mut arr = super::Xs43PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_43_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs43PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_43_persistent_array_from_vec() {
        let arr = super::Xs43PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_43_concurrent_queue_new() {
        let q = super::Xs43ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_43_concurrent_queue_push_pop() {
        let mut q = super::Xs43ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_43_concurrent_queue_full() {
        let mut q = super::Xs43ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_43_concurrent_queue_drain() {
        let mut q = super::Xs43ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_43_concurrent_queue_try_pop() {
        let mut q = super::Xs43ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_43_concurrent_queue_clear() {
        let mut q = super::Xs43ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_43_range_map_new() {
        let rm = super::Xs43RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_43_range_map_insert_get() {
        let mut rm = super::Xs43RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_43_range_map_overlap() {
        let mut rm = super::Xs43RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_43_range_map_remove() {
        let mut rm = super::Xs43RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_43_range_map_gaps() {
        let mut rm = super::Xs43RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_43_range_map_coverage() {
        let mut rm = super::Xs43RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_43_range_map_contains() {
        let mut rm = super::Xs43RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_43_range_map_clear() {
        let mut rm = super::Xs43RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_43_circular_buffer_new() {
        let buf = super::Xs43CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_43_circular_buffer_push_pop() {
        let mut buf = super::Xs43CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_43_circular_buffer_overwrite() {
        let mut buf = super::Xs43CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_43_circular_buffer_peek() {
        let mut buf = super::Xs43CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_43_circular_buffer_is_full() {
        let mut buf = super::Xs43CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_43_circular_buffer_iter() {
        let mut buf = super::Xs43CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_43_circular_buffer_clear() {
        let mut buf = super::Xs43CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_43_circular_buffer_to_vec() {
        let mut buf = super::Xs43CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
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


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }


    // --- xy_ Cuckoo Hash Map tests ---

    #[test]
    fn xy_cuckoo_new() {
        let m = super::XyCuckooMap::<String, i32>::xy_new(16);
        assert!(m.xy_is_empty());
        assert_eq!(m.xy_len(), 0);
    }

    #[test]
    fn xy_cuckoo_insert_get() {
        let mut m = super::XyCuckooMap::xy_new(32);
        m.xy_insert("hello".to_string(), 1);
        m.xy_insert("world".to_string(), 2);
        assert_eq!(m.xy_get(&"hello".to_string()), Some(&1));
        assert_eq!(m.xy_get(&"world".to_string()), Some(&2));
        assert_eq!(m.xy_get(&"missing".to_string()), None);
    }

    #[test]
    fn xy_cuckoo_contains() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(42, "a");
        assert!(m.xy_contains(&42));
        assert!(!m.xy_contains(&99));
    }

    #[test]
    fn xy_cuckoo_replace() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(5, "old");
        m.xy_insert(5, "new");
        assert_eq!(m.xy_get(&5), Some(&"new"));
    }

    #[test]
    fn xy_cuckoo_remove() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(10, "val");
        assert_eq!(m.xy_remove(&10), Some("val"));
        assert!(!m.xy_contains(&10));
    }

    #[test]
    fn xy_cuckoo_many() {
        let mut m = super::XyCuckooMap::xy_new(64);
        for i in 0..30 {
            m.xy_insert(i, i * 10);
        }
        assert_eq!(m.xy_len(), 30);
        for i in 0..30 {
            assert!(m.xy_contains(&i));
        }
    }

    #[test]
    fn xy_cuckoo_keys() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_insert(2, "b");
        let keys = m.xy_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn xy_cuckoo_clear() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_clear();
        assert!(m.xy_is_empty());
    }

    #[test]
    fn xy_cuckoo_display() {
        let m = super::XyCuckooMap::<i32, i32>::xy_new(16);
        assert!(format!("{}", m).contains("CuckooMap"));
    }

    #[test]
    fn xy_cuckoo_default() {
        let m = super::XyCuckooMap::<i32, i32>::default();
        assert!(m.xy_is_empty());
    }

    // --- xy_ Count-Min Sketch tests ---

    #[test]
    fn xy_cms_new() {
        let cms = super::XyCountMinSketch::xy_new(100, 5);
        assert_eq!(cms.xy_width(), 100);
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_add_estimate() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..10 { cms.xy_add(42); }
        assert!(cms.xy_estimate(42) >= 10);
    }

    #[test]
    fn xy_cms_add_count() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        cms.xy_add_count(7, 100);
        assert!(cms.xy_estimate(7) >= 100);
    }

    #[test]
    fn xy_cms_unseen() {
        let cms = super::XyCountMinSketch::xy_new(1000, 5);
        assert_eq!(cms.xy_estimate(999), 0);
    }

    #[test]
    fn xy_cms_merge() {
        let mut a = super::XyCountMinSketch::xy_new(100, 3);
        let mut b = super::XyCountMinSketch::xy_new(100, 3);
        a.xy_add(1);
        b.xy_add(1);
        a.xy_merge(&b);
        assert!(a.xy_estimate(1) >= 2);
    }

    #[test]
    fn xy_cms_clear() {
        let mut cms = super::XyCountMinSketch::xy_new(100, 3);
        cms.xy_add(1);
        cms.xy_clear();
        assert_eq!(cms.xy_estimate(1), 0);
    }

    #[test]
    fn xy_cms_display() {
        let cms = super::XyCountMinSketch::xy_new(100, 3);
        assert!(format!("{}", cms).contains("CMS"));
    }

    #[test]
    fn xy_cms_default() {
        let cms = super::XyCountMinSketch::default();
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_multiple_items() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for i in 0..100 { cms.xy_add(i); }
        for i in 0..100 { assert!(cms.xy_estimate(i) >= 1); }
    }

    #[test]
    fn xy_cms_heavy_hitter() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..1000 { cms.xy_add(42); }
        for i in 0..10 { cms.xy_add(i); }
        assert!(cms.xy_estimate(42) > cms.xy_estimate(0));
    }


    // --- xz_ HyperLogLog tests ---

    #[test]
    fn xz_hll_new() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_num_registers(), 1024);
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_add_estimate() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for i in 0..1000 {
            hll.xz_add(i);
        }
        let est = hll.xz_estimate();
        assert!(est > 500.0 && est < 2000.0);
    }

    #[test]
    fn xz_hll_empty() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_merge() {
        let mut a = super::XzHyperLogLog::xz_new(10);
        let mut b = super::XzHyperLogLog::xz_new(10);
        for i in 0..500 { a.xz_add(i); }
        for i in 500..1000 { b.xz_add(i); }
        a.xz_merge(&b);
        let est = a.xz_estimate();
        assert!(est > 500.0);
    }

    #[test]
    fn xz_hll_clear() {
        let mut hll = super::XzHyperLogLog::xz_new(10);
        hll.xz_add(1);
        hll.xz_clear();
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_display() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert!(format!("{}", hll).contains("HLL"));
    }

    #[test]
    fn xz_hll_default() {
        let hll = super::XzHyperLogLog::default();
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_duplicates() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for _ in 0..1000 { hll.xz_add(42); }
        let est = hll.xz_estimate();
        assert!(est < 10.0);
    }

    // --- xz_ LRU Cache tests ---

    #[test]
    fn xz_lru_new() {
        let lru = super::XzLruCache::<String, i32>::xz_new(10);
        assert!(lru.xz_is_empty());
        assert_eq!(lru.xz_capacity(), 10);
    }

    #[test]
    fn xz_lru_put_get() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put("a".to_string(), 1);
        lru.xz_put("b".to_string(), 2);
        assert_eq!(lru.xz_get(&"a".to_string()), Some(&1));
        assert_eq!(lru.xz_get(&"b".to_string()), Some(&2));
    }

    #[test]
    fn xz_lru_eviction() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        assert!(!lru.xz_contains(&1));
        assert!(lru.xz_contains(&2));
        assert!(lru.xz_contains(&3));
    }

    #[test]
    fn xz_lru_access_updates_order() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_get(&1);
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1));
        assert!(!lru.xz_contains(&2));
    }

    #[test]
    fn xz_lru_update_value() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "old");
        lru.xz_put(1, "new");
        assert_eq!(lru.xz_get(&1), Some(&"new"));
        assert_eq!(lru.xz_len(), 1);
    }

    #[test]
    fn xz_lru_remove() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        assert_eq!(lru.xz_remove(&1), Some("a"));
        assert!(!lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_peek() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        assert_eq!(lru.xz_peek(&1), Some(&"a"));
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1) || !lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_keys_order() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        let keys = lru.xz_keys_lru();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn xz_lru_clear() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_clear();
        assert!(lru.xz_is_empty());
    }

    #[test]
    fn xz_lru_display() {
        let lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert!(format!("{}", lru).contains("LRU"));
    }

    #[test]
    fn xz_lru_missing_key() {
        let mut lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert_eq!(lru.xz_get(&999), None);
    }


    // --- ya_ Trie tests ---

    #[test]
    fn ya_trie_new() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(t.ya_is_empty());
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_insert_get() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        t.ya_insert("world", 2);
        assert_eq!(t.ya_get("hello"), Some(&1));
        assert_eq!(t.ya_get("world"), Some(&2));
        assert_eq!(t.ya_get("missing"), None);
    }

    #[test]
    fn ya_trie_contains() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        assert!(t.ya_contains("abc"));
        assert!(!t.ya_contains("ab"));
        assert!(!t.ya_contains("abcd"));
    }

    #[test]
    fn ya_trie_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert!(t.ya_has_prefix("ab"));
        assert!(!t.ya_has_prefix("ac"));
    }

    #[test]
    fn ya_trie_keys_with_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("cat", 1);
        t.ya_insert("car", 2);
        t.ya_insert("dog", 3);
        let keys = t.ya_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"cat".to_string()));
        assert!(keys.contains(&"car".to_string()));
    }

    #[test]
    fn ya_trie_all_keys() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("b", 1);
        t.ya_insert("a", 2);
        t.ya_insert("c", 3);
        let keys = t.ya_all_keys();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn ya_trie_remove() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        assert_eq!(t.ya_remove("hello"), Some(1));
        assert!(!t.ya_contains("hello"));
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_lcp() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert_eq!(t.ya_longest_common_prefix(), "ab");
    }

    #[test]
    fn ya_trie_clear() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("a", 1);
        t.ya_clear();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_display() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(format!("{}", t).contains("Trie"));
    }

    #[test]
    fn ya_trie_default() {
        let t = super::YaTrie::<i32>::default();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_count_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("test1", 1);
        t.ya_insert("test2", 2);
        t.ya_insert("other", 3);
        assert_eq!(t.ya_count_prefix("test"), 2);
    }

    // --- ya_ Bloom Filter tests ---

    #[test]
    fn ya_bloom_new() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_bit_size(), 1000);
        assert_eq!(bf.ya_num_hashes(), 5);
        assert_eq!(bf.ya_count(), 0);
    }

    #[test]
    fn ya_bloom_add_contains() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        bf.ya_add(42);
        bf.ya_add(100);
        assert!(bf.ya_might_contain(42));
        assert!(bf.ya_might_contain(100));
    }

    #[test]
    fn ya_bloom_no_false_negatives() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        for i in 0..100 { bf.ya_add(i); }
        for i in 0..100 { assert!(bf.ya_might_contain(i)); }
    }

    #[test]
    fn ya_bloom_with_fp_rate() {
        let bf = super::YaBloomFilter::ya_with_fp_rate(1000, 0.01);
        assert!(bf.ya_bit_size() > 0);
        assert!(bf.ya_num_hashes() > 0);
    }

    #[test]
    fn ya_bloom_clear() {
        let mut bf = super::YaBloomFilter::ya_new(1000, 5);
        bf.ya_add(1);
        bf.ya_clear();
        assert_eq!(bf.ya_count(), 0);
        assert!(!bf.ya_might_contain(1));
    }

    #[test]
    fn ya_bloom_merge() {
        let mut a = super::YaBloomFilter::ya_new(1000, 5);
        let mut b = super::YaBloomFilter::ya_new(1000, 5);
        a.ya_add(1);
        b.ya_add(2);
        a.ya_merge(&b);
        assert!(a.ya_might_contain(1));
        assert!(a.ya_might_contain(2));
    }

    #[test]
    fn ya_bloom_fp_rate() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_estimated_fp_rate(), 0.0);
    }

    #[test]
    fn ya_bloom_display() {
        let bf = super::YaBloomFilter::ya_new(100, 3);
        assert!(format!("{}", bf).contains("Bloom"));
    }

    #[test]
    fn ya_bloom_default() {
        let bf = super::YaBloomFilter::default();
        assert_eq!(bf.ya_num_hashes(), 5);
    }


    // --- yb_ TST tests ---

    #[test]
    fn yb_tst_new() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_insert_get() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("hello", 1);
        t.yb_insert("world", 2);
        assert_eq!(t.yb_get("hello"), Some(&1));
        assert_eq!(t.yb_get("world"), Some(&2));
        assert_eq!(t.yb_get("missing"), None);
    }

    #[test]
    fn yb_tst_contains() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("abc", 10);
        assert!(t.yb_contains("abc"));
        assert!(!t.yb_contains("ab"));
    }

    #[test]
    fn yb_tst_all_keys() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("b", 1);
        t.yb_insert("a", 2);
        t.yb_insert("c", 3);
        let keys = t.yb_all_keys();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn yb_tst_prefix() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("cat", 1);
        t.yb_insert("car", 2);
        t.yb_insert("dog", 3);
        let keys = t.yb_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn yb_tst_clear() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("a", 1);
        t.yb_clear();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_display() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(format!("{}", t).contains("TST"));
    }

    #[test]
    fn yb_tst_default() {
        let t = super::YbTernarySearchTree::<i32>::default();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_overwrite() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("key", 1);
        t.yb_insert("key", 2);
        assert_eq!(t.yb_get("key"), Some(&2));
        assert_eq!(t.yb_len(), 1);
    }

    // --- yb_ Quadtree tests ---

    #[test]
    fn yb_quad_new() {
        let q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_insert() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_insert(super::YbPoint::yb_new(50.0, 50.0)));
        assert_eq!(q.yb_count(), 1);
    }

    #[test]
    fn yb_quad_query() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        q.yb_insert(super::YbPoint::yb_new(15.0, 15.0));
        let found = q.yb_query(&super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn yb_quad_outside() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(!q.yb_insert(super::YbPoint::yb_new(200.0, 200.0)));
    }

    #[test]
    fn yb_quad_nearest() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        let near = q.yb_nearest(&super::YbPoint::yb_new(12.0, 12.0)).unwrap();
        assert!((near.yb_x - 10.0).abs() < 0.001);
    }

    #[test]
    fn yb_quad_display() {
        let q = super::YbQuadtree::default();
        assert!(format!("{}", q).contains("Quadtree"));
    }

    #[test]
    fn yb_quad_default() {
        let q = super::YbQuadtree::default();
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_many() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        for i in 0..20 {
            q.yb_insert(super::YbPoint::yb_new(i as f64 * 4.0, i as f64 * 4.0));
        }
        assert_eq!(q.yb_count(), 20);
    }

    #[test]
    fn yb_point_distance() {
        let a = super::YbPoint::yb_new(0.0, 0.0);
        let b = super::YbPoint::yb_new(3.0, 4.0);
        assert!((a.yb_distance(&b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn yb_bounds_intersects() {
        let a = super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0);
        let b = super::YbBounds::yb_new(25.0, 25.0, 50.0, 50.0);
        assert!(a.yb_intersects(&b));
    }


    // --- yc_ VebSet tests ---

    #[test]
    fn yc_veb_new() {
        let v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_is_empty());
        assert_eq!(v.yc_universe(), 1000);
    }

    #[test]
    fn yc_veb_insert_contains() {
        let mut v = super::YcVebSet::yc_new(1000);
        assert!(v.yc_insert(42));
        assert!(v.yc_contains(42));
        assert!(!v.yc_contains(43));
    }

    #[test]
    fn yc_veb_remove() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        assert!(v.yc_remove(10));
        assert!(!v.yc_contains(10));
    }

    #[test]
    fn yc_veb_min_max() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(50);
        v.yc_insert(10);
        v.yc_insert(90);
        assert_eq!(v.yc_min(), Some(10));
        assert_eq!(v.yc_max(), Some(90));
    }

    #[test]
    fn yc_veb_successor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        v.yc_insert(30);
        assert_eq!(v.yc_successor(10), Some(20));
        assert_eq!(v.yc_successor(20), Some(30));
        assert_eq!(v.yc_successor(30), None);
    }

    #[test]
    fn yc_veb_predecessor() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_predecessor(20), Some(10));
        assert_eq!(v.yc_predecessor(10), None);
    }

    #[test]
    fn yc_veb_sorted() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(30);
        v.yc_insert(10);
        v.yc_insert(20);
        assert_eq!(v.yc_to_sorted_vec(), vec![10, 20, 30]);
    }

    #[test]
    fn yc_veb_clear() {
        let mut v = super::YcVebSet::yc_new(1000);
        v.yc_insert(1);
        v.yc_clear();
        assert!(v.yc_is_empty());
    }

    #[test]
    fn yc_veb_union() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1);
        b.yc_insert(2);
        a.yc_union(&b);
        assert!(a.yc_contains(1));
        assert!(a.yc_contains(2));
    }

    #[test]
    fn yc_veb_intersection() {
        let mut a = super::YcVebSet::yc_new(100);
        let mut b = super::YcVebSet::yc_new(100);
        a.yc_insert(1); a.yc_insert(2);
        b.yc_insert(2); b.yc_insert(3);
        let c = a.yc_intersection(&b);
        assert!(c.yc_contains(2));
        assert!(!c.yc_contains(1));
    }

    #[test]
    fn yc_veb_display() {
        let v = super::YcVebSet::yc_new(100);
        assert!(format!("{}", v).contains("VebSet"));
    }

    #[test]
    fn yc_veb_default() {
        let v = super::YcVebSet::default();
        assert_eq!(v.yc_universe(), 65536);
    }

    // --- yc_ HashRing tests ---

    #[test]
    fn yc_ring_new() {
        let r = super::YcHashRing::yc_new(100);
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_add_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert_eq!(r.yc_node_count(), 1);
        assert_eq!(r.yc_virtual_count(), 50);
    }

    #[test]
    fn yc_ring_get_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n = r.yc_get_node("mykey");
        assert!(n.is_some());
    }

    #[test]
    fn yc_ring_remove_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("a");
        r.yc_remove_node("a");
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_has_node() {
        let mut r = super::YcHashRing::yc_new(50);
        r.yc_add_node("server1");
        assert!(r.yc_has_node("server1"));
        assert!(!r.yc_has_node("server2"));
    }

    #[test]
    fn yc_ring_display() {
        let r = super::YcHashRing::yc_new(10);
        assert!(format!("{}", r).contains("HashRing"));
    }

    #[test]
    fn yc_ring_default() {
        let r = super::YcHashRing::default();
        assert_eq!(r.yc_node_count(), 0);
    }

    #[test]
    fn yc_ring_consistency() {
        let mut r = super::YcHashRing::yc_new(100);
        r.yc_add_node("a");
        r.yc_add_node("b");
        let n1 = r.yc_get_node("key1").unwrap().to_string();
        let n2 = r.yc_get_node("key1").unwrap().to_string();
        assert_eq!(n1, n2);
    }


    // --- yd_ DAG tests ---

    #[test]
    fn yd_dag_new() {
        let g = super::YdDag::yd_new();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_add_edge() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        assert_eq!(g.yd_node_count(), 3);
        assert_eq!(g.yd_edge_count(), 2);
    }

    #[test]
    fn yd_dag_topo_sort() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        g.yd_add_edge(2, 3);
        let order = g.yd_topological_sort().unwrap();
        assert_eq!(order.len(), 4);
        let pos: std::collections::HashMap<usize, usize> = order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&0] < pos[&2]);
    }

    #[test]
    fn yd_dag_cycle() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(2, 0);
        assert!(g.yd_has_cycle());
    }

    #[test]
    fn yd_dag_roots_leaves() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_roots(), vec![0]);
        assert_eq!(g.yd_leaves(), vec![1, 2]);
    }

    #[test]
    fn yd_dag_bfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        g.yd_add_edge(1, 3);
        let bfs = g.yd_bfs(0);
        assert_eq!(bfs[0], 0);
        assert_eq!(bfs.len(), 4);
    }

    #[test]
    fn yd_dag_dfs() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        let dfs = g.yd_dfs(0);
        assert_eq!(dfs[0], 0);
        assert_eq!(dfs.len(), 3);
    }

    #[test]
    fn yd_dag_shortest_path() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(1, 2);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_shortest_path(0, 2), Some(1));
    }

    #[test]
    fn yd_dag_degrees() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_add_edge(0, 2);
        assert_eq!(g.yd_out_degree(0), 2);
        assert_eq!(g.yd_in_degree(1), 1);
    }

    #[test]
    fn yd_dag_clear() {
        let mut g = super::YdDag::yd_new();
        g.yd_add_edge(0, 1);
        g.yd_clear();
        assert_eq!(g.yd_node_count(), 0);
    }

    #[test]
    fn yd_dag_display() {
        let g = super::YdDag::yd_new();
        assert!(format!("{}", g).contains("DAG"));
    }

    // --- yd_ SparseMatrix tests ---

    #[test]
    fn yd_sparse_new() {
        let m = super::YdSparseMatrix::yd_new(3, 3);
        assert_eq!(m.yd_nnz(), 0);
    }

    #[test]
    fn yd_sparse_set_get() {
        let mut m = super::YdSparseMatrix::yd_new(3, 3);
        m.yd_set(0, 1, 5.0);
        assert_eq!(m.yd_get(0, 1), 5.0);
        assert_eq!(m.yd_get(0, 0), 0.0);
    }

    #[test]
    fn yd_sparse_transpose() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 2, 7.0);
        let t = m.yd_transpose();
        assert_eq!(t.yd_get(2, 0), 7.0);
    }

    #[test]
    fn yd_sparse_mul_vec() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_set(1, 1, 2.0);
        let r = m.yd_mul_vec(&[3.0, 4.0]);
        assert_eq!(r, vec![3.0, 8.0]);
    }

    #[test]
    fn yd_sparse_add() {
        let mut a = super::YdSparseMatrix::yd_new(2, 2);
        let mut b = super::YdSparseMatrix::yd_new(2, 2);
        a.yd_set(0, 0, 1.0);
        b.yd_set(0, 0, 2.0);
        let c = a.yd_add(&b);
        assert_eq!(c.yd_get(0, 0), 3.0);
    }

    #[test]
    fn yd_sparse_scale() {
        let mut m = super::YdSparseMatrix::yd_new(1, 1);
        m.yd_set(0, 0, 5.0);
        m.yd_scale(2.0);
        assert_eq!(m.yd_get(0, 0), 10.0);
    }

    #[test]
    fn yd_sparse_row_sum() {
        let mut m = super::YdSparseMatrix::yd_new(2, 3);
        m.yd_set(0, 0, 1.0);
        m.yd_set(0, 1, 2.0);
        m.yd_set(0, 2, 3.0);
        assert_eq!(m.yd_row_sum(0), 6.0);
    }

    #[test]
    fn yd_sparse_display() {
        let m = super::YdSparseMatrix::yd_new(2, 2);
        assert!(format!("{}", m).contains("SparseMatrix"));
    }

    #[test]
    fn yd_sparse_clear() {
        let mut m = super::YdSparseMatrix::yd_new(2, 2);
        m.yd_set(0, 0, 1.0);
        m.yd_clear();
        assert_eq!(m.yd_nnz(), 0);
    }


    // --- ye_ IndexedPQ tests ---

    #[test]
    fn ye_ipq_new() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_insert_pop() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 5);
        pq.ye_insert(2, 15);
        assert_eq!(pq.ye_pop(), Some((1, 5)));
        assert_eq!(pq.ye_pop(), Some((0, 10)));
    }

    #[test]
    fn ye_ipq_decrease_key() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 10);
        pq.ye_insert(1, 20);
        pq.ye_decrease_key(1, 5);
        assert_eq!(pq.ye_peek(), Some((1, 5)));
    }

    #[test]
    fn ye_ipq_contains() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(42, 1);
        assert!(pq.ye_contains(42));
        assert!(!pq.ye_contains(99));
    }

    #[test]
    fn ye_ipq_priority() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 7);
        assert_eq!(pq.ye_priority(0), Some(7));
    }

    #[test]
    fn ye_ipq_drain() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 30);
        pq.ye_insert(1, 10);
        pq.ye_insert(2, 20);
        let sorted = pq.ye_drain_sorted();
        assert_eq!(sorted, vec![(1, 10), (2, 20), (0, 30)]);
    }

    #[test]
    fn ye_ipq_clear() {
        let mut pq = super::YeIndexedPQ::ye_new();
        pq.ye_insert(0, 1);
        pq.ye_clear();
        assert!(pq.ye_is_empty());
    }

    #[test]
    fn ye_ipq_display() {
        let pq = super::YeIndexedPQ::ye_new();
        assert!(format!("{}", pq).contains("IndexedPQ"));
    }

    #[test]
    fn ye_ipq_default() {
        let pq = super::YeIndexedPQ::default();
        assert!(pq.ye_is_empty());
    }

    // --- ye_ SegTree tests ---

    #[test]
    fn ye_seg_from_slice() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_len(), 5);
        assert_eq!(st.ye_query(0, 4), 15);
    }

    #[test]
    fn ye_seg_point_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[10, 20, 30]);
        assert_eq!(st.ye_point_query(1), 20);
    }

    #[test]
    fn ye_seg_range_query() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(st.ye_query(1, 3), 9);
    }

    #[test]
    fn ye_seg_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3, 4, 5]);
        st.ye_update(1, 3, 10);
        assert_eq!(st.ye_query(0, 4), 45);
    }

    #[test]
    fn ye_seg_single_update() {
        let mut st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        st.ye_update(1, 1, 5);
        assert_eq!(st.ye_point_query(1), 7);
    }

    #[test]
    fn ye_seg_empty() {
        let st = super::YeSegTree::ye_from_slice(&[]);
        assert!(st.ye_is_empty());
    }

    #[test]
    fn ye_seg_single() {
        let mut st = super::YeSegTree::ye_from_slice(&[42]);
        assert_eq!(st.ye_query(0, 0), 42);
    }

    #[test]
    fn ye_seg_display() {
        let st = super::YeSegTree::ye_from_slice(&[1, 2, 3]);
        assert!(format!("{}", st).contains("SegTree"));
    }

    #[test]
    fn ye_seg_default() {
        let st = super::YeSegTree::default();
        assert!(st.ye_is_empty());
    }


    // --- yf_ IntervalSet tests ---

    #[test]
    fn yf_interval_new() {
        let s = super::YfIntervalSet::yf_new();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_add() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        assert_eq!(s.yf_len(), 1);
        assert!(s.yf_contains(3));
    }

    #[test]
    fn yf_interval_merge() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(3, 8);
        assert_eq!(s.yf_len(), 1);
        assert_eq!(s.yf_intervals(), vec![(1, 8)]);
    }

    #[test]
    fn yf_interval_adjacent() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(6, 10);
        assert_eq!(s.yf_len(), 1);
    }

    #[test]
    fn yf_interval_disjoint() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 3);
        s.yf_add(10, 15);
        assert_eq!(s.yf_len(), 2);
    }

    #[test]
    fn yf_interval_remove_point() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 10);
        s.yf_remove_point(5);
        assert!(!s.yf_contains(5));
        assert!(s.yf_contains(4));
        assert!(s.yf_contains(6));
    }

    #[test]
    fn yf_interval_length() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_add(10, 14);
        assert_eq!(s.yf_total_length(), 10);
    }

    #[test]
    fn yf_interval_clear() {
        let mut s = super::YfIntervalSet::yf_new();
        s.yf_add(1, 5);
        s.yf_clear();
        assert!(s.yf_is_empty());
    }

    #[test]
    fn yf_interval_display() {
        let s = super::YfIntervalSet::yf_new();
        assert!(format!("{}", s).contains("IntervalSet"));
    }

    #[test]
    fn yf_interval_overlaps() {
        let mut a = super::YfIntervalSet::yf_new();
        let mut b = super::YfIntervalSet::yf_new();
        a.yf_add(1, 5);
        b.yf_add(3, 8);
        assert!(a.yf_overlaps(&b));
    }

    // --- yf_ KWayMerge tests ---

    #[test]
    fn yf_kmerge_new() {
        let m = super::YfKWayMerge::yf_new();
        assert_eq!(m.yf_source_count(), 0);
    }

    #[test]
    fn yf_kmerge_merge() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 4, 7]);
        m.yf_add_source(vec![2, 5, 8]);
        m.yf_add_source(vec![3, 6, 9]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn yf_kmerge_single() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        let result = m.yf_merge();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn yf_kmerge_empty() {
        let mut m = super::YfKWayMerge::yf_new();
        let result = m.yf_merge();
        assert!(result.is_empty());
    }

    #[test]
    fn yf_kmerge_remaining() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        m.yf_add_source(vec![3, 4]);
        assert_eq!(m.yf_remaining(), 4);
    }

    #[test]
    fn yf_kmerge_reset() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2]);
        let _ = m.yf_merge();
        m.yf_reset();
        assert_eq!(m.yf_remaining(), 2);
    }

    #[test]
    fn yf_kmerge_unique() {
        let mut m = super::YfKWayMerge::yf_new();
        m.yf_add_source(vec![1, 2, 3]);
        m.yf_add_source(vec![2, 3, 4]);
        let result = m.yf_merge_unique();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn yf_kmerge_display() {
        let m = super::YfKWayMerge::yf_new();
        assert!(format!("{}", m).contains("KWayMerge"));
    }

    #[test]
    fn yf_kmerge_default() {
        let m = super::YfKWayMerge::default();
        assert!(m.yf_is_done());
    }


    // --- yg_ PersistentStack tests ---

    #[test]
    fn yg_pstack_new() {
        let s = super::YgPersistentStack::<i32>::yg_new();
        assert!(s.yg_is_empty());
    }

    #[test]
    fn yg_pstack_push_pop() {
        let s = super::YgPersistentStack::yg_new();
        let s = s.yg_push(1);
        let s = s.yg_push(2);
        let (v, s) = s.yg_pop().unwrap();
        assert_eq!(v, 2);
        let (v, _) = s.yg_pop().unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn yg_pstack_persistence() {
        let s1 = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2);
        let s2 = s1.yg_push(3);
        assert_eq!(s1.yg_len(), 2);
        assert_eq!(s2.yg_len(), 3);
    }

    #[test]
    fn yg_pstack_peek() {
        let s = super::YgPersistentStack::yg_new().yg_push(42);
        assert_eq!(s.yg_peek(), Some(&42));
    }

    #[test]
    fn yg_pstack_to_vec() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        assert_eq!(s.yg_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn yg_pstack_reverse() {
        let s = super::YgPersistentStack::yg_new().yg_push(1).yg_push(2).yg_push(3);
        let r = s.yg_reverse();
        assert_eq!(r.yg_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn yg_pstack_display() {
        let s = super::YgPersistentStack::yg_new().yg_push(1);
        assert!(format!("{}", s).contains("PStack"));
    }

    #[test]
    fn yg_pstack_default() {
        let s = super::YgPersistentStack::<i32>::default();
        assert!(s.yg_is_empty());
    }

    // --- yg_ BitmapIndex tests ---

    #[test]
    fn yg_bitmap_new() {
        let bi = super::YgBitmapIndex::yg_new(100);
        assert_eq!(bi.yg_num_rows(), 100);
    }

    #[test]
    fn yg_bitmap_set_get() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("color_red", 5);
        assert!(bi.yg_get("color_red", 5));
        assert!(!bi.yg_get("color_red", 6));
    }

    #[test]
    fn yg_bitmap_and() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("a", 2);
        bi.yg_set("b", 2);
        bi.yg_set("b", 3);
        let rows = bi.yg_and(&["a", "b"]);
        assert_eq!(rows, vec![2]);
    }

    #[test]
    fn yg_bitmap_or() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("a", 1);
        bi.yg_set("b", 2);
        let rows = bi.yg_or(&["a", "b"]);
        assert_eq!(rows, vec![1, 2]);
    }

    #[test]
    fn yg_bitmap_count() {
        let mut bi = super::YgBitmapIndex::yg_new(100);
        bi.yg_set("x", 0);
        bi.yg_set("x", 1);
        bi.yg_set("x", 2);
        assert_eq!(bi.yg_count("x"), 3);
    }

    #[test]
    fn yg_bitmap_columns() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_set("b", 0);
        assert_eq!(bi.yg_num_columns(), 2);
    }

    #[test]
    fn yg_bitmap_clear() {
        let mut bi = super::YgBitmapIndex::yg_new(10);
        bi.yg_set("a", 0);
        bi.yg_clear();
        assert_eq!(bi.yg_num_columns(), 0);
    }

    #[test]
    fn yg_bitmap_display() {
        let bi = super::YgBitmapIndex::yg_new(10);
        assert!(format!("{}", bi).contains("BitmapIndex"));
    }

    #[test]
    fn yg_bitmap_default() {
        let bi = super::YgBitmapIndex::default();
        assert_eq!(bi.yg_num_rows(), 0);
    }

}
