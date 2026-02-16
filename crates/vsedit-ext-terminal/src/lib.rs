//! Ext API: Terminal.
//!
//! RPC bridge between the extension host and the main thread for terminal management.

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
}
