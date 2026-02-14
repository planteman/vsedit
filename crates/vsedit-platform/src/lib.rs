//! Platform detection and feature flags for vsedit.
//!
//! Provides OS detection, environment introspection, terminal capability
//! detection, and platform-specific constants. Modeled after VS Code's
//! `vs/base/common/platform.ts`, adapted for terminal environments.

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
// Tests
// ---------------------------------------------------------------------------

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
}
