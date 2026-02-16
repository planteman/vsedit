//! Ext API: Output.
//!
//! RPC bridge between the extension host and the main thread for output channels.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_output";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OutputMessage {
    CreateChannel {
        name: String,
        language_id: Option<String>,
    },
    AppendLine {
        channel_id: String,
        line: String,
    },
    Clear {
        channel_id: String,
    },
    Show {
        channel_id: String,
        preserve_focus: bool,
    },
    Dispose {
        channel_id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputChannel {
    pub id: String,
    pub name: String,
    pub language_id: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogOutputChannel {
    pub id: String,
    pub name: String,
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

// ── Bridge ──

pub struct OutputBridge {
    channels: Vec<OutputChannel>,
    next_id: u64,
}

impl OutputBridge {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_channel(&mut self, name: &str, language_id: Option<String>) -> String {
        let id = format!("output-{}", self.next_id);
        self.next_id += 1;
        self.channels.push(OutputChannel {
            id: id.clone(),
            name: name.to_string(),
            language_id,
            content: String::new(),
        });
        id
    }

    pub fn append_line(&mut self, channel_id: &str, line: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel_id) {
            if !ch.content.is_empty() {
                ch.content.push('\n');
            }
            ch.content.push_str(line);
        }
    }

    pub fn clear(&mut self, channel_id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel_id) {
            ch.content.clear();
        }
    }

    pub fn dispose(&mut self, channel_id: &str) {
        self.channels.retain(|c| c.id != channel_id);
    }

    pub fn get_channel(&self, id: &str) -> Option<&OutputChannel> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn handle_message(&mut self, msg: &OutputMessage) -> serde_json::Value {
        match msg {
            OutputMessage::CreateChannel { name, language_id } => {
                let id = self.create_channel(name, language_id.clone());
                serde_json::json!({"channelId": id})
            }
            OutputMessage::AppendLine { channel_id, line } => {
                self.append_line(channel_id, line);
                serde_json::json!({"appended": true})
            }
            OutputMessage::Clear { channel_id } => {
                self.clear(channel_id);
                serde_json::json!({"cleared": true})
            }
            OutputMessage::Show { channel_id, preserve_focus } => {
                let found = self.get_channel(channel_id).is_some();
                serde_json::json!({"shown": found, "preserveFocus": preserve_focus})
            }
            OutputMessage::Dispose { channel_id } => {
                self.dispose(channel_id);
                serde_json::json!({"disposed": true})
            }
        }
    }
}

impl Default for OutputBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the output extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = OutputMessage::AppendLine {
            channel_id: "ch1".into(),
            line: "hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: OutputMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn channel_serialization() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "Build".into(),
            language_id: Some("log".into()),
            content: "line1".into(),
        };
        let json = serde_json::to_string(&ch).unwrap();
        let back: OutputChannel = serde_json::from_str(&json).unwrap();
        assert_eq!(ch, back);
    }

    #[test]
    fn bridge_create_and_append() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Build", None);
        bridge.append_line(&id, "line 1");
        bridge.append_line(&id, "line 2");
        let ch = bridge.get_channel(&id).unwrap();
        assert_eq!(ch.content, "line 1\nline 2");
    }

    #[test]
    fn bridge_clear() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Test", None);
        bridge.append_line(&id, "data");
        bridge.clear(&id);
        let ch = bridge.get_channel(&id).unwrap();
        assert!(ch.content.is_empty());
    }

    #[test]
    fn bridge_dispose() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Temp", None);
        bridge.dispose(&id);
        assert!(bridge.get_channel(&id).is_none());
    }
}
