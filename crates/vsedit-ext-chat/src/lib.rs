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
}
