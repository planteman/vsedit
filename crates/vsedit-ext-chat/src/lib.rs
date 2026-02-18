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

// ---------------------------------------------------------------------------
// ChatMessagePart – rich message content
// ---------------------------------------------------------------------------

/// A single part of a rich chat message.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChatMessagePart {
    Text(String),
    Code { language: String, code: String },
    Markdown(String),
    Reference { uri: String, label: Option<String> },
}

impl ChatMessagePart {
    /// Returns `true` if this part is plain text.
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// Returns `true` if this part is a code block.
    pub fn is_code(&self) -> bool {
        matches!(self, Self::Code { .. })
    }

    /// Returns `true` if this part is markdown.
    pub fn is_markdown(&self) -> bool {
        matches!(self, Self::Markdown(_))
    }

    /// Returns `true` if this part is a reference.
    pub fn is_reference(&self) -> bool {
        matches!(self, Self::Reference { .. })
    }

    /// Returns the textual content of this part.
    pub fn text_content(&self) -> &str {
        match self {
            Self::Text(s) | Self::Markdown(s) => s.as_str(),
            Self::Code { code, .. } => code.as_str(),
            Self::Reference { uri, .. } => uri.as_str(),
        }
    }

    /// Returns the character count of the textual content.
    pub fn char_count(&self) -> usize {
        self.text_content().chars().count()
    }
}

// ---------------------------------------------------------------------------
// chat_format_response – terminal rendering
// ---------------------------------------------------------------------------

/// Formats a slice of [`ChatMessagePart`]s into a single string suitable for
/// terminal rendering.
pub fn chat_format_response(parts: &[ChatMessagePart]) -> String {
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match part {
            ChatMessagePart::Text(s) => out.push_str(s),
            ChatMessagePart::Code { language, code } => {
                out.push_str("```");
                out.push_str(language);
                out.push('\n');
                out.push_str(code);
                out.push_str("\n```");
            }
            ChatMessagePart::Markdown(s) => out.push_str(s),
            ChatMessagePart::Reference { uri, label } => {
                if let Some(lbl) = label {
                    out.push('[');
                    out.push_str(lbl);
                    out.push_str("](");
                    out.push_str(uri);
                    out.push(')');
                } else {
                    out.push_str(uri);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ChatMessageBuilder – builder for multi-part messages
// ---------------------------------------------------------------------------

/// Builder for constructing a list of [`ChatMessagePart`]s.
#[derive(Debug, Clone, Default)]
pub struct ChatMessageBuilder {
    parts: Vec<ChatMessagePart>,
}

impl ChatMessageBuilder {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    /// Appends a plain-text part.
    pub fn add_text(mut self, text: &str) -> Self {
        self.parts.push(ChatMessagePart::Text(text.to_string()));
        self
    }

    /// Appends a code-block part.
    pub fn add_code(mut self, lang: &str, code: &str) -> Self {
        self.parts.push(ChatMessagePart::Code {
            language: lang.to_string(),
            code: code.to_string(),
        });
        self
    }

    /// Appends a markdown part.
    pub fn add_markdown(mut self, md: &str) -> Self {
        self.parts.push(ChatMessagePart::Markdown(md.to_string()));
        self
    }

    /// Appends a reference part.
    pub fn add_reference(mut self, uri: &str, label: Option<&str>) -> Self {
        self.parts.push(ChatMessagePart::Reference {
            uri: uri.to_string(),
            label: label.map(String::from),
        });
        self
    }

    /// Consumes the builder and returns the parts.
    pub fn build(self) -> Vec<ChatMessagePart> {
        self.parts
    }

    /// Returns the total character count across all parts.
    pub fn total_chars(&self) -> usize {
        self.parts.iter().map(ChatMessagePart::char_count).sum()
    }

    /// Returns the number of parts added so far.
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }
}

// ---------------------------------------------------------------------------
// ChatConversationMessage / ChatConversation – conversation tracking
// ---------------------------------------------------------------------------

/// A single message inside a [`ChatConversation`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatConversationMessage {
    pub role: String,
    pub parts: Vec<ChatMessagePart>,
    pub timestamp_ms: u64,
}

/// Tracks a sequence of messages in a conversation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatConversation {
    id: String,
    messages: Vec<ChatConversationMessage>,
}

impl ChatConversation {
    /// Creates a new conversation with the given identifier.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            messages: Vec::new(),
        }
    }

    /// Adds a user message with plain-text content.
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatConversationMessage {
            role: "user".to_string(),
            parts: vec![ChatMessagePart::Text(content.to_string())],
            timestamp_ms: 0,
        });
    }

    /// Adds an assistant message composed of the given parts.
    pub fn add_assistant_message(&mut self, parts: Vec<ChatMessagePart>) {
        self.messages.push(ChatConversationMessage {
            role: "assistant".to_string(),
            parts,
            timestamp_ms: 0,
        });
    }

    /// Returns the number of messages in this conversation.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Returns the last message, if any.
    pub fn last_message(&self) -> Option<&ChatConversationMessage> {
        self.messages.last()
    }

    /// Returns a rough token estimate (total chars / 4).
    pub fn total_tokens_estimate(&self) -> usize {
        let total_chars: usize = self
            .messages
            .iter()
            .flat_map(|m| &m.parts)
            .map(ChatMessagePart::char_count)
            .sum();
        total_chars / 4
    }
}

// ---------------------------------------------------------------------------
// ChatHistory – multi-conversation store
// ---------------------------------------------------------------------------

/// Stores multiple [`ChatConversation`]s and provides lookup, search, and
/// aggregate operations.
#[derive(Debug, Clone, Default)]
pub struct ChatHistory {
    conversations: Vec<ChatConversation>,
}

impl ChatHistory {
    /// Creates a new empty history.
    pub fn new() -> Self {
        Self {
            conversations: Vec::new(),
        }
    }

    /// Adds a conversation to the history.
    pub fn add_conversation(&mut self, conv: ChatConversation) {
        self.conversations.push(conv);
    }

    /// Returns a reference to the conversation with the given id, if present.
    pub fn get_conversation(&self, id: &str) -> Option<&ChatConversation> {
        self.conversations.iter().find(|c| c.id == id)
    }

    /// Returns a mutable reference to the conversation with the given id.
    pub fn get_conversation_mut(&mut self, id: &str) -> Option<&mut ChatConversation> {
        self.conversations.iter_mut().find(|c| c.id == id)
    }

