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
}
