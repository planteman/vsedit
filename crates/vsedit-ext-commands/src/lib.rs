//! Ext API: Commands.
//!
//! RPC bridge between the extension host and the main thread for commands.

use std::fmt;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_commands";

// ── RPC message types ──

/// Messages exchanged for the `commands` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CommandMessage {
    RegisterCommand { command: CommandRegistration },
    ExecuteCommand { command_id: String, args: Vec<Value> },
    GetCommands { filter_internal: bool },
}

/// A command registration from an extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRegistration {
    pub command_id: String,
    /// Opaque handle used to proxy callbacks back to the extension host.
    pub callback_proxy_id: String,
}

/// Response payload for command operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CommandResponse {
    Registered,
    ExecuteResult { value: Value },
    CommandList { command_ids: Vec<String> },
}

// ── Bridge ──

/// Maps extension command IDs to RPC proxy handles.
#[derive(Debug, Default)]
pub struct CommandBridge {
    commands: HashMap<String, CommandRegistration>,
}

impl CommandBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an incoming command message and return a response.
    pub fn handle(&mut self, msg: CommandMessage) -> CommandResponse {
        match msg {
            CommandMessage::RegisterCommand { command } => {
                self.commands.insert(command.command_id.clone(), command);
                CommandResponse::Registered
            }
            CommandMessage::ExecuteCommand { command_id, .. } => {
                // In production this would proxy to the extension host callback.
                if self.commands.contains_key(&command_id) {
                    CommandResponse::ExecuteResult { value: Value::Null }
                } else {
                    CommandResponse::ExecuteResult { value: Value::Null }
                }
            }
            CommandMessage::GetCommands { .. } => {
                let command_ids: Vec<String> = self.commands.keys().cloned().collect();
                CommandResponse::CommandList { command_ids }
            }
        }
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Look up a command registration by ID.
    pub fn get_command(&self, id: &str) -> Option<&CommandRegistration> {
        self.commands.get(id)
    }
}

impl CommandBridge {
    /// Remove a command registration by ID. Returns `true` if it existed.
    pub fn unregister_command(&mut self, id: &str) -> bool {
        self.commands.remove(id).is_some()
    }

    /// Check whether a command is registered.
    pub fn has_command(&self, id: &str) -> bool {
        self.commands.contains_key(id)
    }

    /// Return all registered command IDs.
    pub fn get_all_commands(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
    }

    /// Execute a command and return a structured result.
    pub fn execute_with_result(
        &self,
        command_id: &str,
        _args: &[Value],
    ) -> CommandExecutionResult {
        if self.commands.contains_key(command_id) {
            CommandExecutionResult {
                success: true,
                value: Some(Value::Null),
                error_message: None,
            }
        } else {
            CommandExecutionResult {
                success: false,
                value: None,
                error_message: Some(format!("Command '{}' not found", command_id)),
            }
        }
    }
}

/// Result of executing a command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandExecutionResult {
    pub success: bool,
    pub value: Option<Value>,
    pub error_message: Option<String>,
}

// ── History ──

/// Tracks executed commands with timestamps.
#[derive(Debug, Default)]
pub struct CommandHistory {
    entries: Vec<CommandHistoryEntry>,
}

#[derive(Debug, Clone)]
struct CommandHistoryEntry {
    command_id: String,
    timestamp: std::time::Instant,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record that a command was executed.
    pub fn record_execution(&mut self, command_id: &str) {
        self.entries.push(CommandHistoryEntry {
            command_id: command_id.to_string(),
            timestamp: std::time::Instant::now(),
        });
    }

    /// Get the most recent `n` command IDs (newest first).
    pub fn get_recent(&self, n: usize) -> Vec<&str> {
        self.entries
            .iter()
            .rev()
            .take(n)
            .map(|e| e.command_id.as_str())
            .collect()
    }

    /// Total number of recorded executions.
    pub fn execution_count(&self) -> usize {
        self.entries.len()
    }

    /// The command ID of the most recent execution, if any.
    pub fn last_execution(&self) -> Option<&str> {
        self.entries.last().map(|e| e.command_id.as_str())
    }

    /// Count how many times a specific command was executed.
    pub fn count_for(&self, command_id: &str) -> usize {
        self.entries
            .iter()
            .filter(|e| e.command_id == command_id)
            .count()
    }

    /// Clear all recorded history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ── CommandProxy ──

/// High-level proxy for managing extension commands.
///
/// Wraps a `CommandBridge` and a `CommandHistory` to provide a unified API
/// for registering, executing, querying, and tracking commands.
#[derive(Debug, Default)]
pub struct CommandProxy {
    bridge: CommandBridge,
    history: CommandHistory,
}

impl CommandProxy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a command with the given ID and callback proxy handle.
    pub fn register(&mut self, command_id: &str, callback_proxy_id: &str) {
        self.bridge.handle(CommandMessage::RegisterCommand {
            command: CommandRegistration {
                command_id: command_id.to_string(),
                callback_proxy_id: callback_proxy_id.to_string(),
            },
        });
    }

    /// Number of currently registered commands.
    pub fn command_count(&self) -> usize {
        self.bridge.command_count()
    }

    /// Check whether a command with the given ID is registered.
    pub fn has_command(&self, id: &str) -> bool {
        self.bridge.has_command(id)
    }

    /// Remove a command by ID. Returns `true` if it was registered.
    pub fn remove_command(&mut self, id: &str) -> bool {
        self.bridge.unregister_command(id)
    }

    /// List all registered command IDs (sorted).
    pub fn list_commands(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.bridge.get_all_commands().iter().map(|s| s.to_string()).collect();
        ids.sort();
        ids
    }

    /// Execute a command and return a `CommandProxyResult` with the command ID.
    pub fn execute_with_result(
        &mut self,
        command_id: &str,
        args: &[Value],
    ) -> CommandProxyResult {
        self.history.record_execution(command_id);
        let inner = self.bridge.execute_with_result(command_id, args);
        CommandProxyResult {
            command_id: command_id.to_string(),
            success: inner.success,
            result: inner.value,
            error: inner.error_message,
        }
    }

    /// Find all registered command IDs that start with `prefix`.
    pub fn find_commands(&self, prefix: &str) -> Vec<String> {
        let mut matches: Vec<String> = self
            .bridge
            .get_all_commands()
            .into_iter()
            .filter(|id| id.starts_with(prefix))
            .map(|s| s.to_string())
            .collect();
        matches.sort();
        matches
    }

    /// Return a snapshot of proxy statistics.
    pub fn stats(&self) -> CommandStats {
        let last = self.history.last_execution().map(|s| s.to_string());
        CommandStats {
            total_registered: self.bridge.command_count(),
            total_executed: self.history.execution_count(),
            last_execution: last,
        }
    }

    /// Borrow the underlying command history.
    pub fn history(&self) -> &CommandHistory {
        &self.history
    }
}

// ── CommandProxyResult ──

/// Structured result returned by `CommandProxy::execute_with_result`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandProxyResult {
    pub command_id: String,
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

// ── CommandStats ──

/// Aggregate statistics about registered and executed commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandStats {
    pub total_registered: usize,
    pub total_executed: usize,
    pub last_execution: Option<String>,
}

/// Initialize the commands extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-commands operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtCommandsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtCommandsStats {
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
    pub fn merge(&mut self, other: &ExtCommandsStats) {
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

impl Default for ExtCommandsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtCommandsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtCommandsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-commands.
#[derive(Debug, Clone)]
pub struct ExtCommandsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtCommandsValidator {
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

impl Default for ExtCommandsValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Rich metadata for a command, supporting builder-style construction.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandDescription {
    pub command_id: String,
    pub title: String,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub keybinding: Option<String>,
    pub when_clause: Option<String>,
}

impl CommandDescription {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            command_id: id.to_string(),
            title: title.to_string(),
            category: None,
            icon: None,
            keybinding: None,
            when_clause: None,
        }
    }

    pub fn with_category(mut self, cat: &str) -> Self {
        self.category = Some(cat.to_string());
        self
    }

    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    pub fn with_keybinding(mut self, kb: &str) -> Self {
        self.keybinding = Some(kb.to_string());
        self
    }

    pub fn with_when(mut self, when: &str) -> Self {
        self.when_clause = Some(when.to_string());
        self
    }

    /// Returns "Category: Title" if a category is set, otherwise just "Title".
    pub fn display_label(&self) -> String {
        match &self.category {
            Some(cat) => format!("{}: {}", cat, self.title),
            None => self.title.clone(),
        }
    }

    pub fn has_keybinding(&self) -> bool {
        self.keybinding.is_some()
    }
}

/// Generates a command-palette display string for a command description.
pub fn command_palette_entry(desc: &CommandDescription) -> String {
    let label = desc.display_label();
    match &desc.keybinding {
        Some(kb) => format!(">{label}  ({kb})"),
        None => format!(">{label}"),
    }
}

