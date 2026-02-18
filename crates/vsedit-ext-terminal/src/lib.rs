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

// ---------------------------------------------------------------------------
// TerminalCreationValidator - terminal creation validator
// ---------------------------------------------------------------------------

/// Severity level for terminal creation validator issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalCreationValidatorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for TerminalCreationValidatorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [TerminalCreationValidator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCreationValidatorEntry {
    pub id: String,
    pub label: String,
    pub severity: TerminalCreationValidatorSeverity,
    pub detail: Option<String>,
    pub terminal_count: usize,
    enabled: bool,
}

impl TerminalCreationValidatorEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: TerminalCreationValidatorSeverity::Low,
            detail: None,
            terminal_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: TerminalCreationValidatorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_terminal_count(mut self, val: usize) -> Self {
        self.terminal_count = val;
        self
    }

    pub fn is_valid_config(&self) -> bool {
        self.enabled && self.severity >= TerminalCreationValidatorSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.terminal_count, det)
    }
}

impl fmt::Display for TerminalCreationValidatorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [TerminalCreationValidatorEntry] items.
#[derive(Debug, Clone)]
pub struct TerminalCreationValidator {
    entries: Vec<TerminalCreationValidatorEntry>,
    name: String,
    capacity: usize,
}

impl TerminalCreationValidator {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: TerminalCreationValidatorEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<TerminalCreationValidatorEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&TerminalCreationValidatorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn terminal_count(&self) -> usize { self.entries.len() }

    pub fn is_valid_config(&self) -> bool {
        self.entries.iter().any(|e| e.is_valid_config())
    }

    pub fn entries_by_severity(&self, severity: TerminalCreationValidatorSeverity) -> Vec<&TerminalCreationValidatorEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= TerminalCreationValidatorSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&TerminalCreationValidatorEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&TerminalCreationValidatorEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// TerminalColorMapper - terminal color mapper
// ---------------------------------------------------------------------------

/// Configuration for [TerminalColorMapper].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalColorMapperConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub color_count: usize,
}

impl TerminalColorMapperConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, color_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_color_count(mut self, val: usize) -> Self { self.color_count = val; self }
}

