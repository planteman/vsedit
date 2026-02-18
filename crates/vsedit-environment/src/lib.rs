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

}
