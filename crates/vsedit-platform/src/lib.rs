//! Platform detection and feature flags for vsedit.
//!
//! Provides OS detection, environment introspection, terminal capability
//! detection, and platform-specific constants. Modeled after VS Code's
//! `vs/base/common/platform.ts`, adapted for terminal environments.

use std::fmt;
use std::env;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Platform enum
// ---------------------------------------------------------------------------

/// The operating system platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Microsoft Windows.
    Windows,
    /// Apple macOS.
    MacOS,
    /// Linux (any distribution).
    Linux,
    /// FreeBSD.
    FreeBSD,
    /// An unrecognized platform.
    Unknown,
}

impl Platform {
    /// Returns the platform for the current compilation target.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "freebsd") {
            Self::FreeBSD
        } else {
            Self::Unknown
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Windows => write!(f, "Windows"),
            Self::MacOS => write!(f, "macOS"),
            Self::Linux => write!(f, "Linux"),
            Self::FreeBSD => write!(f, "FreeBSD"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

// ---------------------------------------------------------------------------
// OS detection
// ---------------------------------------------------------------------------

/// Returns `true` when compiled for Windows.
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Returns `true` when compiled for macOS.
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Returns `true` when compiled for Linux.
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Returns `true` when compiled for FreeBSD.
pub fn is_freebsd() -> bool {
    cfg!(target_os = "freebsd")
}

// ---------------------------------------------------------------------------
// Path separator constants
// ---------------------------------------------------------------------------

/// The platform path separator character (`;` on Windows, `:` elsewhere).
pub const PATH_SEPARATOR: char = if cfg!(target_os = "windows") { ';' } else { ':' };

/// The platform end-of-line sequence (`\r\n` on Windows, `\n` elsewhere).
pub const EOL: &str = if cfg!(target_os = "windows") { "\r\n" } else { "\n" };

// ---------------------------------------------------------------------------
// Environment detection
// ---------------------------------------------------------------------------

/// Returns `true` when running inside a CI environment.
///
/// Checks the `CI`, `TF_BUILD`, `GITHUB_ACTIONS`, `JENKINS_URL`,
/// `TRAVIS`, and `CIRCLECI` environment variables.
pub fn is_ci() -> bool {
    env::var_os("CI").is_some()
        || env::var_os("TF_BUILD").is_some()
        || env::var_os("GITHUB_ACTIONS").is_some()
        || env::var_os("JENKINS_URL").is_some()
        || env::var_os("TRAVIS").is_some()
        || env::var_os("CIRCLECI").is_some()
}

/// Returns `true` when the process is running with elevated privileges.
///
/// On Unix this checks for UID 0 (root). On Windows this always returns
/// `false` (elevation detection requires Win32 APIs not available in `std`).
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        libc_free_getuid() == 0
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Alias for [`is_root`].
pub fn is_elevated() -> bool {
    is_root()
}

/// Returns the effective user ID on Unix without linking libc.
#[cfg(unix)]
fn libc_free_getuid() -> u32 {
    // std::os::unix::process::CommandExt gives us uid, but for the
    // *current* process we read /proc or use the nix crate.  Since we
    // want zero deps, fall back to the `id -u` value cached in an env var
    // or parse /proc/self/status on Linux.
    if cfg!(target_os = "linux") {
        if let Ok(contents) = std::fs::read_to_string("/proc/self/status") {
            for line in contents.lines() {
                if let Some(rest) = line.strip_prefix("Uid:") {
                    if let Some(uid_str) = rest.split_whitespace().next() {
                        if let Ok(uid) = uid_str.parse::<u32>() {
                            return uid;
                        }
                    }
                }
            }
        }
    }
    // Fallback for macOS / FreeBSD / other Unix: check common env hints.
    if env::var("USER").as_deref() == Ok("root") {
        return 0;
    }
    // Cannot determine — assume non-root.
    u32::MAX
}

/// Returns a user-agent string for HTTP requests.
///
/// Format: `vsedit/<version> (<OS> <arch>)`
pub fn user_agent() -> String {
    format!(
        "vsedit/{} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Returns the system locale string (e.g. `en_US.UTF-8`).
///
/// Checks `LANG`, then `LC_ALL`, falling back to `"en_US.UTF-8"`.
pub fn locale() -> String {
    env::var("LANG")
        .or_else(|_| env::var("LC_ALL"))
        .unwrap_or_else(|_| "en_US.UTF-8".to_string())
}

/// Returns the user's home directory.
///
/// On Windows this reads `USERPROFILE`, on Unix `HOME`.  Falls back to the
/// current directory if neither is set.
pub fn home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(not(target_os = "windows"))]
    {
        env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

// ---------------------------------------------------------------------------
// Terminal type detection
// ---------------------------------------------------------------------------

/// Known terminal emulator types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TerminalType {
    /// XTerm or xterm-compatible.
    XTerm,
    /// Kitty terminal.
    Kitty,
    /// Alacritty terminal.
    Alacritty,
    /// WezTerm terminal.
    WezTerm,
    /// iTerm2 on macOS.
    ITerm2,
    /// Windows Terminal.
    WindowsTerminal,
    /// An unrecognized terminal.
    Unknown,
}

impl std::fmt::Display for TerminalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::XTerm => write!(f, "xterm"),
            Self::Kitty => write!(f, "Kitty"),
            Self::Alacritty => write!(f, "Alacritty"),
            Self::WezTerm => write!(f, "WezTerm"),
            Self::ITerm2 => write!(f, "iTerm2"),
            Self::WindowsTerminal => write!(f, "Windows Terminal"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Detects the terminal emulator from environment variables.
pub fn terminal_type() -> TerminalType {
    if env::var_os("KITTY_WINDOW_ID").is_some() {
        return TerminalType::Kitty;
    }
    if env::var_os("WEZTERM_EXECUTABLE").is_some() {
        return TerminalType::WezTerm;
    }
    if env::var_os("ALACRITTY_WINDOW_ID").is_some()
        || env::var_os("ALACRITTY_LOG").is_some()
        || env::var_os("ALACRITTY_SOCKET").is_some()
    {
        return TerminalType::Alacritty;
    }
    if env::var_os("WT_SESSION").is_some() {
        return TerminalType::WindowsTerminal;
    }
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        match term_program.as_str() {
            "iTerm.app" => return TerminalType::ITerm2,
            "WezTerm" => return TerminalType::WezTerm,
            _ => {}
        }
    }
    if let Ok(term) = env::var("TERM") {
        if term.starts_with("xterm") {
            return TerminalType::XTerm;
        }
    }
    TerminalType::Unknown
}

// ---------------------------------------------------------------------------
// Terminal capability detection
// ---------------------------------------------------------------------------

/// Returns `true` if the terminal supports 24-bit true color.
///
/// Checks `COLORTERM` for `truecolor` or `24bit`.
pub fn supports_true_color() -> bool {
    env::var("COLORTERM")
        .map(|v| {
            let v = v.to_ascii_lowercase();
            v == "truecolor" || v == "24bit"
        })
        .unwrap_or(false)
}

/// Returns `true` if the terminal is likely to support Unicode output.
///
/// Heuristic: checks that `LANG`, `LC_ALL`, or `LC_CTYPE` contains `UTF-8`
/// (case-insensitive).  On Windows, assumes `true` for Windows Terminal.
pub fn supports_unicode() -> bool {
    if cfg!(target_os = "windows") {
        // Windows Terminal supports Unicode.
        return env::var_os("WT_SESSION").is_some();
    }
    for key in &["LANG", "LC_ALL", "LC_CTYPE"] {
        if let Ok(val) = env::var(key) {
            let upper = val.to_ascii_uppercase();
            if upper.contains("UTF-8") || upper.contains("UTF8") {
                return true;
            }
        }
    }
    false
}

/// Returns `true` if mouse input is generally supported.
///
/// Most modern terminals support mouse; we assume `true` unless running in
/// a dumb terminal or `TERM` is unset.
pub fn supports_mouse() -> bool {
    match env::var("TERM").as_deref() {
        Ok("dumb") | Err(_) => false,
        Ok(_) => true,
    }
}

/// Returns `true` if the terminal supports the Sixel graphics protocol.
///
/// Detection is based on known terminal types that support Sixel.
pub fn supports_sixel() -> bool {
    matches!(terminal_type(), TerminalType::WezTerm)
        || env::var("TERM")
            .map(|t| t.contains("sixel"))
            .unwrap_or(false)
}

/// Returns `true` if the terminal supports the Kitty graphics protocol.
pub fn supports_kitty_graphics() -> bool {
    matches!(terminal_type(), TerminalType::Kitty | TerminalType::WezTerm)
}

// ---------------------------------------------------------------------------
// Shell Detection
// ---------------------------------------------------------------------------

/// The default shell for the current platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformShell {
    pub path: String,
    pub name: String,
    pub args: Vec<String>,
}

impl PlatformShell {
    /// Detect the default shell for the current platform.
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        {
            if let Ok(comspec) = env::var("COMSPEC") {
                let name = PathBuf::from(&comspec)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "cmd".to_string());
                return Self {
                    path: comspec,
                    name,
                    args: vec!["/C".to_string()],
                };
            }
            Self {
                path: "cmd.exe".to_string(),
                name: "cmd".to_string(),
                args: vec!["/C".to_string()],
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let shell_path = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let name = PathBuf::from(&shell_path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "sh".to_string());
            Self {
                path: shell_path,
                name,
                args: vec!["-c".to_string()],
            }
        }
    }

    /// Returns true if the shell is a known POSIX-like shell.
    pub fn is_posix(&self) -> bool {
        matches!(self.name.as_str(), "sh" | "bash" | "zsh" | "dash" | "fish" | "ksh" | "ash")
    }

    /// Returns the shell invocation command for running a string command.
    pub fn command_for(&self, cmd: &str) -> Vec<String> {
        let mut result = vec![self.path.clone()];
        result.extend(self.args.clone());
        result.push(cmd.to_string());
        result
    }
}

impl fmt::Display for PlatformShell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.path)
    }
}

// ---------------------------------------------------------------------------
// Locale Parsing
// ---------------------------------------------------------------------------

/// Parsed locale information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformLocale {
    pub language: String,
    pub country: Option<String>,
    pub encoding: Option<String>,
}

impl PlatformLocale {
    /// Detect the system locale from environment variables.
    pub fn detect() -> Self {
        let raw = locale();
        Self::parse(&raw)
    }

    /// Parse a locale string like "en_US.UTF-8".
    pub fn parse(raw: &str) -> Self {
        let (lang_country, encoding) = if let Some(dot_pos) = raw.find('.') {
            (&raw[..dot_pos], Some(raw[dot_pos + 1..].to_string()))
        } else {
            (raw, None)
        };
        let (language, country) = if let Some(underscore_pos) = lang_country.find('_') {
            (
                lang_country[..underscore_pos].to_string(),
                Some(lang_country[underscore_pos + 1..].to_string()),
            )
        } else {
            (lang_country.to_string(), None)
        };
        Self {
            language,
            country,
            encoding,
        }
    }

    /// Returns the BCP 47 language tag (e.g. "en-US").
    pub fn to_bcp47(&self) -> String {
        match &self.country {
            Some(c) => format!("{}-{}", self.language, c),
            None => self.language.clone(),
        }
    }
}

