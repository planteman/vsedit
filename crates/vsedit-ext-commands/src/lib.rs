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
}
