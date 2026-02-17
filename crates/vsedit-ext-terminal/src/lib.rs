//! Ext API: Terminal.
//!
//! RPC bridge between the extension host and the main thread for terminal management.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// TerminalProfileContribution – extension-provided terminal profiles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerminalProfileContribution {
    pub id: String,
    pub title: String,
    pub shell_path: String,
    pub shell_args: Vec<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub env: HashMap<String, String>,
}

impl TerminalProfileContribution {
    pub fn new(id: &str, title: &str, shell_path: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            shell_path: shell_path.to_string(),
            shell_args: Vec::new(),
            icon: None,
            color: None,
            env: HashMap::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.shell_args = args;
        self
    }

    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    pub fn with_color(mut self, color: &str) -> Self {
        self.color = Some(color.to_string());
        self
    }

    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }

    /// Converts this profile contribution into a [`TerminalOptions`].
    pub fn to_terminal_options(&self) -> TerminalOptions {
        TerminalOptions {
            name: Some(self.title.clone()),
            shell_path: Some(self.shell_path.clone()),
            shell_args: self.shell_args.clone(),
            cwd: None,
            env: self.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            hide_from_user: false,
        }
    }
}

// ---------------------------------------------------------------------------
// terminal_env_from_extension – env injection helper
// ---------------------------------------------------------------------------

/// Builds an enriched environment map for an extension-spawned terminal.
///
/// * Keys that do not already start with `VSEDIT_EXT_` are prefixed.
/// * The key `VSEDIT_EXT_ID` is always set to `ext_id`.
pub fn terminal_env_from_extension(
    ext_id: &str,
    env_pairs: &[(String, String)],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, value) in env_pairs {
        let prefixed = if key.starts_with("VSEDIT_EXT_") {
            key.clone()
        } else {
            format!("VSEDIT_EXT_{key}")
        };
        map.insert(prefixed, value.clone());
    }
    map.insert("VSEDIT_EXT_ID".to_string(), ext_id.to_string());
    map
}

// ---------------------------------------------------------------------------
// TerminalLinkDetector – regex-free link detection in terminal output
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct TerminalLinkDetector;

impl TerminalLinkDetector {
    pub fn new() -> Self {
        Self
    }

    /// Detects URLs (`http://` / `https://`) in `text`.
    pub fn detect_links(text: &str) -> Vec<TerminalLink> {
        let mut links = Vec::new();
        for prefix in &["https://", "http://"] {
            let mut start = 0;
            while let Some(pos) = text[start..].find(prefix) {
                let abs = start + pos;
                let end = text[abs..]
                    .find(|c: char| c.is_whitespace() || c == '>' || c == '"' || c == '\'')
                    .map_or(text.len(), |e| abs + e);
                let length = end - abs;
                if length > prefix.len() {
                    links.push(TerminalLink::new(abs as u32, length as u32));
                }
                start = end;
            }
        }
        links
    }

    /// Detects file-path references matching `path:line` or `path:line:col`.
    pub fn detect_file_links(text: &str) -> Vec<TerminalLink> {
        let mut links = Vec::new();
        let mut i = 0;
        let bytes = text.as_bytes();
        while i < bytes.len() {
            // Look for a `/` or `./` that starts a file path.
            if bytes[i] == b'/' || (bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b'/') {
                let path_start = i;
                // Advance past non-whitespace to find the colon-separated suffix.
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                let segment = &text[path_start..i];
                // Must contain at least one `:` followed by a digit.
                if let Some(colon) = segment.rfind(':') {
                    // Try to find the *first* colon followed by a digit.
                    let first_colon = segment.find(':').unwrap_or(colon);
                    if first_colon + 1 < segment.len()
                        && segment.as_bytes()[first_colon + 1].is_ascii_digit()
                    {
                        links.push(TerminalLink::new(path_start as u32, segment.len() as u32));
                    }
                }
            } else {
                i += 1;
            }
        }
        links
    }
}

// ---------------------------------------------------------------------------
// TerminalProfileRegistry – manages contributed profiles
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct TerminalProfileRegistry {
    profiles: Vec<TerminalProfileContribution>,
}

impl TerminalProfileRegistry {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    pub fn register(&mut self, profile: TerminalProfileContribution) {
        self.profiles.push(profile);
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.id != id);
        self.profiles.len() != before
    }

    pub fn get(&self, id: &str) -> Option<&TerminalProfileContribution> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn all(&self) -> Vec<&TerminalProfileContribution> {
        self.profiles.iter().collect()
    }

    pub fn count(&self) -> usize {
        self.profiles.len()
    }

    /// Returns the first registered profile, if any.
    pub fn default_profile(&self) -> Option<&TerminalProfileContribution> {
        self.profiles.first()
    }
}

