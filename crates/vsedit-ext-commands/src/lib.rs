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


/// Configuration manager for ext_commands functionality.
pub struct ExtCommandsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtCommandsConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &ExtCommandsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_commands operations.
pub struct ExtCommandsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtCommandsRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for ext_commands.
pub struct ExtCommandsValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtCommandsValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &ExtCommandsValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for ext_commands
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtCommandsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtCommandsRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaExtCommandsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtCommandsCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaExtCommandsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 52
// ---------------------------------------------------------------------------

/// Generic object pool `Xc52Pool<T>`.
pub struct Xc52Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc52Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc52PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc52Pool<T> {
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
    pub fn stats(&self) -> Xc52PoolStats {
        Xc52PoolStats {
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

impl<T> Default for Xc52Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc52Scheduler`.
pub struct Xc52Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc52Scheduler {
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

impl Default for Xc52Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_52 hash for the given byte slice.
pub fn xc_52_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_52 convention.
pub fn xc_52_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_95 deepening: state machine + event bus ---

/// States for the Xd95 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd95State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd95State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd95Transition {
    pub from: Xd95State,
    pub to: Xd95State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd95StateMachine {
    current: Xd95State,
    history: Vec<Xd95Transition>,
    step_counter: usize,
}

impl Xd95StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd95State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd95State {
        self.current
    }

    pub fn history(&self) -> &[Xd95Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd95State) -> Result<Xd95State, String> {
        let allowed = match (self.current, target) {
            (Xd95State::Idle, Xd95State::Running) => true,
            (Xd95State::Running, Xd95State::Paused) => true,
            (Xd95State::Running, Xd95State::Done) => true,
            (Xd95State::Paused, Xd95State::Running) => true,
            (Xd95State::Paused, Xd95State::Done) => true,
            (Xd95State::Done, Xd95State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_95: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd95Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd95SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd95State> {
        let prefix = "Xd95SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd95State::Idle),
            "Running" => Some(Xd95State::Running),
            "Paused" => Some(Xd95State::Paused),
            "Done" => Some(Xd95State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd95State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd95 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd95Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd95Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd95HandlerFn = Box<dyn Fn(&Xd95Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd95EventBus {
    handlers: Vec<(usize, Option<String>, Xd95HandlerFn)>,
    next_id: usize,
    published: Vec<Xd95Event>,
}

impl Xd95EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd95Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd95Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd95Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd95Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_19: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg19Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg19Graph {
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

impl Default for Xg19Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_19: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg19Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg19Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg19Heap<T>) {
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

impl<T: Ord> Default for Xg19Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 51).
pub struct Xh51SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh51SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 93 as u64,
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

/// A compact bit set supporting boolean operations (variant 51).
pub struct Xh51BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh51BitSet {
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
    fn extcommands_validator_accepts_and_rejects() {
        let mut v = ExtCommandsValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extcommands_validator_warnings() {
        let mut v = ExtCommandsValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extcommands_validator_clear_and_merge() {
        let mut v = ExtCommandsValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtCommandsValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtCommandsValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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


    #[test]
    fn ext_commands_config_new() {
        let cfg = ExtCommandsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_commands_config_set_get() {
        let mut cfg = ExtCommandsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_commands_config_remove() {
        let mut cfg = ExtCommandsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_commands_config_keys_sorted() {
        let mut cfg = ExtCommandsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_commands_config_bump_version() {
        let mut cfg = ExtCommandsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_commands_config_clear() {
        let mut cfg = ExtCommandsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_commands_config_merge() {
        let mut cfg1 = ExtCommandsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtCommandsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_commands_config_disable() {
        let mut cfg = ExtCommandsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_commands_rate_tracker_empty() {
        let rt = ExtCommandsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_commands_rate_tracker_record() {
        let mut rt = ExtCommandsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_commands_rate_tracker_prune() {
        let mut rt = ExtCommandsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_commands_validator_valid() {
        let v = ExtCommandsValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_commands_validator_errors() {
        let mut v = ExtCommandsValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_commands_validator_clear() {
        let mut v = ExtCommandsValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_commands_validator_merge() {
        let mut v1 = ExtCommandsValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtCommandsValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_commands_rate_tracker_clear() {
        let mut rt = ExtCommandsRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for ext_commands
    #[test]
    fn xa_ext_commands_ring_new() {
        let rb = super::XaExtCommandsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_commands_ring_push_len() {
        let mut rb = super::XaExtCommandsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_commands_ring_wrap() {
        let mut rb = super::XaExtCommandsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_commands_ring_mean_empty() {
        let rb = super::XaExtCommandsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_commands_ring_mean_values() {
        let mut rb = super::XaExtCommandsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_commands_ring_min_max() {
        let mut rb = super::XaExtCommandsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_commands_ring_iter() {
        let mut rb = super::XaExtCommandsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_commands_counter_new() {
        let c = super::XaExtCommandsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_commands_counter_inc() {
        let mut c = super::XaExtCommandsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_commands_counter_inc_by() {
        let mut c = super::XaExtCommandsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_commands_counter_reset() {
        let mut c = super::XaExtCommandsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_commands_counter_clear() {
        let mut c = super::XaExtCommandsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_commands_counter_default() {
        let c = super::XaExtCommandsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 52 ----

    #[test]
    fn xc_52_pool_new_empty() {
        let pool: super::Xc52Pool<i32> = super::Xc52Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_52_pool_release_acquire() {
        let mut pool = super::Xc52Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_52_pool_acquire_empty() {
        let mut pool: super::Xc52Pool<i32> = super::Xc52Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_52_pool_full() {
        let mut pool = super::Xc52Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_52_pool_drain() {
        let mut pool = super::Xc52Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_52_pool_stats() {
        let mut pool = super::Xc52Pool::new(8);
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
    fn xc_52_pool_clear() {
        let mut pool = super::Xc52Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_52_pool_shrink() {
        let mut pool = super::Xc52Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_52_pool_default() {
        let pool: super::Xc52Pool<String> = super::Xc52Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_52_pool_extend() {
        let mut pool = super::Xc52Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_52_pool_retain() {
        let mut pool = super::Xc52Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_52_scheduler_round_robin() {
        let mut sched = super::Xc52Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_52_scheduler_empty() {
        let mut sched = super::Xc52Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_52_scheduler_reset() {
        let mut sched = super::Xc52Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_52_scheduler_add_remove() {
        let mut sched = super::Xc52Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_52_scheduler_targets() {
        let sched = super::Xc52Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_52_hash_empty() {
        assert_eq!(super::xc_52_hash(b""), 5381);
    }

    #[test]
    fn xc_52_hash_data() {
        let h = super::xc_52_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_52_hash(b"hello"), h);
    }

    #[test]
    fn xc_52_reverse_str() {
        assert_eq!(super::xc_52_reverse("abc"), "cba");
        assert_eq!(super::xc_52_reverse(""), "");
    }


    // --- xd_95 deepening tests ---

    #[test]
    fn xd_95_sm_initial_state() {
        let sm = Xd95StateMachine::new();
        assert_eq!(sm.current_state(), Xd95State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_95_sm_valid_idle_to_running() {
        let mut sm = Xd95StateMachine::new();
        assert!(sm.transition(Xd95State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd95State::Running);
    }

    #[test]
    fn xd_95_sm_valid_running_to_paused() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        assert!(sm.transition(Xd95State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd95State::Paused);
    }

    #[test]
    fn xd_95_sm_valid_running_to_done() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        assert!(sm.transition(Xd95State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd95State::Done);
    }

    #[test]
    fn xd_95_sm_valid_paused_to_running() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        sm.transition(Xd95State::Paused).unwrap();
        assert!(sm.transition(Xd95State::Running).is_ok());
    }

    #[test]
    fn xd_95_sm_valid_done_to_idle() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        sm.transition(Xd95State::Done).unwrap();
        assert!(sm.transition(Xd95State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd95State::Idle);
    }

    #[test]
    fn xd_95_sm_invalid_idle_to_done() {
        let mut sm = Xd95StateMachine::new();
        assert!(sm.transition(Xd95State::Done).is_err());
    }

    #[test]
    fn xd_95_sm_invalid_idle_to_paused() {
        let mut sm = Xd95StateMachine::new();
        assert!(sm.transition(Xd95State::Paused).is_err());
    }

    #[test]
    fn xd_95_sm_history_tracking() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        sm.transition(Xd95State::Paused).unwrap();
        sm.transition(Xd95State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd95State::Idle);
        assert_eq!(sm.history()[0].to, Xd95State::Running);
        assert_eq!(sm.history()[1].from, Xd95State::Running);
        assert_eq!(sm.history()[2].to, Xd95State::Done);
    }

    #[test]
    fn xd_95_sm_serialize_deserialize() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd95StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd95State::Running));
    }

    #[test]
    fn xd_95_sm_deserialize_invalid() {
        assert_eq!(Xd95StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_95_sm_reset() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd95State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_95_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd95EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd95Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_95_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd95EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd95Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd95Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_95_bus_unsubscribe() {
        let mut bus = Xd95EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_95_event_kind_and_payload() {
        let e = Xd95Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd95Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_95_bus_clear_history() {
        let mut bus = Xd95EventBus::new();
        bus.publish(Xd95Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_95_sm_step_counter_increments() {
        let mut sm = Xd95StateMachine::new();
        sm.transition(Xd95State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd95State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_19 graph tests ------------------------------------------------

    #[test]
    fn xg_19_graph_empty() {
        let g = super::Xg19Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_19_graph_add_node() {
        let mut g = super::Xg19Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_19_graph_add_edge() {
        let mut g = super::Xg19Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_19_graph_neighbors() {
        let mut g = super::Xg19Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_19_graph_has_path() {
        let mut g = super::Xg19Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_19_graph_self_path() {
        let g = super::Xg19Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_19_graph_topo_sort() {
        let mut g = super::Xg19Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_19_graph_cycle_detect_false() {
        let mut g = super::Xg19Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_19_graph_cycle_detect_true() {
        let mut g = super::Xg19Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_19 heap tests -------------------------------------------------

    #[test]
    fn xg_19_heap_empty() {
        let h: super::Xg19Heap<i32> = super::Xg19Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_19_heap_push_pop() {
        let mut h = super::Xg19Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_19_heap_peek() {
        let mut h = super::Xg19Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_19_heap_drain_sorted() {
        let mut h = super::Xg19Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_19_heap_merge() {
        let mut a = super::Xg19Heap::new();
        let mut b = super::Xg19Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_19_heap_default() {
        let h: super::Xg19Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_19_graph_default() {
        let g: super::Xg19Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh51_skip_insert_contains() {
        let mut sl = super::Xh51SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh51_skip_remove() {
        let mut sl = super::Xh51SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh51_skip_len() {
        let mut sl = super::Xh51SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh51_skip_range_query() {
        let mut sl = super::Xh51SkipList::xh_new(4);
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
    fn xh51_skip_floor_ceiling() {
        let mut sl = super::Xh51SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh51_skip_rank() {
        let mut sl = super::Xh51SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh51_skip_empty() {
        let sl = super::Xh51SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh51_skip_duplicates() {
        let mut sl = super::Xh51SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh51_bitset_set_test() {
        let mut bs = super::Xh51BitSet::xh_new(256);
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
    fn xh51_bitset_clear_count() {
        let mut bs = super::Xh51BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh51_bitset_and_or_xor() {
        let mut a = super::Xh51BitSet::xh_new(128);
        let mut b = super::Xh51BitSet::xh_new(128);
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
    fn xh51_bitset_iter_ones() {
        let mut bs = super::Xh51BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh51_bitset_first_last() {
        let mut bs = super::Xh51BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh51_bitset_empty() {
        let bs = super::Xh51BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