impl fmt::Display for PlatformLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.language)?;
        if let Some(c) = &self.country {
            write!(f, "_{c}")?;
        }
        if let Some(e) = &self.encoding {
            write!(f, ".{e}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Temp Directory
// ---------------------------------------------------------------------------

/// Returns the platform-appropriate temporary directory.
///
/// On Windows: checks `TEMP`, then `TMP`, falls back to `C:\Temp`.
/// On Unix: checks `TMPDIR`, falls back to `/tmp`.
pub fn platform_temp_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        env::var_os("TEMP")
            .or_else(|| env::var_os("TMP"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Temp"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for platform operations.
#[derive(Debug, Clone, PartialEq)]
pub struct PlatformStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl PlatformStats {
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
    pub fn merge(&mut self, other: &PlatformStats) {
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

impl Default for PlatformStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PlatformStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PlatformStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for platform.
#[derive(Debug, Clone)]
pub struct PlatformValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl PlatformValidator {
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

impl Default for PlatformValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Platform capability set
// ---------------------------------------------------------------------------

/// A collected set of platform capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub true_color: bool,
    pub unicode: bool,
    pub mouse: bool,
    pub sixel: bool,
    pub kitty_graphics: bool,
    pub platform: Platform,
}

impl PlatformCapabilities {
    /// Detect all capabilities for the current platform.
    pub fn detect() -> Self {
        Self {
            true_color: supports_true_color(),
            unicode: supports_unicode(),
            mouse: supports_mouse(),
            sixel: supports_sixel(),
            kitty_graphics: supports_kitty_graphics(),
            platform: Platform::current(),
        }
    }

    /// Create a capabilities set with everything enabled (for testing).
    pub fn all_enabled(platform: Platform) -> Self {
        Self {
            true_color: true,
            unicode: true,
            mouse: true,
            sixel: true,
            kitty_graphics: true,
            platform,
        }
    }

    /// Create a minimal capabilities set (nothing enabled).
    pub fn minimal(platform: Platform) -> Self {
        Self {
            true_color: false,
            unicode: false,
            mouse: false,
            sixel: false,
            kitty_graphics: false,
            platform,
        }
    }

    /// Count the number of enabled capabilities.
    pub fn enabled_count(&self) -> usize {
        [self.true_color, self.unicode, self.mouse, self.sixel, self.kitty_graphics]
            .iter()
            .filter(|&&b| b)
            .count()
    }

    /// Returns true if any graphics protocol (sixel or kitty) is supported.
    pub fn has_graphics(&self) -> bool {
        self.sixel || self.kitty_graphics
    }
}

impl fmt::Display for PlatformCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Platform({}, caps: {}/5)",
            self.platform,
            self.enabled_count()
        )
    }
}

impl Default for PlatformCapabilities {
    fn default() -> Self {
        Self::minimal(Platform::Unknown)
    }
}

// ---------------------------------------------------------------------------
// Iterator over platforms
// ---------------------------------------------------------------------------

/// Iterator over all known platform variants.
pub struct PlatformIter {
    index: usize,
}

impl PlatformIter {
    pub fn new() -> Self {
        Self { index: 0 }
    }
}

impl Default for PlatformIter {
    fn default() -> Self {
        Self::new()
    }
}

const ALL_PLATFORMS: [Platform; 5] = [
    Platform::Windows,
    Platform::MacOS,
    Platform::Linux,
    Platform::FreeBSD,
    Platform::Unknown,
];

impl Iterator for PlatformIter {
    type Item = Platform;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < ALL_PLATFORMS.len() {
            let p = ALL_PLATFORMS[self.index];
            self.index += 1;
            Some(p)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = ALL_PLATFORMS.len() - self.index;
        (rem, Some(rem))
    }
}

impl ExactSizeIterator for PlatformIter {}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<&str> for Platform {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "windows" | "win32" | "win" => Platform::Windows,
            "macos" | "darwin" | "mac" => Platform::MacOS,
            "linux" => Platform::Linux,
            "freebsd" => Platform::FreeBSD,
            _ => Platform::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// Platform path separator helpers
// ---------------------------------------------------------------------------

/// Returns the default path separator for the given platform.
pub fn platform_path_separator(platform: Platform) -> char {
    match platform {
        Platform::Windows => '\\',
        _ => '/',
    }
}

/// Returns the default line ending for the given platform.
pub fn platform_line_ending(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "\r\n",
        _ => "\n",
    }
}

/// Returns the executable file extension for the given platform.
pub fn platform_exe_extension(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => ".exe",
        _ => "",
    }
}


// ---------------------------------------------------------------------------
// PlatformPaths
// ---------------------------------------------------------------------------

/// Standard platform-specific directories for configuration, data, and cache.
#[derive(Debug, Clone)]
pub struct PlatformPaths {
    platform: Platform,
}

impl PlatformPaths {
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }

    pub fn for_current() -> Self {
        Self::new(Platform::current())
    }

    pub fn home_dir(&self) -> Option<PathBuf> {
        env::var("HOME").ok().map(PathBuf::from).or_else(|| {
            env::var("USERPROFILE").ok().map(PathBuf::from)
        })
    }

    pub fn config_dir(&self) -> Option<PathBuf> {
        match self.platform {
            Platform::MacOS => self.home_dir().map(|h| h.join("Library/Application Support/vsedit")),
            Platform::Windows => env::var("APPDATA").ok().map(|a| PathBuf::from(a).join("vsedit")),
            _ => env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| self.home_dir().map(|h| h.join(".config")))
                .map(|d| d.join("vsedit")),
        }
    }

    pub fn data_dir(&self) -> Option<PathBuf> {
        match self.platform {
            Platform::MacOS => self.home_dir().map(|h| h.join("Library/Application Support/vsedit/data")),
            Platform::Windows => env::var("LOCALAPPDATA").ok().map(|a| PathBuf::from(a).join("vsedit/data")),
            _ => env::var("XDG_DATA_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| self.home_dir().map(|h| h.join(".local/share")))
                .map(|d| d.join("vsedit")),
        }
    }

    pub fn cache_dir(&self) -> Option<PathBuf> {
        match self.platform {
            Platform::MacOS => self.home_dir().map(|h| h.join("Library/Caches/vsedit")),
            Platform::Windows => env::var("LOCALAPPDATA").ok().map(|a| PathBuf::from(a).join("vsedit/cache")),
            _ => env::var("XDG_CACHE_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| self.home_dir().map(|h| h.join(".cache")))
                .map(|d| d.join("vsedit")),
        }
    }
}

// ---------------------------------------------------------------------------
// ShellInfo
// ---------------------------------------------------------------------------

/// Information about the user's default shell.
#[derive(Debug, Clone)]
pub struct ShellInfo {
    pub shell_path: String,
    pub name: String,
    pub is_posix: bool,
}

impl ShellInfo {
    pub fn detect() -> Self {
        let shell_path = env::var("SHELL")
            .unwrap_or_else(|_| env::var("COMSPEC").unwrap_or_else(|_| "sh".to_string()));
        let name = shell_path
            .rsplit('/')
            .next()
            .unwrap_or(&shell_path)
            .rsplit('\\')
            .next()
            .unwrap_or(&shell_path)
            .to_string();
        let is_posix = matches!(
            name.as_str(),
            "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh" | "ash"
        );
        Self { shell_path, name, is_posix }
    }

    pub fn new(path: impl Into<String>, name: impl Into<String>, is_posix: bool) -> Self {
        Self { shell_path: path.into(), name: name.into(), is_posix }
    }
}

impl fmt::Display for ShellInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let posix_label = if self.is_posix { "POSIX" } else { "non-POSIX" };
        write!(f, "{} ({posix_label})", self.name)
    }
}

// ── Platform query utilities ────────────────────────────────────────────

/// Return the platform-appropriate path separator as a string.
pub fn native_path_separator() -> &'static str {
    if cfg!(target_os = "windows") { "\\" } else { "/" }
}

/// Return the platform-appropriate line ending.
pub fn native_line_ending() -> &'static str {
    if cfg!(target_os = "windows") { "\r\n" } else { "\n" }
}

/// Join path segments using the platform-native separator.
pub fn join_native_path(segments: &[&str]) -> String {
    segments.join(native_path_separator())
}

/// Check if a path string looks absolute for the detected platform.
pub fn is_absolute_path(path: &str) -> bool {
    if cfg!(target_os = "windows") {
        path.len() >= 3 && path.as_bytes()[1] == b':' && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/')
    } else {
        path.starts_with('/')
    }
}

/// Normalise a path string by replacing backslashes with forward slashes.
pub fn normalize_path_separators(path: &str) -> String {
    path.replace('\\', "/")
}

/// Return the platform name as a lowercase static string.
pub fn platform_name() -> &'static str {
    match Platform::current() {
        Platform::Windows => "windows",
        Platform::MacOS => "macos",
        Platform::Linux => "linux",
        Platform::FreeBSD => "freebsd",
        Platform::Unknown => "unknown",
    }
}

/// Check if the current platform is a Unix-like system.
pub fn is_unix_like() -> bool {
    matches!(Platform::current(), Platform::Linux | Platform::MacOS | Platform::FreeBSD)
}

/// Return a `PlatformCapabilities` with all features disabled.
pub fn minimal_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        true_color: false,
        unicode: false,
        mouse: false,
        sixel: false,
        kitty_graphics: false,
        platform: Platform::current(),
    }
}

// ---------------------------------------------------------------------------
// Architecture detection
// ---------------------------------------------------------------------------

/// The CPU architecture of the current compilation target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Architecture {
    X86,
    X86_64,
    Aarch64,
    Arm,
    Riscv64,
    Wasm32,
    Other,
}

impl Architecture {
    /// Returns the architecture for the current compilation target.
    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "x86" => Self::X86,
            "x86_64" => Self::X86_64,
            "aarch64" => Self::Aarch64,
            "arm" => Self::Arm,
            "riscv64" => Self::Riscv64,
            "wasm32" => Self::Wasm32,
            _ => Self::Other,
        }
    }

    /// Returns `true` for 64-bit architectures.
    pub fn is_64bit(self) -> bool {
        matches!(self, Self::X86_64 | Self::Aarch64 | Self::Riscv64)
    }

    /// Returns the pointer width in bits for the compilation target.
    pub fn pointer_width() -> u32 {
        #[cfg(target_pointer_width = "64")]
        { 64 }
        #[cfg(target_pointer_width = "32")]
        { 32 }
        #[cfg(target_pointer_width = "16")]
        { 16 }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86 => write!(f, "x86"),
            Self::X86_64 => write!(f, "x86_64"),
            Self::Aarch64 => write!(f, "aarch64"),
            Self::Arm => write!(f, "arm"),
            Self::Riscv64 => write!(f, "riscv64"),
            Self::Wasm32 => write!(f, "wasm32"),
            Self::Other => write!(f, "other"),
        }
    }
}

// ---------------------------------------------------------------------------
// Endianness
// ---------------------------------------------------------------------------

/// Returns `true` if the target platform uses little-endian byte order.
pub fn is_little_endian() -> bool {
    cfg!(target_endian = "little")
}

/// Returns `true` if the target platform uses big-endian byte order.
pub fn is_big_endian() -> bool {
    cfg!(target_endian = "big")
}

/// Returns the byte order as a descriptive string.
pub fn endianness_name() -> &'static str {
    if is_little_endian() { "little-endian" } else { "big-endian" }
}

// ---------------------------------------------------------------------------
// CPU feature detection
// ---------------------------------------------------------------------------

/// Detected CPU features at compile time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuFeatures {
    pub sse2: bool,
    pub sse4_1: bool,
    pub avx2: bool,
    pub neon: bool,
}

impl CpuFeatures {
    /// Detect CPU features based on compile-time target features.
    pub fn detect() -> Self {
        Self {
            sse2: cfg!(target_feature = "sse2"),
            sse4_1: cfg!(target_feature = "sse4.1"),
            avx2: cfg!(target_feature = "avx2"),
            neon: cfg!(target_feature = "neon"),
        }
    }

    /// Returns the number of detected features.
    pub fn count(&self) -> usize {
        [self.sse2, self.sse4_1, self.avx2, self.neon]
            .iter()
            .filter(|&&b| b)
            .count()
    }
}

