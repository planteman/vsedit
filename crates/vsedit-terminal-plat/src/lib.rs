//! Terminal PTY abstraction for vsedit.
//!
//! This crate provides the platform-level terminal integration (PTY spawning
//! and management), equivalent to VS Code's integrated terminal backend.
//!
//! # Key types
//!
//! - [`TerminalConfig`] — configuration for spawning a terminal.
//! - [`TerminalInstance`] — a running terminal with PTY I/O.
//! - [`TerminalManager`] — manages multiple terminal instances by ID.
//! - [`TerminalId`] — unique identifier for a terminal.
//! - [`TerminalOutput`] — buffered output read from the PTY.

use std::fmt;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};

// ---------------------------------------------------------------------------
// TerminalId
// ---------------------------------------------------------------------------

/// Unique identifier for a terminal instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(u64);

impl TerminalId {
    fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the raw numeric identifier.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for TerminalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "terminal-{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// TerminalConfig
// ---------------------------------------------------------------------------

/// Configuration for spawning a new terminal.
#[derive(Debug, Clone)]
pub struct TerminalConfig {
    /// Shell command to execute (e.g. `/bin/bash`).
    /// Defaults to `$SHELL` or `/bin/sh`.
    pub shell: String,

    /// Arguments to pass to the shell.
    pub args: Vec<String>,

    /// Working directory for the shell process.
    pub cwd: Option<PathBuf>,

    /// Additional environment variables.
    pub env: HashMap<String, String>,

    /// Initial number of columns.
    pub initial_cols: u16,

    /// Initial number of rows.
    pub initial_rows: u16,