// ---------------------------------------------------------------------------
// TerminalOutput – buffered terminal output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TerminalOutput {
    terminal_id: String,
    buffer: String,
}

impl TerminalOutput {
    pub fn new(terminal_id: &str) -> Self {
        Self {
            terminal_id: terminal_id.to_string(),
            buffer: String::new(),
        }
    }

    pub fn append(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    pub fn line_count(&self) -> usize {
        if self.buffer.is_empty() {
            return 0;
        }
        self.buffer.lines().count()
    }

    pub fn last_n_lines(&self, n: usize) -> Vec<&str> {
        let lines: Vec<&str> = self.buffer.lines().collect();
        let skip = lines.len().saturating_sub(n);
        lines[skip..].to_vec()
    }

    pub fn total_bytes(&self) -> usize {
        self.buffer.len()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn terminal_id(&self) -> &str {
        &self.terminal_id
    }
}

// ---------------------------------------------------------------------------
// Terminal command parsing
// ---------------------------------------------------------------------------

/// A parsed terminal command with its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub program: String,
    pub args: Vec<String>,
    pub background: bool,
}

impl fmt::Display for ParsedCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = if self.background { " &" } else { "" };
        if self.args.is_empty() {
            write!(f, "{}{}", self.program, suffix)
        } else {
            write!(f, "{} {}{}", self.program, self.args.join(" "), suffix)
        }
    }
}

/// Parse a simple command line string into program and arguments.
/// Supports double-quoted arguments and background `&` suffix.
pub fn parse_command_line(input: &str) -> Option<ParsedCommand> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (line, background) = if trimmed.ends_with('&') {
        (trimmed[..trimmed.len() - 1].trim(), true)
    } else {
        (trimmed, false)
    };

    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if tokens.is_empty() {
        return None;
    }
    let program = tokens.remove(0);
    Some(ParsedCommand {
        program,
        args: tokens,
        background,
    })
}

// ---------------------------------------------------------------------------
// ANSI escape sequence stripping
// ---------------------------------------------------------------------------

/// Strip ANSI escape sequences from terminal output text.
pub fn strip_ansi_escapes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // CSI sequence: ESC [ ... final_byte
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // consume until we hit a letter (0x40–0x7E)
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c.is_ascii_alphabetic() || c == '~' {
                        break;
                    }
                }
            }
            // else: bare ESC, skip it
        } else {
            output.push(ch);
        }
    }
    output
}

// ---------------------------------------------------------------------------
// Terminal session state
// ---------------------------------------------------------------------------

/// Tracks the state of an active terminal session including working directory
/// and environment variable overrides.
#[derive(Debug, Clone)]
pub struct TerminalSessionState {
    pub terminal_id: String,
    pub cwd: String,
    pub env_overrides: Vec<(String, String)>,
    pub exit_code: Option<i32>,
    pub command_count: u32,
}

impl TerminalSessionState {
    /// Create a new session state for the given terminal.
    pub fn new(terminal_id: &str, initial_cwd: &str) -> Self {
        Self {
            terminal_id: terminal_id.to_string(),
            cwd: initial_cwd.to_string(),
            env_overrides: Vec::new(),
            exit_code: None,
            command_count: 0,
        }
    }

    /// Record a command execution.
    pub fn record_command(&mut self, exit_code: i32) {
        self.command_count += 1;
        self.exit_code = Some(exit_code);
    }

    /// Change the working directory.
    pub fn set_cwd(&mut self, cwd: &str) {
        self.cwd = cwd.to_string();
    }

    /// Set an environment variable override.
    pub fn set_env(&mut self, key: &str, value: &str) {
        if let Some(entry) = self.env_overrides.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value.to_string();
        } else {
            self.env_overrides.push((key.to_string(), value.to_string()));
        }
    }

    /// Get an environment variable override, if set.
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.env_overrides
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Returns true if the last command succeeded (exit code 0).
    pub fn last_command_succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Returns true if any commands have been executed.
    pub fn has_activity(&self) -> bool {
        self.command_count > 0
    }

    /// Remove a specific environment variable override.
    pub fn remove_env(&mut self, key: &str) -> bool {
        let before = self.env_overrides.len();
        self.env_overrides.retain(|(k, _)| k != key);
        self.env_overrides.len() != before
    }

    /// Return the number of environment variable overrides.
    pub fn env_count(&self) -> usize {
        self.env_overrides.len()
    }

    /// Reset the session state to its initial values (preserving the ID).
    pub fn reset(&mut self) {
        self.env_overrides.clear();
        self.exit_code = None;
        self.command_count = 0;
    }
}

