//! Ext API: Terminal.
//!
//! RPC bridge between the extension host and the main thread for terminal management.

use std::fmt;
use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_terminal";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalMessage {
    CreateTerminal {
        options: TerminalOptions,
    },
    DisposeTerminal {
        terminal_id: String,
    },
    SendText {
        terminal_id: String,
        text: String,
        add_newline: bool,
    },
    ShowTerminal {
        terminal_id: String,
        preserve_focus: bool,
    },
    RegisterLinkProvider {
        id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalOptions {
    pub name: Option<String>,
    pub shell_path: Option<String>,
    pub shell_args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub hide_from_user: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Terminal {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub process_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalLink {
    pub start_index: u32,
    pub length: u32,
    pub tooltip: Option<String>,
}

// ── Bridge ──

pub struct TerminalBridge {
    terminals: Vec<Terminal>,
    next_id: u64,
}

impl TerminalBridge {
    pub fn new() -> Self {
        Self {
            terminals: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_terminal(&mut self, options: &TerminalOptions) -> String {
        let id = format!("term-{}", self.next_id);
        self.next_id += 1;
        let name = options
            .name
            .clone()
            .unwrap_or_else(|| format!("Terminal {}", self.terminals.len() + 1));
        self.terminals.push(Terminal {
            id: id.clone(),
            name,
            is_active: true,
            process_id: None,
        });
        id
    }

    pub fn dispose_terminal(&mut self, terminal_id: &str) -> bool {
        let before = self.terminals.len();
        self.terminals.retain(|t| t.id != terminal_id);
        self.terminals.len() < before
    }

    pub fn get_terminal(&self, id: &str) -> Option<&Terminal> {
        self.terminals.iter().find(|t| t.id == id)
    }

    pub fn active_terminals(&self) -> Vec<&Terminal> {
        self.terminals.iter().filter(|t| t.is_active).collect()
    }

    pub fn handle_message(&mut self, msg: &TerminalMessage) -> serde_json::Value {
        match msg {
            TerminalMessage::CreateTerminal { options } => {
                let id = self.create_terminal(options);
                serde_json::json!({"terminalId": id})
            }
            TerminalMessage::DisposeTerminal { terminal_id } => {
                let ok = self.dispose_terminal(terminal_id);
                serde_json::json!({"disposed": ok})
            }
            TerminalMessage::SendText {
                terminal_id,
                text,
                add_newline,
            } => {
                let found = self.get_terminal(terminal_id).is_some();
                serde_json::json!({"sent": found, "text": text, "newline": add_newline})
            }
            TerminalMessage::ShowTerminal {
                terminal_id,
                preserve_focus,
            } => {
                let found = self.get_terminal(terminal_id).is_some();
                serde_json::json!({"shown": found, "preserveFocus": preserve_focus})
            }
            TerminalMessage::RegisterLinkProvider { id } => {
                serde_json::json!({"registered": id})
            }
        }
    }
}

impl Default for TerminalBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error Handling ──

/// Errors that can occur during terminal operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalError {
    /// The referenced terminal does not exist.
    NotFound(String),
    /// A terminal with this name already exists.
    DuplicateName(String),
    /// The shell path is invalid or not executable.
    InvalidShellPath(String),
    /// Validation failed for terminal options.
    ValidationError(String),
    /// An environment variable key is empty.
    InvalidEnvVar(String),
}

impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminalError::NotFound(id) => write!(f, "terminal not found: {id}"),
            TerminalError::DuplicateName(name) => {
                write!(f, "duplicate terminal name: {name}")
            }
            TerminalError::InvalidShellPath(path) => {
                write!(f, "invalid shell path: {path}")
            }
            TerminalError::ValidationError(msg) => write!(f, "validation error: {msg}"),
            TerminalError::InvalidEnvVar(detail) => {
                write!(f, "invalid environment variable: {detail}")
            }
        }
    }
}

impl std::error::Error for TerminalError {}

// ── Builder ──

/// Builder for constructing [`TerminalOptions`] with validation.
#[derive(Debug, Clone, Default)]
pub struct TerminalOptionsBuilder {
    name: Option<String>,
    shell_path: Option<String>,
    shell_args: Vec<String>,
    cwd: Option<String>,
    env: Vec<(String, String)>,
    hide_from_user: bool,
}

impl TerminalOptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn shell_path(mut self, path: impl Into<String>) -> Self {
        self.shell_path = Some(path.into());
        self
    }

    pub fn shell_args(mut self, args: Vec<String>) -> Self {
        self.shell_args = args;
        self
    }

    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn hide_from_user(mut self, hide: bool) -> Self {
        self.hide_from_user = hide;
        self
    }

    /// Validate and build the [`TerminalOptions`].
    pub fn build(self) -> Result<TerminalOptions, TerminalError> {
        if let Some(ref path) = self.shell_path {
            if path.trim().is_empty() {
                return Err(TerminalError::InvalidShellPath(path.clone()));
            }
        }
        for (key, _) in &self.env {
            if key.trim().is_empty() {
                return Err(TerminalError::InvalidEnvVar(
                    "environment variable key is empty".into(),
                ));
            }
        }
        Ok(TerminalOptions {
            name: self.name,
            shell_path: self.shell_path,
            shell_args: self.shell_args,
            cwd: self.cwd,
            env: self.env,
            hide_from_user: self.hide_from_user,
        })
    }
}