    /// Lists all conversation ids.
    pub fn list_conversations(&self) -> Vec<&str> {
        self.conversations.iter().map(|c| c.id.as_str()).collect()
    }

    /// Removes the conversation with the given id. Returns `true` if found.
    pub fn delete_conversation(&mut self, id: &str) -> bool {
        let before = self.conversations.len();
        self.conversations.retain(|c| c.id != id);
        self.conversations.len() < before
    }

    /// Returns the total number of messages across all conversations.
    pub fn total_messages(&self) -> usize {
        self.conversations.iter().map(|c| c.message_count()).sum()
    }

    /// Returns the number of stored conversations.
    pub fn conversation_count(&self) -> usize {
        self.conversations.len()
    }

    /// Searches all conversations for messages whose text content contains
    /// `query` (case-insensitive). Returns `(conversation_id, message_index)`
    /// pairs for each match.
    pub fn search_messages(&self, query: &str) -> Vec<(&str, usize)> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        for conv in &self.conversations {
            for (i, msg) in conv.messages.iter().enumerate() {
                let matches = msg.parts.iter().any(|p| {
                    p.text_content().to_lowercase().contains(&query_lower)
                });
                if matches {
                    results.push((conv.id.as_str(), i));
                }
            }
        }
        results
    }

    /// Computes aggregate [`ChatStatistics`] over all conversations.
    pub fn statistics(&self) -> ChatStatistics {
        ChatStatistics::from_history(self)
    }
}

impl fmt::Display for ChatHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ChatHistory({} conversation{}, {} message{})",
            self.conversations.len(),
            if self.conversations.len() == 1 { "" } else { "s" },
            self.total_messages(),
            if self.total_messages() == 1 { "" } else { "s" },
        )
    }
}

// ---------------------------------------------------------------------------
// ChatStatistics – aggregate metrics
// ---------------------------------------------------------------------------

/// Aggregate statistics computed over a [`ChatHistory`].
#[derive(Debug, Clone, PartialEq)]
pub struct ChatStatistics {
    /// Total number of conversations.
    pub total_conversations: usize,
    /// Total number of messages across all conversations.
    pub total_messages: usize,
    /// Number of messages grouped by role.
    pub messages_per_role: Vec<(String, usize)>,
    /// Average character length of message content (0 when no messages).
    pub avg_message_length: usize,
    /// Rough token estimate (total chars / 4).
    pub total_token_estimate: usize,
}

impl ChatStatistics {
    /// Compute statistics from a [`ChatHistory`].
    pub fn from_history(history: &ChatHistory) -> Self {
        let total_conversations = history.conversation_count();

        let mut role_counts: Vec<(String, usize)> = Vec::new();
        let mut total_chars: usize = 0;
        let mut total_messages: usize = 0;

        for conv in &history.conversations {
            for msg in &conv.messages {
                total_messages += 1;
                let msg_chars: usize = msg.parts.iter().map(|p| p.char_count()).sum();
                total_chars += msg_chars;

                if let Some(entry) = role_counts.iter_mut().find(|(r, _)| r == &msg.role) {
                    entry.1 += 1;
                } else {
                    role_counts.push((msg.role.clone(), 1));
                }
            }
        }

        role_counts.sort_by(|a, b| a.0.cmp(&b.0));

        let avg_message_length = if total_messages == 0 {
            0
        } else {
            total_chars / total_messages
        };

        Self {
            total_conversations,
            total_messages,
            messages_per_role: role_counts,
            avg_message_length,
            total_token_estimate: total_chars / 4,
        }
    }

    /// Look up the message count for a specific role.
    pub fn count_for_role(&self, role: &str) -> usize {
        self.messages_per_role
            .iter()
            .find(|(r, _)| r == role)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    }
}

impl fmt::Display for ChatStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ChatStatistics(convs={}, msgs={}, avg_len={}, tokens≈{})",
            self.total_conversations,
            self.total_messages,
            self.avg_message_length,
            self.total_token_estimate,
        )
    }
}

// ---------------------------------------------------------------------------
// ChatMessageFormatter – message display formatting
// ---------------------------------------------------------------------------

/// Configurable formatter for rendering [`ChatConversationMessage`]s as
/// human-readable strings.
#[derive(Debug, Clone)]
pub struct ChatMessageFormatter {
    /// Maximum characters before the message body is truncated.
    max_body_chars: usize,
    /// Whether to prepend the role label.
    show_role: bool,
    /// Whether to prepend the timestamp.
    show_timestamp: bool,
}

impl ChatMessageFormatter {
    /// Creates a formatter with sensible defaults.
    pub fn new() -> Self {
        Self {
            max_body_chars: 200,
            show_role: true,
            show_timestamp: false,
        }
    }

    /// Set maximum body characters before truncation.
    pub fn max_body_chars(mut self, n: usize) -> Self {
        self.max_body_chars = n;
        self
    }

    /// Enable or disable the role prefix.
    pub fn show_role(mut self, v: bool) -> Self {
        self.show_role = v;
        self
    }

    /// Enable or disable the timestamp prefix.
    pub fn show_timestamp(mut self, v: bool) -> Self {
        self.show_timestamp = v;
        self
    }

    /// Format a single [`ChatConversationMessage`] into a display string.
    pub fn format_message(&self, msg: &ChatConversationMessage) -> String {
        let mut out = String::new();

        if self.show_timestamp {
            out.push_str(&format!("[{}ms] ", msg.timestamp_ms));
        }

        if self.show_role {
            out.push_str(&msg.role);
            out.push_str(": ");
        }

        let body = chat_format_response(&msg.parts);
        if body.chars().count() > self.max_body_chars {
            let truncated: String = body.chars().take(self.max_body_chars.saturating_sub(1)).collect();
            out.push_str(&truncated);
            out.push('…');
        } else {
            out.push_str(&body);
        }

        out
    }

