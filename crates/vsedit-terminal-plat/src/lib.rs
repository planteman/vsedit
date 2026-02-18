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
// TerminalSplitLayout — split terminal layout management
// ---------------------------------------------------------------------------

/// Orientation of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrientation {
    Horizontal,
    Vertical,
}

/// A pane within a split layout, identified by terminal ID.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitPane {
    pub terminal_id: u64,
    pub weight: f32,
}

/// Manages a split layout of terminal panes.
#[derive(Debug, Clone)]
pub struct TerminalSplitLayout {
    pub orientation: SplitOrientation,
    pub panes: Vec<SplitPane>,
}

impl TerminalSplitLayout {
    pub fn new(orientation: SplitOrientation) -> Self {
        Self { orientation, panes: Vec::new() }
    }

    /// Add a pane with equal weight.
    pub fn add_pane(&mut self, terminal_id: u64) {
        self.panes.push(SplitPane { terminal_id, weight: 1.0 });
        self.normalize_weights();
    }

    /// Remove a pane by terminal ID. Returns true if found.
    pub fn remove_pane(&mut self, terminal_id: u64) -> bool {
        let before = self.panes.len();
        self.panes.retain(|p| p.terminal_id != terminal_id);
        if self.panes.len() < before {
            self.normalize_weights();
            true
        } else {
            false
        }
    }

    /// Normalize weights so they sum to 1.0.
    fn normalize_weights(&mut self) {
        if self.panes.is_empty() {
            return;
        }
        let total: f32 = self.panes.iter().map(|p| p.weight).sum();
        if total > 0.0 {
            for pane in &mut self.panes {
                pane.weight /= total;
            }
        }
    }

    /// Compute the pixel size for each pane given a total container size.
    pub fn compute_sizes(&self, total_pixels: u32) -> Vec<(u64, u32)> {
        self.panes
            .iter()
            .map(|p| (p.terminal_id, (p.weight * total_pixels as f32).round() as u32))
            .collect()
    }

    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// TerminalCommandHistory — command history with frequency tracking
// ---------------------------------------------------------------------------

/// Entry in the command history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHistoryEntry {
    pub command: String,
    pub run_count: u32,
    pub last_used_ms: u64,
}

/// Tracks commands executed in terminals with frequency information.
#[derive(Debug, Clone, Default)]
pub struct TerminalCommandHistory {
    entries: Vec<CommandHistoryEntry>,
    max_entries: usize,
}

impl TerminalCommandHistory {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), max_entries }
    }

    /// Record a command execution.
    pub fn record(&mut self, command: &str, timestamp_ms: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.command == command) {
            entry.run_count += 1;
            entry.last_used_ms = timestamp_ms;
        } else {
            if self.entries.len() >= self.max_entries {
                // Evict least-recently-used
                if let Some(idx) = self.entries.iter().enumerate()
                    .min_by_key(|(_, e)| e.last_used_ms)
                    .map(|(i, _)| i)
                {
                    self.entries.remove(idx);
                }
            }
            self.entries.push(CommandHistoryEntry {
                command: command.to_string(),
                run_count: 1,
                last_used_ms: timestamp_ms,
            });
        }
    }

    /// Return entries sorted by frequency (most used first).
    pub fn most_frequent(&self, limit: usize) -> Vec<&CommandHistoryEntry> {
        let mut sorted: Vec<&CommandHistoryEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.run_count.cmp(&a.run_count));
        sorted.truncate(limit);
        sorted
    }

    /// Search for commands matching a prefix.
    pub fn search_prefix(&self, prefix: &str) -> Vec<&CommandHistoryEntry> {
        self.entries.iter().filter(|e| e.command.starts_with(prefix)).collect()
    }

    /// Return the total number of unique commands recorded.
    pub fn unique_count(&self) -> usize {
        self.entries.len()
    }

    /// Return total executions across all commands.
    pub fn total_executions(&self) -> u32 {
        self.entries.iter().map(|e| e.run_count).sum()
    }
}

// ---------------------------------------------------------------------------
// TerminalTheme — theme management for terminals
// ---------------------------------------------------------------------------

/// A named terminal theme combining color scheme and font config.
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    pub name: String,
    pub colors: TerminalColorScheme,
    pub font: TerminalFontConfig,
}

impl TerminalTheme {
    pub fn new(name: impl Into<String>, colors: TerminalColorScheme, font: TerminalFontConfig) -> Self {
        Self { name: name.into(), colors, font }
    }

    pub fn dark_default() -> Self {
        Self::new("Dark Default", TerminalColorScheme::default_dark(), TerminalFontConfig::new())
    }

    pub fn light_default() -> Self {
        Self::new("Light Default", TerminalColorScheme::default_light(), TerminalFontConfig::new())
    }

    pub fn is_dark(&self) -> bool {
        self.colors.is_dark()
    }
}

impl fmt::Display for TerminalTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TerminalTheme({})", self.name)
    }
}

impl TerminalConfig {
    /// Return the total environment variable count.
    pub fn env_var_count(&self) -> usize {
        self.env.len()
    }

    /// Return a display-friendly summary of this config.
    pub fn summary(&self) -> String {
        format!(
            "{}x{} shell={} title={}",
            self.initial_cols, self.initial_rows, self.shell, self.title
        )
    }

    /// Return true if this config uses non-default dimensions.
    pub fn has_custom_dimensions(&self) -> bool {
        self.initial_cols != 80 || self.initial_rows != 24
    }
}

impl TerminalProfile {
    /// Return the number of arguments configured.
    pub fn arg_count(&self) -> usize {
        self.args.len()
    }

    /// Return the number of environment variables configured.
    pub fn env_count(&self) -> usize {
        self.env.len()
    }

    /// Return a display name including shell path.
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.name, self.shell_path)
    }
}

impl TerminalOutputHistory {
    /// Return the last line, if any.
    pub fn last_line(&self) -> Option<&str> {
        self.lines().last().map(|s| s.as_str())
    }

    /// Return true if any line contains the query.
    pub fn contains(&self, query: &str) -> bool {
        self.lines().iter().any(|l| l.contains(query))
    }
}

impl TerminalSessionStats {
    /// Return the total bytes transferred (read + written).
    pub fn total_bytes_transferred(&self) -> u64 {
        self.total_bytes()
    }

    /// Return the number of resize events recorded.
    pub fn resize_event_count(&self) -> u32 {
        self.resize_count
    }
}

impl TerminalCommandHistory {
    /// Clear all recorded history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return true if a command with the given name exists.
    pub fn contains_command(&self, command: &str) -> bool {
        self.entries.iter().any(|e| e.command == command)
    }
}

// ---------------------------------------------------------------------------
// TerminalDimensions — helper for terminal size calculations
// ---------------------------------------------------------------------------