// ── TerminalOptions helpers ──

impl TerminalOptions {
    /// Start building terminal options.
    pub fn builder() -> TerminalOptionsBuilder {
        TerminalOptionsBuilder::new()
    }

    /// Return the resolved display name (falls back to "Terminal").
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or("Terminal")
    }

    /// Merge another set of environment variables, appending without duplicating keys.
    pub fn merge_env(&mut self, extra: &[(String, String)]) {
        for (key, value) in extra {
            if !self.env.iter().any(|(k, _)| k == key) {
                self.env.push((key.clone(), value.clone()));
            }
        }
    }

    /// Look up an environment variable by key.
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.env
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

// ── Terminal helpers ──

impl Terminal {
    /// Create a new terminal instance with the given id and name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            is_active: true,
            process_id: None,
        }
    }
}

impl std::fmt::Display for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.is_active { "active" } else { "inactive" };
        write!(f, "[{}] {} ({})", self.id, self.name, status)
    }
}

// ── TerminalLink helpers ──

impl TerminalLink {
    /// Create a new terminal link spanning `start_index..start_index+length`.
    pub fn new(start_index: u32, length: u32) -> Self {
        Self {
            start_index,
            length,
            tooltip: None,
        }
    }

    /// Set the tooltip text.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Exclusive end index of this link.
    pub fn end_index(&self) -> u32 {
        self.start_index + self.length
    }

    /// Returns true when two links overlap.
    pub fn overlaps(&self, other: &TerminalLink) -> bool {
        self.start_index < other.end_index() && other.start_index < self.end_index()
    }
}

impl std::fmt::Display for TerminalLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.tooltip {
            Some(tip) => write!(f, "link[{}..{}]: {}", self.start_index, self.end_index(), tip),
            None => write!(f, "link[{}..{}]", self.start_index, self.end_index()),
        }
    }
}

// ── Extended TerminalBridge API ──

impl TerminalBridge {
    /// Total number of managed terminals (active + inactive).
    pub fn terminal_count(&self) -> usize {
        self.terminals.len()
    }

    /// Rename a terminal. Returns an error if the terminal is not found.
    pub fn rename_terminal(
        &mut self,
        terminal_id: &str,
        new_name: &str,
    ) -> Result<(), TerminalError> {
        let terminal = self
            .terminals
            .iter_mut()
            .find(|t| t.id == terminal_id)
            .ok_or_else(|| TerminalError::NotFound(terminal_id.into()))?;
        terminal.name = new_name.to_string();
        Ok(())
    }

    /// Set a terminal's active state. Returns an error if not found.
    pub fn set_active(
        &mut self,
        terminal_id: &str,
        active: bool,
    ) -> Result<(), TerminalError> {
        let terminal = self
            .terminals
            .iter_mut()
            .find(|t| t.id == terminal_id)
            .ok_or_else(|| TerminalError::NotFound(terminal_id.into()))?;
        terminal.is_active = active;
        Ok(())
    }

    /// Assign a process id to a terminal. Returns an error if not found.
    pub fn set_process_id(
        &mut self,
        terminal_id: &str,
        pid: u32,
    ) -> Result<(), TerminalError> {
        let terminal = self
            .terminals
            .iter_mut()
            .find(|t| t.id == terminal_id)
            .ok_or_else(|| TerminalError::NotFound(terminal_id.into()))?;
        terminal.process_id = Some(pid);
        Ok(())
    }

    /// Find a terminal by its display name.
    pub fn find_by_name(&self, name: &str) -> Option<&Terminal> {
        self.terminals.iter().find(|t| t.name == name)
    }