    /// Format all messages of a [`ChatConversation`].
    pub fn format_conversation(&self, conv: &ChatConversation) -> String {
        conv.messages
            .iter()
            .map(|m| self.format_message(m))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for ChatMessageFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ChatMessageFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ChatMessageFormatter(max_body={}, role={}, ts={})",
            self.max_body_chars, self.show_role, self.show_timestamp,
        )
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<&str> for ChatMessagePart {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<String> for ChatMessagePart {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<ChatError> for String {
    fn from(e: ChatError) -> Self {
        e.to_string()
    }
}

// ---------------------------------------------------------------------------
// ChatParticipantRegistry with slash commands
// ---------------------------------------------------------------------------

/// A slash command registered by a chat participant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    /// Command name (without leading slash).
    pub name: String,
    /// Description of the command.
    pub description: String,
}

/// Registry of chat participants with their slash commands.
#[derive(Debug, Clone)]
pub struct ChatParticipantRegistry {
    participants: Vec<ChatParticipant>,
    commands: std::collections::HashMap<String, Vec<SlashCommand>>,
}

impl Default for ChatParticipantRegistry {
    fn default() -> Self {
        Self {
            participants: Vec::new(),
            commands: std::collections::HashMap::new(),
        }
    }
}

impl ChatParticipantRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a participant with optional slash commands.
    pub fn register(&mut self, participant: ChatParticipant, commands: Vec<SlashCommand>) {
        let id = participant.id.clone();
        self.participants.push(participant);
        if !commands.is_empty() {
            self.commands.insert(id, commands);
        }
    }

    /// Find a participant by ID.
    pub fn get(&self, id: &str) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.id == id)
    }

    /// Get slash commands for a participant.
    pub fn get_commands(&self, participant_id: &str) -> &[SlashCommand] {
        self.commands
            .get(participant_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find commands matching a prefix across all participants.
    pub fn find_commands(&self, prefix: &str) -> Vec<(&str, &SlashCommand)> {
        let prefix_lower = prefix.to_lowercase();
        self.commands
            .iter()
            .flat_map(|(pid, cmds)| {
                cmds.iter()
                    .filter(|c| c.name.to_lowercase().starts_with(&prefix_lower))
                    .map(move |c| (pid.as_str(), c))
            })
            .collect()
    }

    /// Total number of registered participants.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Total number of registered slash commands.
    pub fn command_count(&self) -> usize {
        self.commands.values().map(|v| v.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// ChatResponseStream – incremental rendering
// ---------------------------------------------------------------------------

/// A stream of chat response fragments for incremental rendering.
#[derive(Debug, Clone)]
pub struct ChatResponseStream {
    fragments: Vec<ResponseFragment>,
    complete: bool,
}

/// A fragment of a chat response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseFragment {
    /// A piece of markdown text.
    Markdown(String),
    /// A code block with optional language.
    CodeBlock { language: Option<String>, code: String },
    /// A progress indicator.
    Progress(String),
}

impl Default for ChatResponseStream {
    fn default() -> Self {
        Self {
            fragments: Vec::new(),
            complete: false,
        }
    }
}

impl ChatResponseStream {
    /// Create a new empty stream.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a markdown fragment.
    pub fn push_markdown(&mut self, text: impl Into<String>) {
        self.fragments.push(ResponseFragment::Markdown(text.into()));
    }

    /// Push a code block fragment.
    pub fn push_code(&mut self, code: impl Into<String>, language: Option<String>) {
        self.fragments.push(ResponseFragment::CodeBlock {
            language,
            code: code.into(),
        });
    }

    /// Push a progress indicator.
    pub fn push_progress(&mut self, message: impl Into<String>) {
        self.fragments
            .push(ResponseFragment::Progress(message.into()));
    }

    /// Mark the stream as complete.
    pub fn finish(&mut self) {
        self.complete = true;
    }

    /// Whether the stream is complete.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Number of fragments.
    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }

    /// Render all fragments into a single string.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for frag in &self.fragments {
            match frag {
                ResponseFragment::Markdown(text) => out.push_str(text),
                ResponseFragment::CodeBlock { language, code } => {
                    out.push_str("```");
                    if let Some(lang) = language {
                        out.push_str(lang);
                    }
                    out.push('\n');
                    out.push_str(code);
                    out.push_str("\n```\n");
                }
                ResponseFragment::Progress(msg) => {
                    out.push_str(&format!("[{}]", msg));
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// ChatHistoryPersistence
// ---------------------------------------------------------------------------

/// Stores chat history for persistence across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistoryEntry {
    /// The user's request.
    pub request: String,
    /// The assistant's response.
    pub response: String,
    /// Participant ID.
    pub participant_id: String,
    /// Timestamp (epoch millis).
    pub timestamp: u64,
}

/// Manages a bounded chat history.
#[derive(Debug, Clone)]
pub struct ChatHistoryPersistence {
    entries: Vec<ChatHistoryEntry>,
    max_entries: usize,
}

impl Default for ChatHistoryPersistence {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 100,
        }
    }
}

impl ChatHistoryPersistence {
    /// Create a new history store with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// Add an entry to the history.
    pub fn add(&mut self, entry: ChatHistoryEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Get all entries.
    pub fn entries(&self) -> &[ChatHistoryEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Search entries by keyword.
    pub fn search(&self, keyword: &str) -> Vec<&ChatHistoryEntry> {
        let kw = keyword.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.request.to_lowercase().contains(&kw)
                    || e.response.to_lowercase().contains(&kw)
            })
            .collect()
    }

    /// Clear history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Chat follow-up suggestions
// ---------------------------------------------------------------------------

/// A suggested follow-up question or action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatFollowUp {
    /// The suggested prompt text.
    pub prompt: String,
    /// Optional label for display.
    pub label: Option<String>,
    /// The participant to route to.
    pub participant_id: Option<String>,
}

/// Generates follow-up suggestions based on the last response.
pub fn generate_follow_ups(response: &str, max: usize) -> Vec<ChatFollowUp> {
    let mut suggestions = Vec::new();
    // Heuristic: suggest elaboration if response is long
    if response.len() > 200 {
        suggestions.push(ChatFollowUp {
            prompt: "Can you explain this in more detail?".into(),
            label: Some("Explain more".into()),
            participant_id: None,
        });
    }
    // Suggest code generation if response mentions code
    if response.contains("```") {
        suggestions.push(ChatFollowUp {
            prompt: "Can you add tests for this code?".into(),
            label: Some("Add tests".into()),
            participant_id: None,
        });
    }
    // Always suggest a summary
    suggestions.push(ChatFollowUp {
        prompt: "Summarize the key points.".into(),
        label: Some("Summarize".into()),
        participant_id: None,
    });
    suggestions.truncate(max);
    suggestions
}

// ---------------------------------------------------------------------------
// ChatFormatter - chat message format helper
// ---------------------------------------------------------------------------

/// Severity level for chat message format helper issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChatFormatterSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ChatFormatterSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ChatFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatFormatterEntry {
    pub id: String,
    pub label: String,
    pub severity: ChatFormatterSeverity,
    pub detail: Option<String>,
    pub message_count: usize,
    enabled: bool,
}

impl ChatFormatterEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ChatFormatterSeverity::Low,
            detail: None,
            message_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ChatFormatterSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_message_count(mut self, val: usize) -> Self {
        self.message_count = val;
        self
    }

    pub fn has_code_block(&self) -> bool {
        self.enabled && self.severity >= ChatFormatterSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.message_count, det)
    }
}