/// Tracks command invocation counts and durations for telemetry purposes.
#[derive(Debug, Clone)]
pub struct CommandTelemetry {
    invocations: std::collections::HashMap<String, Vec<u64>>,
}

impl CommandTelemetry {
    pub fn new() -> Self {
        Self {
            invocations: std::collections::HashMap::new(),
        }
    }

    pub fn record_invocation(&mut self, command_id: &str, duration_ms: u64) {
        self.invocations
            .entry(command_id.to_string())
            .or_default()
            .push(duration_ms);
    }

    pub fn invocation_count(&self, command_id: &str) -> u64 {
        self.invocations
            .get(command_id)
            .map_or(0, |v| v.len() as u64)
    }

    pub fn total_invocations(&self) -> u64 {
        self.invocations.values().map(|v| v.len() as u64).sum()
    }

    pub fn average_duration_ms(&self, command_id: &str) -> Option<f64> {
        self.invocations.get(command_id).and_then(|durations| {
            if durations.is_empty() {
                None
            } else {
                let sum: u64 = durations.iter().sum();
                Some(sum as f64 / durations.len() as f64)
            }
        })
    }

    /// Returns the top `n` commands sorted by invocation count (descending).
    pub fn most_used(&self, n: usize) -> Vec<(&str, u64)> {
        let mut entries: Vec<(&str, u64)> = self
            .invocations
            .iter()
            .map(|(id, v)| (id.as_str(), v.len() as u64))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    pub fn unique_commands(&self) -> usize {
        self.invocations.len()
    }
}

impl Default for CommandTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

/// Combines command descriptions and telemetry into a single registry.
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    descriptions: std::collections::HashMap<String, CommandDescription>,
    pub telemetry: CommandTelemetry,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            descriptions: std::collections::HashMap::new(),
            telemetry: CommandTelemetry::new(),
        }
    }

    pub fn register_description(&mut self, desc: CommandDescription) {
        self.descriptions.insert(desc.command_id.clone(), desc);
    }

    pub fn get_description(&self, id: &str) -> Option<&CommandDescription> {
        self.descriptions.get(id)
    }

    /// Searches descriptions by substring match on title or category.
    pub fn search(&self, query: &str) -> Vec<&CommandDescription> {
        let q = query.to_lowercase();
        self.descriptions
            .values()
            .filter(|d| {
                d.title.to_lowercase().contains(&q)
                    || d.category
                        .as_ref()
                        .is_some_and(|c| c.to_lowercase().contains(&q))
            })
            .collect()
    }

    pub fn all_descriptions(&self) -> Vec<&CommandDescription> {
        self.descriptions.values().collect()
    }

    pub fn by_category(&self, category: &str) -> Vec<&CommandDescription> {
        self.descriptions
            .values()
            .filter(|d| d.category.as_deref() == Some(category))
            .collect()
    }

    pub fn description_count(&self) -> usize {
        self.descriptions.len()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandThrottler
// ---------------------------------------------------------------------------

/// Throttles command execution by tracking invocation timestamps per command.
#[derive(Debug, Clone)]
pub struct CommandThrottler {
    /// Minimum interval in milliseconds between executions of the same command.
    interval_ms: u64,
    /// Last execution timestamp (ms) per command.
    last_execution: HashMap<String, u64>,
}

impl CommandThrottler {
    /// Create a new throttler with the given minimum interval.
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_execution: HashMap::new(),
        }
    }

    /// Check if a command may execute at the given timestamp.
    pub fn may_execute(&self, command_id: &str, now_ms: u64) -> bool {
        match self.last_execution.get(command_id) {
            Some(&last) => now_ms.saturating_sub(last) >= self.interval_ms,
            None => true,
        }
    }

    /// Record that a command executed at the given timestamp.
    pub fn record_execution(&mut self, command_id: &str, now_ms: u64) {
        self.last_execution.insert(command_id.to_string(), now_ms);
    }

    /// Try to execute: returns `true` if allowed (and records it), `false` if throttled.
    pub fn try_execute(&mut self, command_id: &str, now_ms: u64) -> bool {
        if self.may_execute(command_id, now_ms) {
            self.record_execution(command_id, now_ms);
            true
        } else {
            false
        }
    }

    /// Remaining cooldown in ms for a command, or 0 if ready.
    pub fn remaining_ms(&self, command_id: &str, now_ms: u64) -> u64 {
        match self.last_execution.get(command_id) {
            Some(&last) => {
                let elapsed = now_ms.saturating_sub(last);
                self.interval_ms.saturating_sub(elapsed)
            }
            None => 0,
        }
    }

    /// Reset throttle state for a specific command.
    pub fn reset(&mut self, command_id: &str) {
        self.last_execution.remove(command_id);
    }

    /// Reset all throttle state.
    pub fn reset_all(&mut self) {
        self.last_execution.clear();
    }
}

// ---------------------------------------------------------------------------
// CommandPermission
// ---------------------------------------------------------------------------

/// Permission level for an extension command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionLevel {
    /// Command is denied.
    Denied,
    /// Requires user confirmation before execution.
    Prompt,
    /// Allowed without confirmation.
    Allowed,
}

impl fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionLevel::Denied => write!(f, "denied"),
            PermissionLevel::Prompt => write!(f, "prompt"),
            PermissionLevel::Allowed => write!(f, "allowed"),
        }
    }
}

/// Permission model for extension commands.
#[derive(Debug, Clone)]
pub struct CommandPermission {
    default_level: PermissionLevel,
    overrides: HashMap<String, PermissionLevel>,
}

impl CommandPermission {
    pub fn new(default_level: PermissionLevel) -> Self {
        Self {
            default_level,
            overrides: HashMap::new(),
        }
    }

    /// Override the permission for a specific command.
    pub fn set_override(&mut self, command_id: &str, level: PermissionLevel) {
        self.overrides.insert(command_id.to_string(), level);
    }

    /// Get the effective permission level for a command.
    pub fn level_for(&self, command_id: &str) -> PermissionLevel {
        self.overrides
            .get(command_id)
            .copied()
            .unwrap_or(self.default_level)
    }

    /// Returns `true` if the command is allowed (without prompt).
    pub fn is_allowed(&self, command_id: &str) -> bool {
        self.level_for(command_id) == PermissionLevel::Allowed
    }

    /// Returns `true` if the command is denied.
    pub fn is_denied(&self, command_id: &str) -> bool {
        self.level_for(command_id) == PermissionLevel::Denied
    }

    /// Remove any override for a command, reverting to default.
    pub fn remove_override(&mut self, command_id: &str) {
        self.overrides.remove(command_id);
    }

    /// Count how many overrides are set.
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }
}

// ---------------------------------------------------------------------------
// CommandBatchExecutor
// ---------------------------------------------------------------------------

/// Result of a single command in a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCommandResult {
    pub command_id: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Executes multiple commands in sequence, collecting results.
#[derive(Debug, Clone)]
pub struct CommandBatchExecutor {
    results: Vec<BatchCommandResult>,
}

impl CommandBatchExecutor {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Simulate executing a batch of commands against a bridge.
    /// Returns results for each command.
    pub fn execute_batch(
        &mut self,
        bridge: &mut CommandBridge,
        command_ids: &[&str],
    ) -> &[BatchCommandResult] {
        let start = self.results.len();
        for &id in command_ids {
            let msg = CommandMessage::ExecuteCommand {
                command_id: id.to_string(),
                args: vec![],
            };
            let resp = bridge.handle(msg);
            let (success, error) = match resp {
                CommandResponse::ExecuteResult { .. } => (true, None),
                _ => (false, Some("unexpected response".to_string())),
            };
            self.results.push(BatchCommandResult {
                command_id: id.to_string(),
                success,
                error,
            });
        }
        &self.results[start..]
    }

    /// Return all collected results.
    pub fn results(&self) -> &[BatchCommandResult] {
        &self.results
    }

    /// Count of successful executions.
    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.success).count()
    }

    /// Count of failed executions.
    pub fn failure_count(&self) -> usize {
        self.results.iter().filter(|r| !r.success).count()
    }

    /// Clear all results.
    pub fn clear(&mut self) {
        self.results.clear();
    }
}

impl Default for CommandBatchExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandAliasRegistry — user-defined command aliases
// ---------------------------------------------------------------------------

/// Maps user-defined alias names to canonical command IDs.
#[derive(Debug, Clone, Default)]
pub struct CommandAliasRegistry {
    aliases: HashMap<String, String>,
}