    /// Dispose all inactive terminals, returning the number removed.
    pub fn dispose_inactive(&mut self) -> usize {
        let before = self.terminals.len();
        self.terminals.retain(|t| t.is_active);
        before - self.terminals.len()
    }

    /// Create a terminal using validated builder options.
    pub fn create_terminal_validated(
        &mut self,
        options: &TerminalOptions,
    ) -> Result<String, TerminalError> {
        if let Some(ref name) = options.name {
            if self.find_by_name(name).is_some() {
                return Err(TerminalError::DuplicateName(name.clone()));
            }
        }
        Ok(self.create_terminal(options))
    }
}

impl std::fmt::Display for TerminalBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TerminalBridge({} terminals, {} active)",
            self.terminals.len(),
            self.active_terminals().len(),
        )
    }
}

/// Initialize the terminal extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-terminal operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtTerminalStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtTerminalStats {
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
    pub fn merge(&mut self, other: &ExtTerminalStats) {
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

impl Default for ExtTerminalStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtTerminalStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtTerminalStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-terminal.
#[derive(Debug, Clone)]
pub struct ExtTerminalValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtTerminalValidator {
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

impl Default for ExtTerminalValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_opts() -> TerminalOptions {
        TerminalOptions {
            name: Some("Test".into()),
            shell_path: Some("/bin/bash".into()),
            shell_args: vec![],
            cwd: None,
            env: vec![],
            hide_from_user: false,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = TerminalMessage::CreateTerminal {
            options: test_opts(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: TerminalMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn terminal_link_serialization() {
        let link = TerminalLink {
            start_index: 5,
            length: 10,
            tooltip: Some("Click to open".into()),
        };
        let json = serde_json::to_string(&link).unwrap();
        let back: TerminalLink = serde_json::from_str(&json).unwrap();
        assert_eq!(link, back);
    }

    #[test]
    fn bridge_create_and_dispose() {
        let mut bridge = TerminalBridge::new();
        let id = bridge.create_terminal(&test_opts());
        assert!(bridge.get_terminal(&id).is_some());
        assert!(bridge.dispose_terminal(&id));
        assert!(bridge.get_terminal(&id).is_none());
    }

    #[test]
    fn bridge_active_terminals() {
        let mut bridge = TerminalBridge::new();
        bridge.create_terminal(&test_opts());
        bridge.create_terminal(&test_opts());
        assert_eq!(bridge.active_terminals().len(), 2);
    }

    #[test]
    fn bridge_dispose_unknown() {
        let mut bridge = TerminalBridge::new();
        assert!(!bridge.dispose_terminal("nope"));
    }

    // ── Additional tests ──

    #[test]
    fn builder_basic_usage() {
        let opts = TerminalOptions::builder()
            .name("dev-shell")
            .shell_path("/bin/zsh")
            .cwd("/tmp")
            .env_var("TERM", "xterm-256color")
            .build()
            .unwrap();
        assert_eq!(opts.display_name(), "dev-shell");
        assert_eq!(opts.get_env("TERM"), Some("xterm-256color"));
    }

    #[test]
    fn builder_rejects_empty_shell_path() {
        let err = TerminalOptions::builder()
            .shell_path("  ")
            .build()
            .unwrap_err();
        assert_eq!(err, TerminalError::InvalidShellPath("  ".into()));
    }

    #[test]
    fn builder_rejects_empty_env_key() {
        let err = TerminalOptions::builder()
            .env_var("", "value")
            .build()
            .unwrap_err();
        matches!(err, TerminalError::InvalidEnvVar(_));
    }

    #[test]
    fn options_merge_env_no_duplicates() {
        let mut opts = test_opts();
        opts.env.push(("A".into(), "1".into()));
        opts.merge_env(&[("A".into(), "2".into()), ("B".into(), "3".into())]);
        assert_eq!(opts.get_env("A"), Some("1")); // kept original
        assert_eq!(opts.get_env("B"), Some("3")); // added new
    }

    #[test]
    fn terminal_display() {
        let t = Terminal::new("t-1", "Shell");
        assert_eq!(t.to_string(), "[t-1] Shell (active)");
    }

    #[test]
    fn terminal_link_overlap_detection() {
        let a = TerminalLink::new(5, 10); // 5..15
        let b = TerminalLink::new(10, 5); // 10..15
        let c = TerminalLink::new(15, 3); // 15..18
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c)); // adjacent, not overlapping
    }

    #[test]
    fn terminal_link_display() {
        let link = TerminalLink::new(0, 8).with_tooltip("open file");
        assert_eq!(link.to_string(), "link[0..8]: open file");
        assert_eq!(link.end_index(), 8);
    }

    #[test]
    fn bridge_rename_terminal() {
        let mut bridge = TerminalBridge::new();
        let id = bridge.create_terminal(&test_opts());
        bridge.rename_terminal(&id, "Renamed").unwrap();
        assert_eq!(bridge.get_terminal(&id).unwrap().name, "Renamed");
    }

    #[test]
    fn bridge_rename_missing_terminal_errors() {
        let mut bridge = TerminalBridge::new();
        let err = bridge.rename_terminal("ghost", "X").unwrap_err();
        assert_eq!(err, TerminalError::NotFound("ghost".into()));
    }

    #[test]
    fn bridge_set_active_and_dispose_inactive() {
        let mut bridge = TerminalBridge::new();
        let id1 = bridge.create_terminal(&test_opts());
        let _id2 = bridge.create_terminal(&test_opts());
        bridge.set_active(&id1, false).unwrap();
        assert_eq!(bridge.active_terminals().len(), 1);
        let removed = bridge.dispose_inactive();
        assert_eq!(removed, 1);
        assert_eq!(bridge.terminal_count(), 1);
    }

    #[test]
    fn bridge_create_validated_rejects_duplicate_name() {
        let mut bridge = TerminalBridge::new();
        let opts = test_opts(); // name = Some("Test")
        bridge.create_terminal_validated(&opts).unwrap();
        let err = bridge.create_terminal_validated(&opts).unwrap_err();
        assert_eq!(err, TerminalError::DuplicateName("Test".into()));
    }

    #[test]
    fn bridge_set_process_id() {
        let mut bridge = TerminalBridge::new();
        let id = bridge.create_terminal(&test_opts());
        bridge.set_process_id(&id, 42).unwrap();
        assert_eq!(bridge.get_terminal(&id).unwrap().process_id, Some(42));
    }

    #[test]
    fn bridge_find_by_name() {
        let mut bridge = TerminalBridge::new();
        bridge.create_terminal(&test_opts());
        assert!(bridge.find_by_name("Test").is_some());
        assert!(bridge.find_by_name("Missing").is_none());
    }

    #[test]
    fn bridge_display_format() {
        let mut bridge = TerminalBridge::new();
        bridge.create_terminal(&test_opts());
        assert_eq!(bridge.to_string(), "TerminalBridge(1 terminals, 1 active)");
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(
            TerminalError::NotFound("x".into()).to_string(),
            "terminal not found: x"
        );
        assert_eq!(
            TerminalError::DuplicateName("y".into()).to_string(),
            "duplicate terminal name: y"
        );
    }

    #[test]
    fn handle_message_send_text() {
        let mut bridge = TerminalBridge::new();
        let id = bridge.create_terminal(&test_opts());
        let msg = TerminalMessage::SendText {
            terminal_id: id,
            text: "ls\n".into(),
            add_newline: false,
        };
        let result = bridge.handle_message(&msg);
        assert_eq!(result["sent"], true);
    }

    #[test]
    fn ext_terminal_stats_new_defaults() {
        let stats = ExtTerminalStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_terminal_stats_record_success() {
        let mut stats = ExtTerminalStats::new();
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
    fn ext_terminal_stats_record_failure() {
        let mut stats = ExtTerminalStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_terminal_stats_reset() {
        let mut stats = ExtTerminalStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_terminal_stats_merge() {
        let mut a = ExtTerminalStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtTerminalStats::new();
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
    fn ext_terminal_stats_display() {
        let mut stats = ExtTerminalStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_terminal_stats_default() {
        let stats = ExtTerminalStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_terminal_validator_accepts_valid_name() {
        let v = ExtTerminalValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_terminal_validator_rejects_empty() {
        let v = ExtTerminalValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_terminal_validator_rejects_too_long() {
        let v = ExtTerminalValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_terminal_validator_forbidden_prefix() {
        let v = ExtTerminalValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_terminal_validator_allowed_chars() {
        let v = ExtTerminalValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_terminal_validator_range() {
        let v = ExtTerminalValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_terminal_sanitize_removes_control() {
        let result = ExtTerminalValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_terminal_truncate_short_string() {
        assert_eq!(ExtTerminalValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_terminal_truncate_long_string() {
        let result = ExtTerminalValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_terminal_is_ascii_printable() {
        assert!(ExtTerminalValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtTerminalValidator::is_ascii_printable("Hello\x00World"));
    }
}