impl TerminalOutput {
    /// Return all lines as a vector.
    pub fn lines(&self) -> Vec<&str> {
        if self.buffer.is_empty() {
            Vec::new()
        } else {
            self.buffer.lines().collect()
        }
    }

    /// Return true if the output contains the given substring.
    pub fn contains(&self, needle: &str) -> bool {
        self.buffer.contains(needle)
    }

    /// Return the first N lines from the buffer.
    pub fn first_n_lines(&self, n: usize) -> Vec<&str> {
        self.buffer.lines().take(n).collect()
    }
}

impl TerminalBridge {
    /// Return IDs of all terminals.
    pub fn terminal_ids(&self) -> Vec<&str> {
        self.terminals.iter().map(|t| t.id.as_str()).collect()
    }

    /// Return true if a terminal with the given ID exists.
    pub fn has_terminal(&self, id: &str) -> bool {
        self.terminals.iter().any(|t| t.id == id)
    }

    /// Return terminals filtered by active status.
    pub fn terminals_by_active(&self, active: bool) -> Vec<&Terminal> {
        self.terminals.iter().filter(|t| t.is_active == active).collect()
    }
}

impl TerminalProfileContribution {
    /// Returns true if the profile has a custom icon set.
    pub fn has_icon(&self) -> bool {
        self.icon.is_some()
    }

    /// Returns the number of environment variables configured.
    pub fn env_count(&self) -> usize {
        self.env.len()
    }
}


// ---------------------------------------------------------------------------
// TerminalOutput – search and filtering helpers
// ---------------------------------------------------------------------------

impl TerminalOutput {
    /// Return lines matching a substring filter.
    pub fn grep(&self, pattern: &str) -> Vec<&str> {
        self.buffer
            .lines()
            .filter(|line| line.contains(pattern))
            .collect()
    }

    /// Return the byte offset of the first occurrence of `needle`, if any.
    pub fn find_offset(&self, needle: &str) -> Option<usize> {
        self.buffer.find(needle)
    }

    /// Return true if the buffer is non-empty and its last line matches `suffix`.
    pub fn last_line_ends_with(&self, suffix: &str) -> bool {
        self.buffer.lines().last().map_or(false, |l| l.ends_with(suffix))
    }

    /// Count occurrences of `needle` in the output buffer.
    pub fn count_occurrences(&self, needle: &str) -> usize {
        self.buffer.matches(needle).count()
    }

    /// Split the buffer into two halves at the midpoint line.
    pub fn split_at_midpoint(&self) -> (Vec<&str>, Vec<&str>) {
        let lines: Vec<&str> = self.buffer.lines().collect();
        let mid = lines.len() / 2;
        (lines[..mid].to_vec(), lines[mid..].to_vec())
    }
}

// ---------------------------------------------------------------------------
// TerminalProfileRegistry – query helpers
// ---------------------------------------------------------------------------

impl TerminalProfileRegistry {
    /// Return profiles whose shell path contains the given substring.
    pub fn find_by_shell(&self, shell_substr: &str) -> Vec<&TerminalProfileContribution> {
        self.profiles
            .iter()
            .filter(|p| p.shell_path.contains(shell_substr))
            .collect()
    }

    /// Return all unique shell paths across registered profiles.
    pub fn unique_shells(&self) -> Vec<&str> {
        let mut shells: Vec<&str> = self.profiles.iter().map(|p| p.shell_path.as_str()).collect();
        shells.sort();
        shells.dedup();
        shells
    }

    /// Return profile IDs as a sorted vector.
    pub fn sorted_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.profiles.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids
    }
}

// ---------------------------------------------------------------------------
// TerminalBridge – batch and query helpers
// ---------------------------------------------------------------------------