    /// Human-readable title for this terminal.
    pub title: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        Self {
            shell,
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            initial_cols: 80,
            initial_rows: 24,
            title: "Terminal".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// TerminalOutput
// ---------------------------------------------------------------------------

/// Buffered output read from a terminal PTY.
#[derive(Debug, Clone)]
pub struct TerminalOutput {
    /// Raw bytes read from the PTY.
    pub data: Vec<u8>,
}

impl TerminalOutput {
    /// Interpret the output as UTF-8 (lossy).
    pub fn as_str_lossy(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }

    /// Returns `true` if there is no output data.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ---------------------------------------------------------------------------
// TerminalInstance
// ---------------------------------------------------------------------------

/// A running terminal backed by a PTY.
///
/// Provides methods to write input, read output, resize, and check liveness.
pub struct TerminalInstance {
    id: TerminalId,
    title: String,
    config: TerminalConfig,
    pair: Mutex<PtyPair>,
    writer: Mutex<Box<dyn Write + Send>>,
    reader: Mutex<Box<dyn Read + Send>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
}

impl TerminalInstance {
    /// Spawn a new terminal instance from the given configuration.
    pub fn spawn(config: TerminalConfig) -> io::Result<Self> {
        let id = TerminalId::next();

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.initial_rows,
                cols: config.initial_cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut cmd = CommandBuilder::new(&config.shell);
        cmd.args(&config.args);
        if let Some(ref cwd) = config.cwd {
            cmd.cwd(cwd);
        }
        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let title = config.title.clone();

        Ok(Self {
            id,
            title,
            config,
            pair: Mutex::new(pair),
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
            child: Mutex::new(child),
        })
    }

    /// Returns the unique identifier for this terminal.
    pub fn id(&self) -> TerminalId {
        self.id
    }

    /// Returns the terminal title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the terminal title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Returns a reference to the configuration used to create this terminal.
    pub fn config(&self) -> &TerminalConfig {
        &self.config
    }

    /// Write data to the terminal PTY (i.e. send input).
    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()
    }

    /// Try to read available output from the terminal PTY.
    ///
    /// This performs a **non-blocking-style** read: it reads whatever is
    /// currently available in the buffer (up to `buf_size` bytes). If no data
    /// is available yet, the returned `TerminalOutput` will be empty.
    pub fn try_read(&self, buf_size: usize) -> io::Result<TerminalOutput> {
        let mut reader = self.reader.lock().unwrap();
        let mut buf = vec![0u8; buf_size];
        match reader.read(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                Ok(TerminalOutput { data: buf })
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                Ok(TerminalOutput { data: Vec::new() })
            }
            Err(e) => Err(e),
        }
    }

    /// Resize the terminal PTY.
    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        let pair = self.pair.lock().unwrap();
        pair.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// Check whether the child process is still running.
    pub fn is_alive(&self) -> bool {
        let mut child = self.child.lock().unwrap();
        match child.try_wait() {
            Ok(Some(_status)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }

    /// Wait for the child process to exit and return the exit status.
    pub fn wait(&self) -> io::Result<portable_pty::ExitStatus> {
        let mut child = self.child.lock().unwrap();
        child
            .wait()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// Kill the child process.
    pub fn kill(&self) -> io::Result<()> {
        let mut child = self.child.lock().unwrap();
        child
            .kill()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

impl std::fmt::Debug for TerminalInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalInstance")
            .field("id", &self.id)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// TerminalManager
// ---------------------------------------------------------------------------

/// Manages multiple terminal instances, identified by [`TerminalId`].
///
/// Provides creation, destruction, listing, and active-terminal tracking.
#[derive(Debug)]
pub struct TerminalManager {
    terminals: HashMap<TerminalId, TerminalInstance>,
    active: Option<TerminalId>,
}

impl TerminalManager {
    /// Create a new, empty terminal manager.
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            active: None,
        }
    }

    /// Spawn a new terminal from the given configuration and return its ID.
    pub fn create_terminal(&mut self, config: TerminalConfig) -> io::Result<TerminalId> {
        let instance = TerminalInstance::spawn(config)?;
        let id = instance.id();

        // Auto-activate the first terminal.
        if self.terminals.is_empty() {
            self.active = Some(id);
        }

        self.terminals.insert(id, instance);
        Ok(id)
    }

    /// Destroy (kill and remove) a terminal by ID.
    ///
    /// Returns `true` if the terminal existed and was destroyed.
    pub fn destroy_terminal(&mut self, id: TerminalId) -> bool {
        if let Some(instance) = self.terminals.remove(&id) {
            let _ = instance.kill();
            if self.active == Some(id) {
                self.active = self.terminals.keys().next().copied();
            }
            true
        } else {
            false
        }
    }

    /// Get a reference to a terminal instance by ID.
    pub fn get_terminal(&self, id: TerminalId) -> Option<&TerminalInstance> {
        self.terminals.get(&id)
    }

    /// Get a mutable reference to a terminal instance by ID.
    pub fn get_terminal_mut(&mut self, id: TerminalId) -> Option<&mut TerminalInstance> {
        self.terminals.get_mut(&id)
    }

    /// List all terminal IDs.
    pub fn list_terminals(&self) -> Vec<TerminalId> {
        self.terminals.keys().copied().collect()
    }

    /// Set the active terminal.
    ///
    /// Returns `false` if the given ID does not exist.
    pub fn set_active(&mut self, id: TerminalId) -> bool {
        if self.terminals.contains_key(&id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    /// Get the currently active terminal ID.
    pub fn get_active(&self) -> Option<TerminalId> {
        self.active
    }

    /// Get a reference to the currently active terminal instance.
    pub fn get_active_terminal(&self) -> Option<&TerminalInstance> {
        self.active.and_then(|id| self.terminals.get(&id))
    }

    /// Returns the number of managed terminals.
    pub fn len(&self) -> usize {
        self.terminals.len()
    }

    /// Returns `true` if no terminals are managed.
    pub fn is_empty(&self) -> bool {
        self.terminals.is_empty()
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe, reference-counted handle to a [`TerminalManager`].
#[derive(Clone, Debug)]
pub struct SharedTerminalManager {
    inner: Arc<Mutex<TerminalManager>>,
}

impl SharedTerminalManager {
    /// Create a new shared manager.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TerminalManager::new())),
        }
    }

    /// Execute a closure with access to the underlying manager.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut TerminalManager) -> R,
    {
        let mut guard = self.inner.lock().unwrap();
        f(&mut guard)
    }
}

impl Default for SharedTerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Terminal output history and session statistics
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Terminal profiles
// ---------------------------------------------------------------------------

/// A terminal profile representing a configured shell with its arguments and env.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProfile {
    /// Path to the shell executable.
    pub shell_path: String,
    /// Human-readable name for this profile.
    pub name: String,
    /// Arguments to pass to the shell.
    pub args: Vec<String>,
    /// Environment variables specific to this profile.
    pub env: HashMap<String, String>,
    /// Optional icon identifier.
    pub icon: Option<String>,
}

impl TerminalProfile {
    /// Create a new terminal profile.
    pub fn new(shell_path: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            shell_path: shell_path.into(),
            name: name.into(),
            args: Vec::new(),
            env: HashMap::new(),
            icon: None,
        }
    }

    /// Add a shell argument.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the icon identifier.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Convert this profile into a [`TerminalConfig`].
    pub fn to_config(&self) -> TerminalConfig {
        TerminalConfig {
            shell: self.shell_path.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            title: self.name.clone(),
            ..TerminalConfig::default()
        }
    }
}

impl fmt::Display for TerminalProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TerminalProfile('{}', {})", self.name, self.shell_path)
    }
}

/// Detect available shells on the system by checking common paths.
pub fn detect_available_shells() -> Vec<String> {
    let candidates = [
        "/bin/sh",
        "/bin/bash",
        "/bin/zsh",
        "/bin/fish",
        "/usr/bin/fish",
        "/bin/dash",
        "/usr/bin/bash",
        "/usr/bin/zsh",
    ];
    candidates
        .iter()
        .filter(|p| std::path::Path::new(p).exists())
        .map(|p| p.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Terminal environment merging
// ---------------------------------------------------------------------------

/// Merges a shell environment with editor-injected environment variables.
#[derive(Debug, Clone)]
pub struct TerminalEnvironment {
    shell_env: HashMap<String, String>,
    editor_env: HashMap<String, String>,
}

impl TerminalEnvironment {
    /// Create a new terminal environment from shell and editor envs.
    pub fn new(shell_env: HashMap<String, String>, editor_env: HashMap<String, String>) -> Self {
        Self { shell_env, editor_env }
    }

    /// Return the merged environment. Editor env takes precedence over shell env.
    pub fn merged(&self) -> HashMap<String, String> {
        let mut result = self.shell_env.clone();
        for (k, v) in &self.editor_env {
            result.insert(k.clone(), v.clone());
        }
        result
    }

    /// Look up a variable, preferring editor env over shell env.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.editor_env
            .get(key)
            .or_else(|| self.shell_env.get(key))
            .map(|v| v.as_str())
    }

    /// Return all unique keys across both environments.
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.shell_env.keys().cloned().collect();
        for k in self.editor_env.keys() {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        keys
    }
}

// ---------------------------------------------------------------------------
// Terminal output history and session statistics (continued)
// ---------------------------------------------------------------------------

/// Tracks terminal output history and provides search capabilities.
#[derive(Debug, Clone)]
pub struct TerminalOutputHistory {
    lines: Vec<String>,
    max_lines: usize,
}

impl TerminalOutputHistory {
    /// Create a new output history with the given maximum line capacity.
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
        }
    }

    /// Append raw output data, splitting into lines.
    pub fn append(&mut self, data: &str) {
        for line in data.lines() {
            self.lines.push(line.to_string());
            if self.lines.len() > self.max_lines {
                self.lines.remove(0);
            }
        }
    }

    /// Search the history for lines containing the given substring.
    pub fn search(&self, query: &str) -> Vec<(usize, &str)> {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(query))
            .map(|(i, line)| (i, line.as_str()))
            .collect()
    }

