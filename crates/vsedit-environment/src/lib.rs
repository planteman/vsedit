//! Environment and paths service.
//!
//! Equivalent to VS Code's `vs/platform/environment/common/environment.ts`.
//! Provides well-known paths and CLI arguments for vsedit.

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
}
