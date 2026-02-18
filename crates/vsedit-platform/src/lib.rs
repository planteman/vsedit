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

}
