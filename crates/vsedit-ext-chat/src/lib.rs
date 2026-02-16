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

// ── Message Statistics ──

/// Aggregate statistics computed over a collection of chat messages.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessageStats {
    /// Total number of messages analysed.
    pub total_messages: usize,
    /// Average byte-length of message bodies (0 when no messages).
    pub avg_length: usize,
    /// Maximum byte-length among all message bodies (0 when no messages).
    pub max_length: usize,
}

/// Compute [`ChatMessageStats`] from a slice of message body strings.
///
/// Returns zeroed stats when the slice is empty.
pub fn compute_message_stats(messages: &[&str]) -> ChatMessageStats {
    let total_messages = messages.len();
    if total_messages == 0 {
        return ChatMessageStats {
            total_messages: 0,
            avg_length: 0,
            max_length: 0,
        };
    }
    let total_len: usize = messages.iter().map(|m| m.len()).sum();
    let max_length = messages.iter().map(|m| m.len()).max().unwrap_or(0);
    ChatMessageStats {
        total_messages,
        avg_length: total_len / total_messages,
        max_length,
    }
}

/// Initialize the chat extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-chat operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtChatStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtChatStats {
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
    pub fn merge(&mut self, other: &ExtChatStats) {
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

impl Default for ExtChatStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtChatStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtChatStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-chat.
#[derive(Debug, Clone)]
pub struct ExtChatValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtChatValidator {
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

impl Default for ExtChatValidator {
    fn default() -> Self {
        Self::new()
    }
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

    // ── ChatMessageStats tests ──

    #[test]
    fn stats_empty_messages() {
        let stats = compute_message_stats(&[]);
        assert_eq!(
            stats,
            ChatMessageStats {
                total_messages: 0,
                avg_length: 0,
                max_length: 0,
            }
        );
    }

    #[test]
    fn stats_single_message() {
        let stats = compute_message_stats(&["hello"]);
        assert_eq!(stats.total_messages, 1);
        assert_eq!(stats.avg_length, 5);
        assert_eq!(stats.max_length, 5);
    }

    #[test]
    fn stats_multiple_messages() {
        let stats = compute_message_stats(&["hi", "hello", "hey there!"]);
        assert_eq!(stats.total_messages, 3);
        // lengths: 2, 5, 10 → total 17, avg 5 (integer division)
        assert_eq!(stats.avg_length, 5);
        assert_eq!(stats.max_length, 10);
    }

    #[test]
    fn stats_uniform_length_messages() {
        let stats = compute_message_stats(&["aaa", "bbb", "ccc", "ddd"]);
        assert_eq!(stats.total_messages, 4);
        assert_eq!(stats.avg_length, 3);
        assert_eq!(stats.max_length, 3);
    }

    #[test]
    fn stats_with_empty_string_message() {
        let stats = compute_message_stats(&["", "data", ""]);
        assert_eq!(stats.total_messages, 3);
        // lengths: 0, 4, 0 → total 4, avg 1
        assert_eq!(stats.avg_length, 1);
        assert_eq!(stats.max_length, 4);
    }

    #[test]
    fn ext_chat_stats_new_defaults() {
        let stats = ExtChatStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_chat_stats_record_success() {
        let mut stats = ExtChatStats::new();
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
    fn ext_chat_stats_record_failure() {
        let mut stats = ExtChatStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_chat_stats_reset() {
        let mut stats = ExtChatStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_chat_stats_merge() {
        let mut a = ExtChatStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtChatStats::new();
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
    fn ext_chat_stats_display() {
        let mut stats = ExtChatStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_chat_stats_default() {
        let stats = ExtChatStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_chat_validator_accepts_valid_name() {
        let v = ExtChatValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_chat_validator_rejects_empty() {
        let v = ExtChatValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_chat_validator_rejects_too_long() {
        let v = ExtChatValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_chat_validator_forbidden_prefix() {
        let v = ExtChatValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_chat_validator_allowed_chars() {
        let v = ExtChatValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_chat_validator_range() {
        let v = ExtChatValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_chat_sanitize_removes_control() {
        let result = ExtChatValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_chat_truncate_short_string() {
        assert_eq!(ExtChatValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_chat_truncate_long_string() {
        let result = ExtChatValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_chat_is_ascii_printable() {
        assert!(ExtChatValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtChatValidator::is_ascii_printable("Hello\x00World"));
    }
}
