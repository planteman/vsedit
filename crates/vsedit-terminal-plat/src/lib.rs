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
}