impl fmt::Display for ChatFormatterEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ChatFormatterEntry] items.
#[derive(Debug, Clone)]
pub struct ChatFormatter {
    entries: Vec<ChatFormatterEntry>,
    name: String,
    capacity: usize,
}

impl ChatFormatter {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ChatFormatterEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ChatFormatterEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ChatFormatterEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn message_count(&self) -> usize { self.entries.len() }

    pub fn has_code_block(&self) -> bool {
        self.entries.iter().any(|e| e.has_code_block())
    }

    pub fn entries_by_severity(&self, severity: ChatFormatterSeverity) -> Vec<&ChatFormatterEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ChatFormatterSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ChatFormatterEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ChatFormatterEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ChatCtxBuilder - chat context builder
// ---------------------------------------------------------------------------

/// Configuration for [ChatCtxBuilder].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCtxBuilderConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub context_tokens: usize,
}

impl ChatCtxBuilderConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, context_tokens: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_context_tokens(mut self, val: usize) -> Self { self.context_tokens = val; self }
}

impl Default for ChatCtxBuilderConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ChatCtxBuilder].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCtxBuilderItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ChatCtxBuilderItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn exceeds_budget(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ChatCtxBuilderItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ChatCtxBuilderItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ChatCtxBuilder {
    config: ChatCtxBuilderConfig,
    items: Vec<ChatCtxBuilderItem>,
}

impl ChatCtxBuilder {
    pub fn new(config: ChatCtxBuilderConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ChatCtxBuilderItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ChatCtxBuilderItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ChatCtxBuilderItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn context_tokens(&self) -> usize { self.items.len() }

    pub fn exceeds_budget(&self) -> bool {
        self.items.iter().any(|i| i.exceeds_budget())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ChatCtxBuilderItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ChatCtxBuilderItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ChatCtxBuilderConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-ext-chat: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtChatXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ExtChatXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for ExtChatXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ExtChatXRegistry {
    entries: Vec<ExtChatXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ExtChatXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ExtChatXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ExtChatXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ExtChatXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ExtChatXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&ExtChatXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ExtChatXConfig> {
        let mut sorted: Vec<&ExtChatXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ExtChatXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> ExtChatXIterator<'_> {
        ExtChatXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ExtChatXIterator<'a> {
    inner: std::slice::Iter<'a, ExtChatXConfig>,
}

impl<'a> Iterator for ExtChatXIterator<'a> {
    type Item = &'a ExtChatXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ExtChatXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ExtChatXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct ExtChatXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ExtChatXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &ExtChatXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ExtChatXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ExtChatXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ExtChatXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ExtChatXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ExtChatXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &ExtChatXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &ExtChatXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ExtChatXValidator {
    fn default() -> Self {
        Self::new()
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
// xb_ utilities – batch 48
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer48 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer48 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_48(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_48<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_48<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_48(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_48(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 51
// ---------------------------------------------------------------------------

/// Generic object pool `Xc51Pool<T>`.
pub struct Xc51Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc51Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc51PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc51Pool<T> {
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
    pub fn stats(&self) -> Xc51PoolStats {
        Xc51PoolStats {
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

impl<T> Default for Xc51Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc51Scheduler`.
pub struct Xc51Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc51Scheduler {
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

impl Default for Xc51Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_51 hash for the given byte slice.
pub fn xc_51_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_51 convention.
pub fn xc_51_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe61 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe61Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe61PipelineError {
    pub stage: Xe61Stage,
    pub message: String,
}

impl std::fmt::Display for Xe61PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe61Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe61Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError>>>,
    stage_names: Vec<Xe61Stage>,
}

impl Xe61Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe61Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe61Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe61Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe61Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe61Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe61CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe61CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe61Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe61CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe61CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe61Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe61CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_61_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe61CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_61_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe61CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_61_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> {
    Ok(data)
}

pub fn xe_61_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_61_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_61_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_61_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe61PipelineError> {
    Err(Xe61PipelineError {
        stage: Xe61Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_59: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg59Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg59Graph {
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

impl Default for Xg59Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_59: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg59Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg59Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg59Heap<T>) {
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

impl<T: Ord> Default for Xg59Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 50).
pub struct Xh50SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh50SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 92 as u64,
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

/// A compact bit set supporting boolean operations (variant 50).
pub struct Xh50BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh50BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 50).
pub struct Xi50Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi50Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi50Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi50Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 50).
pub struct Xi50IntervalTree {
    xi_intervals: Vec<Xi50Interval>,
}

impl Xi50IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi50Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi50Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi50Interval) -> Vec<&Xi50Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi50Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi50Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi50Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi50Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi50Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi50Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
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

    // -----------------------------------------------------------------------
    // ChatMessagePart tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_message_part_text_creation_and_is_text() {
        let part = ChatMessagePart::Text("hello".into());
        assert!(part.is_text());
        assert!(!part.is_code());
        assert!(!part.is_markdown());
        assert!(!part.is_reference());
        assert_eq!(part.text_content(), "hello");
    }

    #[test]
    fn chat_message_part_code_with_language() {
        let part = ChatMessagePart::Code {
            language: "rust".into(),
            code: "fn main() {}".into(),
        };
        assert!(part.is_code());
        assert!(!part.is_text());
        assert_eq!(part.text_content(), "fn main() {}");
    }

    #[test]
    fn chat_message_part_char_count() {
        let text = ChatMessagePart::Text("abc".into());
        assert_eq!(text.char_count(), 3);

        let code = ChatMessagePart::Code {
            language: "py".into(),
            code: "print()".into(),
        };
        assert_eq!(code.char_count(), 7);

        let md = ChatMessagePart::Markdown("# Title".into());
        assert_eq!(md.char_count(), 7);

        let refp = ChatMessagePart::Reference {
            uri: "https://example.com".into(),
            label: Some("Example".into()),
        };
        assert_eq!(refp.char_count(), 19);
    }

    // -----------------------------------------------------------------------
    // chat_format_response tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_format_response_renders_code_blocks() {
        let parts = vec![ChatMessagePart::Code {
            language: "rust".into(),
            code: "let x = 1;".into(),
        }];
        let out = chat_format_response(&parts);
        assert!(out.starts_with("```rust\n"));
        assert!(out.ends_with("\n```"));
        assert!(out.contains("let x = 1;"));
    }

    #[test]
    fn chat_format_response_renders_references() {
        let with_label = vec![ChatMessagePart::Reference {
            uri: "https://example.com".into(),
            label: Some("Example".into()),
        }];
        assert_eq!(
            chat_format_response(&with_label),
            "[Example](https://example.com)"
        );

        let without_label = vec![ChatMessagePart::Reference {
            uri: "https://bare.com".into(),
            label: None,
        }];
        assert_eq!(chat_format_response(&without_label), "https://bare.com");
    }

    // -----------------------------------------------------------------------
    // ChatMessageBuilder tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_message_builder_builds_multi_part() {
        let parts = ChatMessageBuilder::new()
            .add_text("Hello")
            .add_code("rs", "fn f() {}")
            .add_markdown("**bold**")
            .add_reference("https://a.com", Some("link"))
            .build();

        assert_eq!(parts.len(), 4);
        assert!(parts[0].is_text());
        assert!(parts[1].is_code());
        assert!(parts[2].is_markdown());
        assert!(parts[3].is_reference());

        let builder = ChatMessageBuilder::new()
            .add_text("ab")
            .add_code("py", "cd");
        assert_eq!(builder.part_count(), 2);
        assert_eq!(builder.total_chars(), 4);
    }

    // -----------------------------------------------------------------------
    // ChatConversation tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_conversation_tracks_messages() {
        let mut conv = ChatConversation::new("conv-1");
        assert_eq!(conv.message_count(), 0);
        assert!(conv.last_message().is_none());

        conv.add_user_message("Hi");
        assert_eq!(conv.message_count(), 1);

        let last = conv.last_message().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(last.parts.len(), 1);
        assert!(last.parts[0].is_text());

        conv.add_assistant_message(vec![
            ChatMessagePart::Text("Hello!".into()),
            ChatMessagePart::Code {
                language: "rs".into(),
                code: "let x = 1;".into(),
            },
        ]);
        assert_eq!(conv.message_count(), 2);
        assert_eq!(conv.last_message().unwrap().role, "assistant");
    }

    #[test]
    fn chat_conversation_total_tokens_estimate() {
        let mut conv = ChatConversation::new("conv-2");
        // "abcdefgh" = 8 chars => 8/4 = 2 tokens
        conv.add_user_message("abcdefgh");
        assert_eq!(conv.total_tokens_estimate(), 2);

        // add assistant with 12 chars => total 20 chars => 5 tokens
        conv.add_assistant_message(vec![ChatMessagePart::Text("123456789012".into())]);
        assert_eq!(conv.total_tokens_estimate(), 5);
    }

    // -----------------------------------------------------------------------
    // ChatHistory tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_history_add_get_delete() {
        let mut history = ChatHistory::new();
        assert_eq!(history.conversation_count(), 0);
        assert_eq!(history.total_messages(), 0);

        let mut c1 = ChatConversation::new("c1");
        c1.add_user_message("hello");
        c1.add_assistant_message(vec![ChatMessagePart::Text("hi".into())]);
        history.add_conversation(c1);

        let mut c2 = ChatConversation::new("c2");
        c2.add_user_message("bye");
        history.add_conversation(c2);

        assert_eq!(history.conversation_count(), 2);
        assert_eq!(history.total_messages(), 3);
        assert_eq!(history.list_conversations(), vec!["c1", "c2"]);

        assert!(history.get_conversation("c1").is_some());
        assert_eq!(history.get_conversation("c1").unwrap().message_count(), 2);
        assert!(history.get_conversation("missing").is_none());

        assert!(history.delete_conversation("c1"));
        assert_eq!(history.conversation_count(), 1);
        assert!(!history.delete_conversation("c1")); // already removed
    }

    #[test]
    fn chat_history_search_messages() {
        let mut history = ChatHistory::new();
        let mut c1 = ChatConversation::new("c1");
        c1.add_user_message("How do I write a Rust function?");
        c1.add_assistant_message(vec![ChatMessagePart::Text("Use fn keyword".into())]);
        history.add_conversation(c1);

        let mut c2 = ChatConversation::new("c2");
        c2.add_user_message("What is Python?");
        history.add_conversation(c2);

        let results = history.search_messages("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("c1", 0));

        let results = history.search_messages("fn");
        // matches "fn keyword" in c1 assistant message
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("c1", 1));

        let results = history.search_messages("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn chat_history_display() {
        let mut history = ChatHistory::new();
        assert_eq!(format!("{history}"), "ChatHistory(0 conversations, 0 messages)");

        let mut c = ChatConversation::new("c1");
        c.add_user_message("hi");
        history.add_conversation(c);
        assert_eq!(format!("{history}"), "ChatHistory(1 conversation, 1 message)");
    }

    // -----------------------------------------------------------------------
    // ChatStatistics tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_statistics_from_history() {
        let mut history = ChatHistory::new();

        let mut c1 = ChatConversation::new("c1");
        c1.add_user_message("abcd");        // 4 chars
        c1.add_assistant_message(vec![ChatMessagePart::Text("efghijkl".into())]); // 8 chars
        history.add_conversation(c1);

        let mut c2 = ChatConversation::new("c2");
        c2.add_user_message("mnop");        // 4 chars
        history.add_conversation(c2);

        let stats = history.statistics();
        assert_eq!(stats.total_conversations, 2);
        assert_eq!(stats.total_messages, 3);
        // total chars = 4 + 8 + 4 = 16, avg = 16/3 = 5
        assert_eq!(stats.avg_message_length, 5);
        // token estimate = 16/4 = 4
        assert_eq!(stats.total_token_estimate, 4);
        assert_eq!(stats.count_for_role("user"), 2);
        assert_eq!(stats.count_for_role("assistant"), 1);
        assert_eq!(stats.count_for_role("system"), 0);
    }

    #[test]
    fn chat_statistics_empty() {
        let history = ChatHistory::new();
        let stats = history.statistics();
        assert_eq!(stats.total_conversations, 0);
        assert_eq!(stats.total_messages, 0);
        assert_eq!(stats.avg_message_length, 0);
        assert_eq!(stats.total_token_estimate, 0);
        assert!(stats.messages_per_role.is_empty());
    }

    // -----------------------------------------------------------------------
    // ChatMessageFormatter tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_message_formatter_truncates_long_body() {
        let formatter = ChatMessageFormatter::new().max_body_chars(10).show_role(false);
        let msg = ChatConversationMessage {
            role: "user".into(),
            parts: vec![ChatMessagePart::Text("This is a very long message that should be truncated".into())],
            timestamp_ms: 0,
        };
        let out = formatter.format_message(&msg);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn chat_message_formatter_with_role_and_timestamp() {
        let formatter = ChatMessageFormatter::new()
            .max_body_chars(500)
            .show_role(true)
            .show_timestamp(true);
        let msg = ChatConversationMessage {
            role: "assistant".into(),
            parts: vec![ChatMessagePart::Text("Hello".into())],
            timestamp_ms: 42,
        };
        let out = formatter.format_message(&msg);
        assert!(out.starts_with("[42ms] assistant: Hello"));
    }

    #[test]
    fn chat_message_formatter_format_conversation() {
        let formatter = ChatMessageFormatter::new().max_body_chars(500).show_role(true).show_timestamp(false);
        let mut conv = ChatConversation::new("c1");
        conv.add_user_message("Hi");
        conv.add_assistant_message(vec![ChatMessagePart::Text("Hello!".into())]);

        let out = formatter.format_conversation(&conv);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "user: Hi");
        assert_eq!(lines[1], "assistant: Hello!");
    }

    // -----------------------------------------------------------------------
    // From impl tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_message_part_from_str_and_string() {
        let part: ChatMessagePart = "hello".into();
        assert!(part.is_text());
        assert_eq!(part.text_content(), "hello");

        let part: ChatMessagePart = String::from("world").into();
        assert!(part.is_text());
        assert_eq!(part.text_content(), "world");
    }

    #[test]
    fn chat_error_into_string() {
        let err = ChatError::ValidationError("bad".into());
        let s: String = err.into();
        assert_eq!(s, "validation error: bad");
    }

    // -- ChatParticipantRegistry tests --

    #[test]
    fn registry_register_and_get() {
        let mut reg = ChatParticipantRegistry::new();
        let p = ChatParticipant::builder("copilot", "Copilot").build().unwrap();
        reg.register(p, vec![
            SlashCommand { name: "explain".into(), description: "Explain code".into() },
        ]);
        assert_eq!(reg.participant_count(), 1);
        assert!(reg.get("copilot").is_some());
        assert_eq!(reg.get_commands("copilot").len(), 1);
    }

    #[test]
    fn registry_find_commands() {
        let mut reg = ChatParticipantRegistry::new();
        let p = ChatParticipant::builder("copilot", "Copilot").build().unwrap();
        reg.register(p, vec![
            SlashCommand { name: "explain".into(), description: "".into() },
            SlashCommand { name: "fix".into(), description: "".into() },
        ]);
        let matches = reg.find_commands("ex");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1.name, "explain");
    }

    #[test]
    fn registry_command_count() {
        let mut reg = ChatParticipantRegistry::new();
        let p = ChatParticipant::builder("a", "A").build().unwrap();
        reg.register(p, vec![
            SlashCommand { name: "x".into(), description: "".into() },
            SlashCommand { name: "y".into(), description: "".into() },
        ]);
        assert_eq!(reg.command_count(), 2);
    }

    // -- ChatResponseStream tests --

    #[test]
    fn stream_render() {
        let mut s = ChatResponseStream::new();
        s.push_markdown("Hello ");
        s.push_code("let x = 1;", Some("rust".into()));
        s.finish();
        let output = s.render();
        assert!(output.contains("Hello "));
        assert!(output.contains("```rust"));
        assert!(output.contains("let x = 1;"));
        assert!(s.is_complete());
    }

    #[test]
    fn stream_progress() {
        let mut s = ChatResponseStream::new();
        s.push_progress("Thinking...");
        assert_eq!(s.fragment_count(), 1);
        assert!(s.render().contains("[Thinking...]"));
    }

    // -- ChatHistoryPersistence tests --

    #[test]
    fn history_add_and_search() {
        let mut h = ChatHistoryPersistence::new(10);
        h.add(ChatHistoryEntry {
            request: "How to sort?".into(),
            response: "Use .sort()".into(),
            participant_id: "copilot".into(),
            timestamp: 1000,
        });
        assert_eq!(h.len(), 1);
        let found = h.search("sort");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn history_capacity() {
        let mut h = ChatHistoryPersistence::new(2);
        for i in 0..5 {
            h.add(ChatHistoryEntry {
                request: format!("q{}", i),
                response: "a".into(),
                participant_id: "p".into(),
                timestamp: i,
            });
        }
        assert_eq!(h.len(), 2);
    }

    // -- Follow-up suggestions tests --

    #[test]
    fn follow_ups_with_code() {
        let response = "Here is the code:\n```rust\nfn main() {}\n```\nThat's it.";
        let suggestions = generate_follow_ups(response, 5);
        assert!(suggestions.iter().any(|s| s.prompt.contains("tests")));
    }

    #[test]
    fn follow_ups_always_has_summary() {
        let suggestions = generate_follow_ups("short", 5);
        assert!(suggestions.iter().any(|s| s.prompt.contains("Summarize")));
    }

    #[test]
    fn follow_ups_respects_max() {
        let long = "x".repeat(300);
        let suggestions = generate_follow_ups(&long, 1);
        assert_eq!(suggestions.len(), 1);
    }

#[test]
    fn chatformatter_severity_ordering() {
        assert!(ChatFormatterSeverity::Critical > ChatFormatterSeverity::High);
        assert!(ChatFormatterSeverity::High > ChatFormatterSeverity::Medium);
        assert!(ChatFormatterSeverity::Medium > ChatFormatterSeverity::Low);
    }

    #[test]
    fn chatformatter_severity_display() {
        assert_eq!(ChatFormatterSeverity::Low.to_string(), "low");
        assert_eq!(ChatFormatterSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn chatformatter_entry_creation() {
        let e = ChatFormatterEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ChatFormatterSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn chatformatter_entry_builder() {
        let e = ChatFormatterEntry::new("e2", "Entry 2")
            .with_severity(ChatFormatterSeverity::High)
            .with_detail("some detail")
            .with_message_count(42);
        assert_eq!(e.severity, ChatFormatterSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.message_count, 42);
    }

    #[test]
    fn chatformatter_entry_enable_disable() {
        let mut e = ChatFormatterEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn chatformatter_add_and_count() {
        let mut mgr = ChatFormatter::new("test");
        mgr.add(ChatFormatterEntry::new("a", "A"));
        mgr.add(ChatFormatterEntry::new("b", "B").with_severity(ChatFormatterSeverity::High));
        assert_eq!(mgr.message_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn chatformatter_remove() {
        let mut mgr = ChatFormatter::new("test");
        mgr.add(ChatFormatterEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn chatformatter_capacity() {
        let mut mgr = ChatFormatter::new("test").with_capacity(1);
        assert!(mgr.add(ChatFormatterEntry::new("a", "A")));
        assert!(!mgr.add(ChatFormatterEntry::new("b", "B")));
    }

    #[test]
    fn chatformatter_sorted_by_severity() {
        let mut mgr = ChatFormatter::new("test");
        mgr.add(ChatFormatterEntry::new("lo", "Low"));
        mgr.add(ChatFormatterEntry::new("hi", "High").with_severity(ChatFormatterSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ChatFormatterSeverity::Critical);
    }

    #[test]
    fn chatformatter_summary() {
        let mgr = ChatFormatter::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn chatctxbuilder_config_defaults() {
        let cfg = ChatCtxBuilderConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn chatctxbuilder_item_creation() {
        let item = ChatCtxBuilderItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn chatctxbuilder_add_and_get() {
        let mut mgr = ChatCtxBuilder::new(ChatCtxBuilderConfig::new("test"));
        mgr.add(ChatCtxBuilderItem::new("k1", "v1"));
        assert_eq!(mgr.context_tokens(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn chatctxbuilder_remove_item() {
        let mut mgr = ChatCtxBuilder::new(ChatCtxBuilderConfig::new("test"));
        mgr.add(ChatCtxBuilderItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn chatctxbuilder_sorted_by_priority() {
        let mut mgr = ChatCtxBuilder::new(ChatCtxBuilderConfig::new("test"));
        mgr.add(ChatCtxBuilderItem::new("lo", "low").with_priority(1));
        mgr.add(ChatCtxBuilderItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn chatctxbuilder_items_with_tag() {
        let mut mgr = ChatCtxBuilder::new(ChatCtxBuilderConfig::new("test"));
        mgr.add(ChatCtxBuilderItem::new("a", "1").with_tag("x"));
        mgr.add(ChatCtxBuilderItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn chatctxbuilder_report() {
        let mgr = ChatCtxBuilder::new(ChatCtxBuilderConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn extChat_x_config_new() {
        let c = ExtChatXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn extChat_x_config_builder() {
        let c = ExtChatXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn extChat_x_config_display() {
        let c = ExtChatXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn extChat_x_registry_insert_get() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extChat_x_registry_duplicate() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a")).unwrap();
        assert!(reg.insert(ExtChatXConfig::new("a")).is_err());
    }

    #[test]
    fn extChat_x_registry_remove() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a")).unwrap();
        reg.insert(ExtChatXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extChat_x_registry_active_entries() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a")).unwrap();
        reg.insert(ExtChatXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn extChat_x_registry_by_weight() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ExtChatXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn extChat_x_registry_tags() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ExtChatXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn extChat_x_registry_total_weight() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ExtChatXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn extChat_x_registry_iterator() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a")).unwrap();
        reg.insert(ExtChatXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn extChat_x_cache_put_get() {
        let mut cache = ExtChatXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn extChat_x_cache_eviction() {
        let mut cache = ExtChatXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn extChat_x_cache_lru_order() {
        let mut cache = ExtChatXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn extChat_x_cache_most_least_recent() {
        let mut cache = ExtChatXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn extChat_x_formatter_entry() {
        let e = ExtChatXConfig::new("k").with_value("v");
        let fmt = ExtChatXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn extChat_x_formatter_summary() {
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ExtChatXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn extChat_x_validator_valid() {
        let v = ExtChatXValidator::new();
        let c = ExtChatXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn extChat_x_validator_empty_key() {
        let v = ExtChatXValidator::new();
        let c = ExtChatXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extChat_x_validator_require_value() {
        let v = ExtChatXValidator::new().require_value(true);
        let c = ExtChatXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extChat_x_validator_allowed_tags() {
        let v = ExtChatXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ExtChatXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extChat_x_validator_validate_all() {
        let v = ExtChatXValidator::new();
        let mut reg = ExtChatXRegistry::new();
        reg.insert(ExtChatXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    #[test]
    fn xb_ring_buffer_48_push_and_len() {
        let mut rb = super::XbRingBuffer48::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_48_overwrite() {
        let mut rb = super::XbRingBuffer48::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_48_get_out_of_bounds() {
        let rb = super::XbRingBuffer48::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_48_drain_all() {
        let mut rb = super::XbRingBuffer48::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_48_peek_front_back() {
        let mut rb = super::XbRingBuffer48::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_48_clear() {
        let mut rb = super::XbRingBuffer48::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_48_capacity() {
        let rb = super::XbRingBuffer48::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_48_basic() {
        let h = super::xb_fnv1a_48(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_48(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_48_different_inputs() {
        let h1 = super::xb_fnv1a_48(b"abc");
        let h2 = super::xb_fnv1a_48(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_48_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_48(&data);
        let dec = super::xb_rle_decode_48(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_48_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_48(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_48(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_48_values() {
        assert!((super::xb_clamp_48(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_48(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_48(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_48_values() {
        assert!((super::xb_lerp_48(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_48(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_48(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_48_wrap_around_twice() {
        let mut rb = super::XbRingBuffer48::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 51 ----

    #[test]
    fn xc_51_pool_new_empty() {
        let pool: super::Xc51Pool<i32> = super::Xc51Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_51_pool_release_acquire() {
        let mut pool = super::Xc51Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_51_pool_acquire_empty() {
        let mut pool: super::Xc51Pool<i32> = super::Xc51Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_51_pool_full() {
        let mut pool = super::Xc51Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_51_pool_drain() {
        let mut pool = super::Xc51Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_51_pool_stats() {
        let mut pool = super::Xc51Pool::new(8);
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
    fn xc_51_pool_clear() {
        let mut pool = super::Xc51Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_51_pool_shrink() {
        let mut pool = super::Xc51Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_51_pool_default() {
        let pool: super::Xc51Pool<String> = super::Xc51Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_51_pool_extend() {
        let mut pool = super::Xc51Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_51_pool_retain() {
        let mut pool = super::Xc51Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_51_scheduler_round_robin() {
        let mut sched = super::Xc51Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_51_scheduler_empty() {
        let mut sched = super::Xc51Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_51_scheduler_reset() {
        let mut sched = super::Xc51Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_51_scheduler_add_remove() {
        let mut sched = super::Xc51Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_51_scheduler_targets() {
        let sched = super::Xc51Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_51_hash_empty() {
        assert_eq!(super::xc_51_hash(b""), 5381);
    }

    #[test]
    fn xc_51_hash_data() {
        let h = super::xc_51_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_51_hash(b"hello"), h);
    }

    #[test]
    fn xc_51_reverse_str() {
        assert_eq!(super::xc_51_reverse("abc"), "cba");
        assert_eq!(super::xc_51_reverse(""), "");
    }


    #[test]
    fn xe_61_pipeline_empty() {
        let p = super::Xe61Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_61_pipeline_parse_stage() {
        let p = super::Xe61Pipeline::new()
            .add_parse(super::xe_61_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_61_pipeline_transform_double() {
        let p = super::Xe61Pipeline::new()
            .add_transform(super::xe_61_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_61_pipeline_validate_reverse() {
        let p = super::Xe61Pipeline::new()
            .add_validate(super::xe_61_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_61_pipeline_emit_filter() {
        let p = super::Xe61Pipeline::new()
            .add_emit(super::xe_61_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_61_pipeline_multi_stage() {
        let p = super::Xe61Pipeline::new()
            .add_parse(super::xe_61_pipeline_identity)
            .add_transform(super::xe_61_pipeline_double)
            .add_validate(super::xe_61_pipeline_reverse)
            .add_emit(super::xe_61_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_61_pipeline_error_propagation() {
        let p = super::Xe61Pipeline::new()
            .add_parse(super::xe_61_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe61Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_61_pipeline_compose() {
        let p1 = super::Xe61Pipeline::new()
            .add_parse(super::xe_61_pipeline_identity);
        let p2 = super::Xe61Pipeline::new()
            .add_transform(super::xe_61_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_61_pipeline_error_display() {
        let e = super::Xe61PipelineError {
            stage: super::Xe61Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_61_cache_put_get() {
        let mut c = super::Xe61Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_61_cache_miss() {
        let mut c: super::Xe61Cache<&str, i32> = super::Xe61Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_61_cache_ttl_expiry() {
        let mut c = super::Xe61Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_61_cache_evict() {
        let mut c = super::Xe61Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_61_cache_capacity() {
        let mut c = super::Xe61Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_61_cache_stats() {
        let mut c = super::Xe61Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_61_cache_clear() {
        let mut c = super::Xe61Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_59 graph tests ------------------------------------------------

    #[test]
    fn xg_59_graph_empty() {
        let g = super::Xg59Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_59_graph_add_node() {
        let mut g = super::Xg59Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_59_graph_add_edge() {
        let mut g = super::Xg59Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_59_graph_neighbors() {
        let mut g = super::Xg59Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_59_graph_has_path() {
        let mut g = super::Xg59Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_59_graph_self_path() {
        let g = super::Xg59Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_59_graph_topo_sort() {
        let mut g = super::Xg59Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_59_graph_cycle_detect_false() {
        let mut g = super::Xg59Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_59_graph_cycle_detect_true() {
        let mut g = super::Xg59Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_59 heap tests -------------------------------------------------

    #[test]
    fn xg_59_heap_empty() {
        let h: super::Xg59Heap<i32> = super::Xg59Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_59_heap_push_pop() {
        let mut h = super::Xg59Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_59_heap_peek() {
        let mut h = super::Xg59Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_59_heap_drain_sorted() {
        let mut h = super::Xg59Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_59_heap_merge() {
        let mut a = super::Xg59Heap::new();
        let mut b = super::Xg59Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_59_heap_default() {
        let h: super::Xg59Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_59_graph_default() {
        let g: super::Xg59Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh50_skip_insert_contains() {
        let mut sl = super::Xh50SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh50_skip_remove() {
        let mut sl = super::Xh50SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh50_skip_len() {
        let mut sl = super::Xh50SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh50_skip_range_query() {
        let mut sl = super::Xh50SkipList::xh_new(4);
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
    fn xh50_skip_floor_ceiling() {
        let mut sl = super::Xh50SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh50_skip_rank() {
        let mut sl = super::Xh50SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh50_skip_empty() {
        let sl = super::Xh50SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh50_skip_duplicates() {
        let mut sl = super::Xh50SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh50_bitset_set_test() {
        let mut bs = super::Xh50BitSet::xh_new(256);
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
    fn xh50_bitset_clear_count() {
        let mut bs = super::Xh50BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh50_bitset_and_or_xor() {
        let mut a = super::Xh50BitSet::xh_new(128);
        let mut b = super::Xh50BitSet::xh_new(128);
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
    fn xh50_bitset_iter_ones() {
        let mut bs = super::Xh50BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh50_bitset_first_last() {
        let mut bs = super::Xh50BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh50_bitset_empty() {
        let bs = super::Xh50BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi50_deque_push_pop_back() {
        let mut dq = super::Xi50Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi50_deque_push_pop_front() {
        let mut dq = super::Xi50Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi50_deque_mixed_ops() {
        let mut dq = super::Xi50Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi50_deque_get_and_split() {
        let mut dq = super::Xi50Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi50_deque_rotate_left() {
        let mut dq = super::Xi50Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi50_deque_rotate_right() {
        let mut dq = super::Xi50Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi50_deque_grow() {
        let mut dq = super::Xi50Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi50_deque_empty() {
        let dq = super::Xi50Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi50_interval_tree_insert_query() {
        let mut tree = super::Xi50IntervalTree::xi_new();
        tree.xi_insert(super::Xi50Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi50Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi50Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi50_interval_tree_overlap() {
        let mut tree = super::Xi50IntervalTree::xi_new();
        tree.xi_insert(super::Xi50Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi50Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi50Interval::xi_new(12, 20));
        let q = super::Xi50Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi50_interval_tree_remove() {
        let mut tree = super::Xi50IntervalTree::xi_new();
        tree.xi_insert(super::Xi50Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi50Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi50_interval_tree_gaps() {
        let mut tree = super::Xi50IntervalTree::xi_new();
        tree.xi_insert(super::Xi50Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi50Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi50Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi50Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi50Interval::xi_new(8, 10));
    }

    #[test]
    fn xi50_interval_tree_merge() {
        let mut tree = super::Xi50IntervalTree::xi_new();
        tree.xi_insert(super::Xi50Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi50Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi50Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi50Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi50Interval::xi_new(10, 15));
    }

    #[test]
    fn xi50_interval_tree_all() {
        let mut tree = super::Xi50IntervalTree::xi_new();
        tree.xi_insert(super::Xi50Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi50Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi50_interval_tree_empty() {
        let tree = super::Xi50IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi50_interval_tree_contains_point() {
        let iv = super::Xi50Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
