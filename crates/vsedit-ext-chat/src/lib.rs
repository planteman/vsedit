//! Ext API: Chat.
//!
//! RPC bridge between the extension host and the main thread for chat/AI integration.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_chat";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatMessage {
    SendRequest {
        participant_id: String,
        message: String,
    },
    ReceiveResponse {
        participant_id: String,
        content: String,
    },
    RegisterParticipant {
        id: String,
        name: String,
    },
    UnregisterParticipant {
        id: String,
    },
    CancelRequest {
        request_id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatParticipant {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatRequest {
    pub id: String,
    pub participant_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatResponse {
    pub request_id: String,
    pub content: String,
    pub is_complete: bool,
}

// ── Bridge ──

pub struct ChatBridge {
    participants: Vec<ChatParticipant>,
}

impl ChatBridge {
    pub fn new() -> Self {
        Self {
            participants: Vec::new(),
        }
    }

    pub fn register_participant(&mut self, participant: ChatParticipant) {
        if !self.participants.iter().any(|p| p.id == participant.id) {
            self.participants.push(participant);
        }
    }

    pub fn unregister_participant(&mut self, id: &str) {
        self.participants.retain(|p| p.id != id);
    }

    pub fn get_participant(&self, id: &str) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.id == id)
    }

    pub fn handle_message(&mut self, msg: &ChatMessage) -> serde_json::Value {
        match msg {
            ChatMessage::RegisterParticipant { id, name } => {
                self.register_participant(ChatParticipant {
                    id: id.clone(),
                    name: name.clone(),
                    description: None,
                    is_default: false,
                });
                serde_json::json!({"registered": true})
            }
            ChatMessage::UnregisterParticipant { id } => {
                self.unregister_participant(id);
                serde_json::json!({"unregistered": true})
            }
            ChatMessage::SendRequest {
                participant_id,
                message,
            } => {
                let found = self.get_participant(participant_id).is_some();
                serde_json::json!({"accepted": found, "message": message})
            }
            ChatMessage::ReceiveResponse {
                participant_id,
                content,
            } => {
                serde_json::json!({"participant": participant_id, "content": content})
            }
            ChatMessage::CancelRequest { request_id } => {
                serde_json::json!({"cancelled": request_id})
            }
        }
    }
}

impl Default for ChatBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the chat extension API bridge.
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
        let msg = ChatMessage::SendRequest {
            participant_id: "copilot".into(),
            message: "hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn participant_serialization() {
        let p = ChatParticipant {
            id: "p1".into(),
            name: "Copilot".into(),
            description: Some("AI assistant".into()),
            is_default: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ChatParticipant = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn bridge_register_and_find() {
        let mut bridge = ChatBridge::new();
        bridge.register_participant(ChatParticipant {
            id: "copilot".into(),
            name: "Copilot".into(),
            description: None,
            is_default: false,
        });
        assert!(bridge.get_participant("copilot").is_some());
        assert!(bridge.get_participant("missing").is_none());
    }

    #[test]
    fn bridge_handle_send_to_unknown() {
        let mut bridge = ChatBridge::new();
        let msg = ChatMessage::SendRequest {
            participant_id: "unknown".into(),
            message: "hi".into(),
        };
        let result = bridge.handle_message(&msg);
        assert_eq!(result["accepted"], false);
    }

    #[test]
    fn bridge_unregister() {
        let mut bridge = ChatBridge::new();
        bridge.register_participant(ChatParticipant {
            id: "p1".into(),
            name: "P".into(),
            description: None,
            is_default: false,
        });
        bridge.unregister_participant("p1");
        assert!(bridge.get_participant("p1").is_none());
    }
}
