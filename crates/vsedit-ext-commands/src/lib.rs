//! Ext API: Commands.
//!
//! RPC bridge between the extension host and the main thread for commands.

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
    fn unregister_command() {
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
}