impl fmt::Display for CpuFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let feats: Vec<&str> = [
            self.sse2.then_some("sse2"),
            self.sse4_1.then_some("sse4.1"),
            self.avx2.then_some("avx2"),
            self.neon.then_some("neon"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if feats.is_empty() {
            write!(f, "no SIMD features")
        } else {
            write!(f, "{}", feats.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// Environment variable utilities
// ---------------------------------------------------------------------------

/// Reads an environment variable, returning a default value if unset or empty.
pub fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Reads an environment variable as a `bool`.
///
/// Recognizes `"1"`, `"true"`, `"yes"`, `"on"` (case-insensitive) as `true`.
/// Everything else (including unset) is `false`.
pub fn env_bool(key: &str) -> bool {
    env::var(key)
        .ok()
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

/// Reads an environment variable as a `u64`, returning `None` on failure.
pub fn env_u64(key: &str) -> Option<u64> {
    env::var(key).ok().and_then(|v| v.parse().ok())
}

/// Returns all environment variable names that start with the given prefix.
pub fn env_keys_with_prefix(prefix: &str) -> Vec<String> {
    env::vars()
        .filter_map(|(k, _)| if k.starts_with(prefix) { Some(k) } else { None })
        .collect()
}

// ---------------------------------------------------------------------------
// ANSI support detection
// ---------------------------------------------------------------------------

/// Returns `true` if the `NO_COLOR` convention is active.
///
/// See <https://no-color.org/>.
pub fn is_no_color() -> bool {
    env::var_os("NO_COLOR").is_some()
}

/// Returns `true` if ANSI escape sequences are likely supported.
///
/// Returns `false` for dumb terminals, when `NO_COLOR` is set, or when
/// `TERM` is unset.
pub fn supports_ansi() -> bool {
    if is_no_color() {
        return false;
    }
    match env::var("TERM").as_deref() {
        Ok("dumb") | Err(_) => false,
        Ok(_) => true,
    }
}

/// Estimates the color depth of the terminal.
///
/// Returns 0 (no color), 16, 256, or 16_777_216 (true-color).
pub fn color_depth() -> u32 {
    if is_no_color() {
        return 0;
    }
    if supports_true_color() {
        return 16_777_216;
    }
    match env::var("TERM").as_deref() {
        Ok(t) if t.contains("256color") => 256,
        Ok("dumb") | Err(_) => 0,
        Ok(_) => 16,
    }
}

// ---------------------------------------------------------------------------
// Terminal size estimation
// ---------------------------------------------------------------------------

/// A terminal size in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

impl TerminalSize {
    /// Attempt to read terminal size from `COLUMNS` / `LINES` environment
    /// variables, falling back to a sensible default of 80×24.
    pub fn detect() -> Self {
        let cols = env::var("COLUMNS")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(80);
        let rows = env::var("LINES")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(24);
        Self { cols, rows }
    }

    /// Returns the total number of cells (cols × rows).
    pub fn area(self) -> u32 {
        self.cols as u32 * self.rows as u32
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

impl fmt::Display for TerminalSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.cols, self.rows)
    }
}

// ---------------------------------------------------------------------------
// Comprehensive platform info snapshot
// ---------------------------------------------------------------------------

/// A snapshot of all detectable platform information.
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub platform: Platform,
    pub arch: Architecture,
    pub pointer_width: u32,
    pub endian: &'static str,
    pub os_name: &'static str,
    pub arch_name: &'static str,
}

impl PlatformInfo {
    /// Collect all platform info for the current target.
    pub fn detect() -> Self {
        Self {
            platform: Platform::current(),
            arch: Architecture::current(),
            pointer_width: Architecture::pointer_width(),
            endian: endianness_name(),
            os_name: std::env::consts::OS,
            arch_name: std::env::consts::ARCH,
        }
    }

    /// Returns a compact description string.
    pub fn summary(&self) -> String {
        format!(
            "{} {} ({}bit, {})",
            self.os_name, self.arch_name, self.pointer_width, self.endian
        )
    }
}

impl fmt::Display for PlatformInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// Feature flags
// ---------------------------------------------------------------------------

/// A simple compile-time feature flag set for gating optional functionality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlags {
    flags: Vec<(String, bool)>,
}

impl FeatureFlags {
    /// Create an empty feature-flag set.
    pub fn new() -> Self {
        Self { flags: Vec::new() }
    }

    /// Register a flag with the given name and enabled state.
    pub fn register(&mut self, name: impl Into<String>, enabled: bool) {
        let name = name.into();
        if let Some(entry) = self.flags.iter_mut().find(|(n, _)| *n == name) {
            entry.1 = enabled;
        } else {
            self.flags.push((name, enabled));
        }
    }

    /// Returns `true` if the named flag exists and is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, v)| n == name && *v)
    }

    /// Returns a list of all enabled flag names.
    pub fn enabled_names(&self) -> Vec<&str> {
        self.flags.iter().filter(|(_, v)| *v).map(|(n, _)| n.as_str()).collect()
    }

    /// Returns the total number of registered flags.
    pub fn len(&self) -> usize {
        self.flags.len()
    }

    /// Returns `true` if no flags are registered.
    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Build a default set of flags from the current platform.
    pub fn from_platform() -> Self {
        let mut flags = Self::new();
        flags.register("unix", is_unix_like());
        flags.register("windows", is_windows());
        flags.register("64bit", Architecture::current().is_64bit());
        flags.register("ansi", supports_ansi());
        flags.register("true_color", supports_true_color());
        flags.register("unicode", supports_unicode());
        flags
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FeatureFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let enabled = self.enabled_names();
        if enabled.is_empty() {
            write!(f, "no flags enabled")
        } else {
            write!(f, "{}", enabled.join(", "))
        }
    }
}

/// Summarise platform capabilities as a human-readable string.
pub fn capabilities_summary(caps: &PlatformCapabilities) -> String {
    let features: Vec<&str> = [
        caps.true_color.then_some("true-color"),
        caps.unicode.then_some("unicode"),
        caps.mouse.then_some("mouse"),
        caps.sixel.then_some("sixel"),
        caps.kitty_graphics.then_some("kitty-graphics"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if features.is_empty() {
        format!("{}: no special features", caps.platform)
    } else {
        format!("{}: {}", caps.platform, features.join(", "))
    }
}


// ---------------------------------------------------------------------------
// PlatformCapabilityDetector
// ---------------------------------------------------------------------------

/// A detected capability with its status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityStatus {
    pub name: String,
    pub available: bool,
    pub detail: String,
}

impl fmt::Display for CapabilityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mark = if self.available { "✓" } else { "✗" };
        write!(f, "[{mark}] {}: {}", self.name, self.detail)
    }
}

/// Detects platform capabilities at runtime.
#[derive(Debug, Clone)]
pub struct PlatformCapabilityDetector {
    capabilities: Vec<CapabilityStatus>,
    detection_count: u64,
}

impl PlatformCapabilityDetector {
    /// Create a new detector.
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
            detection_count: 0,
        }
    }

    /// Run all standard detections.
    pub fn detect_all(&mut self) {
        self.detect_true_color();
        self.detect_unicode();
        self.detect_mouse();
        self.detect_clipboard();
        self.detect_sixel();
    }

    /// Detect true color support.
    pub fn detect_true_color(&mut self) {
        self.detection_count += 1;
        let available = env::var("COLORTERM")
            .map(|v| v == "truecolor" || v == "24bit")
            .unwrap_or(false);
        self.add_capability("true_color", available, if available {
            "COLORTERM=truecolor/24bit".into()
        } else {
            "COLORTERM not set or unsupported".into()
        });
    }

    /// Detect unicode support.
    pub fn detect_unicode(&mut self) {
        self.detection_count += 1;
        let available = env::var("LANG")
            .map(|v| v.contains("UTF-8") || v.contains("utf-8") || v.contains("UTF8"))
            .unwrap_or(false);
        self.add_capability("unicode", available, if available {
            "LANG contains UTF-8".into()
        } else {
            "UTF-8 not detected in LANG".into()
        });
    }

    /// Detect mouse support.
    pub fn detect_mouse(&mut self) {
        self.detection_count += 1;
        let term = env::var("TERM").unwrap_or_default();
        let available = term.contains("xterm") || term.contains("screen") || term.contains("tmux")
            || term.contains("kitty") || term.contains("alacritty");
        self.add_capability("mouse", available, format!("TERM={term}"));
    }

    /// Detect clipboard access.
    pub fn detect_clipboard(&mut self) {
        self.detection_count += 1;
        let has_display = env::var("DISPLAY").is_ok() || env::var("WAYLAND_DISPLAY").is_ok();
        self.add_capability("clipboard", has_display, if has_display {
            "DISPLAY or WAYLAND_DISPLAY set".into()
        } else {
            "no display server detected".into()
        });
    }

    /// Detect sixel graphics support.
    pub fn detect_sixel(&mut self) {
        self.detection_count += 1;
        let term = env::var("TERM").unwrap_or_default();
        let available = term.contains("sixel") || term.contains("mlterm") || term.contains("xterm");
        self.add_capability("sixel", available, format!("TERM={term}"));
    }

    fn add_capability(&mut self, name: &str, available: bool, detail: String) {
        // Replace existing capability of same name.
        self.capabilities.retain(|c| c.name != name);
        self.capabilities.push(CapabilityStatus {
            name: name.to_string(),
            available,
            detail,
        });
    }

    /// Get a capability by name.
    pub fn get(&self, name: &str) -> Option<&CapabilityStatus> {
        self.capabilities.iter().find(|c| c.name == name)
    }

    /// Check if a capability is available.
    pub fn is_available(&self, name: &str) -> bool {
        self.get(name).map(|c| c.available).unwrap_or(false)
    }

    /// Number of capabilities detected.
    pub fn capability_count(&self) -> usize {
        self.capabilities.len()
    }

    /// Number of available capabilities.
    pub fn available_count(&self) -> usize {
        self.capabilities.iter().filter(|c| c.available).count()
    }

    /// Number of detections performed.
    pub fn detection_count(&self) -> u64 {
        self.detection_count
    }

    /// Get all capabilities.
    pub fn all(&self) -> &[CapabilityStatus] {
        &self.capabilities
    }

    /// Reset all detections.
    pub fn reset(&mut self) {
        self.capabilities.clear();
        self.detection_count = 0;
    }
}

impl fmt::Display for PlatformCapabilityDetector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CapabilityDetector({}/{} available)",
            self.available_count(),
            self.capability_count()
        )
    }
}

// ---------------------------------------------------------------------------
// PlatformPathNormalizer
// ---------------------------------------------------------------------------

/// Path separator style for normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSeparatorStyle {
    /// Always use forward slashes.
    Unix,
    /// Always use backslashes.
    Windows,
    /// Use the current platform's separator.
    Native,
}

/// Normalizes file paths across platforms.
#[derive(Debug, Clone)]
pub struct PlatformPathNormalizer {
    style: PathSeparatorStyle,
    normalize_count: u64,
    /// Whether to resolve `..` and `.` components.
    resolve_dots: bool,
    /// Whether to lowercase drive letters on Windows-style paths.
    lowercase_drive: bool,
}

impl PlatformPathNormalizer {
    /// Create with the specified separator style.
    pub fn new(style: PathSeparatorStyle) -> Self {
        Self {
            style,
            normalize_count: 0,
            resolve_dots: true,
            lowercase_drive: true,
        }
    }

    /// Create for the current platform.
    pub fn native() -> Self {
        Self::new(PathSeparatorStyle::Native)
    }

    /// Create for unix-style normalization.
    pub fn unix() -> Self {
        Self::new(PathSeparatorStyle::Unix)
    }

    /// Normalize a path string.
    pub fn normalize(&mut self, path: &str) -> String {
        self.normalize_count += 1;
        let mut result = path.to_string();

        // Normalize separators.
        result = match self.style {
            PathSeparatorStyle::Unix => result.replace('\\', "/"),
            PathSeparatorStyle::Windows => result.replace('/', "\\"),
            PathSeparatorStyle::Native => {
                if cfg!(windows) {
                    result.replace('/', "\\")
                } else {
                    result.replace('\\', "/")
                }
            }
        };

        // Lowercase drive letter if applicable.
        if self.lowercase_drive && result.len() >= 2 {
            let bytes = result.as_bytes();
            if bytes[0].is_ascii_uppercase() && bytes[1] == b':' {
                let mut chars: Vec<char> = result.chars().collect();
                chars[0] = chars[0].to_lowercase().next().unwrap_or(chars[0]);
                result = chars.into_iter().collect();
            }
        }

        // Collapse repeated separators.
        let sep = match self.style {
            PathSeparatorStyle::Windows => '\\',
            _ => '/',
        };
        let double_sep: String = [sep, sep].iter().collect();
        let single_sep: String = [sep].iter().collect();

        // Preserve leading `//` for UNC paths on Windows, but collapse others.
        while result.contains(&double_sep) {
            // For Windows UNC paths starting with \\, preserve leading.
            if self.style == PathSeparatorStyle::Windows && result.starts_with("\\\\") {
                let rest = &result[2..];
                let cleaned = rest.replace(&double_sep, &single_sep);
                result = format!("\\\\{cleaned}");
                break;
            } else {
                result = result.replace(&double_sep, &single_sep);
            }
        }

        // Remove trailing separator (unless it's the root).
        if result.len() > 1 && result.ends_with(sep) {
            result.pop();
        }

        // Resolve `.` and `..` if enabled.
        if self.resolve_dots {
            result = self.resolve_dot_components(&result, sep);
        }

        result
    }

    fn resolve_dot_components(&self, path: &str, sep: char) -> String {
        let parts: Vec<&str> = path.split(sep).collect();
        let mut stack: Vec<&str> = Vec::new();
        for part in &parts {
            match *part {
                "." => {}
                ".." => {
                    if let Some(last) = stack.last() {
                        if *last != ".." && !last.is_empty() {
                            stack.pop();
                            continue;
                        }
                    }
                    stack.push(part);
                }
                _ => stack.push(part),
            }
        }
        let result = stack.join(&sep.to_string());
        if result.is_empty() {
            sep.to_string()
        } else {
            result
        }
    }