impl CommandAliasRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an alias that maps to a canonical command ID.
    pub fn add_alias(&mut self, alias: &str, command_id: &str) {
        self.aliases.insert(alias.to_string(), command_id.to_string());
    }

    /// Remove an alias. Returns true if it existed.
    pub fn remove_alias(&mut self, alias: &str) -> bool {
        self.aliases.remove(alias).is_some()
    }

    /// Resolve an alias to its canonical command ID.
    /// If the input is not an alias, returns the input unchanged.
    pub fn resolve<'a>(&'a self, name: &'a str) -> &'a str {
        self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name)
    }

    /// Check if a name is a registered alias.
    pub fn is_alias(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    /// Return all aliases pointing to a given command ID.
    pub fn aliases_for(&self, command_id: &str) -> Vec<&str> {
        self.aliases
            .iter()
            .filter(|(_, v)| v.as_str() == command_id)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Number of registered aliases.
    pub fn count(&self) -> usize {
        self.aliases.len()
    }

    /// Return all alias names sorted alphabetically.
    pub fn all_aliases(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.aliases.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }
}

// ---------------------------------------------------------------------------
// CommandDependencyGraph — declare execution dependencies between commands
// ---------------------------------------------------------------------------

/// Tracks which commands must run before others.
#[derive(Debug, Clone, Default)]
pub struct CommandDependencyGraph {
    /// Maps command_id → list of command_ids that must run first.
    deps: HashMap<String, Vec<String>>,
}

impl CommandDependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare that `command_id` depends on `depends_on`.
    pub fn add_dependency(&mut self, command_id: &str, depends_on: &str) {
        self.deps
            .entry(command_id.to_string())
            .or_default()
            .push(depends_on.to_string());
    }

    /// Return direct dependencies of a command.
    pub fn dependencies_of(&self, command_id: &str) -> &[String] {
        self.deps.get(command_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Return commands that depend on the given command (reverse lookup).
    pub fn dependents_of(&self, command_id: &str) -> Vec<&str> {
        self.deps
            .iter()
            .filter(|(_, deps)| deps.iter().any(|d| d == command_id))
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Produce a topological execution order, or return `None` if cycles exist.
    /// Uses Kahn's algorithm.
    pub fn execution_order(&self) -> Option<Vec<String>> {
        let mut all_nodes: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (k, vs) in &self.deps {
            all_nodes.insert(k.as_str());
            for v in vs {
                all_nodes.insert(v.as_str());
            }
        }

        let mut in_deg: HashMap<&str, usize> = all_nodes.iter().map(|n| (*n, 0)).collect();
        for (node, deps) in &self.deps {
            in_deg.insert(node.as_str(), deps.len());
        }

        let mut queue: std::collections::VecDeque<&str> = in_deg
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(&n, _)| n)
            .collect();
        let mut result = Vec::new();

        while let Some(n) = queue.pop_front() {
            result.push(n.to_string());
            for (node, deps) in &self.deps {
                if deps.iter().any(|d| d == n) {
                    if let Some(deg) = in_deg.get_mut(node.as_str()) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(node.as_str());
                        }
                    }
                }
            }
        }

        if result.len() == all_nodes.len() {
            Some(result)
        } else {
            None // cycle detected
        }
    }

    /// Check if a command has any dependencies declared.
    pub fn has_dependencies(&self, command_id: &str) -> bool {
        self.deps.get(command_id).is_some_and(|v| !v.is_empty())
    }
}

// ---------------------------------------------------------------------------
// CommandSource — tracks whether a command is extension-provided or built-in
// ---------------------------------------------------------------------------

/// Origin of a command registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandSource {
    BuiltIn,
    Extension,
    User,
}

impl fmt::Display for CommandSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandSource::BuiltIn => write!(f, "built-in"),
            CommandSource::Extension => write!(f, "extension"),
            CommandSource::User => write!(f, "user"),
        }
    }
}

/// Extended registration that tracks command source and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEntry {
    pub command_id: String,
    pub source: CommandSource,
    pub extension_id: Option<String>,
    pub overridden_by: Option<String>,
    pub disposed: bool,
}

impl CommandEntry {
    pub fn new(command_id: &str, source: CommandSource) -> Self {
        Self {
            command_id: command_id.to_string(),
            source,
            extension_id: None,
            overridden_by: None,
            disposed: false,
        }
    }

    pub fn with_extension_id(mut self, ext_id: &str) -> Self {
        self.extension_id = Some(ext_id.to_string());
        self
    }

    pub fn is_active(&self) -> bool {
        !self.disposed && self.overridden_by.is_none()
    }
}

// ---------------------------------------------------------------------------
// CommandOverrideManager — manages command override chains
// ---------------------------------------------------------------------------

/// Manages override relationships where one command replaces another.
#[derive(Debug, Clone, Default)]
pub struct CommandOverrideManager {
    entries: HashMap<String, CommandEntry>,
    /// Maps overridden command_id → the command_id that replaced it.
    overrides: HashMap<String, String>,
}

impl CommandOverrideManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a command entry. If a command with the same ID exists and is
    /// active, the old one is marked as overridden.
    pub fn register(&mut self, entry: CommandEntry) -> Option<CommandEntry> {
        let id = entry.command_id.clone();
        let previous = self.entries.insert(id.clone(), entry);
        if let Some(mut prev) = previous {
            prev.overridden_by = Some(id.clone());
            self.overrides.insert(prev.command_id.clone(), id);
            return Some(prev);
        }
        None
    }

    /// Dispose a command, marking it inactive.
    pub fn dispose(&mut self, command_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(command_id) {
            entry.disposed = true;
            true
        } else {
            false
        }
    }

    /// Restore a previously overridden command by removing the override.
    pub fn restore(&mut self, command_id: &str) -> bool {
        if let Some(overrider) = self.overrides.remove(command_id) {
            self.entries.remove(&overrider);
            if let Some(entry) = self.entries.get_mut(command_id) {
                entry.overridden_by = None;
                return true;
            }
        }
        false
    }

    pub fn get(&self, command_id: &str) -> Option<&CommandEntry> {
        self.entries.get(command_id)
    }

    /// Return all active (not disposed, not overridden) command entries.
    pub fn active_commands(&self) -> Vec<&CommandEntry> {
        self.entries.values().filter(|e| e.is_active()).collect()
    }

    /// Return all disposed command IDs.
    pub fn disposed_commands(&self) -> Vec<&str> {
        self.entries
            .values()
            .filter(|e| e.disposed)
            .map(|e| e.command_id.as_str())
            .collect()
    }

    /// Count of commands by source.
    pub fn count_by_source(&self, source: CommandSource) -> usize {
        self.entries.values().filter(|e| e.source == source).count()
    }

    /// Return command IDs registered by a specific extension.
    pub fn commands_by_extension(&self, extension_id: &str) -> Vec<&str> {
        self.entries
            .values()
            .filter(|e| e.extension_id.as_deref() == Some(extension_id))
            .map(|e| e.command_id.as_str())
            .collect()
    }

    pub fn total_count(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// CommandArgValidator — validates arguments before command execution
// ---------------------------------------------------------------------------

/// Describes the expected type of a command argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    String,
    Number,
    Bool,
    Object,
    Array,
    Any,
}

/// Schema for validating command arguments.
#[derive(Debug, Clone)]
pub struct CommandArgSchema {
    pub command_id: String,
    pub required_args: Vec<(String, ArgType)>,
    pub optional_args: Vec<(String, ArgType)>,
    pub min_args: usize,
    pub max_args: Option<usize>,
}

impl CommandArgSchema {
    pub fn new(command_id: &str) -> Self {
        Self {
            command_id: command_id.to_string(),
            required_args: Vec::new(),
            optional_args: Vec::new(),
            min_args: 0,
            max_args: None,
        }
    }

    pub fn require(mut self, name: &str, arg_type: ArgType) -> Self {
        self.required_args.push((name.to_string(), arg_type));
        self.min_args = self.required_args.len();
        self
    }

    pub fn optional(mut self, name: &str, arg_type: ArgType) -> Self {
        self.optional_args.push((name.to_string(), arg_type));
        self
    }

    pub fn max_args(mut self, max: usize) -> Self {
        self.max_args = Some(max);
        self
    }

    /// Validate a list of argument values against this schema.
    pub fn validate(&self, args: &[Value]) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if args.len() < self.min_args {
            errors.push(format!(
                "command '{}' requires at least {} argument(s), got {}",
                self.command_id, self.min_args, args.len()
            ));
        }

        if let Some(max) = self.max_args {
            if args.len() > max {
                errors.push(format!(
                    "command '{}' accepts at most {} argument(s), got {}",
                    self.command_id, max, args.len()
                ));
            }
        }

