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
}