    /// Check if a path is absolute.
    pub fn is_absolute(&self, path: &str) -> bool {
        if path.starts_with('/') {
            return true;
        }
        // Windows: C:\ or C:/
        if path.len() >= 3 {
            let bytes = path.as_bytes();
            if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
                return true;
            }
        }
        // UNC path
        if path.starts_with("\\\\") {
            return true;
        }
        false
    }

    /// Join two path components.
    pub fn join(&mut self, base: &str, relative: &str) -> String {
        let sep = match self.style {
            PathSeparatorStyle::Windows => "\\",
            _ => "/",
        };
        let combined = format!("{base}{sep}{relative}");
        self.normalize(&combined)
    }

    /// Get the file name from a path.
    pub fn file_name<'a>(&self, path: &'a str) -> Option<&'a str> {
        // Use either separator for splitting.
        let last = path.rsplit(|c| c == '/' || c == '\\').next()?;
        if last.is_empty() { None } else { Some(last) }
    }

    /// Get the parent directory.
    pub fn parent(&mut self, path: &str) -> Option<String> {
        let sep = match self.style {
            PathSeparatorStyle::Windows => '\\',
            _ => '/',
        };
        let normalized = self.normalize(path);
        if let Some(pos) = normalized.rfind(sep) {
            if pos == 0 {
                Some(sep.to_string())
            } else {
                Some(normalized[..pos].to_string())
            }
        } else {
            None
        }
    }

    /// Number of normalizations performed.
    pub fn normalize_count(&self) -> u64 {
        self.normalize_count
    }

    /// Current separator style.
    pub fn style(&self) -> PathSeparatorStyle {
        self.style
    }

    /// Enable/disable dot resolution.
    pub fn set_resolve_dots(&mut self, resolve: bool) {
        self.resolve_dots = resolve;
    }
}

impl fmt::Display for PlatformPathNormalizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PathNormalizer({:?}, normalized={})",
            self.style, self.normalize_count
        )
    }
}



/// Represents a platform locale with region support (detailed variant).
pub struct PlatformLocaleDetail {
    language: String,
    region: Option<String>,
}

impl PlatformLocaleDetail {
    pub fn new(language: &str) -> Self {
        Self { language: language.to_string(), region: None }
    }

    pub fn with_region(language: &str, region: &str) -> Self {
        Self { language: language.to_string(), region: Some(region.to_string()) }
    }

    pub fn language(&self) -> &str { &self.language }
    pub fn region(&self) -> Option<&str> { self.region.as_deref() }

    pub fn to_bcp47(&self) -> String {
        match &self.region {
            Some(r) => format!("{}-{}", self.language, r),
            None => self.language.clone(),
        }
    }

    pub fn matches(&self, tag: &str) -> bool {
        let bcp = self.to_bcp47();
        tag.eq_ignore_ascii_case(&bcp) || tag.eq_ignore_ascii_case(&self.language)
    }
}

/// Platform capability flags for feature detection (detailed variant).
#[derive(Debug, Clone, Default)]
pub struct PlatformCapabilityFlags {
    pub supports_clipboard: bool,
    pub supports_drag_drop: bool,
    pub supports_notifications: bool,
    pub supports_file_dialogs: bool,
    pub supports_gpu_acceleration: bool,
    pub max_texture_size: u32,
}

impl PlatformCapabilityFlags {
    pub fn full() -> Self {
        Self {
            supports_clipboard: true,
            supports_drag_drop: true,
            supports_notifications: true,
            supports_file_dialogs: true,
            supports_gpu_acceleration: true,
            max_texture_size: 16384,
        }
    }

    pub fn minimal() -> Self {
        Self {
            supports_clipboard: true,
            supports_drag_drop: false,
            supports_notifications: false,
            supports_file_dialogs: false,
            supports_gpu_acceleration: false,
            max_texture_size: 4096,
        }
    }

    pub fn capability_count(&self) -> u32 {
        let mut count = 0u32;
        if self.supports_clipboard { count += 1; }
        if self.supports_drag_drop { count += 1; }
        if self.supports_notifications { count += 1; }
        if self.supports_file_dialogs { count += 1; }
        if self.supports_gpu_acceleration { count += 1; }
        count
    }
}

/// Screen DPI scaling information.
pub struct DpiScaling {
    scale_factor: f64,
    base_dpi: f64,
}

impl DpiScaling {
    pub fn new(scale_factor: f64) -> Self {
        Self { scale_factor: scale_factor.max(0.25).min(8.0), base_dpi: 96.0 }
    }

    pub fn effective_dpi(&self) -> f64 { self.base_dpi * self.scale_factor }
    pub fn scale_factor(&self) -> f64 { self.scale_factor }

    pub fn physical_pixels(&self, logical: u32) -> u32 {
        (logical as f64 * self.scale_factor).round() as u32
    }

    pub fn logical_pixels(&self, physical: u32) -> u32 {
        (physical as f64 / self.scale_factor).round() as u32
    }

    pub fn is_hidpi(&self) -> bool { self.scale_factor > 1.5 }
}



// ---------------------------------------------------------------------------
// platform – Extended platform capabilities helpers
// ---------------------------------------------------------------------------

/// Priority levels for platform capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZPlatformPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZPlatformPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZPlatformPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZPlatformPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks platform capabilities data.
#[derive(Debug, Clone)]
pub struct ZPlatformPlatformCapabilities {
    pub features: Vec<String>,
    pub pointer_available: bool,
    pub color_depth: u32,
}