impl TerminalBridge {
    /// Return names of all terminals, sorted alphabetically.
    pub fn sorted_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.terminals.iter().map(|t| t.name.as_str()).collect();
        names.sort();
        names
    }

    /// Dispose all terminals, returning how many were removed.
    pub fn dispose_all(&mut self) -> usize {
        let count = self.terminals.len();
        self.terminals.clear();
        count
    }

    /// Return terminals whose name contains the given substring (case-insensitive).
    pub fn search_by_name(&self, query: &str) -> Vec<&Terminal> {
        let q = query.to_lowercase();
        self.terminals
            .iter()
            .filter(|t| t.name.to_lowercase().contains(&q))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ParsedCommand helpers
// ---------------------------------------------------------------------------

impl ParsedCommand {
    /// Returns the total number of tokens (program + arguments).
    pub fn token_count(&self) -> usize {
        1 + self.args.len()
    }

    /// Returns true if the command has no arguments.
    pub fn is_simple(&self) -> bool {
        self.args.is_empty()
    }

    /// Returns the full command as a single string with arguments joined by spaces.
    pub fn to_command_string(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }

    /// Returns a new `ParsedCommand` with the given additional argument appended.
    pub fn with_arg(&self, arg: impl Into<String>) -> Self {
        let mut new_args = self.args.clone();
        new_args.push(arg.into());
        Self {
            program: self.program.clone(),
            args: new_args,
            background: self.background,
        }
    }
}


// ---------------------------------------------------------------------------
// TerminalLinkProvider – clickable URL detection
// ---------------------------------------------------------------------------

/// A detected link in terminal output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedTerminalLink {
    pub url: String,
    pub line: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub link_type: TerminalLinkType,
}

/// Type of link detected in terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalLinkType {
    Url,
    FilePath,
    Search,
}

impl fmt::Display for TerminalLinkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url => write!(f, "URL"),
            Self::FilePath => write!(f, "File"),
            Self::Search => write!(f, "Search"),
        }
    }
}

/// Provides link detection for terminal output lines.
pub struct TerminalLinkProvider {
    patterns: Vec<(String, TerminalLinkType)>,
}

impl TerminalLinkProvider {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                (r"https?://[^\s]+".into(), TerminalLinkType::Url),
                (r"[/\w.-]+\.\w+:\d+".into(), TerminalLinkType::FilePath),
            ],
        }
    }

    /// Detect links in a single line of terminal output.
    pub fn detect_links(&self, line: &str, line_number: u32) -> Vec<DetectedTerminalLink> {
        let mut results = Vec::new();
        for (pattern, link_type) in &self.patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                for m in re.find_iter(line) {
                    results.push(DetectedTerminalLink {
                        url: m.as_str().to_string(),
                        line: line_number,
                        start_col: m.start() as u32,
                        end_col: m.end() as u32,
                        link_type: *link_type,
                    });
                }
            }
        }
        results
    }

    /// Add a custom pattern.
    pub fn add_pattern(&mut self, pattern: impl Into<String>, link_type: TerminalLinkType) {
        self.patterns.push((pattern.into(), link_type));
    }
}

impl Default for TerminalLinkProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TerminalEnvironmentProvider – variable injection
// ---------------------------------------------------------------------------

/// Provides environment variables to inject into terminal processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalEnvironmentProvider {
    pub id: String,
    pub variables: HashMap<String, String>,
}

impl TerminalEnvironmentProvider {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            variables: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.variables.remove(key)
    }

    /// Merge variables from another provider (other overwrites self on conflict).
    pub fn merge_from(&mut self, other: &TerminalEnvironmentProvider) {
        for (k, v) in &other.variables {
            self.variables.insert(k.clone(), v.clone());
        }
    }

    pub fn len(&self) -> usize {
        self.variables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }
}

// ---------------------------------------------------------------------------
// TerminalQuickFix – pattern-matched suggestions
// ---------------------------------------------------------------------------

/// A quick fix suggestion triggered by matching a pattern in terminal output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerminalQuickFix {
    pub pattern: String,
    pub message: String,
    pub command: String,
}

impl TerminalQuickFix {
    pub fn new(
        pattern: impl Into<String>,
        message: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            message: message.into(),
            command: command.into(),
        }
    }

    /// Check if a line of output matches this quick fix pattern.
    pub fn matches(&self, line: &str) -> bool {
        line.contains(&self.pattern)
    }
}

impl fmt::Display for TerminalQuickFix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.message, self.command)
    }
}

/// Registry of terminal quick fixes.
#[derive(Debug, Clone, Default)]
pub struct TerminalQuickFixRegistry {
    fixes: Vec<TerminalQuickFix>,
}

impl TerminalQuickFixRegistry {
    pub fn new() -> Self {
        Self { fixes: Vec::new() }
    }

