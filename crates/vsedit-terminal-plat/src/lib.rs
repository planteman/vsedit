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

}