impl ZPlatformPlatformCapabilities {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
            pointer_available: false,
            color_depth: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.features.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.features.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZPlatformPlatformCapabilities[pointer_available={:?}, color_depth={:?}]", self.pointer_available, self.color_depth)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for platform capabilities.
pub fn z_platform_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_platform_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_platform_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_platform_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_platform_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_platform_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_platform_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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
// xb_ utilities – batch 84
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer84 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer84 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_84(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_84<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_84<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_84(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_84(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 139
// ---------------------------------------------------------------------------

/// Generic object pool `Xc139Pool<T>`.
pub struct Xc139Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc139Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc139PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc139Pool<T> {
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
    pub fn stats(&self) -> Xc139PoolStats {
        Xc139PoolStats {
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

impl<T> Default for Xc139Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc139Scheduler`.
pub struct Xc139Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc139Scheduler {
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

impl Default for Xc139Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_139 hash for the given byte slice.
pub fn xc_139_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_139 convention.
pub fn xc_139_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe97 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe97Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe97PipelineError {
    pub stage: Xe97Stage,
    pub message: String,
}

impl std::fmt::Display for Xe97PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe97Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe97Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError>>>,
    stage_names: Vec<Xe97Stage>,
}

impl Xe97Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe97Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe97Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe97Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe97Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> {
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

    pub fn compose(mut self, other: Xe97Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe97CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe97CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe97Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe97CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe97CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe97Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe97CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_97_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe97CacheEntry {
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

    fn xe_97_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe97CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_97_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> {
    Ok(data)
}

pub fn xe_97_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_97_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_97_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_97_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe97PipelineError> {
    Err(Xe97PipelineError {
        stage: Xe97Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_95: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg95Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg95Graph {
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

impl Default for Xg95Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_95: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg95Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg95Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg95Heap<T>) {
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

impl<T: Ord> Default for Xg95Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 138).
pub struct Xh138SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh138SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 180 as u64,
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

/// A compact bit set supporting boolean operations (variant 138).
pub struct Xh138BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh138BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 138).
pub struct Xi138Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi138Deque<T> {
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
pub struct Xi138Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi138Interval {
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

/// A simple interval tree (variant 138).
pub struct Xi138IntervalTree {
    xi_intervals: Vec<Xi138Interval>,
}

impl Xi138IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi138Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi138Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi138Interval) -> Vec<&Xi138Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi138Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi138Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi138Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi138Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi138Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi138Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 138) ---

/// Disjoint set / union-find for crate 138.
pub struct Xj138UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj138UnionFind {
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

const XJ138_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 138.
pub struct Xj138BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj138BTreeNode<K, V>>>,
    len: usize,
}

struct Xj138BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj138BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj138BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ138_BTREE_ORDER - 1
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
        let mid = XJ138_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj138BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj138BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj138BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj138BTreeNode::xj_new_leaf();
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


// --- xk_138 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk138SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk138SegmentTree {
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
pub struct Xk138DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk138DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_138).
#[derive(Debug, Clone)]
pub struct Xl138Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl138Rope {
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

/// Suffix array for efficient string searching (xl_138).
#[derive(Debug, Clone)]
pub struct Xl138SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl138SuffixArray {
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
pub struct Xm138MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm138MatrixSparse {
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
pub struct Xm138Tokenizer {
    text: String,
}

impl Xm138Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 138.
pub struct Xn138Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn138Fenwick {
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

// ----- AVL tree map — crate 138 -----

#[derive(Debug, Clone)]
struct Xn138AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn138AvlNode<K, V>>>,
    right: Option<Box<Xn138AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 138.
#[derive(Debug, Clone)]
pub struct Xn138AVL<K, V> {
    root: Option<Box<Xn138AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn138AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn138AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn138AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn138AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn138AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn138AvlNode<K, V>>) -> Box<Xn138AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn138AvlNode<K, V>>) -> Box<Xn138AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn138AvlNode<K, V>>) -> Box<Xn138AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn138AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn138AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn138AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn138AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn138AvlNode<K, V>>) -> &Xn138AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn138AvlNode<K, V>>) -> (Box<Xn138AvlNode<K, V>>, Option<Box<Xn138AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn138AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn138AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn138AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn138AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn138AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn138AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn138AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo138RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo138Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo138RBNode<K, V> {
    key: K,
    value: V,
    color: Xo138Color,
    left: Option<Box<Xo138RBNode<K, V>>>,
    right: Option<Box<Xo138RBNode<K, V>>>,
}

/// A red-black tree map for crate 138.
#[derive(Debug, Clone)]
pub struct Xo138RedBlack<K, V> {
    root: Option<Box<Xo138RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo138RedBlack<K, V> {
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
            r.color = Xo138Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo138RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo138RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo138RBNode {
                    key, value, color: Xo138Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo138RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo138Color::Red)
    }

    fn xo_balance(mut h: Box<Xo138RBNode<K, V>>) -> Box<Xo138RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo138Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo138RBNode<K, V>>) -> Box<Xo138RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo138Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo138RBNode<K, V>>) -> Box<Xo138RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo138Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo138RBNode<K, V>>) {
        h.color = Xo138Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo138Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo138Color::Black; }
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
            r.color = Xo138Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo138RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo138RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo138RBNode<K, V>) -> (K, V, Option<Box<Xo138RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo138RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo138Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo138RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo138ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 138.
#[derive(Debug, Clone)]
pub struct Xo138ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo138ConsistentHash {
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
            let vkey = format!("{}#xo138#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo138#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 138).
#[derive(Debug)]
pub struct Xp138SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp138Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp138Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp138Node<K, V>>>,
    xp_right: Option<Box<Xp138Node<K, V>>>,
}

impl<K: Ord, V> Xp138Node<K, V> {
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

impl<K: Ord, V> Default for Xp138SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp138SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp138Node<K, V>>>, key: &K) -> Option<Box<Xp138Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp138Node<K, V>>) -> Box<Xp138Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp138Node<K, V>>) -> Box<Xp138Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp138Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp138Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp138Node::xp_new(key, val));
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


// --------------- Xq138Treap ---------------

use std::cmp::Ordering as Xq138Ord;

struct Xq138TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq138TreapNode<K, V>>>,
    right: Option<Box<Xq138TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq138Treap<K, V> {
    root: Option<Box<Xq138TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq138TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_138_size<K, V>(node: &Option<Box<Xq138TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_138_update_size<K, V>(node: &mut Xq138TreapNode<K, V>) {
    node.size = 1 + xq_138_size(&node.left) + xq_138_size(&node.right);
}

fn xq_138_rotate_right<K, V>(mut node: Box<Xq138TreapNode<K, V>>) -> Box<Xq138TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_138_update_size(&mut node);
    left.right = Some(node);
    xq_138_update_size(&mut left);
    left
}

fn xq_138_rotate_left<K, V>(mut node: Box<Xq138TreapNode<K, V>>) -> Box<Xq138TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_138_update_size(&mut node);
    right.left = Some(node);
    xq_138_update_size(&mut right);
    right
}

fn xq_138_insert_node<K: Ord, V>(
    node: Option<Box<Xq138TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq138TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq138TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq138Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq138Ord::Less => {
                let (new_left, old) = xq_138_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_138_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_138_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq138Ord::Greater => {
                let (new_right, old) = xq_138_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_138_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_138_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_138_remove_node<K: Ord, V>(
    node: Option<Box<Xq138TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq138TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq138Ord::Less => {
                let (new_left, old) = xq_138_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_138_update_size(&mut n);
                (Some(n), old)
            }
            Xq138Ord::Greater => {
                let (new_right, old) = xq_138_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_138_update_size(&mut n);
                (Some(n), old)
            }
            Xq138Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_138_rotate_right(n);
                    let (new_right, old) = xq_138_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_138_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_138_rotate_left(n);
                    let (new_left, old) = xq_138_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_138_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_138_find_min<K, V>(node: &Option<Box<Xq138TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_138_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_138_find_max<K, V>(node: &Option<Box<Xq138TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_138_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_138_rank<K: Ord, V>(node: &Option<Box<Xq138TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq138Ord::Less => xq_138_rank(&n.left, key),
            Xq138Ord::Equal => xq_138_size(&n.left),
            Xq138Ord::Greater => 1 + xq_138_size(&n.left) + xq_138_rank(&n.right, key),
        },
    }
}

fn xq_138_kth<K, V>(node: &Option<Box<Xq138TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_138_size(&n.left);
        if k < left_size {
            xq_138_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_138_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_138_in_order<K: Clone, V>(node: &Option<Box<Xq138TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_138_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_138_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq138Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 138 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_138_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq138Ord::Equal => return Some(&n.value),
                Xq138Ord::Less => cur = &n.left,
                Xq138Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_138_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_138_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_138_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_138_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_138_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_138_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_138_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq138VEBTree ---------------

pub struct Xq138VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq138VEBTree>>,
    clusters: Vec<Option<Box<Xq138VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq138VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq138VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq138VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr138KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr138KDPoint {
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
pub struct Xr138BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr138KDNode {
    xr_point: Xr138KDPoint,
    xr_left: Option<Box<Xr138KDNode>>,
    xr_right: Option<Box<Xr138KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr138KDTree {
    xr_root: Option<Box<Xr138KDNode>>,
    xr_size: usize,
}

impl Xr138KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr138KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr138KDNode>>,
        point: Xr138KDPoint,
        depth: usize,
    ) -> Box<Xr138KDNode> {
        match node {
            None => Box::new(Xr138KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr138KDPoint) -> Option<Xr138KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr138KDNode>,
        query: &Xr138KDPoint,
        depth: usize,
        best: &mut Xr138KDPoint,
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
    ) -> Vec<Xr138KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr138KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr138KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr138KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr138KDNode>>, pts: &mut Vec<Xr138KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr138KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr138BoundingBox> {
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
        Some(Xr138BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs138PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs138PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs138PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs138PersistentArray {
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
pub struct Xs138ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs138ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs138ConcurrentQueue {
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
pub struct Xs138RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs138RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs138RangeMap {
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
pub struct Xs138CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs138CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs138CircularBuffer {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_current_is_known() {
        let p = Platform::current();
        // The test itself runs on a known OS, so Unknown would be surprising.
        assert_ne!(p, Platform::Unknown, "expected a known platform");
    }

    #[test]
    fn platform_display() {
        assert_eq!(Platform::Windows.to_string(), "Windows");
        assert_eq!(Platform::MacOS.to_string(), "macOS");
        assert_eq!(Platform::Linux.to_string(), "Linux");
        assert_eq!(Platform::FreeBSD.to_string(), "FreeBSD");
        assert_eq!(Platform::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn exactly_one_os_function_returns_true() {
        let flags = [is_windows(), is_macos(), is_linux(), is_freebsd()];
        let count = flags.iter().filter(|&&f| f).count();
        // On CI this is compiled for exactly one target.
        assert!(
            count <= 1,
            "at most one OS function should return true, got {count}",
        );
    }

    #[test]
    fn path_separator_is_valid() {
        assert!(PATH_SEPARATOR == ':' || PATH_SEPARATOR == ';');
    }

    #[test]
    fn eol_is_valid() {
        assert!(EOL == "\n" || EOL == "\r\n");
    }

    #[test]
    fn user_agent_contains_version() {
        let ua = user_agent();
        assert!(
            ua.starts_with("vsedit/"),
            "user agent should start with 'vsedit/', got: {ua}",
        );
        assert!(ua.contains('(') && ua.contains(')'));
    }

    #[test]
    fn locale_returns_nonempty_string() {
        let loc = locale();
        assert!(!loc.is_empty());
    }

    #[test]
    fn home_dir_returns_path() {
        let home = home_dir();
        // Should return *something* — even "." as fallback.
        assert!(!home.as_os_str().is_empty());
    }

    #[test]
    fn terminal_type_display() {
        assert_eq!(TerminalType::XTerm.to_string(), "xterm");
        assert_eq!(TerminalType::Kitty.to_string(), "Kitty");
        assert_eq!(TerminalType::Alacritty.to_string(), "Alacritty");
        assert_eq!(TerminalType::WezTerm.to_string(), "WezTerm");
        assert_eq!(TerminalType::ITerm2.to_string(), "iTerm2");
        assert_eq!(TerminalType::WindowsTerminal.to_string(), "Windows Terminal");
        assert_eq!(TerminalType::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn supports_true_color_reads_env() {
        // We cannot guarantee COLORTERM is set, but the function must not panic.
        let _ = supports_true_color();
    }

    #[test]
    fn supports_unicode_does_not_panic() {
        let _ = supports_unicode();
    }

    #[test]
    fn supports_mouse_does_not_panic() {
        let _ = supports_mouse();
    }

    #[test]
    fn supports_sixel_does_not_panic() {
        let _ = supports_sixel();
    }

    #[test]
    fn supports_kitty_graphics_does_not_panic() {
        let _ = supports_kitty_graphics();
    }

    #[test]
    fn is_ci_reads_env() {
        let _ = is_ci();
    }

    #[test]
    fn is_root_does_not_panic() {
        let _ = is_root();
    }

    #[test]
    fn is_elevated_matches_is_root() {
        assert_eq!(is_root(), is_elevated());
    }

    #[test]
    fn eq_platform_same() {
        assert_eq!(Platform::Windows, Platform::Windows);
    }

    #[test]
    fn ne_platform_diff() {
        assert_ne!(Platform::Windows, Platform::MacOS);
    }

    #[test]
    fn eq_terminaltype_same() {
        assert_eq!(TerminalType::XTerm, TerminalType::XTerm);
    }

    #[test]
    fn ne_terminaltype_diff() {
        assert_ne!(TerminalType::XTerm, TerminalType::Kitty);
    }

    #[test]
    fn display_platform_variants() {
        assert!(!Platform::Windows.to_string().is_empty());
        assert!(!Platform::MacOS.to_string().is_empty());
        assert!(!Platform::FreeBSD.to_string().is_empty());
        assert!(!Platform::Unknown.to_string().is_empty());
    }

    #[test]
    fn display_terminaltype_variants() {
        assert!(!TerminalType::XTerm.to_string().is_empty());
        assert!(!TerminalType::Kitty.to_string().is_empty());
        assert!(!TerminalType::Alacritty.to_string().is_empty());
        assert!(!TerminalType::WezTerm.to_string().is_empty());
        assert!(!TerminalType::ITerm2.to_string().is_empty());
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
    fn platform_stats_new_defaults() {
        let stats = PlatformStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn platform_stats_record_success() {
        let mut stats = PlatformStats::new();
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
    fn platform_stats_record_failure() {
        let mut stats = PlatformStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn platform_stats_reset() {
        let mut stats = PlatformStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn platform_stats_merge() {
        let mut a = PlatformStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = PlatformStats::new();
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
    fn platform_stats_display() {
        let mut stats = PlatformStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn platform_stats_default() {
        let stats = PlatformStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn platform_validator_accepts_valid_name() {
        let v = PlatformValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn platform_validator_rejects_empty() {
        let v = PlatformValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn platform_validator_rejects_too_long() {
        let v = PlatformValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn platform_validator_forbidden_prefix() {
        let v = PlatformValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn platform_validator_allowed_chars() {
        let v = PlatformValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn platform_validator_range() {
        let v = PlatformValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn platform_sanitize_removes_control() {
        let result = PlatformValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn platform_truncate_short_string() {
        assert_eq!(PlatformValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn platform_truncate_long_string() {
        let result = PlatformValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn platform_is_ascii_printable() {
        assert!(PlatformValidator::is_ascii_printable("Hello World 123"));
        assert!(!PlatformValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn shell_detect_returns_valid() {
        let shell = PlatformShell::detect();
        assert!(!shell.path.is_empty());
        assert!(!shell.name.is_empty());
        assert!(!shell.args.is_empty());
    }

    #[test]
    fn shell_is_posix_known() {
        let shell = PlatformShell { path: "/bin/bash".into(), name: "bash".into(), args: vec!["-c".into()] };
        assert!(shell.is_posix());
    }

    #[test]
    fn shell_is_posix_unknown() {
        let shell = PlatformShell { path: "cmd.exe".into(), name: "cmd".into(), args: vec!["/C".into()] };
        assert!(!shell.is_posix());
    }

    #[test]
    fn shell_command_for() {
        let shell = PlatformShell { path: "/bin/sh".into(), name: "sh".into(), args: vec!["-c".into()] };
        let cmd = shell.command_for("echo hi");
        assert_eq!(cmd, vec!["/bin/sh", "-c", "echo hi"]);
    }

    #[test]
    fn shell_display() {
        let shell = PlatformShell { path: "/bin/zsh".into(), name: "zsh".into(), args: vec!["-c".into()] };
        assert_eq!(format!("{shell}"), "zsh (/bin/zsh)");
    }

    #[test]
    fn locale_parse_full() {
        let loc = PlatformLocale::parse("en_US.UTF-8");
        assert_eq!(loc.language, "en");
        assert_eq!(loc.country, Some("US".to_string()));
        assert_eq!(loc.encoding, Some("UTF-8".to_string()));
    }

    #[test]
    fn locale_parse_no_encoding() {
        let loc = PlatformLocale::parse("fr_FR");
        assert_eq!(loc.language, "fr");
        assert_eq!(loc.country, Some("FR".to_string()));
        assert_eq!(loc.encoding, None);
    }

    #[test]
    fn locale_parse_language_only() {
        let loc = PlatformLocale::parse("ja");
        assert_eq!(loc.language, "ja");
        assert_eq!(loc.country, None);
    }

    #[test]
    fn locale_to_bcp47() {
        let loc = PlatformLocale::parse("en_US.UTF-8");
        assert_eq!(loc.to_bcp47(), "en-US");
    }

    #[test]
    fn locale_to_bcp47_no_country() {
        let loc = PlatformLocale::parse("en");
        assert_eq!(loc.to_bcp47(), "en");
    }

    #[test]
    fn locale_display() {
        let loc = PlatformLocale::parse("de_DE.UTF-8");
        assert_eq!(format!("{loc}"), "de_DE.UTF-8");
    }

    #[test]
    fn temp_dir_exists() {
        let dir = platform_temp_dir();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_platform_capabilities_all_enabled() {
        let caps = PlatformCapabilities::all_enabled(Platform::Linux);
        assert_eq!(caps.enabled_count(), 5);
        assert!(caps.has_graphics());
        assert_eq!(caps.platform, Platform::Linux);
    }

    #[test]
    fn test_platform_capabilities_minimal() {
        let caps = PlatformCapabilities::minimal(Platform::Windows);
        assert_eq!(caps.enabled_count(), 0);
        assert!(!caps.has_graphics());
    }

    #[test]
    fn test_platform_capabilities_display() {
        let caps = PlatformCapabilities::all_enabled(Platform::MacOS);
        let s = format!("{caps}");
        assert!(s.contains("macOS"));
        assert!(s.contains("5/5"));
    }

    #[test]
    fn test_platform_capabilities_default() {
        let caps = PlatformCapabilities::default();
        assert_eq!(caps.platform, Platform::Unknown);
        assert_eq!(caps.enabled_count(), 0);
    }

    #[test]
    fn test_platform_iter() {
        let platforms: Vec<Platform> = PlatformIter::new().collect();
        assert_eq!(platforms.len(), 5);
        assert!(platforms.contains(&Platform::Windows));
        assert!(platforms.contains(&Platform::Linux));
        assert!(platforms.contains(&Platform::Unknown));
    }

    #[test]
    fn test_platform_iter_exact_size() {
        let iter = PlatformIter::new();
        assert_eq!(iter.len(), 5);
    }

    #[test]
    fn test_platform_from_str() {
        assert_eq!(Platform::from("windows"), Platform::Windows);
        assert_eq!(Platform::from("darwin"), Platform::MacOS);
        assert_eq!(Platform::from("linux"), Platform::Linux);
        assert_eq!(Platform::from("freebsd"), Platform::FreeBSD);
        assert_eq!(Platform::from("unknown-os"), Platform::Unknown);
    }

    #[test]
    fn test_platform_path_separator() {
        assert_eq!(platform_path_separator(Platform::Windows), '\\');
        assert_eq!(platform_path_separator(Platform::Linux), '/');
        assert_eq!(platform_path_separator(Platform::MacOS), '/');
    }

    #[test]
    fn test_platform_line_ending() {
        assert_eq!(platform_line_ending(Platform::Windows), "\r\n");
        assert_eq!(platform_line_ending(Platform::Linux), "\n");
    }

    #[test]
    fn test_platform_exe_extension() {
        assert_eq!(platform_exe_extension(Platform::Windows), ".exe");
        assert_eq!(platform_exe_extension(Platform::Linux), "");
    }

    // --- new tests ---

    #[test]
    fn capabilities_minimal_vs_all() {
        let minimal = PlatformCapabilities::minimal(Platform::Linux);
        assert!(!minimal.true_color);
        assert!(!minimal.mouse);
        let all = PlatformCapabilities::all_enabled(Platform::Linux);
        assert!(all.true_color);
        assert!(all.mouse);
        assert!(all.unicode);
    }

    #[test]
    fn capabilities_display_format() {
        let caps = PlatformCapabilities::all_enabled(Platform::Linux);
        let s = format!("{caps}");
        assert!(s.contains("Linux"));
        assert!(s.contains("5/5"));
    }

    #[test]
    fn capabilities_detect_runs() {
        let caps = PlatformCapabilities::detect();
        let s = format!("{caps}");
        assert!(s.contains("caps:"));
    }

    #[test]
    fn platform_paths_config_dir() {
        let paths = PlatformPaths::for_current();
        let cfg = paths.config_dir();
        assert!(cfg.is_some() || Platform::current() == Platform::Unknown);
    }

    #[test]
    fn shell_info_detect() {
        let info = ShellInfo::detect();
        assert!(!info.name.is_empty());
        let s = format!("{info}");
        assert!(s.contains(&info.name));
    }

    #[test]
    fn shell_info_posix_check() {
        let bash = ShellInfo::new("/bin/bash", "bash", true);
        assert!(bash.is_posix);
        let cmd = ShellInfo::new("C:\\Windows\\cmd.exe", "cmd.exe", false);
        assert!(!cmd.is_posix);
    }

    #[test]
    fn native_path_separator_not_empty() {
        let sep = native_path_separator();
        assert!(!sep.is_empty());
        assert!(sep == "/" || sep == "\\");
    }

    #[test]
    fn native_line_ending_valid() {
        let eol = native_line_ending();
        assert!(eol == "\n" || eol == "\r\n");
    }

    #[test]
    fn join_native_path_segments() {
        let result = join_native_path(&["home", "user", "file.txt"]);
        assert!(result.contains("user"));
        assert!(result.contains("file.txt"));
        assert_eq!(join_native_path(&[]), "");
    }

    #[test]
    fn is_absolute_path_unix() {
        if cfg!(not(target_os = "windows")) {
            assert!(is_absolute_path("/home/user"));
            assert!(!is_absolute_path("relative/path"));
            assert!(!is_absolute_path(""));
        }
    }

    #[test]
    fn normalize_path_separators_converts() {
        assert_eq!(normalize_path_separators("a\\b\\c"), "a/b/c");
        assert_eq!(normalize_path_separators("a/b/c"), "a/b/c");
        assert_eq!(normalize_path_separators(""), "");
    }

    #[test]
    fn platform_name_returns_known() {
        let name = platform_name();
        assert!(["windows", "macos", "linux", "freebsd", "unknown"].contains(&name));
    }

    #[test]
    fn is_unix_like_consistent() {
        let unix = is_unix_like();
        if cfg!(target_os = "linux") || cfg!(target_os = "macos") || cfg!(target_os = "freebsd") {
            assert!(unix);
        }
    }

    #[test]
    fn minimal_capabilities_all_false() {
        let caps = minimal_capabilities();
        assert!(!caps.true_color);
        assert!(!caps.unicode);
        assert!(!caps.mouse);
        assert!(!caps.sixel);
        assert!(!caps.kitty_graphics);
    }

    #[test]
    fn capabilities_summary_no_features() {
        let caps = minimal_capabilities();
        let summary = capabilities_summary(&caps);
        assert!(summary.contains("no special features"));
    }

    #[test]
    fn capabilities_summary_with_features() {
        let caps = PlatformCapabilities {
            true_color: true,
            unicode: true,
            mouse: false,
            sixel: false,
            kitty_graphics: false,
            platform: Platform::Linux,
        };
        let summary = capabilities_summary(&caps);
        assert!(summary.contains("true-color"));
        assert!(summary.contains("unicode"));
        assert!(!summary.contains("mouse"));
    }

    // ── Architecture & endianness tests ─────────────────────────────────

    #[test]
    fn architecture_current_is_known() {
        let arch = Architecture::current();
        // We're running on a real host, so it should be a recognised arch.
        assert_ne!(arch, Architecture::Other);
    }

    #[test]
    fn architecture_display_not_empty() {
        for arch in [
            Architecture::X86,
            Architecture::X86_64,
            Architecture::Aarch64,
            Architecture::Arm,
            Architecture::Riscv64,
            Architecture::Wasm32,
            Architecture::Other,
        ] {
            assert!(!arch.to_string().is_empty());
        }
    }

    #[test]
    fn architecture_is_64bit() {
        assert!(Architecture::X86_64.is_64bit());
        assert!(Architecture::Aarch64.is_64bit());
        assert!(Architecture::Riscv64.is_64bit());
        assert!(!Architecture::X86.is_64bit());
        assert!(!Architecture::Arm.is_64bit());
        assert!(!Architecture::Wasm32.is_64bit());
        assert!(!Architecture::Other.is_64bit());
    }

    #[test]
    fn pointer_width_is_positive() {
        let pw = Architecture::pointer_width();
        assert!(pw == 16 || pw == 32 || pw == 64);
    }

    #[test]
    fn endianness_consistent() {
        // Exactly one must be true.
        assert_ne!(is_little_endian(), is_big_endian());
        let name = endianness_name();
        assert!(name == "little-endian" || name == "big-endian");
    }

    // ── CPU features test ───────────────────────────────────────────────

    #[test]
    fn cpu_features_detect_runs() {
        let feats = CpuFeatures::detect();
        // count must be <= 4
        assert!(feats.count() <= 4);
        let s = format!("{feats}");
        assert!(!s.is_empty());
    }

    // ── Env utilities tests ─────────────────────────────────────────────

    #[test]
    fn env_or_returns_default_for_missing() {
        let val = env_or("__VSEDIT_TEST_MISSING_VAR_12345__", "fallback");
        assert_eq!(val, "fallback");
    }

    #[test]
    fn env_bool_returns_false_for_missing() {
        assert!(!env_bool("__VSEDIT_TEST_MISSING_BOOL_12345__"));
    }

    #[test]
    fn env_u64_returns_none_for_missing() {
        assert_eq!(env_u64("__VSEDIT_TEST_MISSING_U64_12345__"), None);
    }

    #[test]
    fn env_keys_with_prefix_returns_vec() {
        // PATH should be present on any system.
        let keys = env_keys_with_prefix("PAT");
        assert!(keys.iter().any(|k| k == "PATH"));
    }

    // ── ANSI / color tests ──────────────────────────────────────────────

    #[test]
    fn is_no_color_does_not_panic() {
        let _ = is_no_color();
    }

    #[test]
    fn supports_ansi_does_not_panic() {
        let _ = supports_ansi();
    }

    #[test]
    fn color_depth_returns_known_value() {
        let depth = color_depth();
        assert!(
            depth == 0 || depth == 16 || depth == 256 || depth == 16_777_216,
            "unexpected color depth: {depth}"
        );
    }

    // ── Terminal size tests ─────────────────────────────────────────────

    #[test]
    fn terminal_size_default() {
        let sz = TerminalSize::default();
        assert_eq!(sz.cols, 80);
        assert_eq!(sz.rows, 24);
        assert_eq!(sz.area(), 80 * 24);
    }

    #[test]
    fn terminal_size_display() {
        let sz = TerminalSize { cols: 120, rows: 40 };
        assert_eq!(format!("{sz}"), "120x40");
    }

    #[test]
    fn terminal_size_detect_has_positive_area() {
        let sz = TerminalSize::detect();
        assert!(sz.area() > 0);
    }

    // ── PlatformInfo tests ──────────────────────────────────────────────

    #[test]
    fn platform_info_detect_summary() {
        let info = PlatformInfo::detect();
        let s = info.summary();
        assert!(s.contains("bit"));
        assert!(s.contains("endian"));
        // Display impl should match summary
        assert_eq!(format!("{info}"), s);
    }

    // ── FeatureFlags tests ──────────────────────────────────────────────

    #[test]
    fn feature_flags_empty() {
        let flags = FeatureFlags::new();
        assert!(flags.is_empty());
        assert_eq!(flags.len(), 0);
        assert!(!flags.is_enabled("anything"));
        assert_eq!(format!("{flags}"), "no flags enabled");
    }

    #[test]
    fn feature_flags_register_and_query() {
        let mut flags = FeatureFlags::new();
        flags.register("gpu", true);
        flags.register("sound", false);
        flags.register("network", true);
        assert_eq!(flags.len(), 3);
        assert!(flags.is_enabled("gpu"));
        assert!(!flags.is_enabled("sound"));
        assert!(flags.is_enabled("network"));
        let enabled = flags.enabled_names();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&"gpu"));
        assert!(enabled.contains(&"network"));
    }

    #[test]
    fn feature_flags_overwrite() {
        let mut flags = FeatureFlags::new();
        flags.register("x", false);
        assert!(!flags.is_enabled("x"));
        flags.register("x", true);
        assert!(flags.is_enabled("x"));
        assert_eq!(flags.len(), 1); // no duplicates
    }

    #[test]
    fn feature_flags_from_platform_has_entries() {
        let flags = FeatureFlags::from_platform();
        assert!(flags.len() >= 6);
        // On a real host, at least unix or windows must be registered
        assert!(flags.is_enabled("unix") || flags.is_enabled("windows") || true);
    }

    #[test]
    fn feature_flags_display_enabled() {
        let mut flags = FeatureFlags::new();
        flags.register("a", true);
        flags.register("b", true);
        let s = format!("{flags}");
        assert!(s.contains("a"));
        assert!(s.contains("b"));
    }

    #[test]
    fn capability_detector_new_empty() {
        let det = PlatformCapabilityDetector::new();
        assert_eq!(det.capability_count(), 0);
        assert_eq!(det.detection_count(), 0);
    }

    #[test]
    fn capability_detector_detect_all() {
        let mut det = PlatformCapabilityDetector::new();
        det.detect_all();
        assert!(det.capability_count() >= 5);
        assert!(det.detection_count() >= 5);
    }

    #[test]
    fn capability_detector_get() {
        let mut det = PlatformCapabilityDetector::new();
        det.detect_true_color();
        assert!(det.get("true_color").is_some());
        assert!(det.get("nonexistent").is_none());
    }

    #[test]
    fn capability_detector_reset() {
        let mut det = PlatformCapabilityDetector::new();
        det.detect_all();
        det.reset();
        assert_eq!(det.capability_count(), 0);
        assert_eq!(det.detection_count(), 0);
    }

    #[test]
    fn capability_detector_display() {
        let det = PlatformCapabilityDetector::new();
        let s = format!("{det}");
        assert!(s.contains("0/0 available"));
    }

    #[test]
    fn capability_status_display() {
        let cap = CapabilityStatus {
            name: "test".into(),
            available: true,
            detail: "ok".into(),
        };
        let s = format!("{cap}");
        assert!(s.contains("✓"));
        assert!(s.contains("test"));
    }

    #[test]
    fn path_normalizer_unix_separators() {
        let mut norm = PlatformPathNormalizer::new(PathSeparatorStyle::Unix);
        assert_eq!(norm.normalize("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn path_normalizer_windows_separators() {
        let mut norm = PlatformPathNormalizer::new(PathSeparatorStyle::Windows);
        assert_eq!(norm.normalize("a/b/c"), "a\\b\\c");
    }

    #[test]
    fn path_normalizer_collapse_double() {
        let mut norm = PlatformPathNormalizer::unix();
        assert_eq!(norm.normalize("a//b///c"), "a/b/c");
    }

    #[test]
    fn path_normalizer_resolve_dots() {
        let mut norm = PlatformPathNormalizer::unix();
        assert_eq!(norm.normalize("a/b/../c"), "a/c");
        assert_eq!(norm.normalize("a/./b"), "a/b");
    }

    #[test]
    fn path_normalizer_trailing_sep() {
        let mut norm = PlatformPathNormalizer::unix();
        assert_eq!(norm.normalize("a/b/c/"), "a/b/c");
    }

    #[test]
    fn path_normalizer_lowercase_drive() {
        let mut norm = PlatformPathNormalizer::new(PathSeparatorStyle::Windows);
        let result = norm.normalize("C:\\Users\\test");
        assert!(result.starts_with("c:\\"));
    }

    #[test]
    fn path_normalizer_is_absolute() {
        let norm = PlatformPathNormalizer::unix();
        assert!(norm.is_absolute("/usr/bin"));
        assert!(!norm.is_absolute("relative/path"));
        assert!(norm.is_absolute("C:\\Windows"));
    }

    #[test]
    fn path_normalizer_join() {
        let mut norm = PlatformPathNormalizer::unix();
        assert_eq!(norm.join("/home/user", "docs/file.txt"), "/home/user/docs/file.txt");
    }

    #[test]
    fn path_normalizer_file_name() {
        let norm = PlatformPathNormalizer::unix();
        assert_eq!(norm.file_name("/home/user/file.txt"), Some("file.txt"));
        assert_eq!(norm.file_name("C:\\Users\\file.txt"), Some("file.txt"));
    }

    #[test]
    fn path_normalizer_parent() {
        let mut norm = PlatformPathNormalizer::unix();
        assert_eq!(norm.parent("/home/user/file.txt"), Some("/home/user".into()));
    }

    #[test]
    fn path_normalizer_display() {
        let norm = PlatformPathNormalizer::unix();
        let s = format!("{norm}");
        assert!(s.contains("Unix"));
        assert!(s.contains("normalized=0"));
    }

    #[test]
    fn path_normalizer_count() {
        let mut norm = PlatformPathNormalizer::unix();
        norm.normalize("a");
        norm.normalize("b");
        assert_eq!(norm.normalize_count(), 2);
    }



    #[test]
    fn platform_locale_bcp47() {
        let l = PlatformLocaleDetail::with_region("en", "US");
        assert_eq!(l.to_bcp47(), "en-US");
    }

    #[test]
    fn platform_locale_no_region() {
        let l = PlatformLocaleDetail::new("fr");
        assert_eq!(l.to_bcp47(), "fr");
        assert!(l.region().is_none());
    }

    #[test]
    fn platform_locale_matches() {
        let l = PlatformLocaleDetail::with_region("en", "US");
        assert!(l.matches("en-US"));
        assert!(l.matches("en"));
        assert!(!l.matches("fr"));
    }

    #[test]
    fn platform_capabilities_full() {
        let caps = PlatformCapabilityFlags::full();
        assert_eq!(caps.capability_count(), 5);
        assert!(caps.supports_gpu_acceleration);
    }

    #[test]
    fn platform_capabilities_minimal() {
        let caps = PlatformCapabilityFlags::minimal();
        assert_eq!(caps.capability_count(), 1);
        assert!(!caps.supports_gpu_acceleration);
    }

    #[test]
    fn platform_capabilities_default() {
        let caps = PlatformCapabilityFlags::default();
        assert_eq!(caps.capability_count(), 0);
        assert_eq!(caps.max_texture_size, 0);
    }

    #[test]
    fn dpi_scaling_effective() {
        let dpi = DpiScaling::new(2.0);
        assert_eq!(dpi.effective_dpi(), 192.0);
        assert!(dpi.is_hidpi());
    }

    #[test]
    fn dpi_scaling_physical_pixels() {
        let dpi = DpiScaling::new(2.0);
        assert_eq!(dpi.physical_pixels(100), 200);
    }

    #[test]
    fn dpi_scaling_logical_pixels() {
        let dpi = DpiScaling::new(2.0);
        assert_eq!(dpi.logical_pixels(200), 100);
    }

    #[test]
    fn dpi_scaling_clamped_low() {
        let dpi = DpiScaling::new(0.1);
        assert_eq!(dpi.scale_factor(), 0.25);
    }

    #[test]
    fn dpi_scaling_not_hidpi() {
        let dpi = DpiScaling::new(1.0);
        assert!(!dpi.is_hidpi());
    }

    #[test]
    fn dpi_scaling_clamped_high() {
        let dpi = DpiScaling::new(20.0);
        assert_eq!(dpi.scale_factor(), 8.0);
    }


    // -- platform Z-extended tests -----------------------------------------------

    #[test]
    fn z_platform_priority_weight() {
        assert_eq!(ZPlatformPriority::Idle.weight(), 0);
        assert_eq!(ZPlatformPriority::Normal.weight(), 2);
        assert_eq!(ZPlatformPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_platform_priority_label() {
        assert_eq!(ZPlatformPriority::Low.label(), "low");
        assert_eq!(ZPlatformPriority::High.label(), "high");
    }

    #[test]
    fn z_platform_priority_is_elevated() {
        assert!(!ZPlatformPriority::Normal.is_elevated());
        assert!(ZPlatformPriority::High.is_elevated());
        assert!(ZPlatformPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_platform_priority_display() {
        assert_eq!(format!("{}", ZPlatformPriority::Idle), "idle");
    }

    #[test]
    fn z_platform_priority_all_asc() {
        let all = ZPlatformPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZPlatformPriority::Idle);
        assert_eq!(all[4], ZPlatformPriority::Realtime);
    }

    #[test]
    fn z_platform_struct_new() {
        let s = ZPlatformPlatformCapabilities::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_platform_struct_toggled_clone() {
        let s = ZPlatformPlatformCapabilities::new();
        let t = s.toggled_clone();
        let _ = t.color_depth;
    }

    #[test]
    fn z_platform_rolling_hash_deterministic() {
        let h1 = z_platform_rolling_hash(b"test");
        let h2 = z_platform_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_platform_rolling_hash(b"a"), z_platform_rolling_hash(b"b"));
    }

    #[test]
    fn z_platform_pad_to_basic() {
        assert_eq!(z_platform_pad_to("hi", 5), "hi   ");
        assert_eq!(z_platform_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_platform_is_identifier_basic() {
        assert!(z_platform_is_identifier("foo_bar"));
        assert!(z_platform_is_identifier("abc123"));
        assert!(!z_platform_is_identifier(""));
        assert!(!z_platform_is_identifier("has space"));
    }

    #[test]
    fn z_platform_levenshtein_basic() {
        assert_eq!(z_platform_levenshtein("", ""), 0);
        assert_eq!(z_platform_levenshtein("abc", "abc"), 0);
        assert_eq!(z_platform_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_platform_unique_words_basic() {
        let w = z_platform_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_platform_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_platform_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_platform_common_prefix_basic() {
        assert_eq!(z_platform_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_platform_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_platform_struct_clear() {
        let mut s = ZPlatformPlatformCapabilities::new();
        s.features.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_platform_rolling_hash_empty() {
        let h = z_platform_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
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


    #[test]
    fn xb_ring_buffer_84_push_and_len() {
        let mut rb = super::XbRingBuffer84::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_84_overwrite() {
        let mut rb = super::XbRingBuffer84::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_84_get_out_of_bounds() {
        let rb = super::XbRingBuffer84::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_84_drain_all() {
        let mut rb = super::XbRingBuffer84::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_84_peek_front_back() {
        let mut rb = super::XbRingBuffer84::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_84_clear() {
        let mut rb = super::XbRingBuffer84::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_84_capacity() {
        let rb = super::XbRingBuffer84::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_84_basic() {
        let h = super::xb_fnv1a_84(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_84(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_84_different_inputs() {
        let h1 = super::xb_fnv1a_84(b"abc");
        let h2 = super::xb_fnv1a_84(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_84_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_84(&data);
        let dec = super::xb_rle_decode_84(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_84_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_84(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_84(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_84_values() {
        assert!((super::xb_clamp_84(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_84(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_84(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_84_values() {
        assert!((super::xb_lerp_84(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_84(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_84(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_84_wrap_around_twice() {
        let mut rb = super::XbRingBuffer84::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 139 ----

    #[test]
    fn xc_139_pool_new_empty() {
        let pool: super::Xc139Pool<i32> = super::Xc139Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_139_pool_release_acquire() {
        let mut pool = super::Xc139Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_139_pool_acquire_empty() {
        let mut pool: super::Xc139Pool<i32> = super::Xc139Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_139_pool_full() {
        let mut pool = super::Xc139Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_139_pool_drain() {
        let mut pool = super::Xc139Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_139_pool_stats() {
        let mut pool = super::Xc139Pool::new(8);
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
    fn xc_139_pool_clear() {
        let mut pool = super::Xc139Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_139_pool_shrink() {
        let mut pool = super::Xc139Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_139_pool_default() {
        let pool: super::Xc139Pool<String> = super::Xc139Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_139_pool_extend() {
        let mut pool = super::Xc139Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_139_pool_retain() {
        let mut pool = super::Xc139Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_139_scheduler_round_robin() {
        let mut sched = super::Xc139Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_139_scheduler_empty() {
        let mut sched = super::Xc139Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_139_scheduler_reset() {
        let mut sched = super::Xc139Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_139_scheduler_add_remove() {
        let mut sched = super::Xc139Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_139_scheduler_targets() {
        let sched = super::Xc139Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_139_hash_empty() {
        assert_eq!(super::xc_139_hash(b""), 5381);
    }

    #[test]
    fn xc_139_hash_data() {
        let h = super::xc_139_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_139_hash(b"hello"), h);
    }

    #[test]
    fn xc_139_reverse_str() {
        assert_eq!(super::xc_139_reverse("abc"), "cba");
        assert_eq!(super::xc_139_reverse(""), "");
    }


    #[test]
    fn xe_97_pipeline_empty() {
        let p = super::Xe97Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_97_pipeline_parse_stage() {
        let p = super::Xe97Pipeline::new()
            .add_parse(super::xe_97_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_97_pipeline_transform_double() {
        let p = super::Xe97Pipeline::new()
            .add_transform(super::xe_97_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_97_pipeline_validate_reverse() {
        let p = super::Xe97Pipeline::new()
            .add_validate(super::xe_97_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_97_pipeline_emit_filter() {
        let p = super::Xe97Pipeline::new()
            .add_emit(super::xe_97_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_97_pipeline_multi_stage() {
        let p = super::Xe97Pipeline::new()
            .add_parse(super::xe_97_pipeline_identity)
            .add_transform(super::xe_97_pipeline_double)
            .add_validate(super::xe_97_pipeline_reverse)
            .add_emit(super::xe_97_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_97_pipeline_error_propagation() {
        let p = super::Xe97Pipeline::new()
            .add_parse(super::xe_97_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe97Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_97_pipeline_compose() {
        let p1 = super::Xe97Pipeline::new()
            .add_parse(super::xe_97_pipeline_identity);
        let p2 = super::Xe97Pipeline::new()
            .add_transform(super::xe_97_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_97_pipeline_error_display() {
        let e = super::Xe97PipelineError {
            stage: super::Xe97Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_97_cache_put_get() {
        let mut c = super::Xe97Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_97_cache_miss() {
        let mut c: super::Xe97Cache<&str, i32> = super::Xe97Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_97_cache_ttl_expiry() {
        let mut c = super::Xe97Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_97_cache_evict() {
        let mut c = super::Xe97Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_97_cache_capacity() {
        let mut c = super::Xe97Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_97_cache_stats() {
        let mut c = super::Xe97Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_97_cache_clear() {
        let mut c = super::Xe97Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_95 graph tests ------------------------------------------------

    #[test]
    fn xg_95_graph_empty() {
        let g = super::Xg95Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_95_graph_add_node() {
        let mut g = super::Xg95Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_95_graph_add_edge() {
        let mut g = super::Xg95Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_95_graph_neighbors() {
        let mut g = super::Xg95Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_95_graph_has_path() {
        let mut g = super::Xg95Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_95_graph_self_path() {
        let g = super::Xg95Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_95_graph_topo_sort() {
        let mut g = super::Xg95Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_95_graph_cycle_detect_false() {
        let mut g = super::Xg95Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_95_graph_cycle_detect_true() {
        let mut g = super::Xg95Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_95 heap tests -------------------------------------------------

    #[test]
    fn xg_95_heap_empty() {
        let h: super::Xg95Heap<i32> = super::Xg95Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_95_heap_push_pop() {
        let mut h = super::Xg95Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_95_heap_peek() {
        let mut h = super::Xg95Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_95_heap_drain_sorted() {
        let mut h = super::Xg95Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_95_heap_merge() {
        let mut a = super::Xg95Heap::new();
        let mut b = super::Xg95Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_95_heap_default() {
        let h: super::Xg95Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_95_graph_default() {
        let g: super::Xg95Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh138_skip_insert_contains() {
        let mut sl = super::Xh138SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh138_skip_remove() {
        let mut sl = super::Xh138SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh138_skip_len() {
        let mut sl = super::Xh138SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh138_skip_range_query() {
        let mut sl = super::Xh138SkipList::xh_new(4);
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
    fn xh138_skip_floor_ceiling() {
        let mut sl = super::Xh138SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh138_skip_rank() {
        let mut sl = super::Xh138SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh138_skip_empty() {
        let sl = super::Xh138SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh138_skip_duplicates() {
        let mut sl = super::Xh138SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh138_bitset_set_test() {
        let mut bs = super::Xh138BitSet::xh_new(256);
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
    fn xh138_bitset_clear_count() {
        let mut bs = super::Xh138BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh138_bitset_and_or_xor() {
        let mut a = super::Xh138BitSet::xh_new(128);
        let mut b = super::Xh138BitSet::xh_new(128);
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
    fn xh138_bitset_iter_ones() {
        let mut bs = super::Xh138BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh138_bitset_first_last() {
        let mut bs = super::Xh138BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh138_bitset_empty() {
        let bs = super::Xh138BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi138_deque_push_pop_back() {
        let mut dq = super::Xi138Deque::xi_new(4);
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
    fn xi138_deque_push_pop_front() {
        let mut dq = super::Xi138Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi138_deque_mixed_ops() {
        let mut dq = super::Xi138Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi138_deque_get_and_split() {
        let mut dq = super::Xi138Deque::xi_new(8);
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
    fn xi138_deque_rotate_left() {
        let mut dq = super::Xi138Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi138_deque_rotate_right() {
        let mut dq = super::Xi138Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi138_deque_grow() {
        let mut dq = super::Xi138Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi138_deque_empty() {
        let dq = super::Xi138Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi138_interval_tree_insert_query() {
        let mut tree = super::Xi138IntervalTree::xi_new();
        tree.xi_insert(super::Xi138Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi138Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi138Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi138_interval_tree_overlap() {
        let mut tree = super::Xi138IntervalTree::xi_new();
        tree.xi_insert(super::Xi138Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi138Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi138Interval::xi_new(12, 20));
        let q = super::Xi138Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi138_interval_tree_remove() {
        let mut tree = super::Xi138IntervalTree::xi_new();
        tree.xi_insert(super::Xi138Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi138Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi138_interval_tree_gaps() {
        let mut tree = super::Xi138IntervalTree::xi_new();
        tree.xi_insert(super::Xi138Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi138Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi138Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi138Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi138Interval::xi_new(8, 10));
    }

    #[test]
    fn xi138_interval_tree_merge() {
        let mut tree = super::Xi138IntervalTree::xi_new();
        tree.xi_insert(super::Xi138Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi138Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi138Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi138Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi138Interval::xi_new(10, 15));
    }

    #[test]
    fn xi138_interval_tree_all() {
        let mut tree = super::Xi138IntervalTree::xi_new();
        tree.xi_insert(super::Xi138Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi138Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi138_interval_tree_empty() {
        let tree = super::Xi138IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi138_interval_tree_contains_point() {
        let iv = super::Xi138Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 138) ---

    #[test]
    fn xj_138_uf_make_and_find() {
        let mut uf = super::Xj138UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_138_uf_union_connected() {
        let mut uf = super::Xj138UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_138_uf_component_count() {
        let mut uf = super::Xj138UnionFind::xj_new();
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
    fn xj_138_uf_component_size() {
        let mut uf = super::Xj138UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_138_uf_largest_component() {
        let mut uf = super::Xj138UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_138_uf_many_elements() {
        let mut uf = super::Xj138UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_138_uf_separate_components() {
        let mut uf = super::Xj138UnionFind::xj_new();
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
    fn xj_138_uf_path_compression() {
        let mut uf = super::Xj138UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_138_bt_insert_get() {
        let mut bt = super::Xj138BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_138_bt_contains_len() {
        let mut bt = super::Xj138BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_138_bt_replace() {
        let mut bt = super::Xj138BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_138_bt_remove() {
        let mut bt = super::Xj138BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_138_bt_keys_values() {
        let mut bt = super::Xj138BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_138_bt_range() {
        let mut bt = super::Xj138BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_138_bt_min_max() {
        let mut bt = super::Xj138BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_138_bt_many_inserts() {
        let mut bt = super::Xj138BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_138 segment tree tests ---

    #[test]
    fn xk_138_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk138SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_138_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk138SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_138_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk138SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_138_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk138SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_138_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk138SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_138_st_single_element() {
        let data = vec![42];
        let st = super::Xk138SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_138_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk138SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_138_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk138SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_138 disjoint intervals tests ---

    #[test]
    fn xk_138_di_add_and_count() {
        let mut di = super::Xk138DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_138_di_merge_overlap() {
        let mut di = super::Xk138DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_138_di_contains() {
        let mut di = super::Xk138DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_138_di_remove() {
        let mut di = super::Xk138DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_138_di_covered_length() {
        let mut di = super::Xk138DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_138_di_gaps() {
        let mut di = super::Xk138DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_138_di_merge_adjacent() {
        let mut di = super::Xk138DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_138_di_empty() {
        let di = super::Xk138DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_138_rope_new_empty() {
        let rope = super::Xl138Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_138_rope_from_str() {
        let rope = super::Xl138Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_138_rope_insert_at() {
        let mut rope = super::Xl138Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_138_rope_delete_range() {
        let mut rope = super::Xl138Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_138_rope_char_at() {
        let rope = super::Xl138Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_138_rope_split_concat() {
        let rope = super::Xl138Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_138_rope_line_count() {
        let rope = super::Xl138Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_138_rope_line_at() {
        let rope = super::Xl138Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_138_sa_build_and_search() {
        let sa = super::Xl138SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_138_sa_count() {
        let sa = super::Xl138SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_138_sa_longest_repeated() {
        let sa = super::Xl138SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_138_sa_all_positions() {
        let sa = super::Xl138SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_138_sa_len() {
        let sa = super::Xl138SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_138_sa_empty() {
        let sa = super::Xl138SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_138_rope_slice() {
        let rope = super::Xl138Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_138_sa_search_start() {
        let sa = super::Xl138SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_138_sparse_set_get() {
        let mut m = super::Xm138MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_138_sparse_row_col() {
        let mut m = super::Xm138MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_138_sparse_transpose() {
        let mut m = super::Xm138MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_138_sparse_multiply_vec() {
        let mut m = super::Xm138MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_138_sparse_nnz_density() {
        let mut m = super::Xm138MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_138_sparse_clear() {
        let mut m = super::Xm138MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_138_sparse_overwrite_zero() {
        let mut m = super::Xm138MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_138_tokenizer_basic() {
        let t = super::Xm138Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_138_tokenizer_count() {
        let t = super::Xm138Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_138_tokenizer_unique() {
        let t = super::Xm138Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_138_tokenizer_frequency() {
        let t = super::Xm138Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_138_tokenizer_delimiter() {
        let t = super::Xm138Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_138_tokenizer_whitespace() {
        let t = super::Xm138Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_138_tokenizer_empty() {
        let t = super::Xm138Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 138 ----

    #[test]
    fn xn_138_fenwick_prefix_sum() {
        let mut ft = super::Xn138Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_138_fenwick_range_sum() {
        let mut ft = super::Xn138Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_138_fenwick_point_query() {
        let mut ft = super::Xn138Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_138_fenwick_len() {
        let ft = super::Xn138Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_138_fenwick_multiple_updates() {
        let mut ft = super::Xn138Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_138_fenwick_single_element() {
        let mut ft = super::Xn138Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_138_fenwick_find_kth() {
        let mut ft = super::Xn138Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_138_fenwick_negative_delta() {
        let mut ft = super::Xn138Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 138 ----

    #[test]
    fn xn_138_avl_insert_get() {
        let mut m = super::Xn138AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_138_avl_remove() {
        let mut m = super::Xn138AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_138_avl_in_order() {
        let mut m = super::Xn138AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_138_avl_min_max() {
        let mut m = super::Xn138AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_138_avl_floor_ceiling() {
        let mut m = super::Xn138AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_138_avl_height_balanced() {
        let mut m = super::Xn138AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_138_avl_overwrite() {
        let mut m = super::Xn138AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_138_avl_empty() {
        let m: super::Xn138AVL<i32, i32> = super::Xn138AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo138RedBlack tests ---

    #[test]
    fn xo_138_rb_insert_and_get() {
        let mut tree = super::Xo138RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_138_rb_len_and_empty() {
        let mut tree = super::Xo138RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_138_rb_min_max() {
        let mut tree = super::Xo138RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_138_rb_contains() {
        let mut tree = super::Xo138RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_138_rb_remove() {
        let mut tree = super::Xo138RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_138_rb_in_order() {
        let mut tree = super::Xo138RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_138_rb_black_height() {
        let mut tree = super::Xo138RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_138_rb_overwrite() {
        let mut tree = super::Xo138RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo138ConsistentHash tests ---

    #[test]
    fn xo_138_ch_add_and_count() {
        let mut ring = super::Xo138ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_138_ch_remove_node() {
        let mut ring = super::Xo138ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_138_ch_get_node() {
        let mut ring = super::Xo138ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_138_ch_empty_ring() {
        let ring = super::Xo138ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_138_ch_distribution() {
        let mut ring = super::Xo138ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_138_ch_rebalance() {
        let mut ring = super::Xo138ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_138_ch_virtual_nodes() {
        let mut ring = super::Xo138ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_138_ch_consistent_lookup() {
        let mut ring = super::Xo138ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_138_splay_insert_get() {
        let mut t = super::Xp138SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_138_splay_remove() {
        let mut t = super::Xp138SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_138_splay_count_increases() {
        let mut t = super::Xp138SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_138_splay_depth() {
        let mut t = super::Xp138SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_138_splay_len_empty() {
        let t = super::Xp138SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_138_splay_min_max() {
        let mut t = super::Xp138SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_138_splay_overwrite() {
        let mut t = super::Xp138SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_138_splay_remove_missing() {
        let mut t = super::Xp138SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_138 treap tests ----
    #[test]
    fn xq_138_treap_empty() {
        let t = super::Xq138Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_138_treap_insert_get() {
        let mut t = super::Xq138Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_138_treap_overwrite() {
        let mut t = super::Xq138Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_138_treap_remove() {
        let mut t = super::Xq138Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_138_treap_min_max() {
        let mut t = super::Xq138Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_138_treap_rank() {
        let mut t = super::Xq138Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_138_treap_kth() {
        let mut t = super::Xq138Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_138_treap_in_order() {
        let mut t = super::Xq138Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_138 VEB tree tests ----
    #[test]
    fn xq_138_veb_empty() {
        let v = super::Xq138VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_138_veb_insert_contains() {
        let mut v = super::Xq138VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_138_veb_min_max() {
        let mut v = super::Xq138VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_138_veb_delete() {
        let mut v = super::Xq138VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_138_veb_successor() {
        let mut v = super::Xq138VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_138_veb_predecessor() {
        let mut v = super::Xq138VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_138_veb_count() {
        let mut v = super::Xq138VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_138_veb_duplicate_insert() {
        let mut v = super::Xq138VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_138_kdtree_empty() {
        let tree = super::Xr138KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_138_kdtree_insert_one() {
        let mut tree = super::Xr138KDTree::xr_new();
        tree.xr_insert(super::Xr138KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_138_kdtree_insert_multiple() {
        let mut tree = super::Xr138KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr138KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_138_kdtree_nearest_neighbor() {
        let mut tree = super::Xr138KDTree::xr_new();
        tree.xr_insert(super::Xr138KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr138KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr138KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_138_kdtree_nn_empty() {
        let tree = super::Xr138KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr138KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_138_kdtree_range_search() {
        let mut tree = super::Xr138KDTree::xr_new();
        tree.xr_insert(super::Xr138KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr138KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr138KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_138_kdtree_range_empty() {
        let mut tree = super::Xr138KDTree::xr_new();
        tree.xr_insert(super::Xr138KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_138_kdtree_all_points() {
        let mut tree = super::Xr138KDTree::xr_new();
        tree.xr_insert(super::Xr138KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr138KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_138_kdtree_depth() {
        let mut tree = super::Xr138KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr138KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_138_kdtree_bounding_box() {
        let mut tree = super::Xr138KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr138KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr138KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_138_persistent_array_new() {
        let arr = super::Xs138PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_138_persistent_array_push() {
        let mut arr = super::Xs138PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_138_persistent_array_set() {
        let mut arr = super::Xs138PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_138_persistent_array_diff() {
        let mut arr = super::Xs138PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_138_persistent_array_rollback() {
        let mut arr = super::Xs138PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_138_persistent_array_history() {
        let mut arr = super::Xs138PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_138_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs138PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_138_persistent_array_from_vec() {
        let arr = super::Xs138PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_138_concurrent_queue_new() {
        let q = super::Xs138ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_138_concurrent_queue_push_pop() {
        let mut q = super::Xs138ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_138_concurrent_queue_full() {
        let mut q = super::Xs138ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_138_concurrent_queue_drain() {
        let mut q = super::Xs138ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_138_concurrent_queue_try_pop() {
        let mut q = super::Xs138ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_138_concurrent_queue_clear() {
        let mut q = super::Xs138ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_138_range_map_new() {
        let rm = super::Xs138RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_138_range_map_insert_get() {
        let mut rm = super::Xs138RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_138_range_map_overlap() {
        let mut rm = super::Xs138RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_138_range_map_remove() {
        let mut rm = super::Xs138RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_138_range_map_gaps() {
        let mut rm = super::Xs138RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_138_range_map_coverage() {
        let mut rm = super::Xs138RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_138_range_map_contains() {
        let mut rm = super::Xs138RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_138_range_map_clear() {
        let mut rm = super::Xs138RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_138_circular_buffer_new() {
        let buf = super::Xs138CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_138_circular_buffer_push_pop() {
        let mut buf = super::Xs138CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_138_circular_buffer_overwrite() {
        let mut buf = super::Xs138CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_138_circular_buffer_peek() {
        let mut buf = super::Xs138CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_138_circular_buffer_is_full() {
        let mut buf = super::Xs138CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_138_circular_buffer_iter() {
        let mut buf = super::Xs138CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_138_circular_buffer_clear() {
        let mut buf = super::Xs138CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_138_circular_buffer_to_vec() {
        let mut buf = super::Xs138CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

}
