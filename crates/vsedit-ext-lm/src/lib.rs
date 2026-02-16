//! Ext API: Language models.
//!
//! RPC bridge between the extension host and the main thread for language model access.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_lm";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LmMessage {
    ListModels,
    SendRequest {
        model_id: String,
        messages: Vec<LanguageModelMessage>,
    },
    CancelRequest {
        request_id: String,
    },
    CountTokens {
        model_id: String,
        text: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageModelChat {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub family: String,
    pub version: String,
    pub max_input_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role", rename_all = "camelCase")]
pub enum LanguageModelMessage {
    System { content: String },
    User { content: String },
    Assistant { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LmResponse {
    pub request_id: String,
    pub text: String,
    pub is_complete: bool,
}

// ── Bridge ──

pub struct LmBridge {
    models: Vec<LanguageModelChat>,
}

impl LmBridge {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
        }
    }

    pub fn register_model(&mut self, model: LanguageModelChat) {
        if !self.models.iter().any(|m| m.id == model.id) {
            self.models.push(model);
        }
    }

    pub fn list_models(&self) -> &[LanguageModelChat] {
        &self.models
    }

    pub fn get_model(&self, id: &str) -> Option<&LanguageModelChat> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn handle_message(&self, msg: &LmMessage) -> serde_json::Value {
        match msg {
            LmMessage::ListModels => {
                let names: Vec<&str> = self.models.iter().map(|m| m.name.as_str()).collect();
                serde_json::json!({"models": names})
            }
            LmMessage::SendRequest {
                model_id,
                messages,
            } => {
                let found = self.get_model(model_id).is_some();
                serde_json::json!({"accepted": found, "messageCount": messages.len()})
            }
            LmMessage::CancelRequest { request_id } => {
                serde_json::json!({"cancelled": request_id})
            }
            LmMessage::CountTokens { model_id, text } => {
                let found = self.get_model(model_id).is_some();
                // Rough estimate: split on whitespace
                let count = text.split_whitespace().count();
                serde_json::json!({"found": found, "tokens": count})
            }
        }
    }
}

impl Default for LmBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the lm extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model() -> LanguageModelChat {
        LanguageModelChat {
            id: "gpt-4".into(),
            name: "GPT-4".into(),
            vendor: "openai".into(),
            family: "gpt".into(),
            version: "4".into(),
            max_input_tokens: 8192,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = LmMessage::SendRequest {
            model_id: "gpt-4".into(),
            messages: vec![LanguageModelMessage::User {
                content: "hello".into(),
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: LmMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn model_serialization() {
        let m = test_model();
        let json = serde_json::to_string(&m).unwrap();
        let back: LanguageModelChat = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn bridge_register_and_list() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        assert_eq!(bridge.list_models().len(), 1);
        assert!(bridge.get_model("gpt-4").is_some());
    }

    #[test]
    fn bridge_handle_list() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        let result = bridge.handle_message(&LmMessage::ListModels);
        let models = result["models"].as_array().unwrap();
        assert_eq!(models.len(), 1);
    }

    #[test]
    fn bridge_count_tokens() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        let result = bridge.handle_message(&LmMessage::CountTokens {
            model_id: "gpt-4".into(),
            text: "hello world foo".into(),
        });
        assert_eq!(result["tokens"], 3);
    }
}