        for (i, (name, expected_type)) in self.required_args.iter().enumerate() {
            if let Some(val) = args.get(i) {
                if !value_matches_type(val, *expected_type) {
                    errors.push(format!(
                        "argument '{}' (position {}) expected {:?}, got {:?}",
                        name, i, expected_type, json_type_name(val)
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn value_matches_type(val: &Value, expected: ArgType) -> bool {
    match expected {
        ArgType::Any => true,
        ArgType::String => val.is_string(),
        ArgType::Number => val.is_number(),
        ArgType::Bool => val.is_boolean(),
        ArgType::Object => val.is_object(),
        ArgType::Array => val.is_array(),
    }
}

fn json_type_name(val: &Value) -> &'static str {
    match val {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ---------------------------------------------------------------------------
// CommandErrorAggregator — collects and summarizes command errors
// ---------------------------------------------------------------------------

/// A single recorded command error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub command_id: String,
    pub message: String,
    pub is_user_facing: bool,
}

/// Collects errors from command execution for reporting.
#[derive(Debug, Clone, Default)]
pub struct CommandErrorAggregator {
    errors: Vec<CommandError>,
}

impl CommandErrorAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, command_id: &str, message: &str, user_facing: bool) {
        self.errors.push(CommandError {
            command_id: command_id.to_string(),
            message: message.to_string(),
            is_user_facing: user_facing,
        });
    }

    pub fn total_errors(&self) -> usize {
        self.errors.len()
    }

    /// Errors that should be shown to the user.
    pub fn user_facing_errors(&self) -> Vec<&CommandError> {
        self.errors.iter().filter(|e| e.is_user_facing).collect()
    }

    /// All errors for a specific command.
    pub fn errors_for(&self, command_id: &str) -> Vec<&CommandError> {
        self.errors
            .iter()
            .filter(|e| e.command_id == command_id)
            .collect()
    }

    /// Unique command IDs that have errors.
    pub fn failing_commands(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self
            .errors
            .iter()
            .map(|e| e.command_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Produce a summary string of all errors grouped by command.
    pub fn summary(&self) -> String {
        if self.errors.is_empty() {
            return "No errors".to_string();
        }
        let mut by_cmd: HashMap<&str, Vec<&str>> = HashMap::new();
        for err in &self.errors {
            by_cmd
                .entry(err.command_id.as_str())
                .or_default()
                .push(err.message.as_str());
        }
        let mut lines: Vec<String> = by_cmd
            .iter()
            .map(|(cmd, msgs)| format!("{} ({} error(s)): {}", cmd, msgs.len(), msgs.join("; ")))
            .collect();
        lines.sort();
        lines.join("\n")
    }

    pub fn clear(&mut self) {
        self.errors.clear();
    }
}

// ---------------------------------------------------------------------------
// CommandEnablementCondition — evaluates when-clause style conditions
// ---------------------------------------------------------------------------

/// Evaluates simple boolean conditions to determine if a command is enabled.
#[derive(Debug, Clone, Default)]
pub struct CommandEnablementEvaluator {
    context: HashMap<String, bool>,
}

impl CommandEnablementEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a context key to a boolean value.
    pub fn set_context(&mut self, key: &str, value: bool) {
        self.context.insert(key.to_string(), value);
    }

    /// Remove a context key.
    pub fn remove_context(&mut self, key: &str) {
        self.context.remove(key);
    }

    /// Evaluate a simple when-clause. Supports:
    /// - Single key lookup (e.g. `"editorFocus"`)
    /// - Negation with `!` prefix (e.g. `"!editorReadonly"`)
    /// - Conjunction with `&&` (e.g. `"editorFocus && !editorReadonly"`)
    pub fn evaluate(&self, when_clause: &str) -> bool {
        let clause = when_clause.trim();
        if clause.is_empty() {
            return true;
        }
        clause.split("&&").all(|part| {
            let part = part.trim();
            if let Some(key) = part.strip_prefix('!') {
                !self.context.get(key.trim()).copied().unwrap_or(false)
            } else {
                self.context.get(part).copied().unwrap_or(false)
            }
        })
    }

    /// Check if a command described by `desc` is enabled under the current context.
    pub fn is_command_enabled(&self, desc: &CommandDescription) -> bool {
        match &desc.when_clause {
            Some(clause) => self.evaluate(clause),
            None => true,
        }
    }

    /// Filter a list of command descriptions to only those currently enabled.
    pub fn enabled_commands<'a>(
        &self,
        descs: &'a [&CommandDescription],
    ) -> Vec<&'a CommandDescription> {
        descs
            .iter()
            .filter(|d| self.is_command_enabled(d))
            .copied()
            .collect()
    }
}

// ── CommandMetrics ───────────────────────────────────────────────────────

/// Tracks per-command execution metrics.
#[derive(Debug, Clone)]
struct CommandMetricEntry {
    execution_count: u64,
    total_duration_ms: u64,
    error_count: u64,
}

#[derive(Debug, Clone)]
pub struct CommandMetrics {
    metrics: HashMap<String, CommandMetricEntry>,
}

impl CommandMetrics {
    pub fn new() -> Self { Self { metrics: HashMap::new() } }

    pub fn record_execution(&mut self, command_id: &str, duration_ms: u64, is_error: bool) {
        let entry = self.metrics.entry(command_id.to_string()).or_insert(CommandMetricEntry {
            execution_count: 0, total_duration_ms: 0, error_count: 0,
        });
        entry.execution_count += 1;
        entry.total_duration_ms += duration_ms;
        if is_error { entry.error_count += 1; }
    }

    pub fn average_duration(&self, command_id: &str) -> Option<f64> {
        self.metrics.get(command_id).map(|e| {
            if e.execution_count == 0 { 0.0 } else { e.total_duration_ms as f64 / e.execution_count as f64 }
        })
    }

    pub fn error_rate(&self, command_id: &str) -> Option<f64> {
        self.metrics.get(command_id).map(|e| {
            if e.execution_count == 0 { 0.0 } else { e.error_count as f64 / e.execution_count as f64 }
        })
    }

    pub fn execution_count(&self, command_id: &str) -> u64 {
        self.metrics.get(command_id).map_or(0, |e| e.execution_count)
    }

    /// Return top N commands by execution count.
    pub fn top_by_count(&self, n: usize) -> Vec<(String, u64)> {
        let mut entries: Vec<_> = self.metrics.iter().map(|(k, v)| (k.clone(), v.execution_count)).collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    /// Return top N commands by error count.
    pub fn top_by_errors(&self, n: usize) -> Vec<(String, u64)> {
        let mut entries: Vec<_> = self.metrics.iter().map(|(k, v)| (k.clone(), v.error_count)).collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    pub fn tracked_command_count(&self) -> usize { self.metrics.len() }
}

// ── CommandMiddleware ────────────────────────────────────────────────────

/// A middleware hook that can run before/after command execution.
#[derive(Debug, Clone)]
pub struct MiddlewareResult {
    pub proceed: bool,
    pub message: Option<String>,
}

impl MiddlewareResult {
    pub fn allow() -> Self { Self { proceed: true, message: None } }
    pub fn deny(msg: &str) -> Self { Self { proceed: false, message: Some(msg.to_string()) } }
}

/// Validates that a command ID is non-empty.
pub fn validation_middleware(command_id: &str) -> MiddlewareResult {
    if command_id.is_empty() {
        MiddlewareResult::deny("command ID must not be empty")
    } else {
        MiddlewareResult::allow()
    }
}

/// Returns a logging message for a command execution.
pub fn logging_middleware_message(command_id: &str, phase: &str) -> String {
    format!("[{}] command: {}", phase, command_id)
}

// ── CommandBatcher ──────────────────────────────────────────────────────

/// Batches multiple commands for sequential execution.
#[derive(Debug, Clone)]
pub struct CommandBatchEntry {
    pub command_id: String,
    pub args: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct CommandBatchResult {
    pub command_id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommandBatcher {
    commands: Vec<CommandBatchEntry>,
}

impl CommandBatcher {
    pub fn new() -> Self { Self { commands: Vec::new() } }

    pub fn add(&mut self, command_id: &str, args: Option<Value>) {
        self.commands.push(CommandBatchEntry { command_id: command_id.to_string(), args });
    }

    pub fn count(&self) -> usize { self.commands.len() }

    pub fn clear(&mut self) { self.commands.clear(); }

    pub fn commands(&self) -> &[CommandBatchEntry] { &self.commands }

    /// Simulate execution: returns results for each command. Uses a validator function.
    pub fn execute_all<F>(&self, mut executor: F) -> Vec<CommandBatchResult>
    where
        F: FnMut(&str, &Option<Value>) -> Result<(), String>,
    {
        self.commands.iter().map(|entry| {
            match executor(&entry.command_id, &entry.args) {
                Ok(()) => CommandBatchResult { command_id: entry.command_id.clone(), success: true, error: None },
                Err(e) => CommandBatchResult { command_id: entry.command_id.clone(), success: false, error: Some(e) },
            }
        }).collect()
    }

    pub fn successful_count(results: &[CommandBatchResult]) -> usize {
        results.iter().filter(|r| r.success).count()
    }

    pub fn failed_count(results: &[CommandBatchResult]) -> usize {
        results.iter().filter(|r| !r.success).count()
    }
}


// ---------------------------------------------------------------------------
// ext_commands – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtCommandsActivationKind {
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

impl XExtCommandsActivationKind {
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
pub struct XExtCommandsRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtCommandsRpcEnvelope {
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
pub fn x_ext_commands_collect_sequences(envelopes: &[XExtCommandsRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_commands_filter_by_method<'a>(
    envelopes: &'a [XExtCommandsRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtCommandsRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_commands_dedup_by_seq(envelopes: Vec<XExtCommandsRpcEnvelope>) -> Vec<XExtCommandsRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_commands_negotiate_capabilities(
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
pub struct XExtCommandsApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtCommandsApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtCommandsApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bridge_with(ids: &[&str]) -> CommandBridge {
        let mut bridge = CommandBridge::new();
        for id in ids {
            bridge.handle(CommandMessage::RegisterCommand {
                command: CommandRegistration {
                    command_id: id.to_string(),
                    callback_proxy_id: format!("proxy-{}", id),
                },
            });
        }
        bridge
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn register_command() {
        let mut bridge = CommandBridge::new();
        let resp = bridge.handle(CommandMessage::RegisterCommand {
            command: CommandRegistration {
                command_id: "myext.hello".into(),
                callback_proxy_id: "proxy-1".into(),
            },
        });
        assert_eq!(resp, CommandResponse::Registered);
        assert_eq!(bridge.command_count(), 1);
    }

    #[test]
    fn execute_registered_command() {
        let mut bridge = CommandBridge::new();
        bridge.handle(CommandMessage::RegisterCommand {
            command: CommandRegistration {
                command_id: "myext.hello".into(),
                callback_proxy_id: "proxy-1".into(),
            },
        });
        let resp = bridge.handle(CommandMessage::ExecuteCommand {
            command_id: "myext.hello".into(),
            args: vec![Value::String("world".into())],
        });
        assert_eq!(resp, CommandResponse::ExecuteResult { value: Value::Null });
    }

    #[test]
    fn get_commands_list() {
        let mut bridge = CommandBridge::new();
        bridge.handle(CommandMessage::RegisterCommand {
            command: CommandRegistration {
                command_id: "a".into(),
                callback_proxy_id: "p1".into(),
            },
        });
        bridge.handle(CommandMessage::RegisterCommand {
            command: CommandRegistration {
                command_id: "b".into(),
                callback_proxy_id: "p2".into(),
            },
        });
        let resp = bridge.handle(CommandMessage::GetCommands { filter_internal: false });
        if let CommandResponse::CommandList { command_ids } = resp {
            assert_eq!(command_ids.len(), 2);
        } else {
            panic!("expected CommandList");
        }
    }

    #[test]
    fn lookup_command() {
        let mut bridge = CommandBridge::new();
        bridge.handle(CommandMessage::RegisterCommand {
            command: CommandRegistration {
                command_id: "myext.run".into(),
                callback_proxy_id: "proxy-99".into(),
            },
        });
        let reg = bridge.get_command("myext.run").unwrap();
        assert_eq!(reg.callback_proxy_id, "proxy-99");
    }

    #[test]
    fn serde_round_trip() {
        let msg = CommandMessage::ExecuteCommand {
            command_id: "editor.action.format".into(),
            args: vec![Value::Bool(true)],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: CommandMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn unregister_command_works() {
        let mut bridge = make_bridge_with(&["cmd.a", "cmd.b"]);
        assert!(bridge.unregister_command("cmd.a"));
        assert!(!bridge.has_command("cmd.a"));
        assert!(bridge.has_command("cmd.b"));
        assert!(!bridge.unregister_command("cmd.nonexistent"));
    }

    #[test]
    fn has_command_check() {
        let bridge = make_bridge_with(&["editor.format"]);
        assert!(bridge.has_command("editor.format"));
        assert!(!bridge.has_command("editor.missing"));
    }

    #[test]
    fn get_all_commands_returns_ids() {
        let bridge = make_bridge_with(&["a", "b", "c"]);
        let mut cmds = bridge.get_all_commands();
        cmds.sort();
        assert_eq!(cmds, vec!["a", "b", "c"]);
    }

    #[test]
    fn execute_with_result_success() {
        let bridge = make_bridge_with(&["run.me"]);
        let result = bridge.execute_with_result("run.me", &[]);
        assert!(result.success);
        assert!(result.error_message.is_none());
    }

    #[test]
    fn execute_with_result_not_found() {
        let bridge = make_bridge_with(&[]);
        let result = bridge.execute_with_result("missing", &[]);
        assert!(!result.success);
        assert!(result.error_message.unwrap().contains("not found"));
    }

    #[test]
    fn command_execution_result_serde() {
        let result = CommandExecutionResult {
            success: true,
            value: Some(Value::String("ok".into())),
            error_message: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: CommandExecutionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    #[test]
    fn command_history_record_and_recent() {
        let mut history = CommandHistory::new();
        history.record_execution("cmd.a");
        history.record_execution("cmd.b");
        history.record_execution("cmd.c");
        assert_eq!(history.execution_count(), 3);
        let recent = history.get_recent(2);
        assert_eq!(recent, vec!["cmd.c", "cmd.b"]);
    }

    #[test]
    fn command_history_empty() {
        let history = CommandHistory::new();
        assert_eq!(history.execution_count(), 0);
        assert!(history.get_recent(5).is_empty());
    }

    #[test]
    fn command_history_get_recent_more_than_available() {
        let mut history = CommandHistory::new();
        history.record_execution("only");
        let recent = history.get_recent(10);
        assert_eq!(recent, vec!["only"]);
    }

    #[test]
    fn unregister_reduces_count() {
        let mut bridge = make_bridge_with(&["x", "y", "z"]);
        assert_eq!(bridge.command_count(), 3);
        bridge.unregister_command("y");
        assert_eq!(bridge.command_count(), 2);
    }

    // ── CommandProxy tests ──

    fn make_proxy_with(ids: &[&str]) -> CommandProxy {
        let mut proxy = CommandProxy::new();
        for id in ids {
            proxy.register(id, &format!("proxy-{}", id));
        }
        proxy
    }

    #[test]
    fn proxy_command_count() {
        let proxy = make_proxy_with(&["a", "b", "c"]);
        assert_eq!(proxy.command_count(), 3);
    }

    #[test]
    fn proxy_has_command() {
        let proxy = make_proxy_with(&["editor.format"]);
        assert!(proxy.has_command("editor.format"));
        assert!(!proxy.has_command("editor.missing"));
    }

    #[test]
    fn proxy_remove_command() {
        let mut proxy = make_proxy_with(&["a", "b"]);
        assert!(proxy.remove_command("a"));
        assert!(!proxy.has_command("a"));
        assert!(!proxy.remove_command("nonexistent"));
    }

    #[test]
    fn proxy_list_commands_sorted() {
        let proxy = make_proxy_with(&["c", "a", "b"]);
        assert_eq!(proxy.list_commands(), vec!["a", "b", "c"]);
    }

    #[test]
    fn proxy_execute_with_result_success() {
        let mut proxy = make_proxy_with(&["run.me"]);
        let result = proxy.execute_with_result("run.me", &[]);
        assert_eq!(result.command_id, "run.me");
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn proxy_execute_with_result_not_found() {
        let mut proxy = make_proxy_with(&[]);
        let result = proxy.execute_with_result("missing", &[]);
        assert_eq!(result.command_id, "missing");
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("not found"));
    }

    #[test]
    fn proxy_execute_records_history() {
        let mut proxy = make_proxy_with(&["cmd.a"]);
        proxy.execute_with_result("cmd.a", &[]);
        proxy.execute_with_result("cmd.a", &[]);
        assert_eq!(proxy.history().execution_count(), 2);
    }

    #[test]
    fn proxy_find_commands_by_prefix() {
        let proxy = make_proxy_with(&["editor.format", "editor.save", "file.open"]);
        let found = proxy.find_commands("editor.");
        assert_eq!(found, vec!["editor.format", "editor.save"]);
    }

    #[test]
    fn proxy_find_commands_no_match() {
        let proxy = make_proxy_with(&["a.x", "b.y"]);
        assert!(proxy.find_commands("z.").is_empty());
    }

    #[test]
    fn proxy_stats_initial() {
        let proxy = CommandProxy::new();
        let stats = proxy.stats();
        assert_eq!(stats.total_registered, 0);
        assert_eq!(stats.total_executed, 0);
        assert!(stats.last_execution.is_none());
    }

    #[test]
    fn proxy_stats_after_activity() {
        let mut proxy = make_proxy_with(&["cmd.a", "cmd.b"]);
        proxy.execute_with_result("cmd.a", &[]);
        proxy.execute_with_result("cmd.b", &[]);
        let stats = proxy.stats();
        assert_eq!(stats.total_registered, 2);
        assert_eq!(stats.total_executed, 2);
        assert_eq!(stats.last_execution.as_deref(), Some("cmd.b"));
    }

    #[test]
    fn command_proxy_result_serde() {
        let result = CommandProxyResult {
            command_id: "test.cmd".into(),
            success: true,
            result: Some(Value::String("ok".into())),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: CommandProxyResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }

    #[test]
    fn command_stats_serde() {
        let stats = CommandStats {
            total_registered: 5,
            total_executed: 12,
            last_execution: Some("cmd.last".into()),
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: CommandStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, parsed);
    }

    #[test]
    fn command_history_count_for() {
        let mut history = CommandHistory::new();
        history.record_execution("cmd.a");
        history.record_execution("cmd.b");
        history.record_execution("cmd.a");
        assert_eq!(history.count_for("cmd.a"), 2);
        assert_eq!(history.count_for("cmd.b"), 1);
        assert_eq!(history.count_for("cmd.c"), 0);
    }

    #[test]
    fn command_history_clear() {
        let mut history = CommandHistory::new();
        history.record_execution("cmd.a");
        history.record_execution("cmd.b");
        history.clear();
        assert_eq!(history.execution_count(), 0);
        assert!(history.last_execution().is_none());
    }

    #[test]
    fn command_history_last_execution() {
        let mut history = CommandHistory::new();
        assert!(history.last_execution().is_none());
        history.record_execution("first");
        history.record_execution("second");
        assert_eq!(history.last_execution(), Some("second"));
    }

    #[test]
    fn proxy_register_overwrites() {
        let mut proxy = CommandProxy::new();
        proxy.register("cmd.a", "proxy-1");
        proxy.register("cmd.a", "proxy-2");
        assert_eq!(proxy.command_count(), 1);
    }

    #[test]
    fn proxy_remove_then_find() {
        let mut proxy = make_proxy_with(&["ns.a", "ns.b", "ns.c"]);
        proxy.remove_command("ns.b");
        let found = proxy.find_commands("ns.");
        assert_eq!(found, vec!["ns.a", "ns.c"]);
    }

    #[test]
    fn ext_commands_stats_new_defaults() {
        let stats = ExtCommandsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_commands_stats_record_success() {
        let mut stats = ExtCommandsStats::new();
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
    fn ext_commands_stats_record_failure() {
        let mut stats = ExtCommandsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_commands_stats_reset() {
        let mut stats = ExtCommandsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_commands_stats_merge() {
        let mut a = ExtCommandsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtCommandsStats::new();
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
    fn ext_commands_stats_display() {
        let mut stats = ExtCommandsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_commands_stats_default() {
        let stats = ExtCommandsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_commands_validator_accepts_valid_name() {
        let v = ExtCommandsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_commands_validator_rejects_empty() {
        let v = ExtCommandsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_commands_validator_rejects_too_long() {
        let v = ExtCommandsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_commands_validator_forbidden_prefix() {
        let v = ExtCommandsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_commands_validator_allowed_chars() {
        let v = ExtCommandsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_commands_validator_range() {
        let v = ExtCommandsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_commands_sanitize_removes_control() {
        let result = ExtCommandsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_commands_truncate_short_string() {
        assert_eq!(ExtCommandsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_commands_truncate_long_string() {
        let result = ExtCommandsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_commands_is_ascii_printable() {
        assert!(ExtCommandsValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtCommandsValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn command_description_builder() {
        let desc = CommandDescription::new("editor.save", "Save File")
            .with_category("File")
            .with_icon("save-icon")
            .with_keybinding("Ctrl+S")
            .with_when("editorTextFocus");
        assert_eq!(desc.command_id, "editor.save");
        assert_eq!(desc.title, "Save File");
        assert_eq!(desc.category.as_deref(), Some("File"));
        assert_eq!(desc.icon.as_deref(), Some("save-icon"));
        assert_eq!(desc.keybinding.as_deref(), Some("Ctrl+S"));
        assert_eq!(desc.when_clause.as_deref(), Some("editorTextFocus"));
        assert!(desc.has_keybinding());
    }

    #[test]
    fn command_description_display_label_with_category() {
        let desc = CommandDescription::new("fmt", "Format Document").with_category("Editor");
        assert_eq!(desc.display_label(), "Editor: Format Document");
    }

    #[test]
    fn command_description_display_label_without_category() {
        let desc = CommandDescription::new("about", "About");
        assert_eq!(desc.display_label(), "About");
        assert!(!desc.has_keybinding());
    }

    #[test]
    fn command_palette_entry_with_keybinding() {
        let desc = CommandDescription::new("save", "Save")
            .with_category("File")
            .with_keybinding("Ctrl+S");
        let entry = command_palette_entry(&desc);
        assert_eq!(entry, ">File: Save  (Ctrl+S)");
    }

    #[test]
    fn command_palette_entry_without_keybinding() {
        let desc = CommandDescription::new("about", "About");
        let entry = command_palette_entry(&desc);
        assert_eq!(entry, ">About");
    }

    #[test]
    fn telemetry_tracks_invocations() {
        let mut t = CommandTelemetry::new();
        t.record_invocation("cmd.a", 10);
        t.record_invocation("cmd.a", 20);
        t.record_invocation("cmd.b", 5);
        assert_eq!(t.invocation_count("cmd.a"), 2);
        assert_eq!(t.invocation_count("cmd.b"), 1);
        assert_eq!(t.invocation_count("cmd.c"), 0);
        assert_eq!(t.total_invocations(), 3);
        assert_eq!(t.unique_commands(), 2);
        let avg = t.average_duration_ms("cmd.a").unwrap();
        assert!((avg - 15.0).abs() < f64::EPSILON);
        assert!(t.average_duration_ms("cmd.c").is_none());
    }

    #[test]
    fn telemetry_most_used_returns_sorted() {
        let mut t = CommandTelemetry::new();
        for _ in 0..5 {
            t.record_invocation("cmd.x", 1);
        }
        for _ in 0..3 {
            t.record_invocation("cmd.y", 1);
        }
        t.record_invocation("cmd.z", 1);
        let top = t.most_used(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "cmd.x");
        assert_eq!(top[0].1, 5);
        assert_eq!(top[1].0, "cmd.y");
        assert_eq!(top[1].1, 3);
    }

    #[test]
    fn registry_search_finds_by_title() {
        let mut reg = CommandRegistry::new();
        reg.register_description(CommandDescription::new("a", "Open File").with_category("File"));
        reg.register_description(CommandDescription::new("b", "Close Editor"));
        reg.register_description(CommandDescription::new("c", "Open Terminal"));
        let results = reg.search("open");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|d| d.command_id == "a"));
        assert!(results.iter().any(|d| d.command_id == "c"));
    }

    #[test]
    fn registry_by_category_filters() {
        let mut reg = CommandRegistry::new();
        reg.register_description(CommandDescription::new("a", "Save").with_category("File"));
        reg.register_description(CommandDescription::new("b", "Undo").with_category("Edit"));
        reg.register_description(CommandDescription::new("c", "Open").with_category("File"));
        let file_cmds = reg.by_category("File");
        assert_eq!(file_cmds.len(), 2);
        assert!(file_cmds.iter().all(|d| d.category.as_deref() == Some("File")));
        assert_eq!(reg.description_count(), 3);
        assert!(reg.get_description("b").is_some());
        assert!(reg.get_description("missing").is_none());
    }

    // -- CommandThrottler --

    #[test]
    fn throttler_allows_first_call() {
        let mut throttler = CommandThrottler::new(100);
        assert!(throttler.try_execute("cmd.save", 0));
    }

    #[test]
    fn throttler_blocks_rapid_calls() {
        let mut throttler = CommandThrottler::new(100);
        assert!(throttler.try_execute("cmd.save", 0));
        assert!(!throttler.try_execute("cmd.save", 50));
        assert!(throttler.try_execute("cmd.save", 100));
    }

    #[test]
    fn throttler_remaining_ms() {
        let mut throttler = CommandThrottler::new(100);
        throttler.record_execution("cmd.save", 200);
        assert_eq!(throttler.remaining_ms("cmd.save", 250), 50);
        assert_eq!(throttler.remaining_ms("cmd.save", 300), 0);
        assert_eq!(throttler.remaining_ms("cmd.unknown", 0), 0);
    }

    #[test]
    fn throttler_reset() {
        let mut throttler = CommandThrottler::new(100);
        throttler.record_execution("cmd.save", 0);
        assert!(!throttler.may_execute("cmd.save", 50));
        throttler.reset("cmd.save");
        assert!(throttler.may_execute("cmd.save", 50));
    }

    // -- CommandPermission --

    #[test]
    fn permission_default_and_override() {
        let mut perm = CommandPermission::new(PermissionLevel::Prompt);
        assert_eq!(perm.level_for("cmd.open"), PermissionLevel::Prompt);
        assert!(!perm.is_allowed("cmd.open"));

        perm.set_override("cmd.open", PermissionLevel::Allowed);
        assert!(perm.is_allowed("cmd.open"));
        assert_eq!(perm.override_count(), 1);

        perm.remove_override("cmd.open");
        assert!(!perm.is_allowed("cmd.open"));
    }

    #[test]
    fn permission_denied() {
        let mut perm = CommandPermission::new(PermissionLevel::Allowed);
        perm.set_override("cmd.dangerous", PermissionLevel::Denied);
        assert!(perm.is_denied("cmd.dangerous"));
        assert!(!perm.is_denied("cmd.safe"));
    }

    #[test]
    fn permission_level_display() {
        assert_eq!(format!("{}", PermissionLevel::Denied), "denied");
        assert_eq!(format!("{}", PermissionLevel::Prompt), "prompt");
        assert_eq!(format!("{}", PermissionLevel::Allowed), "allowed");
    }

    // -- CommandBatchExecutor --

    #[test]
    fn batch_executor_registered_commands() {
        let mut bridge = make_bridge_with(&["cmd.a", "cmd.b"]);
        let mut batch = CommandBatchExecutor::new();
        batch.execute_batch(&mut bridge, &["cmd.a", "cmd.b"]);
        assert_eq!(batch.success_count(), 2);
        assert_eq!(batch.failure_count(), 0);
    }

    #[test]
    fn batch_executor_clear() {
        let mut bridge = make_bridge_with(&["cmd.a"]);
        let mut batch = CommandBatchExecutor::new();
        batch.execute_batch(&mut bridge, &["cmd.a"]);
        assert_eq!(batch.results().len(), 1);
        batch.clear();
        assert_eq!(batch.results().len(), 0);
    }

    // -- CommandAliasRegistry --

    #[test]
    fn alias_registry_resolve() {
        let mut reg = CommandAliasRegistry::new();
        reg.add_alias("fmt", "editor.format");
        reg.add_alias("save", "editor.save");
        assert_eq!(reg.resolve("fmt"), "editor.format");
        assert_eq!(reg.resolve("save"), "editor.save");
        assert_eq!(reg.resolve("unknown"), "unknown");
        assert!(reg.is_alias("fmt"));
        assert!(!reg.is_alias("editor.format"));
    }

    #[test]
    fn alias_registry_remove_and_count() {
        let mut reg = CommandAliasRegistry::new();
        reg.add_alias("fmt", "editor.format");
        reg.add_alias("s", "editor.save");
        assert_eq!(reg.count(), 2);
        assert!(reg.remove_alias("fmt"));
        assert_eq!(reg.count(), 1);
        assert!(!reg.remove_alias("fmt"));
    }

    #[test]
    fn alias_registry_aliases_for() {
        let mut reg = CommandAliasRegistry::new();
        reg.add_alias("fmt", "editor.format");
        reg.add_alias("f", "editor.format");
        reg.add_alias("s", "editor.save");
        let mut aliases = reg.aliases_for("editor.format");
        aliases.sort();
        assert_eq!(aliases, vec!["f", "fmt"]);
        assert_eq!(reg.aliases_for("editor.save"), vec!["s"]);
    }

    #[test]
    fn alias_registry_all_aliases_sorted() {
        let mut reg = CommandAliasRegistry::new();
        reg.add_alias("z", "cmd.z");
        reg.add_alias("a", "cmd.a");
        reg.add_alias("m", "cmd.m");
        assert_eq!(reg.all_aliases(), vec!["a", "m", "z"]);
    }

    // -- CommandDependencyGraph --

    #[test]
    fn dependency_graph_basic() {
        let mut graph = CommandDependencyGraph::new();
        graph.add_dependency("build", "compile");
        graph.add_dependency("build", "lint");
        assert_eq!(graph.dependencies_of("build"), &["compile", "lint"]);
        assert!(graph.has_dependencies("build"));
        assert!(!graph.has_dependencies("compile"));
    }

    #[test]
    fn dependency_graph_dependents_of() {
        let mut graph = CommandDependencyGraph::new();
        graph.add_dependency("test", "build");
        graph.add_dependency("deploy", "build");
        let mut deps = graph.dependents_of("build");
        deps.sort();
        assert_eq!(deps, vec!["deploy", "test"]);
    }

    #[test]
    fn dependency_graph_execution_order() {
        let mut graph = CommandDependencyGraph::new();
        graph.add_dependency("test", "build");
        graph.add_dependency("build", "compile");
        let order = graph.execution_order().unwrap();
        let compile_pos = order.iter().position(|s| s == "compile").unwrap();
        let build_pos = order.iter().position(|s| s == "build").unwrap();
        let test_pos = order.iter().position(|s| s == "test").unwrap();
        assert!(compile_pos < build_pos);
        assert!(build_pos < test_pos);
    }

    // -- CommandSource & CommandEntry --

    #[test]
    fn command_source_display() {
        assert_eq!(format!("{}", CommandSource::BuiltIn), "built-in");
        assert_eq!(format!("{}", CommandSource::Extension), "extension");
        assert_eq!(format!("{}", CommandSource::User), "user");
    }

    #[test]
    fn command_source_serde_round_trip() {
        let src = CommandSource::Extension;
        let json = serde_json::to_string(&src).unwrap();
        let parsed: CommandSource = serde_json::from_str(&json).unwrap();
        assert_eq!(src, parsed);
    }

    #[test]
    fn command_entry_active_logic() {
        let entry = CommandEntry::new("cmd.a", CommandSource::Extension);
        assert!(entry.is_active());

        let mut disposed = entry.clone();
        disposed.disposed = true;
        assert!(!disposed.is_active());

        let mut overridden = CommandEntry::new("cmd.a", CommandSource::Extension);
        overridden.overridden_by = Some("cmd.b".to_string());
        assert!(!overridden.is_active());
    }

    #[test]
    fn command_entry_with_extension_id() {
        let entry = CommandEntry::new("cmd.a", CommandSource::Extension)
            .with_extension_id("my-ext");
        assert_eq!(entry.extension_id.as_deref(), Some("my-ext"));
    }

    // -- CommandOverrideManager --

    #[test]
    fn override_manager_register_and_dispose() {
        let mut mgr = CommandOverrideManager::new();
        let entry = CommandEntry::new("cmd.save", CommandSource::BuiltIn);
        assert!(mgr.register(entry).is_none());
        assert_eq!(mgr.total_count(), 1);
        assert_eq!(mgr.active_commands().len(), 1);

        assert!(mgr.dispose("cmd.save"));
        assert!(mgr.disposed_commands().contains(&"cmd.save"));
        assert_eq!(mgr.active_commands().len(), 0);
        assert!(!mgr.dispose("nonexistent"));
    }

    #[test]
    fn override_manager_override_and_restore() {
        let mut mgr = CommandOverrideManager::new();
        let original = CommandEntry::new("cmd.save", CommandSource::BuiltIn);
        mgr.register(original);

        let replacement = CommandEntry::new("cmd.save", CommandSource::Extension);
        let prev = mgr.register(replacement);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().source, CommandSource::BuiltIn);
    }

    #[test]
    fn override_manager_count_by_source() {
        let mut mgr = CommandOverrideManager::new();
        mgr.register(CommandEntry::new("a", CommandSource::BuiltIn));
        mgr.register(CommandEntry::new("b", CommandSource::Extension));
        mgr.register(CommandEntry::new("c", CommandSource::Extension));
        assert_eq!(mgr.count_by_source(CommandSource::BuiltIn), 1);
        assert_eq!(mgr.count_by_source(CommandSource::Extension), 2);
        assert_eq!(mgr.count_by_source(CommandSource::User), 0);
    }

    #[test]
    fn override_manager_commands_by_extension() {
        let mut mgr = CommandOverrideManager::new();
        mgr.register(CommandEntry::new("a", CommandSource::Extension).with_extension_id("ext1"));
        mgr.register(CommandEntry::new("b", CommandSource::Extension).with_extension_id("ext1"));
        mgr.register(CommandEntry::new("c", CommandSource::Extension).with_extension_id("ext2"));
        let mut ext1_cmds = mgr.commands_by_extension("ext1");
        ext1_cmds.sort();
        assert_eq!(ext1_cmds, vec!["a", "b"]);
        assert_eq!(mgr.commands_by_extension("ext2"), vec!["c"]);
        assert!(mgr.commands_by_extension("ext3").is_empty());
    }

    // -- CommandArgValidator --

    #[test]
    fn arg_schema_validates_required_args() {
        let schema = CommandArgSchema::new("editor.goto")
            .require("line", ArgType::Number)
            .require("column", ArgType::Number);

        let good = vec![Value::from(10), Value::from(5)];
        assert!(schema.validate(&good).is_ok());

        let too_few: Vec<Value> = vec![Value::from(10)];
        let errs = schema.validate(&too_few).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("at least 2")));

        let wrong_type = vec![Value::from("not a number"), Value::from(5)];
        let errs = schema.validate(&wrong_type).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("expected Number")));
    }

    #[test]
    fn arg_schema_max_args_enforced() {
        let schema = CommandArgSchema::new("cmd.simple")
            .require("name", ArgType::String)
            .max_args(2);

        let ok = vec![Value::from("hello"), Value::from(42)];
        assert!(schema.validate(&ok).is_ok());

        let too_many = vec![Value::from("a"), Value::from("b"), Value::from("c")];
        let errs = schema.validate(&too_many).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("at most 2")));
    }

    #[test]
    fn arg_schema_any_type_accepts_all() {
        let schema = CommandArgSchema::new("cmd.flex").require("arg", ArgType::Any);
        assert!(schema.validate(&[Value::from("str")]).is_ok());
        assert!(schema.validate(&[Value::from(42)]).is_ok());
        assert!(schema.validate(&[Value::Bool(true)]).is_ok());
        assert!(schema.validate(&[Value::Null]).is_ok());
    }

    // -- CommandErrorAggregator --

    #[test]
    fn error_aggregator_record_and_query() {
        let mut agg = CommandErrorAggregator::new();
        agg.record("cmd.a", "file not found", true);
        agg.record("cmd.a", "timeout", false);
        agg.record("cmd.b", "permission denied", true);

        assert_eq!(agg.total_errors(), 3);
        assert_eq!(agg.errors_for("cmd.a").len(), 2);
        assert_eq!(agg.errors_for("cmd.b").len(), 1);
        assert_eq!(agg.errors_for("cmd.c").len(), 0);

        assert_eq!(agg.user_facing_errors().len(), 2);
        assert_eq!(agg.failing_commands(), vec!["cmd.a", "cmd.b"]);
    }

    #[test]
    fn error_aggregator_summary_and_clear() {
        let mut agg = CommandErrorAggregator::new();
        assert_eq!(agg.summary(), "No errors");

        agg.record("cmd.a", "err1", true);
        agg.record("cmd.a", "err2", false);
        let summary = agg.summary();
        assert!(summary.contains("cmd.a"));
        assert!(summary.contains("2 error(s)"));

        agg.clear();
        assert_eq!(agg.total_errors(), 0);
        assert_eq!(agg.summary(), "No errors");
    }

    // -- CommandEnablementEvaluator --

    #[test]
    fn enablement_simple_key() {
        let mut eval = CommandEnablementEvaluator::new();
        eval.set_context("editorFocus", true);
        assert!(eval.evaluate("editorFocus"));
        assert!(!eval.evaluate("editorReadonly"));
    }

    #[test]
    fn enablement_negation() {
        let mut eval = CommandEnablementEvaluator::new();
        eval.set_context("editorReadonly", false);
        assert!(eval.evaluate("!editorReadonly"));
        eval.set_context("editorReadonly", true);
        assert!(!eval.evaluate("!editorReadonly"));
    }

    #[test]
    fn enablement_conjunction() {
        let mut eval = CommandEnablementEvaluator::new();
        eval.set_context("editorFocus", true);
        eval.set_context("editorReadonly", false);
        assert!(eval.evaluate("editorFocus && !editorReadonly"));
        eval.set_context("editorReadonly", true);
        assert!(!eval.evaluate("editorFocus && !editorReadonly"));
    }

    #[test]
    fn enablement_empty_clause_always_true() {
        let eval = CommandEnablementEvaluator::new();
        assert!(eval.evaluate(""));
        assert!(eval.evaluate("   "));
    }

    #[test]
    fn enablement_filters_command_descriptions() {
        let mut eval = CommandEnablementEvaluator::new();
        eval.set_context("editorFocus", true);
        eval.set_context("terminalFocus", false);

        let cmd_a = CommandDescription::new("a", "A").with_when("editorFocus");
        let cmd_b = CommandDescription::new("b", "B").with_when("terminalFocus");
        let cmd_c = CommandDescription::new("c", "C"); // no when clause → always enabled

        assert!(eval.is_command_enabled(&cmd_a));
        assert!(!eval.is_command_enabled(&cmd_b));
        assert!(eval.is_command_enabled(&cmd_c));

        let all = vec![&cmd_a, &cmd_b, &cmd_c];
        let enabled = eval.enabled_commands(&all);
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().any(|d| d.command_id == "a"));
        assert!(enabled.iter().any(|d| d.command_id == "c"));
    }

    #[test]
    fn enablement_remove_context() {
        let mut eval = CommandEnablementEvaluator::new();
        eval.set_context("key", true);
        assert!(eval.evaluate("key"));
        eval.remove_context("key");
        assert!(!eval.evaluate("key"));
    }

    // ── CommandMetrics tests ──

    #[test]
    fn metrics_record_and_query() {
        let mut m = CommandMetrics::new();
        m.record_execution("cmd.a", 10, false);
        m.record_execution("cmd.a", 20, false);
        m.record_execution("cmd.a", 30, true);
        assert_eq!(m.execution_count("cmd.a"), 3);
        assert!((m.average_duration("cmd.a").unwrap() - 20.0).abs() < 0.01);
    }

    #[test]
    fn metrics_error_rate() {
        let mut m = CommandMetrics::new();
        m.record_execution("cmd.b", 10, true);
        m.record_execution("cmd.b", 10, false);
        assert!((m.error_rate("cmd.b").unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn metrics_top_by_count() {
        let mut m = CommandMetrics::new();
        m.record_execution("a", 1, false);
        m.record_execution("b", 1, false);
        m.record_execution("b", 1, false);
        let top = m.top_by_count(1);
        assert_eq!(top[0].0, "b");
    }

    #[test]
    fn metrics_unknown_command() {
        let m = CommandMetrics::new();
        assert!(m.average_duration("nope").is_none());
        assert_eq!(m.execution_count("nope"), 0);
    }

    // ── CommandMiddleware tests ──

    #[test]
    fn validation_middleware_allows_valid() {
        let r = validation_middleware("cmd.test");
        assert!(r.proceed);
    }

    #[test]
    fn validation_middleware_denies_empty() {
        let r = validation_middleware("");
        assert!(!r.proceed);
        assert!(r.message.is_some());
    }

    #[test]
    fn logging_middleware_msg() {
        let msg = logging_middleware_message("cmd.run", "pre");
        assert!(msg.contains("pre"));
        assert!(msg.contains("cmd.run"));
    }

    // ── CommandBatcher tests ──

    #[test]
    fn batcher_add_and_count() {
        let mut b = CommandBatcher::new();
        b.add("cmd.a", None);
        b.add("cmd.b", Some(serde_json::json!({"x": 1})));
        assert_eq!(b.count(), 2);
    }

    #[test]
    fn batcher_execute_all_success() {
        let mut b = CommandBatcher::new();
        b.add("cmd.a", None);
        b.add("cmd.b", None);
        let results = b.execute_all(|_, _| Ok(()));
        assert_eq!(CommandBatcher::successful_count(&results), 2);
        assert_eq!(CommandBatcher::failed_count(&results), 0);
    }

    #[test]
    fn batcher_execute_partial_failure() {
        let mut b = CommandBatcher::new();
        b.add("cmd.a", None);
        b.add("cmd.fail", None);
        let results = b.execute_all(|id, _| {
            if id == "cmd.fail" { Err("boom".into()) } else { Ok(()) }
        });
        assert_eq!(CommandBatcher::successful_count(&results), 1);
        assert_eq!(CommandBatcher::failed_count(&results), 1);
    }

    #[test]
    fn batcher_clear() {
        let mut b = CommandBatcher::new();
        b.add("cmd.a", None);
        b.clear();
        assert_eq!(b.count(), 0);
    }

    // -- ext_commands additional tests -------------------------------------------

    #[test]
    fn x_ext_commands_activation_parse_language() {
        let ak = XExtCommandsActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtCommandsActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_commands_activation_parse_command() {
        let ak = XExtCommandsActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtCommandsActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_commands_activation_parse_star() {
        assert_eq!(XExtCommandsActivationKind::parse("*"), Some(XExtCommandsActivationKind::Star));
    }

    #[test]
    fn x_ext_commands_activation_parse_unknown() {
        assert!(XExtCommandsActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_commands_activation_parse_workspace() {
        let ak = XExtCommandsActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtCommandsActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_commands_rpc_envelope_basic() {
        let env = XExtCommandsRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_commands_rpc_envelope_response() {
        let env = XExtCommandsRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_commands_rpc_payload_checksum() {
        let env = XExtCommandsRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_commands_collect_sequences_works() {
        let envs = vec![
            XExtCommandsRpcEnvelope::new(10, "a", ""),
            XExtCommandsRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_commands_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_commands_filter_by_method_works() {
        let envs = vec![
            XExtCommandsRpcEnvelope::new(1, "textDocument/open", ""),
            XExtCommandsRpcEnvelope::new(2, "workspace/config", ""),
            XExtCommandsRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_commands_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_commands_dedup_by_seq_works() {
        let envs = vec![
            XExtCommandsRpcEnvelope::new(1, "a", "first"),
            XExtCommandsRpcEnvelope::new(1, "a", "second"),
            XExtCommandsRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_commands_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_commands_negotiate_capabilities_basic() {
        let result = x_ext_commands_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_commands_api_version_satisfies() {
        let v1 = XExtCommandsApiVersion::new(1, 80, 0);
        let min = XExtCommandsApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_commands_api_version_display() {
        let v = XExtCommandsApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_commands_api_version_ord() {
        let v1 = XExtCommandsApiVersion::new(1, 0, 0);
        let v2 = XExtCommandsApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }

}