/// Represents the dimensions of a terminal in both cells and pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalDimensions {
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl TerminalDimensions {
    /// Create dimensions from cell counts and cell size in pixels.
    pub fn from_cells(cols: u16, rows: u16, cell_width: u16, cell_height: u16) -> Self {
        Self {
            cols,
            rows,
            pixel_width: cols as u32 * cell_width as u32,
            pixel_height: rows as u32 * cell_height as u32,
        }
    }

    /// Create dimensions from a target pixel area and cell size.
    pub fn from_pixels(pixel_width: u32, pixel_height: u32, cell_width: u16, cell_height: u16) -> Self {
        let cols = if cell_width > 0 { (pixel_width / cell_width as u32) as u16 } else { 1 };
        let rows = if cell_height > 0 { (pixel_height / cell_height as u32) as u16 } else { 1 };
        Self { cols, rows, pixel_width, pixel_height }
    }

    /// Return the total number of character cells.
    pub fn total_cells(&self) -> u32 {
        self.cols as u32 * self.rows as u32
    }

    /// Return the aspect ratio (width / height) in cells.
    pub fn aspect_ratio(&self) -> f64 {
        if self.rows == 0 {
            return 0.0;
        }
        self.cols as f64 / self.rows as f64
    }

    /// Clamp the dimensions to min/max column and row bounds.
    pub fn clamp(self, min_cols: u16, max_cols: u16, min_rows: u16, max_rows: u16) -> Self {
        Self {
            cols: self.cols.clamp(min_cols, max_cols),
            rows: self.rows.clamp(min_rows, max_rows),
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
        }
    }

    /// Return true if either dimension is zero.
    pub fn is_zero(&self) -> bool {
        self.cols == 0 || self.rows == 0
    }
}

impl fmt::Display for TerminalDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{} ({}x{}px)", self.cols, self.rows, self.pixel_width, self.pixel_height)
    }
}

impl Default for TerminalFontConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalEnvironment {
    /// Return the number of variables from the shell environment.
    pub fn shell_env_count(&self) -> usize {
        self.shell_env.len()
    }

    /// Return the number of variables from the editor environment.
    pub fn editor_env_count(&self) -> usize {
        self.editor_env.len()
    }

    /// Return total unique variable count across both environments.
    pub fn total_count(&self) -> usize {
        self.keys().len()
    }

    /// Remove a variable from both environments. Returns true if found in either.
    pub fn remove(&mut self, key: &str) -> bool {
        let a = self.shell_env.remove(key).is_some();
        let b = self.editor_env.remove(key).is_some();
        a || b
    }
}

// ---------------------------------------------------------------------------
// MergeStrategy
// ---------------------------------------------------------------------------

/// Strategy for resolving conflicts when merging environment variable sets.
#[derive(Debug, Clone)]
pub enum MergeStrategy {
    /// Shell value wins on conflict.
    ShellWins,
    /// Editor value wins on conflict.
    EditorWins,
    /// Concatenate both values with the given separator.
    Concatenate(String),
}

// ---------------------------------------------------------------------------
// TerminalProfileDiscovery
// ---------------------------------------------------------------------------

/// Discovers available shell profiles on the system.
pub struct TerminalProfileDiscovery {
    profiles: Vec<TerminalProfile>,
    scanned: bool,
}

impl TerminalProfileDiscovery {
    /// Create a new discovery instance (not yet scanned).
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
            scanned: false,
        }
    }

    /// Scan common shell paths and populate the profiles list.
    pub fn scan(&mut self) {
        self.profiles.clear();
        let candidates: &[(&str, &str)] = &[
            ("/bin/bash", "Bash"),
            ("/bin/zsh", "Zsh"),
            ("/usr/bin/fish", "Fish"),
            ("/bin/sh", "POSIX Shell"),
            ("/usr/bin/bash", "Bash"),
            ("/usr/bin/zsh", "Zsh"),
        ];
        let mut seen = std::collections::HashSet::new();
        for &(path, name) in candidates {
            if std::path::Path::new(path).exists() && seen.insert(name.to_string()) {
                self.profiles.push(TerminalProfile::new(path, name));
            }
        }
        self.scanned = true;
    }

    /// Return the discovered profiles.
    pub fn profiles(&self) -> &[TerminalProfile] {
        &self.profiles
    }

    /// Find a profile by its human-readable name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&TerminalProfile> {
        let lower = name.to_lowercase();
        self.profiles.iter().find(|p| p.name.to_lowercase() == lower)
    }

    /// Return the first profile that looks like a sensible default.
    pub fn default_profile(&self) -> Option<&TerminalProfile> {
        // Prefer bash, then zsh, then the first available.
        self.find_by_name("Bash")
            .or_else(|| self.find_by_name("Zsh"))
            .or_else(|| self.profiles.first())
    }

    /// Number of discovered profiles.
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }
}

// ---------------------------------------------------------------------------
// TerminalEnvMerger
// ---------------------------------------------------------------------------

/// Merges two sets of environment variables with a configurable conflict
/// resolution strategy.
pub struct TerminalEnvMerger {
    strategy: MergeStrategy,
}

impl TerminalEnvMerger {
    pub fn new(strategy: MergeStrategy) -> Self {
        Self { strategy }
    }

    /// Merge `shell` and `editor` maps according to the configured strategy.
    pub fn merge(
        &self,
        shell: &HashMap<String, String>,
        editor: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for k in shell.keys().chain(editor.keys()) {
            keys.insert(k.as_str());
        }
        let mut out = HashMap::new();
        for key in keys {
            if let Some(val) =
                self.merge_single(key, shell.get(key).map(|s| s.as_str()), editor.get(key).map(|s| s.as_str()))
            {
                out.insert(key.to_string(), val);
            }
        }
        out
    }