    /// Return the total number of lines in the history.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Return all lines in the history.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Clear all stored output.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Return the last N lines.
    pub fn tail(&self, n: usize) -> &[String] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }
}

/// Statistics about a terminal session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSessionStats {
    pub total_bytes_written: u64,
    pub total_bytes_read: u64,
    pub resize_count: u32,
    pub current_cols: u16,
    pub current_rows: u16,
}

impl TerminalSessionStats {
    /// Create stats initialized with the given terminal dimensions.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            total_bytes_written: 0,
            total_bytes_read: 0,
            resize_count: 0,
            current_cols: cols,
            current_rows: rows,
        }
    }

    /// Record bytes written to the terminal.
    pub fn record_write(&mut self, bytes: u64) {
        self.total_bytes_written += bytes;
    }

    /// Record bytes read from the terminal.
    pub fn record_read(&mut self, bytes: u64) {
        self.total_bytes_read += bytes;
    }

    /// Record a resize event with new dimensions.
    pub fn record_resize(&mut self, cols: u16, rows: u16) {
        self.resize_count += 1;
        self.current_cols = cols;
        self.current_rows = rows;
    }

    /// Total bytes transferred (read + written).
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes_written + self.total_bytes_read
    }
}

impl fmt::Display for TerminalSessionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionStats(written={}, read={}, resizes={}, {}x{})",
            self.total_bytes_written,
            self.total_bytes_read,
            self.resize_count,
            self.current_cols,
            self.current_rows,
        )
    }
}