    pub fn add(&mut self, fix: TerminalQuickFix) {
        self.fixes.push(fix);
    }

    /// Find all quick fixes that match the given output line.
    pub fn find_matches(&self, line: &str) -> Vec<&TerminalQuickFix> {
        self.fixes.iter().filter(|f| f.matches(line)).collect()
    }

    pub fn len(&self) -> usize {
        self.fixes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fixes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Terminal profile contribution point
// ---------------------------------------------------------------------------

/// A contributed terminal profile from an extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtTerminalProfile {
    pub id: String,
    pub title: String,
    pub shell_path: String,
    pub shell_args: Vec<String>,
    pub icon: Option<String>,
}

impl ExtTerminalProfile {
    pub fn new(id: impl Into<String>, title: impl Into<String>, shell_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            shell_path: shell_path.into(),
            shell_args: Vec::new(),
            icon: None,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.shell_args = args;
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Build TerminalOptions from this profile.
    pub fn to_options(&self) -> TerminalOptions {
        TerminalOptions {
            name: Some(self.title.clone()),
            shell_path: Some(self.shell_path.clone()),
            shell_args: self.shell_args.clone(),
            cwd: None,
            env: Vec::new(),
            hide_from_user: false,
        }
    }
}

impl fmt::Display for ExtTerminalProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.title, self.shell_path)
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