    /// Resolve a single key given optional values from each source.
    pub fn merge_single(
        &self,
        _key: &str,
        shell_val: Option<&str>,
        editor_val: Option<&str>,
    ) -> Option<String> {
        match (&self.strategy, shell_val, editor_val) {
            (_, Some(s), None) => Some(s.to_string()),
            (_, None, Some(e)) => Some(e.to_string()),
            (_, None, None) => None,
            (MergeStrategy::ShellWins, Some(s), Some(_)) => Some(s.to_string()),
            (MergeStrategy::EditorWins, _, Some(e)) => Some(e.to_string()),
            (MergeStrategy::Concatenate(sep), Some(s), Some(e)) => {
                Some(format!("{}{}{}", s, sep, e))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ShellIntegrationHandler
// ---------------------------------------------------------------------------

/// Handles shell-integration OSC sequences emitted by the shell.
pub struct ShellIntegrationHandler {
    pub prompt_mark: Option<String>,
    pub command_start: Option<String>,
    pub command_end: Option<String>,
    pub cwd: Option<PathBuf>,
}

impl ShellIntegrationHandler {
    pub fn new() -> Self {
        Self {
            prompt_mark: None,
            command_start: None,
            command_end: None,
            cwd: None,
        }
    }

    /// Parse an OSC-like sequence string and update internal state.
    pub fn handle_sequence(&mut self, seq: &str) {
        if seq.contains("PromptMark") {
            self.prompt_mark = Some(seq.to_string());
        }
        if seq.contains("CommandStart") {
            self.command_start = Some(seq.to_string());
        }
        if seq.contains("CommandEnd") {
            self.command_end = Some(seq.to_string());
        }
        if let Some(idx) = seq.find("CWD=") {
            let path_str = &seq[idx + 4..];
            // Take until the next ';' or end of string.
            let end = path_str.find(';').unwrap_or(path_str.len());
            self.cwd = Some(PathBuf::from(&path_str[..end]));
        }
    }

    /// Return the current working directory if one has been reported.
    pub fn current_directory(&self) -> Option<&PathBuf> {
        self.cwd.as_ref()
    }

    /// Whether a prompt mark has been received.
    pub fn has_prompt(&self) -> bool {
        self.prompt_mark.is_some()
    }

    /// Reset all tracked state.
    pub fn reset(&mut self) {
        self.prompt_mark = None;
        self.command_start = None;
        self.command_end = None;
        self.cwd = None;
    }
}

// ---------------------------------------------------------------------------
// TerminalFontResolver
// ---------------------------------------------------------------------------

/// Resolves a [`TerminalFontConfig`] for a given profile, falling back to a
/// default when no override exists.
pub struct TerminalFontResolver {
    default_config: TerminalFontConfig,
    overrides: HashMap<String, TerminalFontConfig>,
}

impl TerminalFontResolver {
    pub fn new(default: TerminalFontConfig) -> Self {
        Self {
            default_config: default,
            overrides: HashMap::new(),
        }
    }

    /// Register a font-config override for a specific profile name.
    pub fn add_override(&mut self, profile_name: &str, config: TerminalFontConfig) {
        self.overrides.insert(profile_name.to_string(), config);
    }

    /// Resolve the font config for the given profile name.
    pub fn resolve(&self, profile_name: &str) -> &TerminalFontConfig {
        self.overrides.get(profile_name).unwrap_or(&self.default_config)
    }

    /// Number of registered overrides.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    /// Check whether an override exists for the given profile.
    pub fn has_override(&self, profile_name: &str) -> bool {
        self.overrides.contains_key(profile_name)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// === Terminal Resize Handler ===

/// Terminal Resize Handler implementation.
#[derive(Debug, Clone)]
pub struct TerminalResizeHandler {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TerminalResizeHandlerStats,
}

/// Statistics for TerminalResizeHandler.
#[derive(Debug, Clone, Default)]
pub struct TerminalResizeHandlerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TerminalResizeHandlerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TerminalResizeHandler {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TerminalResizeHandlerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TerminalResizeHandlerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TerminalResizeHandler {
    fn default() -> Self {
        Self::new()
    }
}

// === Terminal Signal Mapper ===

/// Priority level for TerminalSignalMapper items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalSignalMapperPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TerminalSignalMapperPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TerminalSignalMapperPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Terminal Signal Mapper implementation.
#[derive(Debug, Clone)]
pub struct TerminalSignalMapper {
    items: Vec<TerminalSignalMapperItem>,
    max_items: usize,
    default_priority: TerminalSignalMapperPriority,
}

/// A single item in TerminalSignalMapper.
#[derive(Debug, Clone)]
pub struct TerminalSignalMapperItem {
    pub id: String,
    pub label: String,
    pub priority: TerminalSignalMapperPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TerminalSignalMapperItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TerminalSignalMapperPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TerminalSignalMapperPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TerminalSignalMapper {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TerminalSignalMapperPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TerminalSignalMapperItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TerminalSignalMapperItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TerminalSignalMapperItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TerminalSignalMapperPriority) -> Vec<&TerminalSignalMapperItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TerminalSignalMapperItem> {
        let mut sorted: Vec<&TerminalSignalMapperItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TerminalSignalMapperItem> {
        let mut sorted: Vec<&TerminalSignalMapperItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TerminalSignalMapperItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TerminalSignalMapperPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TerminalSignalMapperPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TerminalSignalMapperItem> {
        self.items.iter()
    }
}

impl Default for TerminalSignalMapper {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// terminal_plat – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XTerminalPlatCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XTerminalPlatCapabilities {
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

impl Default for XTerminalPlatCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XTerminalPlatServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XTerminalPlatServiceRegistry {
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
pub fn x_terminal_plat_sanitize_path(p: &str) -> String {
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



// ---------------------------------------------------------------------------
// terminal_plat – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for terminal platform layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YTerminalPlatTerminalColorScheme {
    Dark,
    Light,
    Solarized,
    Monokai,
}

impl YTerminalPlatTerminalColorScheme {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::Solarized => 2,
            Self::Monokai => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Solarized => "Solarized",
            Self::Monokai => "Monokai",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YTerminalPlatTerminalColorScheme] {
        &[
            YTerminalPlatTerminalColorScheme::Dark,
            YTerminalPlatTerminalColorScheme::Light,
            YTerminalPlatTerminalColorScheme::Solarized,
            YTerminalPlatTerminalColorScheme::Monokai,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YTerminalPlatTerminalColorScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks terminal dimensions data.
#[derive(Debug, Clone)]
pub struct YTerminalPlatTerminalDimensions {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: f32,
}

impl YTerminalPlatTerminalDimensions {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            cols: 0,
            rows: 0,
            cell_width: 0.0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YTerminalPlatTerminalDimensions({}: {:?})", "cols", self.cols)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_terminal_plat_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_terminal_plat_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_terminal_plat_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_terminal_plat_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_terminal_plat_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_terminal_plat_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_terminal_plat_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_terminal_plat_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// terminal_plat – Extended terminal font metrics helpers
// ---------------------------------------------------------------------------

/// Priority levels for terminal font metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZTerminalPlatPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZTerminalPlatPriority {
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
    pub fn all_asc() -> [ZTerminalPlatPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZTerminalPlatPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks terminal font metrics data.
#[derive(Debug, Clone)]
pub struct ZTerminalPlatTerminalFontMetrics {
    pub char_widths: Vec<(char, f32)>,
    pub line_height: f32,
    pub baseline: f32,
}

impl ZTerminalPlatTerminalFontMetrics {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            char_widths: Vec::new(),
            line_height: 0.0,
            baseline: 0.0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.char_widths.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.char_widths.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.char_widths.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZTerminalPlatTerminalFontMetrics[line_height={:?}, baseline={:?}]", self.line_height, self.baseline)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for terminal font metrics.
pub fn z_terminal_plat_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_terminal_plat_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_terminal_plat_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_terminal_plat_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_terminal_plat_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_terminal_plat_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_terminal_plat_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 56
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer56 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer56 {
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
pub fn xb_fnv1a_56(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_56<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_56<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_56(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_56(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 176
// ---------------------------------------------------------------------------

/// Generic object pool `Xc176Pool<T>`.
pub struct Xc176Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc176Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc176PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc176Pool<T> {
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
    pub fn stats(&self) -> Xc176PoolStats {
        Xc176PoolStats {
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

impl<T> Default for Xc176Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc176Scheduler`.
pub struct Xc176Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc176Scheduler {
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

impl Default for Xc176Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_176 hash for the given byte slice.
pub fn xc_176_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_176 convention.
pub fn xc_176_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe69 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe69Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe69PipelineError {
    pub stage: Xe69Stage,
    pub message: String,
}

impl std::fmt::Display for Xe69PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe69Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe69Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError>>>,
    stage_names: Vec<Xe69Stage>,
}

impl Xe69Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe69Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe69Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe69Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe69Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> {
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

    pub fn compose(mut self, other: Xe69Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe69CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe69CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe69Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe69CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe69CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe69Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe69CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_69_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe69CacheEntry {
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

    fn xe_69_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe69CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_69_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> {
    Ok(data)
}

pub fn xe_69_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_69_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_69_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_69_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe69PipelineError> {
    Err(Xe69PipelineError {
        stage: Xe69Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_67: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg67Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg67Graph {
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

impl Default for Xg67Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_67: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg67Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg67Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg67Heap<T>) {
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

impl<T: Ord> Default for Xg67Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 175).
pub struct Xh175SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh175SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 217 as u64,
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

/// A compact bit set supporting boolean operations (variant 175).
pub struct Xh175BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh175BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 175).
pub struct Xi175Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi175Deque<T> {
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
pub struct Xi175Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi175Interval {
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

/// A simple interval tree (variant 175).
pub struct Xi175IntervalTree {
    xi_intervals: Vec<Xi175Interval>,
}

impl Xi175IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi175Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi175Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi175Interval) -> Vec<&Xi175Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi175Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi175Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi175Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi175Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi175Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi175Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 175) ---

/// Disjoint set / union-find for crate 175.
pub struct Xj175UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj175UnionFind {
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

const XJ175_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 175.
pub struct Xj175BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj175BTreeNode<K, V>>>,
    len: usize,
}

struct Xj175BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj175BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj175BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ175_BTREE_ORDER - 1
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
        let mid = XJ175_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj175BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj175BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj175BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj175BTreeNode::xj_new_leaf();
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

    // -- TerminalSplitLayout tests ------------------------------------------

    #[test]
    fn split_layout_add_and_remove_panes() {
        let mut layout = TerminalSplitLayout::new(SplitOrientation::Horizontal);
        assert!(layout.is_empty());
        layout.add_pane(1);
        layout.add_pane(2);
        assert_eq!(layout.pane_count(), 2);

        // Weights should sum to ~1.0
        let total_weight: f32 = layout.panes.iter().map(|p| p.weight).sum();
        assert!((total_weight - 1.0).abs() < 0.01);

        assert!(layout.remove_pane(1));
        assert_eq!(layout.pane_count(), 1);
        assert!(!layout.remove_pane(99));
    }

    #[test]
    fn split_layout_compute_sizes() {
        let mut layout = TerminalSplitLayout::new(SplitOrientation::Vertical);
        layout.add_pane(10);
        layout.add_pane(20);
        let sizes = layout.compute_sizes(1000);
        assert_eq!(sizes.len(), 2);
        let total: u32 = sizes.iter().map(|(_, s)| *s).sum();
        assert!(total >= 999 && total <= 1001); // rounding tolerance
    }

    // -- TerminalCommandHistory tests ---------------------------------------

    #[test]
    fn command_history_records_and_counts() {
        let mut hist = TerminalCommandHistory::new(100);
        hist.record("ls -la", 1000);
        hist.record("git status", 2000);
        hist.record("ls -la", 3000);
        assert_eq!(hist.unique_count(), 2);
        assert_eq!(hist.total_executions(), 3);

        let freq = hist.most_frequent(10);
        assert_eq!(freq[0].command, "ls -la");
        assert_eq!(freq[0].run_count, 2);
    }

    #[test]
    fn command_history_search_prefix() {
        let mut hist = TerminalCommandHistory::new(100);
        hist.record("git status", 100);
        hist.record("git commit", 200);
        hist.record("ls", 300);
        let results = hist.search_prefix("git");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn command_history_evicts_lru() {
        let mut hist = TerminalCommandHistory::new(2);
        hist.record("cmd1", 100);
        hist.record("cmd2", 200);
        hist.record("cmd3", 300); // should evict cmd1 (oldest)
        assert_eq!(hist.unique_count(), 2);
        assert!(hist.search_prefix("cmd1").is_empty());
    }

    // -- TerminalTheme tests ------------------------------------------------

    #[test]
    fn theme_dark_and_light_defaults() {
        let dark = TerminalTheme::dark_default();
        assert!(dark.is_dark());
        assert_eq!(dark.name, "Dark Default");

        let light = TerminalTheme::light_default();
        assert!(!light.is_dark());
        assert_eq!(light.name, "Light Default");
    }

    #[test]
    fn theme_display() {
        let t = TerminalTheme::dark_default();
        assert_eq!(format!("{t}"), "TerminalTheme(Dark Default)");
    }

    #[test]
    fn config_env_var_count_default() {
        let cfg = TerminalConfig::default();
        assert_eq!(cfg.env_var_count(), 0);
    }

    #[test]
    fn config_summary_format() {
        let cfg = TerminalConfig::default();
        let s = cfg.summary();
        assert!(s.contains("80x24"));
        assert!(s.contains("Terminal"));
    }

    #[test]
    fn config_has_custom_dimensions_false() {
        let cfg = TerminalConfig::default();
        assert!(!cfg.has_custom_dimensions());
    }

    #[test]
    fn config_has_custom_dimensions_true() {
        let mut cfg = TerminalConfig::default();
        cfg.initial_cols = 120;
        assert!(cfg.has_custom_dimensions());
    }

    #[test]
    fn profile_arg_and_env_count() {
        let p = TerminalProfile::new("/bin/bash", "Bash")
            .with_arg("-l")
            .with_env("TERM", "xterm");
        assert_eq!(p.arg_count(), 1);
        assert_eq!(p.env_count(), 1);
    }

    #[test]
    fn profile_display_name() {
        let p = TerminalProfile::new("/bin/bash", "Bash");
        assert_eq!(p.display_name(), "Bash (/bin/bash)");
    }

    #[test]
    fn output_history_last_line() {
        let mut h = TerminalOutputHistory::new(100);
        assert!(h.last_line().is_none());
        h.append("first\nsecond\n");
        assert_eq!(h.last_line(), Some("second"));
    }

    #[test]
    fn output_history_contains_query() {
        let mut h = TerminalOutputHistory::new(100);
        h.append("hello world\nfoo bar\n");
        assert!(h.contains("foo"));
        assert!(!h.contains("missing"));
    }

    #[test]
    fn session_stats_total_bytes() {
        let mut stats = TerminalSessionStats::new(80, 24);
        stats.record_write(100);
        stats.record_read(50);
        assert_eq!(stats.total_bytes_transferred(), 150);
    }

    #[test]
    fn session_stats_resize_events() {
        let mut stats = TerminalSessionStats::new(80, 24);
        stats.record_resize(100, 50);
        assert_eq!(stats.resize_event_count(), 1);
    }

    #[test]
    fn command_history_clear_removes_all() {
        let mut hist = TerminalCommandHistory::new(100);
        hist.record("ls", 1000);
        hist.clear();
        assert_eq!(hist.unique_count(), 0);
    }

    #[test]
    fn command_history_contains_command() {
        let mut hist = TerminalCommandHistory::new(100);
        hist.record("ls", 1000);
        assert!(hist.contains_command("ls"));
        assert!(!hist.contains_command("cd"));
    }

    // -- TerminalDimensions tests -------------------------------------------

    #[test]
    fn dimensions_from_cells() {
        let d = TerminalDimensions::from_cells(80, 24, 8, 16);
        assert_eq!(d.cols, 80);
        assert_eq!(d.rows, 24);
        assert_eq!(d.pixel_width, 640);
        assert_eq!(d.pixel_height, 384);
        assert_eq!(d.total_cells(), 1920);
    }

    #[test]
    fn dimensions_from_pixels() {
        let d = TerminalDimensions::from_pixels(640, 384, 8, 16);
        assert_eq!(d.cols, 80);
        assert_eq!(d.rows, 24);
    }

    #[test]
    fn dimensions_aspect_ratio() {
        let d = TerminalDimensions::from_cells(80, 24, 8, 16);
        let ratio = d.aspect_ratio();
        assert!((ratio - 80.0 / 24.0).abs() < 0.001);
    }

    #[test]
    fn dimensions_clamp() {
        let d = TerminalDimensions::from_cells(200, 5, 8, 16);
        let clamped = d.clamp(10, 120, 10, 50);
        assert_eq!(clamped.cols, 120);
        assert_eq!(clamped.rows, 10);
    }

    #[test]
    fn dimensions_is_zero() {
        let d = TerminalDimensions::from_cells(0, 24, 8, 16);
        assert!(d.is_zero());
        let d2 = TerminalDimensions::from_cells(80, 24, 8, 16);
        assert!(!d2.is_zero());
    }

    #[test]
    fn dimensions_display() {
        let d = TerminalDimensions::from_cells(80, 24, 8, 16);
        let s = format!("{}", d);
        assert!(s.contains("80x24"));
    }

    // -- TerminalEnvironment extended tests ----------------------------------

    #[test]
    fn env_shell_and_editor_counts() {
        let shell: HashMap<String, String> = [("PATH".into(), "/usr/bin".into())].into();
        let editor: HashMap<String, String> = [("TERM".into(), "xterm".into()), ("EDITOR".into(), "vi".into())].into();
        let env = TerminalEnvironment::new(shell, editor);
        assert_eq!(env.shell_env_count(), 1);
        assert_eq!(env.editor_env_count(), 2);
        assert_eq!(env.total_count(), 3);
    }

    #[test]
    fn env_remove_variable() {
        let shell: HashMap<String, String> = [("PATH".into(), "/usr/bin".into())].into();
        let editor: HashMap<String, String> = [("PATH".into(), "/editor/bin".into())].into();
        let mut env = TerminalEnvironment::new(shell, editor);
        assert!(env.remove("PATH"));
        assert_eq!(env.shell_env_count(), 0);
        assert_eq!(env.editor_env_count(), 0);
    }

    #[test]
    fn font_config_default() {
        let f = TerminalFontConfig::default();
        assert_eq!(f.family, "monospace");
        assert!((f.size - 14.0).abs() < 0.1);
        assert_eq!(f.weight, 400);
    }

    // -- TerminalProfileDiscovery tests ------------------------------------

    #[test]
    fn test_profile_discovery_scan() {
        let mut disc = TerminalProfileDiscovery::new();
        assert!(!disc.scanned);
        assert_eq!(disc.profile_count(), 0);
        disc.scan();
        assert!(disc.scanned);
        // On most Linux systems at least /bin/sh exists.
        assert!(disc.profile_count() > 0);
    }

    #[test]
    fn test_profile_discovery_find_by_name() {
        let mut disc = TerminalProfileDiscovery::new();
        disc.scan();
        // /bin/sh should always exist as "POSIX Shell".
        let profile = disc.find_by_name("POSIX Shell");
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().shell_path, "/bin/sh");
    }

    #[test]
    fn test_profile_discovery_default() {
        let mut disc = TerminalProfileDiscovery::new();
        disc.scan();
        let def = disc.default_profile();
        assert!(def.is_some(), "should always find at least one default");
    }

    // -- TerminalEnvMerger tests -------------------------------------------

    #[test]
    fn test_env_merger_shell_wins() {
        let merger = TerminalEnvMerger::new(MergeStrategy::ShellWins);
        let shell: HashMap<String, String> =
            [("PATH".into(), "/shell/bin".into())].into();
        let editor: HashMap<String, String> =
            [("PATH".into(), "/editor/bin".into()), ("EDITOR".into(), "vsedit".into())].into();
        let merged = merger.merge(&shell, &editor);
        assert_eq!(merged["PATH"], "/shell/bin");
        assert_eq!(merged["EDITOR"], "vsedit");
    }

    #[test]
    fn test_env_merger_editor_wins() {
        let merger = TerminalEnvMerger::new(MergeStrategy::EditorWins);
        let shell: HashMap<String, String> =
            [("PATH".into(), "/shell/bin".into())].into();
        let editor: HashMap<String, String> =
            [("PATH".into(), "/editor/bin".into())].into();
        let merged = merger.merge(&shell, &editor);
        assert_eq!(merged["PATH"], "/editor/bin");
    }

    #[test]
    fn test_env_merger_concatenate() {
        let merger = TerminalEnvMerger::new(MergeStrategy::Concatenate(":".into()));
        let shell: HashMap<String, String> =
            [("PATH".into(), "/shell/bin".into())].into();
        let editor: HashMap<String, String> =
            [("PATH".into(), "/editor/bin".into())].into();
        let merged = merger.merge(&shell, &editor);
        assert_eq!(merged["PATH"], "/shell/bin:/editor/bin");
    }

    // -- ShellIntegrationHandler tests -------------------------------------

    #[test]
    fn test_shell_integration_prompt() {
        let mut handler = ShellIntegrationHandler::new();
        assert!(!handler.has_prompt());
        handler.handle_sequence("OSC;PromptMark;A");
        assert!(handler.has_prompt());
    }

    #[test]
    fn test_shell_integration_cwd() {
        let mut handler = ShellIntegrationHandler::new();
        handler.handle_sequence("OSC;CWD=/home/user/project;end");
        let dir = handler.current_directory().unwrap();
        assert_eq!(dir, &PathBuf::from("/home/user/project"));
    }

    #[test]
    fn test_shell_integration_reset() {
        let mut handler = ShellIntegrationHandler::new();
        handler.handle_sequence("OSC;PromptMark;CommandStart;CWD=/tmp");
        assert!(handler.has_prompt());
        assert!(handler.current_directory().is_some());
        handler.reset();
        assert!(!handler.has_prompt());
        assert!(handler.current_directory().is_none());
    }

    // -- TerminalFontResolver tests ----------------------------------------

    #[test]
    fn test_font_resolver_default() {
        let resolver = TerminalFontResolver::new(TerminalFontConfig::default());
        let cfg = resolver.resolve("anything");
        assert_eq!(cfg.family, "monospace");
        assert_eq!(resolver.override_count(), 0);
    }

    #[test]
    fn test_font_resolver_override() {
        let mut resolver = TerminalFontResolver::new(TerminalFontConfig::default());
        let custom = TerminalFontConfig::new().with_family("Fira Code").with_size(16.0);
        resolver.add_override("Bash", custom);
        let cfg = resolver.resolve("Bash");
        assert_eq!(cfg.family, "Fira Code");
        assert!((cfg.size - 16.0).abs() < 0.1);
        // Default still returned for other profiles.
        let def = resolver.resolve("Zsh");
        assert_eq!(def.family, "monospace");
    }

    #[test]
    fn test_font_resolver_has_override() {
        let mut resolver = TerminalFontResolver::new(TerminalFontConfig::default());
        assert!(!resolver.has_override("Bash"));
        resolver.add_override("Bash", TerminalFontConfig::default());
        assert!(resolver.has_override("Bash"));
        assert!(!resolver.has_override("Fish"));
        assert_eq!(resolver.override_count(), 1);
    }

    #[test]
    fn terminalResizeHandler_new() {
        let s = TerminalResizeHandler::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn terminalResizeHandler_add_contains() {
        let mut s = TerminalResizeHandler::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn terminalResizeHandler_add_duplicate() {
        let mut s = TerminalResizeHandler::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn terminalResizeHandler_remove() {
        let mut s = TerminalResizeHandler::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn terminalResizeHandler_capacity() {
        let s = TerminalResizeHandler::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn terminalResizeHandler_search() {
        let mut s = TerminalResizeHandler::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn terminalResizeHandler_stats() {
        let mut s = TerminalResizeHandler::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn terminalSignalMapper_new() {
        let m = TerminalSignalMapper::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn terminalSignalMapper_add_find() {
        let mut m = TerminalSignalMapper::new();
        m.add(TerminalSignalMapperItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn terminalSignalMapper_priority_filter() {
        let mut m = TerminalSignalMapper::new();
        m.add(TerminalSignalMapperItem::new("a", "A").with_priority(TerminalSignalMapperPriority::High));
        m.add(TerminalSignalMapperItem::new("b", "B").with_priority(TerminalSignalMapperPriority::Low));
        m.add(TerminalSignalMapperItem::new("c", "C").with_priority(TerminalSignalMapperPriority::High));
        assert_eq!(m.by_priority(TerminalSignalMapperPriority::High).len(), 2);
    }

    #[test]
    fn terminalSignalMapper_remove() {
        let mut m = TerminalSignalMapper::new();
        m.add(TerminalSignalMapperItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn terminalSignalMapper_search() {
        let mut m = TerminalSignalMapper::new();
        m.add(TerminalSignalMapperItem::new("id1", "Hello World"));
        m.add(TerminalSignalMapperItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn terminalSignalMapper_total_weight() {
        let mut m = TerminalSignalMapper::new();
        m.add(TerminalSignalMapperItem::new("a", "A").with_priority(TerminalSignalMapperPriority::Critical));
        m.add(TerminalSignalMapperItem::new("b", "B").with_priority(TerminalSignalMapperPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn terminalSignalMapper_capacity_limit() {
        let mut m = TerminalSignalMapper::new().with_max_items(2);
        m.add(TerminalSignalMapperItem::new("1", "one"));
        m.add(TerminalSignalMapperItem::new("2", "two"));
        assert!(!m.add(TerminalSignalMapperItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn terminalSignalMapper_sorted_by_priority() {
        let mut m = TerminalSignalMapper::new();
        m.add(TerminalSignalMapperItem::new("lo", "Low").with_priority(TerminalSignalMapperPriority::Low));
        m.add(TerminalSignalMapperItem::new("hi", "High").with_priority(TerminalSignalMapperPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn terminalSignalMapper_item_metadata() {
        let mut item = TerminalSignalMapperItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn terminalResizeHandler_enabled_toggle() {
        let mut s = TerminalResizeHandler::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn terminalSignalMapper_priority_display() {
        assert_eq!(format!("{}", TerminalSignalMapperPriority::High), "high");
        assert_eq!(format!("{}", TerminalSignalMapperPriority::Low), "low");
    }


    // -- terminal_plat additional tests -------------------------------------------

    #[test]
    fn x_terminal_plat_capabilities_register_and_has() {
        let mut caps = XTerminalPlatCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_terminal_plat_capabilities_len() {
        let mut caps = XTerminalPlatCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_terminal_plat_capabilities_intersect() {
        let mut a = XTerminalPlatCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XTerminalPlatCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_terminal_plat_capabilities_diff() {
        let mut a = XTerminalPlatCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XTerminalPlatCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_terminal_plat_service_registry_basic() {
        let mut reg = XTerminalPlatServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_terminal_plat_service_registry_replace() {
        let mut reg = XTerminalPlatServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_terminal_plat_service_registry_remove() {
        let mut reg = XTerminalPlatServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_terminal_plat_service_registry_names() {
        let mut reg = XTerminalPlatServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_terminal_plat_sanitize_path_basic() {
        assert_eq!(x_terminal_plat_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_terminal_plat_sanitize_path_backslash() {
        assert_eq!(x_terminal_plat_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_terminal_plat_sanitize_path_single() {
        assert_eq!(x_terminal_plat_sanitize_path("/"), "/");
    }

    #[test]
    fn x_terminal_plat_capabilities_default() {
        let caps = XTerminalPlatCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_terminal_plat_capabilities_all() {
        let mut caps = XTerminalPlatCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    // -- terminal_plat extended domain tests ----------------------------------------

    #[test]
    fn y_terminal_plat_enum_index() {
        assert_eq!(YTerminalPlatTerminalColorScheme::Dark.index(), 0);
        assert_eq!(YTerminalPlatTerminalColorScheme::Light.index(), 1);
        assert_eq!(YTerminalPlatTerminalColorScheme::Solarized.index(), 2);
        assert_eq!(YTerminalPlatTerminalColorScheme::Monokai.index(), 3);
    }

    #[test]
    fn y_terminal_plat_enum_label() {
        assert_eq!(YTerminalPlatTerminalColorScheme::Dark.label(), "Dark");
        assert_eq!(YTerminalPlatTerminalColorScheme::Light.label(), "Light");
        assert_eq!(YTerminalPlatTerminalColorScheme::Solarized.label(), "Solarized");
        assert_eq!(YTerminalPlatTerminalColorScheme::Monokai.label(), "Monokai");
    }

    #[test]
    fn y_terminal_plat_enum_all() {
        let all = YTerminalPlatTerminalColorScheme::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_terminal_plat_enum_is_default() {
        assert!(YTerminalPlatTerminalColorScheme::Dark.is_default());
        assert!(!YTerminalPlatTerminalColorScheme::Monokai.is_default());
    }

    #[test]
    fn y_terminal_plat_enum_display() {
        assert_eq!(format!("{}", YTerminalPlatTerminalColorScheme::Dark), "Dark");
    }

    #[test]
    fn y_terminal_plat_struct_new() {
        let s = YTerminalPlatTerminalDimensions::new();
        let _ = s.summary();
    }

    #[test]
    fn y_terminal_plat_fingerprint_deterministic() {
        let h1 = y_terminal_plat_fingerprint("hello");
        let h2 = y_terminal_plat_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_terminal_plat_fingerprint("a"), y_terminal_plat_fingerprint("b"));
    }

    #[test]
    fn y_terminal_plat_truncate_short() {
        assert_eq!(y_terminal_plat_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_terminal_plat_truncate_long() {
        let r = y_terminal_plat_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_terminal_plat_normalize_key_basic() {
        assert_eq!(y_terminal_plat_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_terminal_plat_split_path_basic() {
        let parts = y_terminal_plat_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_terminal_plat_count_occurrences_basic() {
        assert_eq!(y_terminal_plat_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_terminal_plat_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_terminal_plat_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_terminal_plat_in_range_basic() {
        assert!(y_terminal_plat_in_range(5, 1, 10));
        assert!(y_terminal_plat_in_range(1, 1, 10));
        assert!(y_terminal_plat_in_range(10, 1, 10));
        assert!(!y_terminal_plat_in_range(0, 1, 10));
        assert!(!y_terminal_plat_in_range(11, 1, 10));
    }

    #[test]
    fn y_terminal_plat_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_terminal_plat_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_terminal_plat_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_terminal_plat_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- terminal_plat Z-extended tests -----------------------------------------------

    #[test]
    fn z_terminal_plat_priority_weight() {
        assert_eq!(ZTerminalPlatPriority::Idle.weight(), 0);
        assert_eq!(ZTerminalPlatPriority::Normal.weight(), 2);
        assert_eq!(ZTerminalPlatPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_terminal_plat_priority_label() {
        assert_eq!(ZTerminalPlatPriority::Low.label(), "low");
        assert_eq!(ZTerminalPlatPriority::High.label(), "high");
    }

    #[test]
    fn z_terminal_plat_priority_is_elevated() {
        assert!(!ZTerminalPlatPriority::Normal.is_elevated());
        assert!(ZTerminalPlatPriority::High.is_elevated());
        assert!(ZTerminalPlatPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_terminal_plat_priority_display() {
        assert_eq!(format!("{}", ZTerminalPlatPriority::Idle), "idle");
    }

    #[test]
    fn z_terminal_plat_priority_all_asc() {
        let all = ZTerminalPlatPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZTerminalPlatPriority::Idle);
        assert_eq!(all[4], ZTerminalPlatPriority::Realtime);
    }

    #[test]
    fn z_terminal_plat_struct_new() {
        let s = ZTerminalPlatTerminalFontMetrics::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_terminal_plat_struct_toggled_clone() {
        let s = ZTerminalPlatTerminalFontMetrics::new();
        let t = s.toggled_clone();
        let _ = t.baseline;
    }

    #[test]
    fn z_terminal_plat_rolling_hash_deterministic() {
        let h1 = z_terminal_plat_rolling_hash(b"test");
        let h2 = z_terminal_plat_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_terminal_plat_rolling_hash(b"a"), z_terminal_plat_rolling_hash(b"b"));
    }

    #[test]
    fn z_terminal_plat_pad_to_basic() {
        assert_eq!(z_terminal_plat_pad_to("hi", 5), "hi   ");
        assert_eq!(z_terminal_plat_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_terminal_plat_is_identifier_basic() {
        assert!(z_terminal_plat_is_identifier("foo_bar"));
        assert!(z_terminal_plat_is_identifier("abc123"));
        assert!(!z_terminal_plat_is_identifier(""));
        assert!(!z_terminal_plat_is_identifier("has space"));
    }

    #[test]
    fn z_terminal_plat_levenshtein_basic() {
        assert_eq!(z_terminal_plat_levenshtein("", ""), 0);
        assert_eq!(z_terminal_plat_levenshtein("abc", "abc"), 0);
        assert_eq!(z_terminal_plat_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_terminal_plat_unique_words_basic() {
        let w = z_terminal_plat_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_terminal_plat_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_terminal_plat_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_terminal_plat_common_prefix_basic() {
        assert_eq!(z_terminal_plat_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_terminal_plat_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_terminal_plat_struct_clear() {
        let mut s = ZTerminalPlatTerminalFontMetrics::new();
        s.char_widths.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_terminal_plat_rolling_hash_empty() {
        let h = z_terminal_plat_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_56_push_and_len() {
        let mut rb = super::XbRingBuffer56::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_56_overwrite() {
        let mut rb = super::XbRingBuffer56::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_56_get_out_of_bounds() {
        let rb = super::XbRingBuffer56::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_56_drain_all() {
        let mut rb = super::XbRingBuffer56::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_56_peek_front_back() {
        let mut rb = super::XbRingBuffer56::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_56_clear() {
        let mut rb = super::XbRingBuffer56::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_56_capacity() {
        let rb = super::XbRingBuffer56::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_56_basic() {
        let h = super::xb_fnv1a_56(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_56(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_56_different_inputs() {
        let h1 = super::xb_fnv1a_56(b"abc");
        let h2 = super::xb_fnv1a_56(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_56_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_56(&data);
        let dec = super::xb_rle_decode_56(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_56_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_56(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_56(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_56_values() {
        assert!((super::xb_clamp_56(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_56(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_56(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_56_values() {
        assert!((super::xb_lerp_56(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_56(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_56(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_56_wrap_around_twice() {
        let mut rb = super::XbRingBuffer56::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 176 ----

    #[test]
    fn xc_176_pool_new_empty() {
        let pool: super::Xc176Pool<i32> = super::Xc176Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_176_pool_release_acquire() {
        let mut pool = super::Xc176Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_176_pool_acquire_empty() {
        let mut pool: super::Xc176Pool<i32> = super::Xc176Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_176_pool_full() {
        let mut pool = super::Xc176Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_176_pool_drain() {
        let mut pool = super::Xc176Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_176_pool_stats() {
        let mut pool = super::Xc176Pool::new(8);
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
    fn xc_176_pool_clear() {
        let mut pool = super::Xc176Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_176_pool_shrink() {
        let mut pool = super::Xc176Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_176_pool_default() {
        let pool: super::Xc176Pool<String> = super::Xc176Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_176_pool_extend() {
        let mut pool = super::Xc176Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_176_pool_retain() {
        let mut pool = super::Xc176Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_176_scheduler_round_robin() {
        let mut sched = super::Xc176Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_176_scheduler_empty() {
        let mut sched = super::Xc176Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_176_scheduler_reset() {
        let mut sched = super::Xc176Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_176_scheduler_add_remove() {
        let mut sched = super::Xc176Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_176_scheduler_targets() {
        let sched = super::Xc176Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_176_hash_empty() {
        assert_eq!(super::xc_176_hash(b""), 5381);
    }

    #[test]
    fn xc_176_hash_data() {
        let h = super::xc_176_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_176_hash(b"hello"), h);
    }

    #[test]
    fn xc_176_reverse_str() {
        assert_eq!(super::xc_176_reverse("abc"), "cba");
        assert_eq!(super::xc_176_reverse(""), "");
    }


    #[test]
    fn xe_69_pipeline_empty() {
        let p = super::Xe69Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_69_pipeline_parse_stage() {
        let p = super::Xe69Pipeline::new()
            .add_parse(super::xe_69_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_69_pipeline_transform_double() {
        let p = super::Xe69Pipeline::new()
            .add_transform(super::xe_69_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_69_pipeline_validate_reverse() {
        let p = super::Xe69Pipeline::new()
            .add_validate(super::xe_69_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_69_pipeline_emit_filter() {
        let p = super::Xe69Pipeline::new()
            .add_emit(super::xe_69_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_69_pipeline_multi_stage() {
        let p = super::Xe69Pipeline::new()
            .add_parse(super::xe_69_pipeline_identity)
            .add_transform(super::xe_69_pipeline_double)
            .add_validate(super::xe_69_pipeline_reverse)
            .add_emit(super::xe_69_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_69_pipeline_error_propagation() {
        let p = super::Xe69Pipeline::new()
            .add_parse(super::xe_69_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe69Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_69_pipeline_compose() {
        let p1 = super::Xe69Pipeline::new()
            .add_parse(super::xe_69_pipeline_identity);
        let p2 = super::Xe69Pipeline::new()
            .add_transform(super::xe_69_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_69_pipeline_error_display() {
        let e = super::Xe69PipelineError {
            stage: super::Xe69Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_69_cache_put_get() {
        let mut c = super::Xe69Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_69_cache_miss() {
        let mut c: super::Xe69Cache<&str, i32> = super::Xe69Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_69_cache_ttl_expiry() {
        let mut c = super::Xe69Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_69_cache_evict() {
        let mut c = super::Xe69Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_69_cache_capacity() {
        let mut c = super::Xe69Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_69_cache_stats() {
        let mut c = super::Xe69Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_69_cache_clear() {
        let mut c = super::Xe69Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_67 graph tests ------------------------------------------------

    #[test]
    fn xg_67_graph_empty() {
        let g = super::Xg67Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_67_graph_add_node() {
        let mut g = super::Xg67Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_67_graph_add_edge() {
        let mut g = super::Xg67Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_67_graph_neighbors() {
        let mut g = super::Xg67Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_67_graph_has_path() {
        let mut g = super::Xg67Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_67_graph_self_path() {
        let g = super::Xg67Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_67_graph_topo_sort() {
        let mut g = super::Xg67Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_67_graph_cycle_detect_false() {
        let mut g = super::Xg67Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_67_graph_cycle_detect_true() {
        let mut g = super::Xg67Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_67 heap tests -------------------------------------------------

    #[test]
    fn xg_67_heap_empty() {
        let h: super::Xg67Heap<i32> = super::Xg67Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_67_heap_push_pop() {
        let mut h = super::Xg67Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_67_heap_peek() {
        let mut h = super::Xg67Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_67_heap_drain_sorted() {
        let mut h = super::Xg67Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_67_heap_merge() {
        let mut a = super::Xg67Heap::new();
        let mut b = super::Xg67Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_67_heap_default() {
        let h: super::Xg67Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_67_graph_default() {
        let g: super::Xg67Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh175_skip_insert_contains() {
        let mut sl = super::Xh175SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh175_skip_remove() {
        let mut sl = super::Xh175SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh175_skip_len() {
        let mut sl = super::Xh175SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh175_skip_range_query() {
        let mut sl = super::Xh175SkipList::xh_new(4);
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
    fn xh175_skip_floor_ceiling() {
        let mut sl = super::Xh175SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh175_skip_rank() {
        let mut sl = super::Xh175SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh175_skip_empty() {
        let sl = super::Xh175SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh175_skip_duplicates() {
        let mut sl = super::Xh175SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh175_bitset_set_test() {
        let mut bs = super::Xh175BitSet::xh_new(256);
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
    fn xh175_bitset_clear_count() {
        let mut bs = super::Xh175BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh175_bitset_and_or_xor() {
        let mut a = super::Xh175BitSet::xh_new(128);
        let mut b = super::Xh175BitSet::xh_new(128);
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
    fn xh175_bitset_iter_ones() {
        let mut bs = super::Xh175BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh175_bitset_first_last() {
        let mut bs = super::Xh175BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh175_bitset_empty() {
        let bs = super::Xh175BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi175_deque_push_pop_back() {
        let mut dq = super::Xi175Deque::xi_new(4);
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
    fn xi175_deque_push_pop_front() {
        let mut dq = super::Xi175Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi175_deque_mixed_ops() {
        let mut dq = super::Xi175Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi175_deque_get_and_split() {
        let mut dq = super::Xi175Deque::xi_new(8);
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
    fn xi175_deque_rotate_left() {
        let mut dq = super::Xi175Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi175_deque_rotate_right() {
        let mut dq = super::Xi175Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi175_deque_grow() {
        let mut dq = super::Xi175Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi175_deque_empty() {
        let dq = super::Xi175Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi175_interval_tree_insert_query() {
        let mut tree = super::Xi175IntervalTree::xi_new();
        tree.xi_insert(super::Xi175Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi175Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi175Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi175_interval_tree_overlap() {
        let mut tree = super::Xi175IntervalTree::xi_new();
        tree.xi_insert(super::Xi175Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi175Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi175Interval::xi_new(12, 20));
        let q = super::Xi175Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi175_interval_tree_remove() {
        let mut tree = super::Xi175IntervalTree::xi_new();
        tree.xi_insert(super::Xi175Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi175Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi175_interval_tree_gaps() {
        let mut tree = super::Xi175IntervalTree::xi_new();
        tree.xi_insert(super::Xi175Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi175Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi175Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi175Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi175Interval::xi_new(8, 10));
    }

    #[test]
    fn xi175_interval_tree_merge() {
        let mut tree = super::Xi175IntervalTree::xi_new();
        tree.xi_insert(super::Xi175Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi175Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi175Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi175Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi175Interval::xi_new(10, 15));
    }

    #[test]
    fn xi175_interval_tree_all() {
        let mut tree = super::Xi175IntervalTree::xi_new();
        tree.xi_insert(super::Xi175Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi175Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi175_interval_tree_empty() {
        let tree = super::Xi175IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi175_interval_tree_contains_point() {
        let iv = super::Xi175Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 175) ---

    #[test]
    fn xj_175_uf_make_and_find() {
        let mut uf = super::Xj175UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_175_uf_union_connected() {
        let mut uf = super::Xj175UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_175_uf_component_count() {
        let mut uf = super::Xj175UnionFind::xj_new();
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
    fn xj_175_uf_component_size() {
        let mut uf = super::Xj175UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_175_uf_largest_component() {
        let mut uf = super::Xj175UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_175_uf_many_elements() {
        let mut uf = super::Xj175UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_175_uf_separate_components() {
        let mut uf = super::Xj175UnionFind::xj_new();
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
    fn xj_175_uf_path_compression() {
        let mut uf = super::Xj175UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_175_bt_insert_get() {
        let mut bt = super::Xj175BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_175_bt_contains_len() {
        let mut bt = super::Xj175BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_175_bt_replace() {
        let mut bt = super::Xj175BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_175_bt_remove() {
        let mut bt = super::Xj175BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_175_bt_keys_values() {
        let mut bt = super::Xj175BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_175_bt_range() {
        let mut bt = super::Xj175BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_175_bt_min_max() {
        let mut bt = super::Xj175BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_175_bt_many_inserts() {
        let mut bt = super::Xj175BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
