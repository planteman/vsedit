//! Ext API: Chat.
//!
//! RPC bridge between the extension host and the main thread for chat/AI integration.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_chat";

/// Maximum allowed length for a chat message body.
pub const MAX_MESSAGE_LENGTH: usize = 32_768;

/// Maximum number of participants a single bridge can hold.
pub const MAX_PARTICIPANTS: usize = 256;

// ── Error Types ──

/// Errors that can occur during chat operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatError {
    /// The referenced participant was not found.
    ParticipantNotFound(String),
    /// A participant with this ID is already registered.
    DuplicateParticipant(String),
    /// The message body exceeds [`MAX_MESSAGE_LENGTH`].
    MessageTooLong { length: usize, max: usize },
    /// The participant limit has been reached.
    ParticipantLimitReached { max: usize },
    /// A required field was empty or invalid.
    ValidationError(String),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::ParticipantNotFound(id) => {
                write!(f, "participant not found: {id}")
            }
            ChatError::DuplicateParticipant(id) => {
                write!(f, "participant already registered: {id}")
            }
            ChatError::MessageTooLong { length, max } => {
                write!(f, "message length {length} exceeds maximum {max}")
            }
            ChatError::ParticipantLimitReached { max } => {
                write!(f, "participant limit of {max} reached")
            }
            ChatError::ValidationError(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

impl std::error::Error for ChatError {}

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

impl ChatParticipant {
    /// Start building a new [`ChatParticipant`].
    pub fn builder(id: impl Into<String>, name: impl Into<String>) -> ChatParticipantBuilder {
        ChatParticipantBuilder {
            id: id.into(),
            name: name.into(),
            description: None,
            is_default: false,
        }
    }

    /// Validate that this participant has non-empty id and name.
    pub fn validate(&self) -> Result<(), ChatError> {
        if self.id.trim().is_empty() {
            return Err(ChatError::ValidationError(
                "participant id must not be empty".into(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(ChatError::ValidationError(
                "participant name must not be empty".into(),
            ));
        }
        Ok(())
    }
}

impl fmt::Display for ChatParticipant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

/// Builder for [`ChatParticipant`].
#[derive(Debug, Clone)]
pub struct ChatParticipantBuilder {
    id: String,
    name: String,
    description: Option<String>,
    is_default: bool,
}

impl ChatParticipantBuilder {
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn is_default(mut self, val: bool) -> Self {
        self.is_default = val;
        self
    }

    /// Build the participant, validating required fields.
    pub fn build(self) -> Result<ChatParticipant, ChatError> {
        let participant = ChatParticipant {
            id: self.id,
            name: self.name,
            description: self.description,
            is_default: self.is_default,
        };
        participant.validate()?;
        Ok(participant)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatRequest {
    pub id: String,
    pub participant_id: String,
    pub message: String,
}

impl ChatRequest {
    /// Validate that the request has non-empty fields and the message is within limits.
    pub fn validate(&self) -> Result<(), ChatError> {
        if self.id.trim().is_empty() {
            return Err(ChatError::ValidationError("request id must not be empty".into()));
        }
        if self.participant_id.trim().is_empty() {
            return Err(ChatError::ValidationError(
                "participant_id must not be empty".into(),
            ));
        }
        if self.message.len() > MAX_MESSAGE_LENGTH {
            return Err(ChatError::MessageTooLong {
                length: self.message.len(),
                max: MAX_MESSAGE_LENGTH,
            });
        }
        Ok(())
    }

    /// Return the number of whitespace-separated words in the message.
    pub fn word_count(&self) -> usize {
        self.message.split_whitespace().count()
    }
}

impl fmt::Display for ChatRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let preview: String = self.message.chars().take(60).collect();
        write!(f, "[{}] -> {}: {}", self.id, self.participant_id, preview)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatResponse {
    pub request_id: String,
    pub content: String,
    pub is_complete: bool,
}

impl ChatResponse {
    /// Create a complete response for the given request.
    pub fn complete(request_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            content: content.into(),
            is_complete: true,
        }
    }

    /// Create a partial (streaming) response for the given request.
    pub fn partial(request_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            content: content.into(),
            is_complete: false,
        }
    }

    /// Byte length of the response content.
    pub fn content_len(&self) -> usize {
        self.content.len()
    }
}

impl fmt::Display for ChatResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_complete { "complete" } else { "partial" };
        write!(f, "[{}] ({}) {} bytes", self.request_id, status, self.content.len())
    }
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

    /// Register a participant with full validation.
    pub fn register_validated(
        &mut self,
        participant: ChatParticipant,
    ) -> Result<(), ChatError> {
        participant.validate()?;
        if self.participants.iter().any(|p| p.id == participant.id) {
            return Err(ChatError::DuplicateParticipant(participant.id));
        }
        if self.participants.len() >= MAX_PARTICIPANTS {
            return Err(ChatError::ParticipantLimitReached {
                max: MAX_PARTICIPANTS,
            });
        }
        self.participants.push(participant);
        Ok(())
    }

    pub fn unregister_participant(&mut self, id: &str) {
        self.participants.retain(|p| p.id != id);
    }

    pub fn get_participant(&self, id: &str) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.id == id)
    }

    /// Return the number of registered participants.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Return the default participant, if any.
    pub fn default_participant(&self) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.is_default)
    }

    /// List all registered participant IDs.
    pub fn participant_ids(&self) -> Vec<&str> {
        self.participants.iter().map(|p| p.id.as_str()).collect()
    }

    /// Route a [`ChatRequest`] to the appropriate participant, returning an error
    /// if the participant is not registered or the request is invalid.
    pub fn route_request(&self, request: &ChatRequest) -> Result<ChatResponse, ChatError> {
        request.validate()?;
        if self.get_participant(&request.participant_id).is_none() {
            return Err(ChatError::ParticipantNotFound(
                request.participant_id.clone(),
            ));
        }
        Ok(ChatResponse::partial(
            &request.id,
            format!("Processing request for {}", request.participant_id),
        ))
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

impl fmt::Display for ChatBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ChatBridge({} participant{})",
            self.participants.len(),
            if self.participants.len() == 1 { "" } else { "s" }
        )
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

    // ── Additional tests ──

    #[test]
    fn chat_error_display() {
        let err = ChatError::ParticipantNotFound("abc".into());
        assert_eq!(err.to_string(), "participant not found: abc");

        let err = ChatError::MessageTooLong {
            length: 50_000,
            max: MAX_MESSAGE_LENGTH,
        };
        assert!(err.to_string().contains("50000"));

        let err = ChatError::DuplicateParticipant("dup".into());
        assert!(err.to_string().contains("dup"));
    }

    #[test]
    fn participant_builder_success() {
        let p = ChatParticipant::builder("copilot", "GitHub Copilot")
            .description("AI pair programmer")
            .is_default(true)
            .build()
            .unwrap();
        assert_eq!(p.id, "copilot");
        assert_eq!(p.name, "GitHub Copilot");
        assert_eq!(p.description.as_deref(), Some("AI pair programmer"));
        assert!(p.is_default);
    }

    #[test]
    fn participant_builder_empty_id_fails() {
        let result = ChatParticipant::builder("", "Name").build();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ChatError::ValidationError("participant id must not be empty".into())
        );
    }

    #[test]
    fn participant_display() {
        let p = ChatParticipant::builder("copilot", "Copilot").build().unwrap();
        assert_eq!(format!("{p}"), "Copilot (copilot)");
    }

    #[test]
    fn request_validation_ok() {
        let req = ChatRequest {
            id: "r1".into(),
            participant_id: "copilot".into(),
            message: "hello".into(),
        };
        assert!(req.validate().is_ok());
        assert_eq!(req.word_count(), 1);
    }

    #[test]
    fn request_validation_message_too_long() {
        let req = ChatRequest {
            id: "r1".into(),
            participant_id: "copilot".into(),
            message: "x".repeat(MAX_MESSAGE_LENGTH + 1),
        };
        let err = req.validate().unwrap_err();
        assert!(matches!(err, ChatError::MessageTooLong { .. }));
    }

    #[test]
    fn response_constructors() {
        let full = ChatResponse::complete("r1", "done");
        assert!(full.is_complete);
        assert_eq!(full.content_len(), 4);

        let part = ChatResponse::partial("r1", "chunk");
        assert!(!part.is_complete);
        assert_eq!(format!("{part}"), "[r1] (partial) 5 bytes");
    }

    #[test]
    fn bridge_register_validated_duplicate() {
        let mut bridge = ChatBridge::new();
        let p1 = ChatParticipant::builder("p1", "P1").build().unwrap();
        let p1_dup = ChatParticipant::builder("p1", "P1 Again").build().unwrap();
        bridge.register_validated(p1).unwrap();
        let err = bridge.register_validated(p1_dup).unwrap_err();
        assert_eq!(err, ChatError::DuplicateParticipant("p1".into()));
    }

    #[test]
    fn bridge_participant_helpers() {
        let mut bridge = ChatBridge::new();
        assert_eq!(bridge.participant_count(), 0);
        assert_eq!(format!("{bridge}"), "ChatBridge(0 participants)");

        bridge.register_participant(ChatParticipant {
            id: "a".into(),
            name: "A".into(),
            description: None,
            is_default: true,
        });
        bridge.register_participant(ChatParticipant {
            id: "b".into(),
            name: "B".into(),
            description: None,
            is_default: false,
        });

        assert_eq!(bridge.participant_count(), 2);
        assert_eq!(bridge.default_participant().unwrap().id, "a");
        assert_eq!(bridge.participant_ids(), vec!["a", "b"]);
        assert_eq!(format!("{bridge}"), "ChatBridge(2 participants)");
    }

    #[test]
    fn bridge_route_request_success() {
        let mut bridge = ChatBridge::new();
        bridge
            .register_validated(ChatParticipant::builder("copilot", "Copilot").build().unwrap())
            .unwrap();
        let req = ChatRequest {
            id: "r1".into(),
            participant_id: "copilot".into(),
            message: "explain this".into(),
        };
        let resp = bridge.route_request(&req).unwrap();
        assert!(!resp.is_complete);
        assert!(resp.content.contains("copilot"));
    }

    #[test]
    fn bridge_route_request_unknown_participant() {
        let bridge = ChatBridge::new();
        let req = ChatRequest {
            id: "r1".into(),
            participant_id: "unknown".into(),
            message: "hi".into(),
        };
        let err = bridge.route_request(&req).unwrap_err();
        assert_eq!(err, ChatError::ParticipantNotFound("unknown".into()));
    }

    #[test]
    fn cancel_message_roundtrip() {
        let msg = ChatMessage::CancelRequest {
            request_id: "req-42".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }
}