// ---------------------------------------------------------------------------
// TerminalColorScheme
// ---------------------------------------------------------------------------

/// Describes a terminal color scheme (foreground, background, cursor, 16-color
/// ANSI palette).
#[derive(Debug, Clone)]
pub struct TerminalColorScheme {
    pub name: String,
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub palette: Vec<String>,
}

impl TerminalColorScheme {
    pub fn new(name: impl Into<String>) -> Self {
        Self::default_dark_with_name(name)
    }

    pub fn with_foreground(mut self, fg: impl Into<String>) -> Self {
        self.foreground = fg.into();
        self
    }

    pub fn with_background(mut self, bg: impl Into<String>) -> Self {
        self.background = bg.into();
        self
    }

    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = cursor.into();
        self
    }

    /// Heuristic: considers the scheme dark if the background hex color starts
    /// with `#0`, `#1`, `#2`, or `#3`.
    pub fn is_dark(&self) -> bool {
        matches!(
            self.background.get(..2),
            Some("#0") | Some("#1") | Some("#2") | Some("#3")
        )
    }

    pub fn default_dark() -> Self {
        Self::default_dark_with_name("Dark")
    }

    pub fn default_light() -> Self {
        Self {
            name: "Light".to_string(),
            foreground: "#000000".to_string(),
            background: "#ffffff".to_string(),
            cursor: "#000000".to_string(),
            palette: vec![
                "#000000".into(), "#cd3131".into(), "#0dbc79".into(), "#e5e510".into(),
                "#2472c8".into(), "#bc3fbc".into(), "#11a8cd".into(), "#e5e5e5".into(),
                "#666666".into(), "#f14c4c".into(), "#23d18b".into(), "#f5f543".into(),
                "#3b8eea".into(), "#d670d6".into(), "#29b8db".into(), "#ffffff".into(),
            ],
        }
    }

    fn default_dark_with_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            foreground: "#d4d4d4".to_string(),
            background: "#1e1e1e".to_string(),
            cursor: "#aeafad".to_string(),
            palette: vec![
                "#000000".into(), "#cd3131".into(), "#0dbc79".into(), "#e5e510".into(),
                "#2472c8".into(), "#bc3fbc".into(), "#11a8cd".into(), "#e5e5e5".into(),
                "#666666".into(), "#f14c4c".into(), "#23d18b".into(), "#f5f543".into(),
                "#3b8eea".into(), "#d670d6".into(), "#29b8db".into(), "#e5e5e5".into(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// TerminalFontConfig
// ---------------------------------------------------------------------------

/// Configuration for terminal font rendering.
#[derive(Debug, Clone)]
pub struct TerminalFontConfig {
    pub family: String,
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
    pub ligatures: bool,
}

impl TerminalFontConfig {
    pub fn new() -> Self {
        Self {
            family: "monospace".to_string(),
            size: 14.0,
            line_height: 1.2,
            weight: 400,
            ligatures: false,
        }
    }

    pub fn with_family(mut self, f: impl Into<String>) -> Self {
        self.family = f.into();
        self
    }

    pub fn with_size(mut self, s: f32) -> Self {
        self.size = s.clamp(6.0, 72.0);
        self
    }

    pub fn with_line_height(mut self, h: f32) -> Self {
        self.line_height = h.clamp(0.5, 3.0);
        self
    }

    pub fn with_weight(mut self, w: u16) -> Self {
        self.weight = w.clamp(100, 900);
        self
    }

    pub fn with_ligatures(mut self, enabled: bool) -> Self {
        self.ligatures = enabled;
        self
    }

    pub fn cell_height(&self) -> f32 {
        self.size * self.line_height
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- TerminalConfig tests -----------------------------------------------

    #[test]
    fn config_default_has_sensible_values() {
        let cfg = TerminalConfig::default();
        assert!(!cfg.shell.is_empty());
        assert_eq!(cfg.initial_cols, 80);
        assert_eq!(cfg.initial_rows, 24);
        assert_eq!(cfg.title, "Terminal");
        assert!(cfg.args.is_empty());
        assert!(cfg.env.is_empty());
    }

    #[test]
    fn config_custom_values() {
        let cfg = TerminalConfig {
            shell: "/bin/zsh".to_string(),
            args: vec!["-l".to_string()],
            cwd: Some(PathBuf::from("/tmp")),
            env: HashMap::from([("FOO".to_string(), "bar".to_string())]),
            initial_cols: 120,
            initial_rows: 40,
            title: "My Shell".to_string(),
        };
        assert_eq!(cfg.shell, "/bin/zsh");
        assert_eq!(cfg.args, vec!["-l"]);
        assert_eq!(cfg.cwd, Some(PathBuf::from("/tmp")));
        assert_eq!(cfg.env.get("FOO").unwrap(), "bar");
        assert_eq!(cfg.initial_cols, 120);
        assert_eq!(cfg.initial_rows, 40);
        assert_eq!(cfg.title, "My Shell");
    }

    // -- TerminalId tests ---------------------------------------------------

    #[test]
    fn terminal_id_is_unique() {
        let a = TerminalId::next();
        let b = TerminalId::next();
        assert_ne!(a, b);
    }

    #[test]
    fn terminal_id_display() {
        let id = TerminalId(42);
        assert_eq!(format!("{id}"), "terminal-42");
    }

    #[test]
    fn terminal_id_raw() {
        let id = TerminalId(7);
        assert_eq!(id.raw(), 7);
    }

    // -- TerminalOutput tests -----------------------------------------------

    #[test]
    fn output_is_empty() {
        let empty = TerminalOutput { data: Vec::new() };
        assert!(empty.is_empty());

        let non_empty = TerminalOutput {
            data: vec![b'x'],
        };
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn output_as_str_lossy() {
        let output = TerminalOutput {
            data: b"hello world".to_vec(),
        };
        assert_eq!(output.as_str_lossy(), "hello world");
    }

    #[test]
    fn output_as_str_lossy_invalid_utf8() {
        let output = TerminalOutput {
            data: vec![0xff, 0xfe],
        };
        let s = output.as_str_lossy();
        assert!(!s.is_empty());
    }

    // -- TerminalManager (no PTY) tests -------------------------------------

    #[test]
    fn manager_new_is_empty() {
        let mgr = TerminalManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
        assert!(mgr.list_terminals().is_empty());
        assert!(mgr.get_active().is_none());
    }

    #[test]
    fn manager_default_matches_new() {
        let mgr = TerminalManager::default();
        assert!(mgr.is_empty());
        assert!(mgr.get_active().is_none());
    }

    #[test]
    fn manager_set_active_nonexistent() {
        let mut mgr = TerminalManager::new();
        let fake_id = TerminalId(9999);
        assert!(!mgr.set_active(fake_id));
        assert!(mgr.get_active().is_none());
    }

    #[test]
    fn manager_destroy_nonexistent() {
        let mut mgr = TerminalManager::new();
        let fake_id = TerminalId(9999);
        assert!(!mgr.destroy_terminal(fake_id));
    }

    #[test]
    fn manager_get_nonexistent() {
        let mgr = TerminalManager::new();
        let fake_id = TerminalId(9999);
        assert!(mgr.get_terminal(fake_id).is_none());
    }

    // -- Integration tests (spawn real PTY) ---------------------------------

    fn echo_config() -> TerminalConfig {
        TerminalConfig {
            shell: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "echo HELLO_PTY && exit 0".to_string()],
            cwd: Some(PathBuf::from("/tmp")),
            env: HashMap::new(),
            initial_cols: 80,
            initial_rows: 24,
            title: "echo test".to_string(),
        }
    }

    #[test]
    fn spawn_and_read_output() {
        let instance = TerminalInstance::spawn(echo_config()).expect("spawn failed");
        assert_eq!(instance.title(), "echo test");

        // Wait for process to finish.
        let status = instance.wait().expect("wait failed");
        assert!(status.success());

        // Read output; the PTY should contain our echo.
        let output = instance.try_read(4096).expect("read failed");
        let text = output.as_str_lossy();
        assert!(
            text.contains("HELLO_PTY"),
            "expected HELLO_PTY in output, got: {text:?}"
        );
    }

    #[test]
    fn spawn_and_check_alive() {
        let config = TerminalConfig {
            shell: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "sleep 30".to_string()],
            title: "sleep test".to_string(),
            ..TerminalConfig::default()
        };
        let instance = TerminalInstance::spawn(config).expect("spawn failed");
        assert!(instance.is_alive());

        instance.kill().expect("kill failed");
        let _ = instance.wait();
        assert!(!instance.is_alive());
    }

    #[test]
    fn resize_terminal() {
        let config = TerminalConfig {
            shell: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "sleep 30".to_string()],
            title: "resize test".to_string(),
            ..TerminalConfig::default()
        };
        let instance = TerminalInstance::spawn(config).expect("spawn failed");
        instance.resize(120, 40).expect("resize failed");
        instance.resize(40, 10).expect("resize failed again");

        instance.kill().expect("kill failed");
        let _ = instance.wait();
    }

    #[test]
    fn write_to_terminal() {
        let config = TerminalConfig {
            shell: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "cat".to_string()],
            title: "write test".to_string(),
            ..TerminalConfig::default()
        };
        let instance = TerminalInstance::spawn(config).expect("spawn failed");
        instance.write(b"test input\n").expect("write failed");

        instance.kill().expect("kill failed");
        let _ = instance.wait();
    }

    #[test]
    fn manager_create_and_destroy() {
        let mut mgr = TerminalManager::new();
        let id = mgr.create_terminal(echo_config()).expect("create failed");

        assert_eq!(mgr.len(), 1);
        assert!(!mgr.is_empty());
        assert!(mgr.get_terminal(id).is_some());
        assert_eq!(mgr.list_terminals(), vec![id]);
        // First terminal is auto-activated.
        assert_eq!(mgr.get_active(), Some(id));

        assert!(mgr.destroy_terminal(id));
        assert!(mgr.is_empty());
        assert!(mgr.get_terminal(id).is_none());
    }

    #[test]
    fn manager_active_tracking() {
        let mut mgr = TerminalManager::new();
        let id1 = mgr.create_terminal(echo_config()).expect("create 1");
        let id2 = mgr.create_terminal(echo_config()).expect("create 2");

        // First terminal is auto-activated.
        assert_eq!(mgr.get_active(), Some(id1));

        // Switch active.
        assert!(mgr.set_active(id2));
        assert_eq!(mgr.get_active(), Some(id2));

        // Destroy active — should fall back to another.
        mgr.destroy_terminal(id2);
        assert_eq!(mgr.get_active(), Some(id1));

        mgr.destroy_terminal(id1);
        assert!(mgr.get_active().is_none());
    }

    #[test]
    fn shared_manager_basic() {
        let shared = SharedTerminalManager::new();
        let cloned = shared.clone();

        let id = shared.with(|mgr| mgr.create_terminal(echo_config()).expect("create"));

        let found = cloned.with(|mgr| mgr.get_terminal(id).is_some());
        assert!(found);

        shared.with(|mgr| {
            mgr.destroy_terminal(id);
        });

        let empty = cloned.with(|mgr| mgr.is_empty());
        assert!(empty);
    }

    #[test]
    fn terminal_instance_debug() {
        let instance = TerminalInstance::spawn(echo_config()).expect("spawn failed");
        let dbg = format!("{instance:?}");
        assert!(dbg.contains("TerminalInstance"));
        assert!(dbg.contains("echo test"));
        let _ = instance.kill();
        let _ = instance.wait();
    }

    #[test]
    fn config_with_env_vars() {
        let config = TerminalConfig {
            shell: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "echo $MY_VAR && exit 0".to_string()],
            env: HashMap::from([("MY_VAR".to_string(), "CUSTOM_VALUE".to_string())]),
            title: "env test".to_string(),
            ..TerminalConfig::default()
        };
        let instance = TerminalInstance::spawn(config).expect("spawn failed");
        let status = instance.wait().expect("wait failed");
        assert!(status.success());

        let output = instance.try_read(4096).expect("read failed");
        let text = output.as_str_lossy();
        assert!(
            text.contains("CUSTOM_VALUE"),
            "expected CUSTOM_VALUE in output, got: {text:?}"
        );
    }

    // -- TerminalOutputHistory tests ----------------------------------------

    #[test]
    fn output_history_append_and_search() {
        let mut hist = TerminalOutputHistory::new(100);
        hist.append("first line\nsecond line\nthird line");
        assert_eq!(hist.line_count(), 3);
        let results = hist.search("second");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
        assert_eq!(results[0].1, "second line");
    }

    #[test]
    fn output_history_max_lines() {
        let mut hist = TerminalOutputHistory::new(3);
        hist.append("a\nb\nc\nd\ne");
        assert_eq!(hist.line_count(), 3);
        assert_eq!(hist.lines()[0], "c");
    }

    #[test]
    fn output_history_tail() {
        let mut hist = TerminalOutputHistory::new(100);
        hist.append("line1\nline2\nline3\nline4\nline5");
        let tail = hist.tail(2);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0], "line4");
        assert_eq!(tail[1], "line5");
    }

    #[test]
    fn output_history_clear() {
        let mut hist = TerminalOutputHistory::new(100);
        hist.append("data\nmore data");
        hist.clear();
        assert_eq!(hist.line_count(), 0);
        assert!(hist.search("data").is_empty());
    }

    #[test]
    fn session_stats_tracking() {
        let mut stats = TerminalSessionStats::new(80, 24);
        assert_eq!(stats.total_bytes(), 0);
        stats.record_write(100);
        stats.record_read(50);
        assert_eq!(stats.total_bytes_written, 100);
        assert_eq!(stats.total_bytes_read, 50);
        assert_eq!(stats.total_bytes(), 150);
        stats.record_resize(120, 40);
        assert_eq!(stats.resize_count, 1);
        assert_eq!(stats.current_cols, 120);
        assert_eq!(stats.current_rows, 40);
    }

    #[test]
    fn session_stats_display() {
        let stats = TerminalSessionStats::new(80, 24);
        let s = format!("{stats}");
        assert!(s.contains("80x24"));
        assert!(s.contains("written=0"));
    }

    #[test]
    fn output_history_search_no_match() {
        let mut hist = TerminalOutputHistory::new(100);
        hist.append("hello\nworld");
        assert!(hist.search("zzz").is_empty());
    }

    // -- TerminalProfile tests ----------------------------------------------

    #[test]
    fn terminal_profile_new() {
        let profile = TerminalProfile::new("/bin/zsh", "Zsh");
        assert_eq!(profile.shell_path, "/bin/zsh");
        assert_eq!(profile.name, "Zsh");
        assert!(profile.args.is_empty());
        assert!(profile.env.is_empty());
        assert!(profile.icon.is_none());
    }

    #[test]
    fn terminal_profile_with_args_and_env() {
        let profile = TerminalProfile::new("/bin/bash", "Bash")
            .with_arg("-l")
            .with_env("TERM", "xterm-256color")
            .with_icon("terminal-bash");
        assert_eq!(profile.args, vec!["-l"]);
        assert_eq!(profile.env.get("TERM").unwrap(), "xterm-256color");
        assert_eq!(profile.icon.as_deref(), Some("terminal-bash"));
    }

    #[test]
    fn terminal_profile_to_config() {
        let profile = TerminalProfile::new("/bin/zsh", "Zsh")
            .with_arg("-i")
            .with_env("FOO", "bar");
        let config = profile.to_config();
        assert_eq!(config.shell, "/bin/zsh");
        assert_eq!(config.args, vec!["-i"]);
        assert_eq!(config.env.get("FOO").unwrap(), "bar");
        assert_eq!(config.title, "Zsh");
    }

    #[test]
    fn terminal_profile_display() {
        let profile = TerminalProfile::new("/bin/bash", "Bash");
        let s = format!("{profile}");
        assert!(s.contains("Bash"));
        assert!(s.contains("/bin/bash"));
    }

    // -- detect_available_shells tests --------------------------------------

    #[test]
    fn detect_available_shells_finds_sh() {
        let shells = detect_available_shells();
        // /bin/sh should exist on any Unix system
        assert!(shells.iter().any(|s| s.contains("sh")));
    }

    // -- TerminalEnvironment tests ------------------------------------------

    #[test]
    fn terminal_environment_merge() {
        let mut shell_env = HashMap::new();
        shell_env.insert("PATH".to_string(), "/usr/bin".to_string());
        shell_env.insert("HOME".to_string(), "/home/user".to_string());

        let mut editor_env = HashMap::new();
        editor_env.insert("VSEDIT".to_string(), "1".to_string());
        editor_env.insert("PATH".to_string(), "/usr/local/bin:/usr/bin".to_string());

        let env = TerminalEnvironment::new(shell_env, editor_env);
        let merged = env.merged();
        // editor_env overrides shell_env for PATH
        assert_eq!(merged.get("PATH").unwrap(), "/usr/local/bin:/usr/bin");
        assert_eq!(merged.get("HOME").unwrap(), "/home/user");
        assert_eq!(merged.get("VSEDIT").unwrap(), "1");
    }

    #[test]
    fn terminal_environment_get() {
        let mut shell_env = HashMap::new();
        shell_env.insert("KEY".to_string(), "shell_val".to_string());
        let mut editor_env = HashMap::new();
        editor_env.insert("KEY".to_string(), "editor_val".to_string());
        let env = TerminalEnvironment::new(shell_env, editor_env);
        assert_eq!(env.get("KEY"), Some("editor_val"));
    }

    #[test]
    fn terminal_environment_keys() {
        let mut shell_env = HashMap::new();
        shell_env.insert("A".to_string(), "1".to_string());
        let mut editor_env = HashMap::new();
        editor_env.insert("B".to_string(), "2".to_string());
        let env = TerminalEnvironment::new(shell_env, editor_env);
        let keys = env.keys();
        assert!(keys.contains(&"A".to_string()));
        assert!(keys.contains(&"B".to_string()));
    }

    // -- TerminalColorScheme tests ------------------------------------------

    #[test]
    fn test_color_scheme_default_dark() {
        let scheme = TerminalColorScheme::default_dark();
        assert_eq!(scheme.name, "Dark");
        assert_eq!(scheme.foreground, "#d4d4d4");
        assert_eq!(scheme.background, "#1e1e1e");
        assert_eq!(scheme.palette.len(), 16);
        assert!(scheme.is_dark());
    }

    #[test]
    fn test_color_scheme_default_light() {
        let scheme = TerminalColorScheme::default_light();
        assert_eq!(scheme.name, "Light");
        assert_eq!(scheme.foreground, "#000000");
        assert_eq!(scheme.background, "#ffffff");
        assert_eq!(scheme.palette.len(), 16);
        assert!(!scheme.is_dark());
    }

    #[test]
    fn test_color_scheme_is_dark() {
        let dark = TerminalColorScheme::new("d").with_background("#0a0a0a");
        assert!(dark.is_dark());
        let also_dark = TerminalColorScheme::new("d").with_background("#3f3f3f");
        assert!(also_dark.is_dark());
        let light = TerminalColorScheme::new("l").with_background("#f0f0f0");
        assert!(!light.is_dark());
    }

    #[test]
    fn test_color_scheme_builder() {
        let scheme = TerminalColorScheme::new("Custom")
            .with_foreground("#aabbcc")
            .with_background("#112233")
            .with_cursor("#ddeeff");
        assert_eq!(scheme.name, "Custom");
        assert_eq!(scheme.foreground, "#aabbcc");
        assert_eq!(scheme.background, "#112233");
        assert_eq!(scheme.cursor, "#ddeeff");
    }

    // -- TerminalFontConfig tests -------------------------------------------

    #[test]
    fn test_font_config_defaults() {
        let fc = TerminalFontConfig::new();
        assert_eq!(fc.family, "monospace");
        assert!((fc.size - 14.0).abs() < f32::EPSILON);
        assert!((fc.line_height - 1.2).abs() < f32::EPSILON);
        assert_eq!(fc.weight, 400);
        assert!(!fc.ligatures);
    }

    #[test]
    fn test_font_config_clamping() {
        let fc = TerminalFontConfig::new().with_size(2.0);
        assert!((fc.size - 6.0).abs() < f32::EPSILON);
        let fc = TerminalFontConfig::new().with_size(100.0);
        assert!((fc.size - 72.0).abs() < f32::EPSILON);
        let fc = TerminalFontConfig::new().with_line_height(0.1);
        assert!((fc.line_height - 0.5).abs() < f32::EPSILON);
        let fc = TerminalFontConfig::new().with_line_height(5.0);
        assert!((fc.line_height - 3.0).abs() < f32::EPSILON);
        let fc = TerminalFontConfig::new().with_weight(50);
        assert_eq!(fc.weight, 100);
        let fc = TerminalFontConfig::new().with_weight(1000);
        assert_eq!(fc.weight, 900);
    }

    #[test]
    fn test_font_config_cell_height() {
        let fc = TerminalFontConfig::new().with_size(20.0).with_line_height(1.5);
        assert!((fc.cell_height() - 30.0).abs() < f32::EPSILON);
    }
}