impl Default for TerminalColorMapperConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [TerminalColorMapper].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalColorMapperItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl TerminalColorMapperItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_custom_colors(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for TerminalColorMapperItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [TerminalColorMapperItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct TerminalColorMapper {
    config: TerminalColorMapperConfig,
    items: Vec<TerminalColorMapperItem>,
}

impl TerminalColorMapper {
    pub fn new(config: TerminalColorMapperConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: TerminalColorMapperItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<TerminalColorMapperItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&TerminalColorMapperItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn color_count(&self) -> usize { self.items.len() }

    pub fn has_custom_colors(&self) -> bool {
        self.items.iter().any(|i| i.has_custom_colors())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&TerminalColorMapperItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TerminalColorMapperItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &TerminalColorMapperConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// ext_terminal – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtTerminalActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtTerminalActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtTerminalRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtTerminalRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_terminal_collect_sequences(envelopes: &[XExtTerminalRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_terminal_filter_by_method<'a>(
    envelopes: &'a [XExtTerminalRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtTerminalRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_terminal_dedup_by_seq(envelopes: Vec<XExtTerminalRpcEnvelope>) -> Vec<XExtTerminalRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_terminal_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtTerminalApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtTerminalApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtTerminalApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}



// ---------------------------------------------------------------------------
// ext_terminal – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension terminal integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtTerminalExtTerminalShell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

impl YExtTerminalExtTerminalShell {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Bash => 0,
            Self::Zsh => 1,
            Self::Fish => 2,
            Self::Powershell => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bash => "Bash",
            Self::Zsh => "Zsh",
            Self::Fish => "Fish",
            Self::Powershell => "Powershell",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtTerminalExtTerminalShell] {
        &[
            YExtTerminalExtTerminalShell::Bash,
            YExtTerminalExtTerminalShell::Zsh,
            YExtTerminalExtTerminalShell::Fish,
            YExtTerminalExtTerminalShell::Powershell,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtTerminalExtTerminalShell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks terminal profile data.
#[derive(Debug, Clone)]
pub struct YExtTerminalExtTerminalProfile {
    pub name: String,
    pub shell_path: String,
    pub args: Vec<String>,
}

impl YExtTerminalExtTerminalProfile {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            shell_path: String::new(),
            args: Vec::new(),
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.args.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtTerminalExtTerminalProfile({}: {:?})", "name", self.name)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_terminal_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_terminal_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_terminal_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_terminal_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_terminal_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_terminal_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_terminal_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_terminal_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_terminal – Extended extension terminal env helpers
// ---------------------------------------------------------------------------

/// Priority levels for extension terminal env.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtTerminalPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtTerminalPriority {
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
    pub fn all_asc() -> [ZExtTerminalPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtTerminalPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks extension terminal env data.
#[derive(Debug, Clone)]
pub struct ZExtTerminalExtTerminalEnv {
    pub env_vars: Vec<(String, String)>,
    pub inherit_env: bool,
    pub cwd: String,
}

impl ZExtTerminalExtTerminalEnv {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            env_vars: Vec::new(),
            inherit_env: false,
            cwd: String::new(),
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.env_vars.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.env_vars.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.env_vars.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtTerminalExtTerminalEnv[inherit_env={:?}, cwd={:?}]", self.inherit_env, self.cwd)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for extension terminal env.
pub fn z_ext_terminal_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_terminal_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_terminal_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_terminal_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_ext_terminal_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_terminal_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_terminal_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 82
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer82 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer82 {
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
pub fn xb_fnv1a_82(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_82<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_82<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_82(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_82(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 72
// ---------------------------------------------------------------------------

/// Generic object pool `Xc72Pool<T>`.
pub struct Xc72Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc72Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc72PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc72Pool<T> {
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
    pub fn stats(&self) -> Xc72PoolStats {
        Xc72PoolStats {
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

impl<T> Default for Xc72Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc72Scheduler`.
pub struct Xc72Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc72Scheduler {
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

impl Default for Xc72Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_72 hash for the given byte slice.
pub fn xc_72_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_72 convention.
pub fn xc_72_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe95 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe95Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe95PipelineError {
    pub stage: Xe95Stage,
    pub message: String,
}

impl std::fmt::Display for Xe95PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe95Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe95Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError>>>,
    stage_names: Vec<Xe95Stage>,
}

impl Xe95Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe95Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe95Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe95Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe95Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> {
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

    pub fn compose(mut self, other: Xe95Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe95CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe95CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe95Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe95CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe95CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe95Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe95CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_95_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe95CacheEntry {
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

    fn xe_95_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe95CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_95_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> {
    Ok(data)
}

pub fn xe_95_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_95_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_95_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_95_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe95PipelineError> {
    Err(Xe95PipelineError {
        stage: Xe95Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_93: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg93Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg93Graph {
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

impl Default for Xg93Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_93: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg93Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg93Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg93Heap<T>) {
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

impl<T: Ord> Default for Xg93Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 71).
pub struct Xh71SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh71SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 113 as u64,
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

/// A compact bit set supporting boolean operations (variant 71).
pub struct Xh71BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh71BitSet {
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
    fn detect_file_links_works() {
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

#[test]
    fn terminalcreationvalidator_severity_ordering() {
        assert!(TerminalCreationValidatorSeverity::Critical > TerminalCreationValidatorSeverity::High);
        assert!(TerminalCreationValidatorSeverity::High > TerminalCreationValidatorSeverity::Medium);
        assert!(TerminalCreationValidatorSeverity::Medium > TerminalCreationValidatorSeverity::Low);
    }

    #[test]
    fn terminalcreationvalidator_severity_display() {
        assert_eq!(TerminalCreationValidatorSeverity::Low.to_string(), "low");
        assert_eq!(TerminalCreationValidatorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn terminalcreationvalidator_entry_creation() {
        let e = TerminalCreationValidatorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, TerminalCreationValidatorSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn terminalcreationvalidator_entry_builder() {
        let e = TerminalCreationValidatorEntry::new("e2", "Entry 2")
            .with_severity(TerminalCreationValidatorSeverity::High)
            .with_detail("some detail")
            .with_terminal_count(42);
        assert_eq!(e.severity, TerminalCreationValidatorSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.terminal_count, 42);
    }

    #[test]
    fn terminalcreationvalidator_entry_enable_disable() {
        let mut e = TerminalCreationValidatorEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn terminalcreationvalidator_add_and_count() {
        let mut mgr = TerminalCreationValidator::new("test");
        mgr.add(TerminalCreationValidatorEntry::new("a", "A"));
        mgr.add(TerminalCreationValidatorEntry::new("b", "B").with_severity(TerminalCreationValidatorSeverity::High));
        assert_eq!(mgr.terminal_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn terminalcreationvalidator_remove() {
        let mut mgr = TerminalCreationValidator::new("test");
        mgr.add(TerminalCreationValidatorEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn terminalcreationvalidator_capacity() {
        let mut mgr = TerminalCreationValidator::new("test").with_capacity(1);
        assert!(mgr.add(TerminalCreationValidatorEntry::new("a", "A")));
        assert!(!mgr.add(TerminalCreationValidatorEntry::new("b", "B")));
    }

    #[test]
    fn terminalcreationvalidator_sorted_by_severity() {
        let mut mgr = TerminalCreationValidator::new("test");
        mgr.add(TerminalCreationValidatorEntry::new("lo", "Low"));
        mgr.add(TerminalCreationValidatorEntry::new("hi", "High").with_severity(TerminalCreationValidatorSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, TerminalCreationValidatorSeverity::Critical);
    }

    #[test]
    fn terminalcreationvalidator_summary() {
        let mgr = TerminalCreationValidator::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn terminalcolormapper_config_defaults() {
        let cfg = TerminalColorMapperConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn terminalcolormapper_item_creation() {
        let item = TerminalColorMapperItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn terminalcolormapper_add_and_get() {
        let mut mgr = TerminalColorMapper::new(TerminalColorMapperConfig::new("test"));
        mgr.add(TerminalColorMapperItem::new("k1", "v1"));
        assert_eq!(mgr.color_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn terminalcolormapper_remove_item() {
        let mut mgr = TerminalColorMapper::new(TerminalColorMapperConfig::new("test"));
        mgr.add(TerminalColorMapperItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn terminalcolormapper_sorted_by_priority() {
        let mut mgr = TerminalColorMapper::new(TerminalColorMapperConfig::new("test"));
        mgr.add(TerminalColorMapperItem::new("lo", "low").with_priority(1));
        mgr.add(TerminalColorMapperItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn terminalcolormapper_items_with_tag() {
        let mut mgr = TerminalColorMapper::new(TerminalColorMapperConfig::new("test"));
        mgr.add(TerminalColorMapperItem::new("a", "1").with_tag("x"));
        mgr.add(TerminalColorMapperItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn terminalcolormapper_report() {
        let mgr = TerminalColorMapper::new(TerminalColorMapperConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    // -- ext_terminal additional tests -------------------------------------------

    #[test]
    fn x_ext_terminal_activation_parse_language() {
        let ak = XExtTerminalActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtTerminalActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_terminal_activation_parse_command() {
        let ak = XExtTerminalActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtTerminalActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_terminal_activation_parse_star() {
        assert_eq!(XExtTerminalActivationKind::parse("*"), Some(XExtTerminalActivationKind::Star));
    }

    #[test]
    fn x_ext_terminal_activation_parse_unknown() {
        assert!(XExtTerminalActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_terminal_activation_parse_workspace() {
        let ak = XExtTerminalActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtTerminalActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_terminal_rpc_envelope_basic() {
        let env = XExtTerminalRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_terminal_rpc_envelope_response() {
        let env = XExtTerminalRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_terminal_rpc_payload_checksum() {
        let env = XExtTerminalRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_terminal_collect_sequences_works() {
        let envs = vec![
            XExtTerminalRpcEnvelope::new(10, "a", ""),
            XExtTerminalRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_terminal_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_terminal_filter_by_method_works() {
        let envs = vec![
            XExtTerminalRpcEnvelope::new(1, "textDocument/open", ""),
            XExtTerminalRpcEnvelope::new(2, "workspace/config", ""),
            XExtTerminalRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_terminal_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_terminal_dedup_by_seq_works() {
        let envs = vec![
            XExtTerminalRpcEnvelope::new(1, "a", "first"),
            XExtTerminalRpcEnvelope::new(1, "a", "second"),
            XExtTerminalRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_terminal_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_terminal_negotiate_capabilities_basic() {
        let result = x_ext_terminal_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_terminal_api_version_satisfies() {
        let v1 = XExtTerminalApiVersion::new(1, 80, 0);
        let min = XExtTerminalApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_terminal_api_version_display() {
        let v = XExtTerminalApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_terminal_api_version_ord() {
        let v1 = XExtTerminalApiVersion::new(1, 0, 0);
        let v2 = XExtTerminalApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    // -- ext_terminal extended domain tests ----------------------------------------

    #[test]
    fn y_ext_terminal_enum_index() {
        assert_eq!(YExtTerminalExtTerminalShell::Bash.index(), 0);
        assert_eq!(YExtTerminalExtTerminalShell::Zsh.index(), 1);
        assert_eq!(YExtTerminalExtTerminalShell::Fish.index(), 2);
        assert_eq!(YExtTerminalExtTerminalShell::Powershell.index(), 3);
    }

    #[test]
    fn y_ext_terminal_enum_label() {
        assert_eq!(YExtTerminalExtTerminalShell::Bash.label(), "Bash");
        assert_eq!(YExtTerminalExtTerminalShell::Zsh.label(), "Zsh");
        assert_eq!(YExtTerminalExtTerminalShell::Fish.label(), "Fish");
        assert_eq!(YExtTerminalExtTerminalShell::Powershell.label(), "Powershell");
    }

    #[test]
    fn y_ext_terminal_enum_all() {
        let all = YExtTerminalExtTerminalShell::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_terminal_enum_is_default() {
        assert!(YExtTerminalExtTerminalShell::Bash.is_default());
        assert!(!YExtTerminalExtTerminalShell::Powershell.is_default());
    }

    #[test]
    fn y_ext_terminal_enum_display() {
        assert_eq!(format!("{}", YExtTerminalExtTerminalShell::Bash), "Bash");
    }

    #[test]
    fn y_ext_terminal_struct_new() {
        let s = YExtTerminalExtTerminalProfile::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_ext_terminal_struct_clear() {
        let mut s = YExtTerminalExtTerminalProfile::new();
        s.args.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_ext_terminal_fingerprint_deterministic() {
        let h1 = y_ext_terminal_fingerprint("hello");
        let h2 = y_ext_terminal_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_terminal_fingerprint("a"), y_ext_terminal_fingerprint("b"));
    }

    #[test]
    fn y_ext_terminal_truncate_short() {
        assert_eq!(y_ext_terminal_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_terminal_truncate_long() {
        let r = y_ext_terminal_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_terminal_normalize_key_basic() {
        assert_eq!(y_ext_terminal_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_terminal_split_path_basic() {
        let parts = y_ext_terminal_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_terminal_count_occurrences_basic() {
        assert_eq!(y_ext_terminal_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_terminal_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_terminal_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_terminal_in_range_basic() {
        assert!(y_ext_terminal_in_range(5, 1, 10));
        assert!(y_ext_terminal_in_range(1, 1, 10));
        assert!(y_ext_terminal_in_range(10, 1, 10));
        assert!(!y_ext_terminal_in_range(0, 1, 10));
        assert!(!y_ext_terminal_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_terminal_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_terminal_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_terminal_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_terminal_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_terminal Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_terminal_priority_weight() {
        assert_eq!(ZExtTerminalPriority::Idle.weight(), 0);
        assert_eq!(ZExtTerminalPriority::Normal.weight(), 2);
        assert_eq!(ZExtTerminalPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_terminal_priority_label() {
        assert_eq!(ZExtTerminalPriority::Low.label(), "low");
        assert_eq!(ZExtTerminalPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_terminal_priority_is_elevated() {
        assert!(!ZExtTerminalPriority::Normal.is_elevated());
        assert!(ZExtTerminalPriority::High.is_elevated());
        assert!(ZExtTerminalPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_terminal_priority_display() {
        assert_eq!(format!("{}", ZExtTerminalPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_terminal_priority_all_asc() {
        let all = ZExtTerminalPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtTerminalPriority::Idle);
        assert_eq!(all[4], ZExtTerminalPriority::Realtime);
    }

    #[test]
    fn z_ext_terminal_struct_new() {
        let s = ZExtTerminalExtTerminalEnv::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_terminal_struct_toggled_clone() {
        let s = ZExtTerminalExtTerminalEnv::new();
        let t = s.toggled_clone();
        let _ = t.cwd;
    }

    #[test]
    fn z_ext_terminal_rolling_hash_deterministic() {
        let h1 = z_ext_terminal_rolling_hash(b"test");
        let h2 = z_ext_terminal_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_terminal_rolling_hash(b"a"), z_ext_terminal_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_terminal_pad_to_basic() {
        assert_eq!(z_ext_terminal_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_terminal_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_terminal_is_identifier_basic() {
        assert!(z_ext_terminal_is_identifier("foo_bar"));
        assert!(z_ext_terminal_is_identifier("abc123"));
        assert!(!z_ext_terminal_is_identifier(""));
        assert!(!z_ext_terminal_is_identifier("has space"));
    }

    #[test]
    fn z_ext_terminal_levenshtein_basic() {
        assert_eq!(z_ext_terminal_levenshtein("", ""), 0);
        assert_eq!(z_ext_terminal_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_terminal_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_terminal_unique_words_basic() {
        let w = z_ext_terminal_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_terminal_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_terminal_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_terminal_common_prefix_basic() {
        assert_eq!(z_ext_terminal_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_terminal_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_terminal_struct_clear() {
        let mut s = ZExtTerminalExtTerminalEnv::new();
        s.env_vars.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_terminal_rolling_hash_empty() {
        let h = z_ext_terminal_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_82_push_and_len() {
        let mut rb = super::XbRingBuffer82::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_82_overwrite() {
        let mut rb = super::XbRingBuffer82::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_82_get_out_of_bounds() {
        let rb = super::XbRingBuffer82::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_82_drain_all() {
        let mut rb = super::XbRingBuffer82::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_82_peek_front_back() {
        let mut rb = super::XbRingBuffer82::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_82_clear() {
        let mut rb = super::XbRingBuffer82::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_82_capacity() {
        let rb = super::XbRingBuffer82::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_82_basic() {
        let h = super::xb_fnv1a_82(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_82(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_82_different_inputs() {
        let h1 = super::xb_fnv1a_82(b"abc");
        let h2 = super::xb_fnv1a_82(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_82_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_82(&data);
        let dec = super::xb_rle_decode_82(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_82_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_82(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_82(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_82_values() {
        assert!((super::xb_clamp_82(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_82(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_82(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_82_values() {
        assert!((super::xb_lerp_82(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_82(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_82(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_82_wrap_around_twice() {
        let mut rb = super::XbRingBuffer82::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 72 ----

    #[test]
    fn xc_72_pool_new_empty() {
        let pool: super::Xc72Pool<i32> = super::Xc72Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_72_pool_release_acquire() {
        let mut pool = super::Xc72Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_72_pool_acquire_empty() {
        let mut pool: super::Xc72Pool<i32> = super::Xc72Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_72_pool_full() {
        let mut pool = super::Xc72Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_72_pool_drain() {
        let mut pool = super::Xc72Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_72_pool_stats() {
        let mut pool = super::Xc72Pool::new(8);
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
    fn xc_72_pool_clear() {
        let mut pool = super::Xc72Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_72_pool_shrink() {
        let mut pool = super::Xc72Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_72_pool_default() {
        let pool: super::Xc72Pool<String> = super::Xc72Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_72_pool_extend() {
        let mut pool = super::Xc72Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_72_pool_retain() {
        let mut pool = super::Xc72Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_72_scheduler_round_robin() {
        let mut sched = super::Xc72Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_72_scheduler_empty() {
        let mut sched = super::Xc72Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_72_scheduler_reset() {
        let mut sched = super::Xc72Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_72_scheduler_add_remove() {
        let mut sched = super::Xc72Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_72_scheduler_targets() {
        let sched = super::Xc72Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_72_hash_empty() {
        assert_eq!(super::xc_72_hash(b""), 5381);
    }

    #[test]
    fn xc_72_hash_data() {
        let h = super::xc_72_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_72_hash(b"hello"), h);
    }

    #[test]
    fn xc_72_reverse_str() {
        assert_eq!(super::xc_72_reverse("abc"), "cba");
        assert_eq!(super::xc_72_reverse(""), "");
    }


    #[test]
    fn xe_95_pipeline_empty() {
        let p = super::Xe95Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_95_pipeline_parse_stage() {
        let p = super::Xe95Pipeline::new()
            .add_parse(super::xe_95_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_95_pipeline_transform_double() {
        let p = super::Xe95Pipeline::new()
            .add_transform(super::xe_95_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_95_pipeline_validate_reverse() {
        let p = super::Xe95Pipeline::new()
            .add_validate(super::xe_95_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_95_pipeline_emit_filter() {
        let p = super::Xe95Pipeline::new()
            .add_emit(super::xe_95_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_95_pipeline_multi_stage() {
        let p = super::Xe95Pipeline::new()
            .add_parse(super::xe_95_pipeline_identity)
            .add_transform(super::xe_95_pipeline_double)
            .add_validate(super::xe_95_pipeline_reverse)
            .add_emit(super::xe_95_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_95_pipeline_error_propagation() {
        let p = super::Xe95Pipeline::new()
            .add_parse(super::xe_95_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe95Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_95_pipeline_compose() {
        let p1 = super::Xe95Pipeline::new()
            .add_parse(super::xe_95_pipeline_identity);
        let p2 = super::Xe95Pipeline::new()
            .add_transform(super::xe_95_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_95_pipeline_error_display() {
        let e = super::Xe95PipelineError {
            stage: super::Xe95Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_95_cache_put_get() {
        let mut c = super::Xe95Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_95_cache_miss() {
        let mut c: super::Xe95Cache<&str, i32> = super::Xe95Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_95_cache_ttl_expiry() {
        let mut c = super::Xe95Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_95_cache_evict() {
        let mut c = super::Xe95Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_95_cache_capacity() {
        let mut c = super::Xe95Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_95_cache_stats() {
        let mut c = super::Xe95Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_95_cache_clear() {
        let mut c = super::Xe95Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_93 graph tests ------------------------------------------------

    #[test]
    fn xg_93_graph_empty() {
        let g = super::Xg93Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_93_graph_add_node() {
        let mut g = super::Xg93Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_93_graph_add_edge() {
        let mut g = super::Xg93Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_93_graph_neighbors() {
        let mut g = super::Xg93Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_93_graph_has_path() {
        let mut g = super::Xg93Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_93_graph_self_path() {
        let g = super::Xg93Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_93_graph_topo_sort() {
        let mut g = super::Xg93Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_93_graph_cycle_detect_false() {
        let mut g = super::Xg93Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_93_graph_cycle_detect_true() {
        let mut g = super::Xg93Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_93 heap tests -------------------------------------------------

    #[test]
    fn xg_93_heap_empty() {
        let h: super::Xg93Heap<i32> = super::Xg93Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_93_heap_push_pop() {
        let mut h = super::Xg93Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_93_heap_peek() {
        let mut h = super::Xg93Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_93_heap_drain_sorted() {
        let mut h = super::Xg93Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_93_heap_merge() {
        let mut a = super::Xg93Heap::new();
        let mut b = super::Xg93Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_93_heap_default() {
        let h: super::Xg93Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_93_graph_default() {
        let g: super::Xg93Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh71_skip_insert_contains() {
        let mut sl = super::Xh71SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh71_skip_remove() {
        let mut sl = super::Xh71SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh71_skip_len() {
        let mut sl = super::Xh71SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh71_skip_range_query() {
        let mut sl = super::Xh71SkipList::xh_new(4);
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
    fn xh71_skip_floor_ceiling() {
        let mut sl = super::Xh71SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh71_skip_rank() {
        let mut sl = super::Xh71SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh71_skip_empty() {
        let sl = super::Xh71SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh71_skip_duplicates() {
        let mut sl = super::Xh71SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh71_bitset_set_test() {
        let mut bs = super::Xh71BitSet::xh_new(256);
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
    fn xh71_bitset_clear_count() {
        let mut bs = super::Xh71BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh71_bitset_and_or_xor() {
        let mut a = super::Xh71BitSet::xh_new(128);
        let mut b = super::Xh71BitSet::xh_new(128);
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
    fn xh71_bitset_iter_ones() {
        let mut bs = super::Xh71BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh71_bitset_first_last() {
        let mut bs = super::Xh71BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh71_bitset_empty() {
        let bs = super::Xh71BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