    // -----------------------------------------------------------------------
    // New tests for added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn profile_contribution_creation_and_to_terminal_options() {
        let profile = TerminalProfileContribution::new("my-shell", "My Shell", "/bin/zsh")
            .with_args(vec!["-l".into()]);
        let opts = profile.to_terminal_options();
        assert_eq!(opts.name, Some("My Shell".into()));
        assert_eq!(opts.shell_path, Some("/bin/zsh".into()));
        assert_eq!(opts.shell_args, vec!["-l".to_string()]);
        assert!(!opts.hide_from_user);
    }

    #[test]
    fn profile_contribution_with_env() {
        let profile = TerminalProfileContribution::new("p", "P", "/bin/sh")
            .with_env("FOO", "bar")
            .with_env("BAZ", "qux");
        assert_eq!(profile.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(profile.env.get("BAZ"), Some(&"qux".to_string()));
        let opts = profile.to_terminal_options();
        assert!(opts.env.contains(&("FOO".into(), "bar".into())));
    }

    #[test]
    fn terminal_env_from_extension_adds_prefix() {
        let pairs = vec![("MY_VAR".into(), "value".into())];
        let env = terminal_env_from_extension("ext.foo", &pairs);
        assert_eq!(env.get("VSEDIT_EXT_MY_VAR"), Some(&"value".to_string()));
        // Already-prefixed keys should not be double-prefixed.
        let pairs2 = vec![("VSEDIT_EXT_KEEP".into(), "yes".into())];
        let env2 = terminal_env_from_extension("ext.foo", &pairs2);
        assert_eq!(env2.get("VSEDIT_EXT_KEEP"), Some(&"yes".to_string()));
    }

    #[test]
    fn terminal_env_from_extension_adds_ext_id() {
        let env = terminal_env_from_extension("my.extension", &[]);
        assert_eq!(env.get("VSEDIT_EXT_ID"), Some(&"my.extension".to_string()));
    }

    #[test]
    fn link_detector_detect_file_links() {
        let text = "Error at /home/user/project/src/main.rs:42:10 something";
        let links = TerminalLinkDetector::detect_file_links(text);
        assert!(!links.is_empty());
        let link = &links[0];
        let matched = &text[link.start_index as usize..(link.start_index + link.length) as usize];
        assert!(matched.contains("/home/user/project/src/main.rs:42:10"));
    }

    #[test]
    fn profile_registry_register_and_get() {
        let mut reg = TerminalProfileRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register(TerminalProfileContribution::new("bash", "Bash", "/bin/bash"));
        reg.register(TerminalProfileContribution::new("zsh", "Zsh", "/bin/zsh"));
        assert_eq!(reg.count(), 2);
        assert_eq!(reg.get("bash").unwrap().shell_path, "/bin/bash");
        assert!(reg.unregister("bash"));
        assert_eq!(reg.count(), 1);
        assert!(reg.get("bash").is_none());
        assert_eq!(reg.default_profile().unwrap().id, "zsh");
    }

    #[test]
    fn terminal_output_append_and_line_count() {
        let mut out = TerminalOutput::new("t1");
        assert_eq!(out.line_count(), 0);
        assert_eq!(out.total_bytes(), 0);
        out.append("hello\nworld\n");
        assert_eq!(out.line_count(), 2);
        assert_eq!(out.total_bytes(), 12);
        assert_eq!(out.terminal_id(), "t1");
    }

    #[test]
    fn terminal_output_last_n_lines() {
        let mut out = TerminalOutput::new("t2");
        out.append("line1\nline2\nline3\nline4\nline5");
        let last2 = out.last_n_lines(2);
        assert_eq!(last2, vec!["line4", "line5"]);
        let last10 = out.last_n_lines(10);
        assert_eq!(last10.len(), 5);
    }

    #[test]
    fn terminal_output_clear() {
        let mut out = TerminalOutput::new("t3");
        out.append("data");
        assert!(out.total_bytes() > 0);
        out.clear();
        assert_eq!(out.total_bytes(), 0);
        assert_eq!(out.line_count(), 0);
    }

    // ── Command parsing ───────────────────────────────────────────

    #[test]
    fn parse_simple_command() {
        let cmd = parse_command_line("ls -la /tmp").unwrap();
        assert_eq!(cmd.program, "ls");
        assert_eq!(cmd.args, vec!["-la", "/tmp"]);
        assert!(!cmd.background);
    }

    #[test]
    fn parse_command_with_quotes() {
        let cmd = parse_command_line(r#"echo "hello world" done"#).unwrap();
        assert_eq!(cmd.program, "echo");
        assert_eq!(cmd.args, vec!["hello world", "done"]);
    }

    #[test]
    fn parse_background_command() {
        let cmd = parse_command_line("sleep 10 &").unwrap();
        assert_eq!(cmd.program, "sleep");
        assert_eq!(cmd.args, vec!["10"]);
        assert!(cmd.background);
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_command_line("").is_none());
        assert!(parse_command_line("   ").is_none());
    }

    #[test]
    fn parsed_command_display() {
        let cmd = parse_command_line("git commit -m msg &").unwrap();
        assert_eq!(format!("{}", cmd), "git commit -m msg &");
    }

    // ── ANSI escape stripping ─────────────────────────────────────

    #[test]
    fn strip_ansi_color_codes() {
        let input = "\x1b[31mERROR\x1b[0m: something failed";
        assert_eq!(strip_ansi_escapes(input), "ERROR: something failed");
    }

    #[test]
    fn strip_ansi_preserves_plain_text() {
        assert_eq!(strip_ansi_escapes("hello world"), "hello world");
    }

    // ── Terminal session state ────────────────────────────────────

    #[test]
    fn session_state_tracks_commands() {
        let mut state = TerminalSessionState::new("t1", "/home");
        assert!(!state.has_activity());
        state.record_command(0);
        assert!(state.has_activity());
        assert!(state.last_command_succeeded());
        state.record_command(1);
        assert!(!state.last_command_succeeded());
        assert_eq!(state.command_count, 2);
    }

    #[test]
    fn session_state_env_overrides() {
        let mut state = TerminalSessionState::new("t1", "/home");
        state.set_env("PATH", "/usr/bin");
        assert_eq!(state.get_env("PATH"), Some("/usr/bin"));
        state.set_env("PATH", "/usr/local/bin");
        assert_eq!(state.get_env("PATH"), Some("/usr/local/bin"));
        assert_eq!(state.env_overrides.len(), 1);
        assert_eq!(state.get_env("MISSING"), None);
    }

    #[test]
    fn session_state_remove_env() {
        let mut state = TerminalSessionState::new("t1", "/home");
        state.set_env("A", "1");
        state.set_env("B", "2");
        assert!(state.remove_env("A"));
        assert_eq!(state.env_count(), 1);
        assert!(!state.remove_env("A"));
        assert_eq!(state.get_env("A"), None);
    }

    #[test]
    fn session_state_reset() {
        let mut state = TerminalSessionState::new("t1", "/home");
        state.record_command(0);
        state.set_env("X", "Y");
        state.reset();
        assert_eq!(state.command_count, 0);
        assert_eq!(state.exit_code, None);
        assert_eq!(state.env_count(), 0);
    }

    #[test]
    fn terminal_output_lines_and_contains() {
        let mut out = TerminalOutput::new("t1");
        out.append("line1\nline2\nline3\n");
        assert_eq!(out.lines().len(), 3);
        assert!(out.contains("line2"));
        assert!(!out.contains("missing"));
    }

    #[test]
    fn terminal_output_first_n_lines() {
        let mut out = TerminalOutput::new("t1");
        out.append("a\nb\nc\nd\n");
        let first2 = out.first_n_lines(2);
        assert_eq!(first2, vec!["a", "b"]);
        let all = out.first_n_lines(100);
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn bridge_terminal_ids_and_has() {
        let mut bridge = TerminalBridge::new();
        let id = bridge.create_terminal(&test_opts());
        assert!(bridge.has_terminal(&id));
        assert!(!bridge.has_terminal("nonexistent"));
        let ids = bridge.terminal_ids();
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn bridge_terminals_by_active() {
        let mut bridge = TerminalBridge::new();
        let id1 = bridge.create_terminal(&test_opts());
        let id2 = bridge.create_terminal(&test_opts());
        // Both start active; deactivate id2
        bridge.set_active(&id2, false).unwrap();
        let active = bridge.terminals_by_active(true);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id1);
        let inactive = bridge.terminals_by_active(false);
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].id, id2);
    }

    #[test]
    fn profile_contribution_has_icon_and_env_count() {
        let profile = TerminalProfileContribution::new("p1", "Profile 1", "/bin/bash")
            .with_icon("terminal")
            .with_env("A", "1")
            .with_env("B", "2");
        assert!(profile.has_icon());
        assert_eq!(profile.env_count(), 2);
        let plain = TerminalProfileContribution::new("p2", "Profile 2", "/bin/sh");
        assert!(!plain.has_icon());
        assert_eq!(plain.env_count(), 0);
    }

    #[test]
    fn terminal_output_empty_lines() {
        let out = TerminalOutput::new("t1");
        assert!(out.lines().is_empty());
        assert!(!out.contains("anything"));
        let first = out.first_n_lines(5);
        assert!(first.is_empty());
    }

    #[test]
    fn terminal_output_grep() {
        let mut out = TerminalOutput::new("t1");
        out.append("error: file not found\nwarning: unused var\nerror: syntax\n");
        let errors = out.grep("error");
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("file not found"));
        assert!(errors[1].contains("syntax"));
    }

    #[test]
    fn terminal_output_count_occurrences() {
        let mut out = TerminalOutput::new("t1");
        out.append("aaa bbb aaa ccc aaa\n");
        assert_eq!(out.count_occurrences("aaa"), 3);
        assert_eq!(out.count_occurrences("zzz"), 0);
    }

    #[test]
    fn terminal_output_split_at_midpoint() {
        let mut out = TerminalOutput::new("t1");
        out.append("line1\nline2\nline3\nline4\n");
        let (first, second) = out.split_at_midpoint();
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 2);
    }

    #[test]
    fn terminal_output_last_line_ends_with() {
        let mut out = TerminalOutput::new("t1");
        out.append("hello world\ndone!");
        assert!(out.last_line_ends_with("!"));
        assert!(!out.last_line_ends_with("world"));
    }

    #[test]
    fn terminal_output_find_offset() {
        let mut out = TerminalOutput::new("t1");
        out.append("prefix:target:suffix");
        assert_eq!(out.find_offset("target"), Some(7));
        assert_eq!(out.find_offset("missing"), None);
    }

    #[test]
    fn profile_registry_find_by_shell() {
        let mut reg = TerminalProfileRegistry::new();
        reg.register(TerminalProfileContribution::new("a", "Bash", "/bin/bash"));
        reg.register(TerminalProfileContribution::new("b", "Zsh", "/bin/zsh"));
        reg.register(TerminalProfileContribution::new("c", "Fish", "/usr/bin/fish"));
        let bash = reg.find_by_shell("bash");
        assert_eq!(bash.len(), 1);
        assert_eq!(bash[0].id, "a");
    }

    #[test]
    fn profile_registry_unique_shells() {
        let mut reg = TerminalProfileRegistry::new();
        reg.register(TerminalProfileContribution::new("a", "B1", "/bin/bash"));
        reg.register(TerminalProfileContribution::new("b", "B2", "/bin/bash"));
        reg.register(TerminalProfileContribution::new("c", "Z1", "/bin/zsh"));
        let shells = reg.unique_shells();
        assert_eq!(shells.len(), 2);
    }

    #[test]
    fn bridge_dispose_all() {
        let mut bridge = TerminalBridge::new();
        bridge.create_terminal(&test_opts());
        bridge.create_terminal(&test_opts());
        assert_eq!(bridge.terminal_count(), 2);
        let removed = bridge.dispose_all();
        assert_eq!(removed, 2);
        assert_eq!(bridge.terminal_count(), 0);
    }

    #[test]
    fn bridge_search_by_name() {
        let mut bridge = TerminalBridge::new();
        bridge.create_terminal(&TerminalOptions {
            name: Some("Dev Server".into()),
            ..test_opts()
        });
        bridge.create_terminal(&TerminalOptions {
            name: Some("Build".into()),
            ..test_opts()
        });
        let results = bridge.search_by_name("dev");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Dev Server");
    }

    #[test]
    fn parsed_command_helpers() {
        let cmd = parse_command_line("git commit -m \"hello world\"").unwrap();
        assert_eq!(cmd.token_count(), 4);
        assert!(!cmd.is_simple());
        let with_flag = cmd.with_arg("--amend");
        assert_eq!(with_flag.args.len(), 4);
        let simple = parse_command_line("ls").unwrap();
        assert!(simple.is_simple());
        assert_eq!(simple.to_command_string(), "ls");
    }

    // -- TerminalLinkProvider tests --

    #[test]
    fn detect_url_links() {
        let provider = TerminalLinkProvider::new();
        let links = provider.detect_links("Visit https://example.com for info", 0);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[0].link_type, TerminalLinkType::Url);
    }

    #[test]
    fn detect_file_links() {
        let provider = TerminalLinkProvider::new();
        let links = provider.detect_links("error at src/main.rs:42", 1);
        assert!(links.iter().any(|l| l.link_type == TerminalLinkType::FilePath));
    }

    #[test]
    fn detect_no_links() {
        let provider = TerminalLinkProvider::default();
        let links = provider.detect_links("just plain text", 0);
        assert!(links.is_empty());
    }

    #[test]
    fn link_type_display() {
        assert_eq!(format!("{}", TerminalLinkType::Url), "URL");
        assert_eq!(format!("{}", TerminalLinkType::FilePath), "File");
    }

    // -- TerminalEnvironmentProvider tests --

    #[test]
    fn env_provider_basic() {
        let mut prov = TerminalEnvironmentProvider::new("test");
        prov.set("PATH", "/usr/bin");
        prov.set("HOME", "/home/user");
        assert_eq!(prov.get("PATH"), Some("/usr/bin"));
        assert_eq!(prov.len(), 2);
        prov.remove("HOME");
        assert_eq!(prov.len(), 1);
    }

    #[test]
    fn env_provider_merge() {
        let mut a = TerminalEnvironmentProvider::new("a");
        a.set("X", "1");
        a.set("Y", "2");
        let mut b = TerminalEnvironmentProvider::new("b");
        b.set("X", "override");
        b.set("Z", "3");
        a.merge_from(&b);
        assert_eq!(a.get("X"), Some("override"));
        assert_eq!(a.get("Z"), Some("3"));
        assert_eq!(a.len(), 3);
    }

    // -- TerminalQuickFix tests --

    #[test]
    fn quick_fix_matches() {
        let fix = TerminalQuickFix::new("command not found", "Install missing command", "apt install");
        assert!(fix.matches("bash: foo: command not found"));
        assert!(!fix.matches("everything is fine"));
        assert_eq!(format!("{}", fix), "Install missing command: apt install");
    }

    #[test]
    fn quick_fix_registry() {
        let mut reg = TerminalQuickFixRegistry::new();
        reg.add(TerminalQuickFix::new("not found", "Install", "apt install"));
        reg.add(TerminalQuickFix::new("permission denied", "Use sudo", "sudo !!"));
        let matches = reg.find_matches("command not found: foo");
        assert_eq!(matches.len(), 1);
        assert!(reg.find_matches("all good").is_empty());
    }

    // -- ExtTerminalProfile tests --

    #[test]
    fn ext_profile_to_options() {
        let profile = ExtTerminalProfile::new("bash", "Bash", "/bin/bash")
            .with_args(vec!["-l".into()])
            .with_icon("terminal-bash");
        let opts = profile.to_options();
        assert_eq!(opts.shell_path, Some("/bin/bash".into()));
        assert_eq!(opts.shell_args, vec!["-l"]);
        assert_eq!(opts.name, Some("Bash".into()));
        assert_eq!(format!("{}", profile), "Bash (/bin/bash)");
    }

    #[test]
    fn ext_profile_display() {
        let profile = ExtTerminalProfile::new("zsh", "Zsh", "/bin/zsh");
        assert!(format!("{}", profile).contains("Zsh"));
    }
}
