//! Chat view panel.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur in chat operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatError {
    /// Message with the given ID was not found.
    MessageNotFound(u64),
    /// Session has no messages to summarize or export.
    EmptySession,
    /// Content exceeds the maximum allowed length.
    ContentTooLong { max: usize, actual: usize },
    /// Invalid session ID (empty string).
    InvalidSessionId,
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::MessageNotFound(id) => write!(f, "message not found: {id}"),
            ChatError::EmptySession => write!(f, "session contains no messages"),
            ChatError::ContentTooLong { max, actual } => {
                write!(f, "content length {actual} exceeds maximum {max}")
            }
            ChatError::InvalidSessionId => write!(f, "session id must not be empty"),
        }
    }
}

impl std::error::Error for ChatError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMessageStatus {
    Pending,
    Streaming,
    Complete,
    Error,
}

impl fmt::Display for ChatRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatRole::User => write!(f, "user"),
            ChatRole::Assistant => write!(f, "assistant"),
            ChatRole::System => write!(f, "system"),
        }
    }
}

impl fmt::Display for ChatMessageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatMessageStatus::Pending => write!(f, "pending"),
            ChatMessageStatus::Streaming => write!(f, "streaming"),
            ChatMessageStatus::Complete => write!(f, "complete"),
            ChatMessageStatus::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub id: u64,
    pub role: ChatRole,
    pub content: String,
    pub timestamp: u64,
    pub status: ChatMessageStatus,
}

impl fmt::Display for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.status, self.role, self.content)
    }
}

/// Maximum allowed content length for a single message.
pub const MAX_CONTENT_LENGTH: usize = 32_768;

#[derive(Debug, Clone, PartialEq)]
pub struct ChatSession {
    pub id: String,
    pub messages: Vec<ChatMessage>,
    next_msg_id: u64,
    pub title: Option<String>,
    pub created_at: u64,
}

impl ChatSession {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            messages: Vec::new(),
            next_msg_id: 0,
            title: None,
            created_at: 0,
        }
    }

    pub fn add_message(&mut self, role: ChatRole, content: impl Into<String>, timestamp: u64) -> u64 {
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        self.messages.push(ChatMessage {
            id,
            role,
            content: content.into(),
            timestamp,
            status: ChatMessageStatus::Complete,
        });
        id
    }

    pub fn add_streaming_message(&mut self, role: ChatRole, timestamp: u64) -> u64 {
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        self.messages.push(ChatMessage {
            id,
            role,
            content: String::new(),
            timestamp,
            status: ChatMessageStatus::Pending,
        });
        id
    }

    pub fn update_message_content(&mut self, id: u64, content: &str) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.content = content.to_string();
            msg.status = ChatMessageStatus::Streaming;
        }
    }

    pub fn complete_message(&mut self, id: u64) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.status = ChatMessageStatus::Complete;
        }
    }

    pub fn fail_message(&mut self, id: u64, error: &str) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.content = error.to_string();
            msg.status = ChatMessageStatus::Error;
        }
    }

    pub fn get_message(&self, id: u64) -> Option<&ChatMessage> {
        self.messages.iter().find(|m| m.id == id)
    }

    pub fn delete_message(&mut self, id: u64) -> bool {
        let len_before = self.messages.len();
        self.messages.retain(|m| m.id != id);
        self.messages.len() != len_before
    }

    pub fn messages_by_role(&self, role: ChatRole) -> Vec<&ChatMessage> {
        self.messages.iter().filter(|m| m.role == role).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    pub fn last_message(&self) -> Option<&ChatMessage> {
        self.messages.last()
    }

    /// Validate and create a new session, returning an error if the id is empty.
    pub fn try_new(id: impl Into<String>) -> Result<Self, ChatError> {
        let id = id.into();
        if id.is_empty() {
            return Err(ChatError::InvalidSessionId);
        }
        Ok(Self {
            id,
            messages: Vec::new(),
            next_msg_id: 0,
            title: None,
            created_at: 0,
        })
    }

    /// Add a message with content-length validation.
    pub fn try_add_message(
        &mut self,
        role: ChatRole,
        content: impl Into<String>,
        timestamp: u64,
    ) -> Result<u64, ChatError> {
        let content = content.into();
        if content.len() > MAX_CONTENT_LENGTH {
            return Err(ChatError::ContentTooLong {
                max: MAX_CONTENT_LENGTH,
                actual: content.len(),
            });
        }
        Ok(self.add_message(role, content, timestamp))
    }

    /// Update message content with validation, returning an error if the message is not found.
    pub fn try_update_message_content(
        &mut self,
        id: u64,
        content: &str,
    ) -> Result<(), ChatError> {
        if content.len() > MAX_CONTENT_LENGTH {
            return Err(ChatError::ContentTooLong {
                max: MAX_CONTENT_LENGTH,
                actual: content.len(),
            });
        }
        if self.messages.iter_mut().find(|m| m.id == id).is_none() {
            return Err(ChatError::MessageNotFound(id));
        }
        self.update_message_content(id, content);
        Ok(())
    }

    /// Complete a message, returning an error if the message is not found.
    pub fn try_complete_message(&mut self, id: u64) -> Result<(), ChatError> {
        match self.messages.iter_mut().find(|m| m.id == id) {
            Some(msg) => {
                msg.status = ChatMessageStatus::Complete;
                Ok(())
            }
            None => Err(ChatError::MessageNotFound(id)),
        }
    }

    /// Export the conversation as a plain-text transcript.
    pub fn export_transcript(&self) -> Result<String, ChatError> {
        if self.messages.is_empty() {
            return Err(ChatError::EmptySession);
        }
        let mut buf = String::new();
        if let Some(title) = &self.title {
            buf.push_str(&format!("# {title}\n\n"));
        }
        for msg in &self.messages {
            buf.push_str(&format!("[{}] {}\n", msg.role, msg.content));
        }
        Ok(buf)
    }

    /// Compute total character count across all messages.
    pub fn total_content_length(&self) -> usize {
        self.messages.iter().map(|m| m.content.len()).sum()
    }

    /// Count messages that are in an error state.
    pub fn error_count(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.status == ChatMessageStatus::Error)
            .count()
    }

    /// Return the timestamp range (earliest, latest) of messages, or `None` if empty.
    pub fn timestamp_range(&self) -> Option<(u64, u64)> {
        let min = self.messages.iter().map(|m| m.timestamp).min()?;
        let max = self.messages.iter().map(|m| m.timestamp).max()?;
        Some((min, max))
    }

    /// Check whether any message is still pending or streaming.
    pub fn has_pending_work(&self) -> bool {
        self.messages.iter().any(|m| {
            m.status == ChatMessageStatus::Pending || m.status == ChatMessageStatus::Streaming
        })
    }
}

impl fmt::Display for ChatSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.title.as_deref().unwrap_or("(untitled)");
        write!(
            f,
            "ChatSession({}, {}, {} messages)",
            self.id,
            title,
            self.messages.len()
        )
    }
}

/// Builder for constructing a [`ChatSession`] with optional fields.
#[derive(Debug, Clone)]
pub struct ChatSessionBuilder {
    id: String,
    title: Option<String>,
    created_at: u64,
    system_prompt: Option<String>,
}

impl ChatSessionBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            created_at: 0,
            system_prompt: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn created_at(mut self, ts: u64) -> Self {
        self.created_at = ts;
        self
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn build(self) -> Result<ChatSession, ChatError> {
        if self.id.is_empty() {
            return Err(ChatError::InvalidSessionId);
        }
        let mut session = ChatSession {
            id: self.id,
            messages: Vec::new(),
            next_msg_id: 0,
            title: self.title,
            created_at: self.created_at,
        };
        if let Some(prompt) = self.system_prompt {
            session.add_message(ChatRole::System, prompt, self.created_at);
        }
        Ok(session)
    }
}

/// Accumulated statistics for chat-view operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatViewStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ChatViewStats {
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
    pub fn merge(&mut self, other: &ChatViewStats) {
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

impl Default for ChatViewStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ChatViewStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ChatViewStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for chat-view.
#[derive(Debug, Clone)]
pub struct ChatViewValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ChatViewValidator {
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

impl Default for ChatViewValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ChatMessageRenderer – renders messages/sessions to plain text
// ---------------------------------------------------------------------------

/// Renders chat messages and sessions to plain text with configurable formatting.
#[derive(Debug, Clone)]
pub struct ChatMessageRenderer {
    pub show_timestamps: bool,
    pub show_role_prefix: bool,
    pub max_line_width: Option<usize>,
}

impl ChatMessageRenderer {
    /// Creates a new renderer with timestamps and role prefixes enabled and no
    /// maximum line width.
    pub fn new() -> Self {
        Self {
            show_timestamps: true,
            show_role_prefix: true,
            max_line_width: None,
        }
    }

    /// Builder helper – enable or disable timestamp display.
    pub fn with_timestamps(mut self, show: bool) -> Self {
        self.show_timestamps = show;
        self
    }

    /// Builder helper – set a maximum line width for word-wrapping.
    pub fn with_max_width(mut self, width: usize) -> Self {
        self.max_line_width = Some(width);
        self
    }

    /// Render a single [`ChatMessage`] to a plain-text string.
    ///
    /// When `show_role_prefix` is true the line starts with `[role]`.
    /// When `show_timestamps` is true the timestamp is prepended.
    pub fn render_message(&self, msg: &ChatMessage) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.show_timestamps {
            parts.push(format!("[{}]", msg.timestamp));
        }
        if self.show_role_prefix {
            parts.push(format!("[{}]", msg.role));
        }

        parts.push(msg.content.clone());

        let line = parts.join(" ");

        match self.max_line_width {
            Some(w) => Self::word_wrap(&line, w),
            None => line,
        }
    }

    /// Render every message in a [`ChatSession`], separated by newlines.
    pub fn render_session(&self, session: &ChatSession) -> String {
        session
            .messages
            .iter()
            .map(|m| self.render_message(m))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Word-wrap `text` so that no output line exceeds `max_width` characters.
    ///
    /// Words that are themselves longer than `max_width` are placed on their
    /// own line without breaking.
    pub fn word_wrap(text: &str, max_width: usize) -> String {
        if max_width == 0 {
            return text.to_string();
        }

        let mut result = String::new();
        for (i, input_line) in text.lines().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            let words: Vec<&str> = input_line.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            let mut current_len: usize = 0;
            for (j, word) in words.iter().enumerate() {
                let wlen = word.len();
                if j == 0 {
                    result.push_str(word);
                    current_len = wlen;
                } else if current_len + 1 + wlen > max_width {
                    result.push('\n');
                    result.push_str(word);
                    current_len = wlen;
                } else {
                    result.push(' ');
                    result.push_str(word);
                    current_len += 1 + wlen;
                }
            }
        }
        result
    }
}

impl Default for ChatMessageRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ChatCodeBlock – extract fenced code blocks from message content
// ---------------------------------------------------------------------------

/// A fenced code block extracted from a chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCodeBlock {
    /// Optional language tag (e.g. `"rust"`, `"python"`).
    pub language: Option<String>,
    /// The raw code inside the fences.
    pub code: String,
    /// 1-based start line of the opening fence in the source content.
    pub start_line: usize,
    /// 1-based end line of the closing fence in the source content.
    pub end_line: usize,
}

impl ChatCodeBlock {
    /// Scan `content` for fenced code blocks (` ```lang\n…\n``` `) and return
    /// all that are found.
    pub fn extract_code_blocks(content: &str) -> Vec<ChatCodeBlock> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut idx = 0;
        while idx < lines.len() {
            let trimmed = lines[idx].trim();
            if trimmed.starts_with("```") {
                let lang_tag = trimmed.trim_start_matches('`');
                let language = if lang_tag.is_empty() {
                    None
                } else {
                    Some(lang_tag.to_string())
                };
                let start_line = idx + 1; // 1-based
                let mut code_lines: Vec<&str> = Vec::new();
                idx += 1;
                while idx < lines.len() {
                    let t = lines[idx].trim();
                    if t == "```" {
                        break;
                    }
                    code_lines.push(lines[idx]);
                    idx += 1;
                }
                let end_line = idx + 1; // 1-based (line of closing fence)
                blocks.push(ChatCodeBlock {
                    language,
                    code: code_lines.join("\n"),
                    start_line,
                    end_line,
                });
            }
            idx += 1;
        }
        blocks
    }

    /// Number of lines in the code body.
    pub fn line_count(&self) -> usize {
        if self.code.is_empty() {
            return 0;
        }
        self.code.lines().count()
    }

    /// Human-readable label such as `"rust (5 lines)"` or `"code (3 lines)"`.
    pub fn display_label(&self) -> String {
        let tag = self
            .language
            .as_deref()
            .unwrap_or("code");
        format!("{} ({} lines)", tag, self.line_count())
    }
}

// ---------------------------------------------------------------------------
// Standalone helper functions
// ---------------------------------------------------------------------------

/// Produce a copy-friendly plain-text version of a chat message.
///
/// When `include_role` is true the role is prepended as `[role] `.
pub fn chat_message_copy(msg: &ChatMessage, include_role: bool) -> String {
    if include_role {
        format!("[{}] {}", msg.role, msg.content)
    } else {
        msg.content.clone()
    }
}

/// Export an entire [`ChatSession`] as a Markdown document.
///
/// Each message is rendered under a heading that matches its role:
///
/// ```text
/// ## User
///
/// message content
///
/// ## Assistant
///
/// reply content
/// ```
///
/// Returns [`ChatError::EmptySession`] when the session contains no messages.
pub fn chat_session_export_markdown(session: &ChatSession) -> Result<String, ChatError> {
    if session.messages.is_empty() {
        return Err(ChatError::EmptySession);
    }

    let mut md = String::new();
    if let Some(ref title) = session.title {
        md.push_str(&format!("# {}\n\n", title));
    }

    for (i, msg) in session.messages.iter().enumerate() {
        if i > 0 {
            md.push('\n');
        }
        let heading = match msg.role {
            ChatRole::User => "User",
            ChatRole::Assistant => "Assistant",
            ChatRole::System => "System",
        };
        md.push_str(&format!("## {}\n\n{}\n", heading, msg.content));
    }

    Ok(md)
}

// ---------------------------------------------------------------------------
// Message search and filtering
// ---------------------------------------------------------------------------

/// Search results from scanning chat messages.
#[derive(Debug, Clone)]
pub struct ChatSearchResult {
    /// The message ID that matched.
    pub message_id: u64,
    /// Byte offset of the match within the message content.
    pub offset: usize,
    /// The matched substring.
    pub snippet: String,
}

/// Search messages in a session for a query string (case-insensitive).
pub fn search_messages(session: &ChatSession, query: &str) -> Vec<ChatSearchResult> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    for msg in session.get_messages() {
        let content_lower = msg.content.to_lowercase();
        for (offset, _) in content_lower.match_indices(&query_lower) {
            let end = (offset + query.len()).min(msg.content.len());
            results.push(ChatSearchResult {
                message_id: msg.id,
                offset,
                snippet: msg.content[offset..end].to_string(),
            });
        }
    }
    results
}

/// Filter messages by role.
pub fn filter_messages_by_role(session: &ChatSession, role: ChatRole) -> Vec<&ChatMessage> {
    session.messages_by_role(role)
}

/// Filter messages by status.
pub fn filter_messages_by_status(
    session: &ChatSession,
    status: ChatMessageStatus,
) -> Vec<&ChatMessage> {
    session
        .get_messages()
        .iter()
        .filter(|m| m.status == status)
        .collect()
}

// ---------------------------------------------------------------------------
// Conversation threading
// ---------------------------------------------------------------------------

/// A thread of conversation: a user message and its assistant reply.
#[derive(Debug, Clone)]
pub struct ConversationThread {
    /// The user's message.
    pub user_message: ChatMessage,
    /// The assistant's reply, if any.
    pub assistant_reply: Option<ChatMessage>,
}

/// Extract conversation threads from a session.
///
/// Pairs each user message with the immediately following assistant message.
pub fn extract_threads(session: &ChatSession) -> Vec<ConversationThread> {
    let messages = session.get_messages();
    let mut threads = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        if messages[i].role == ChatRole::User {
            let reply = if i + 1 < messages.len() && messages[i + 1].role == ChatRole::Assistant {
                Some(messages[i + 1].clone())
            } else {
                None
            };
            threads.push(ConversationThread {
                user_message: messages[i].clone(),
                assistant_reply: reply.clone(),
            });
            if reply.is_some() {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    threads
}

// ---------------------------------------------------------------------------
// Message statistics
// ---------------------------------------------------------------------------

/// Statistics about a chat session's content.
#[derive(Debug, Clone, Default)]
pub struct MessageStatistics {
    pub total_messages: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub system_messages: usize,
    pub total_words: usize,
    pub total_chars: usize,
    pub avg_message_length: f64,
}

/// Compute statistics about the messages in a session.
pub fn compute_message_statistics(session: &ChatSession) -> MessageStatistics {
    let messages = session.get_messages();
    let total_messages = messages.len();
    let mut user_messages = 0;
    let mut assistant_messages = 0;
    let mut system_messages = 0;
    let mut total_words = 0;
    let mut total_chars: usize = 0;

    for msg in messages {
        match msg.role {
            ChatRole::User => user_messages += 1,
            ChatRole::Assistant => assistant_messages += 1,
            ChatRole::System => system_messages += 1,
        }
        total_words += msg.content.split_whitespace().count();
        total_chars += msg.content.len();
    }

    let avg_message_length = if total_messages > 0 {
        total_chars as f64 / total_messages as f64
    } else {
        0.0
    };

    MessageStatistics {
        total_messages,
        user_messages,
        assistant_messages,
        system_messages,
        total_words,
        total_chars,
        avg_message_length,
    }
}

/// Count code blocks (triple-backtick fenced) in a message.
pub fn count_code_blocks(content: &str) -> usize {
    let fence_count = content.matches("```").count();
    fence_count / 2
}

/// Compute the average message length (in characters) in a session.
pub fn average_message_length(session: &ChatSession) -> f64 {
    let msgs = session.get_messages();
    if msgs.is_empty() {
        return 0.0;
    }
    let total: usize = msgs.iter().map(|m| m.content.len()).sum();
    total as f64 / msgs.len() as f64
}

/// Return the number of distinct roles that appear in a session.
pub fn distinct_role_count(session: &ChatSession) -> usize {
    let mut roles = Vec::new();
    for m in session.get_messages() {
        if !roles.contains(&m.role) {
            roles.push(m.role.clone());
        }
    }
    roles.len()
}

/// Find messages in a session that contain code blocks.
pub fn messages_with_code(session: &ChatSession) -> Vec<&ChatMessage> {
    session
        .get_messages()
        .iter()
        .filter(|m| count_code_blocks(&m.content) > 0)
        .collect()
}

/// Return the longest message (by content length) in a session.
pub fn longest_message(session: &ChatSession) -> Option<&ChatMessage> {
    session
        .get_messages()
        .iter()
        .max_by_key(|m| m.content.len())
}

/// Summarize a chat session into a human-readable string.
pub fn session_summary(session: &ChatSession) -> String {
    let total = session.message_count();
    let user_count = session.messages_by_role(ChatRole::User).len();
    let assistant_count = session.messages_by_role(ChatRole::Assistant).len();
    format!(
        "{} messages ({} user, {} assistant)",
        total, user_count, assistant_count
    )
}

/// Extract all unique "languages" from code blocks in a session.
pub fn code_block_languages(session: &ChatSession) -> Vec<String> {
    let mut langs = Vec::new();
    for msg in session.get_messages() {
        for block in ChatCodeBlock::extract_code_blocks(&msg.content) {
            if let Some(ref lang) = block.language {
                if !langs.contains(lang) {
                    langs.push(lang.clone());
                }
            }
        }
    }
    langs
}

/// Count how many messages are in each status.
pub fn count_by_status(session: &ChatSession) -> Vec<(ChatMessageStatus, usize)> {
    let mut counts: Vec<(ChatMessageStatus, usize)> = Vec::new();
    for m in session.get_messages() {
        if let Some(entry) = counts.iter_mut().find(|(s, _)| *s == m.status) {
            entry.1 += 1;
        } else {
            counts.push((m.status.clone(), 1));
        }
    }
    counts
}

/// Return the total character count of all message content.
pub fn total_content_length(session: &ChatSession) -> usize {
    session.get_messages().iter().map(|m| m.content.len()).sum()
}

/// A user→assistant exchange pair.
#[derive(Debug, Clone, PartialEq)]
pub struct Exchange<'a> {
    pub user_message: &'a ChatMessage,
    pub assistant_message: &'a ChatMessage,
}

/// Extract consecutive user→assistant exchanges.
pub fn extract_exchanges(session: &ChatSession) -> Vec<Exchange<'_>> {
    let msgs = session.get_messages();
    let mut exchanges = Vec::new();
    let mut i = 0;
    while i + 1 < msgs.len() {
        if msgs[i].role == ChatRole::User && msgs[i + 1].role == ChatRole::Assistant {
            exchanges.push(Exchange { user_message: &msgs[i], assistant_message: &msgs[i + 1] });
            i += 2;
        } else { i += 1; }
    }
    exchanges
}

/// Response ratio: assistant messages / user messages.
pub fn response_ratio(session: &ChatSession) -> Option<f64> {
    let user_count = session.messages_by_role(ChatRole::User).len();
    if user_count == 0 { return None; }
    Some(session.messages_by_role(ChatRole::Assistant).len() as f64 / user_count as f64)
}

/// Average response length (assistant messages only).
pub fn avg_response_length(session: &ChatSession) -> Option<f64> {
    let msgs: Vec<&ChatMessage> = session.messages_by_role(ChatRole::Assistant);
    if msgs.is_empty() { return None; }
    let total: usize = msgs.iter().map(|m| m.content.len()).sum();
    Some(total as f64 / msgs.len() as f64)
}

/// Find first message containing text (case-insensitive).
pub fn find_message_containing<'a>(session: &'a ChatSession, text: &str) -> Option<&'a ChatMessage> {
    let lower = text.to_lowercase();
    session.get_messages().iter().find(|m| m.content.to_lowercase().contains(&lower))
}

/// Return messages longer than a given threshold.
pub fn messages_longer_than(session: &ChatSession, threshold: usize) -> Vec<&ChatMessage> {
    session.get_messages().iter().filter(|m| m.content.len() > threshold).collect()
}

/// Time span between first and last message.
pub fn session_duration(session: &ChatSession) -> u64 {
    let msgs = session.get_messages();
    if msgs.len() < 2 { return 0; }
    msgs.last().map(|m| m.timestamp).unwrap_or(0).saturating_sub(msgs.first().map(|m| m.timestamp).unwrap_or(0))
}

// ---------------------------------------------------------------------------
// ChatViewScrollManager – auto-scroll logic
// ---------------------------------------------------------------------------

/// Manages auto-scroll behavior for the chat view.
pub struct ChatViewScrollManager {
    auto_scroll: bool,
    scroll_offset: usize,
    total_lines: usize,
    viewport_lines: usize,
}

impl ChatViewScrollManager {
    /// Create a scroll manager with the given viewport size.
    pub fn new(viewport_lines: usize) -> Self {
        Self {
            auto_scroll: true,
            scroll_offset: 0,
            total_lines: 0,
            viewport_lines,
        }
    }

    /// Update the total content lines and auto-scroll if enabled.
    pub fn set_total_lines(&mut self, total: usize) {
        self.total_lines = total;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Scroll to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.total_lines.saturating_sub(self.viewport_lines);
    }

    /// Scroll up by a number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.auto_scroll = false;
    }

    /// Scroll down by a number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = (self.scroll_offset + lines)
            .min(self.total_lines.saturating_sub(self.viewport_lines));
        if self.is_at_bottom() {
            self.auto_scroll = true;
        }
    }

    /// Whether the view is scrolled to the bottom.
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset >= self.total_lines.saturating_sub(self.viewport_lines)
    }

    /// Whether auto-scroll is active.
    pub fn is_auto_scroll(&self) -> bool {
        self.auto_scroll
    }

    /// Current scroll offset.
    pub fn offset(&self) -> usize {
        self.scroll_offset
    }

    /// Re-enable auto-scroll.
    pub fn enable_auto_scroll(&mut self) {
        self.auto_scroll = true;
        self.scroll_to_bottom();
    }
}

// ---------------------------------------------------------------------------
// ChatInputHistory – up/down navigation through past inputs
// ---------------------------------------------------------------------------

/// Stores chat input history for up/down arrow navigation.
pub struct ChatInputHistory {
    entries: Vec<String>,
    cursor: Option<usize>,
    max_entries: usize,
}

impl ChatInputHistory {
    /// Create a history with a max number of entries.
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), cursor: None, max_entries }
    }

    /// Add an input to history.
    pub fn push(&mut self, input: impl Into<String>) {
        let s = input.into();
        if s.is_empty() { return; }
        // Remove duplicates
        self.entries.retain(|e| e != &s);
        self.entries.push(s);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.cursor = None;
    }

    /// Navigate up (older). Returns the entry if available.
    pub fn up(&mut self) -> Option<&str> {
        if self.entries.is_empty() { return None; }
        let idx = match self.cursor {
            None => self.entries.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.cursor = Some(idx);
        Some(&self.entries[idx])
    }

    /// Navigate down (newer). Returns the entry if available.
    pub fn down(&mut self) -> Option<&str> {
        if self.entries.is_empty() { return None; }
        match self.cursor {
            None => None,
            Some(i) if i + 1 >= self.entries.len() => {
                self.cursor = None;
                None
            }
            Some(i) => {
                self.cursor = Some(i + 1);
                Some(&self.entries[i + 1])
            }
        }
    }

    /// Reset the cursor (after submitting a new input).
    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    /// Number of entries in history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ChatMentionPicker – participant mention picking (@user)
// ---------------------------------------------------------------------------

/// A participant that can be mentioned in chat.
#[derive(Debug, Clone)]
pub struct ChatParticipant {
    pub id: String,
    pub display_name: String,
}

/// Picks mentions from a list of participants.
pub struct ChatMentionPicker {
    participants: Vec<ChatParticipant>,
}

impl ChatMentionPicker {
    /// Create a picker with no participants.
    pub fn new() -> Self {
        Self { participants: Vec::new() }
    }

    /// Register a participant.
    pub fn add_participant(&mut self, id: impl Into<String>, display_name: impl Into<String>) {
        self.participants.push(ChatParticipant {
            id: id.into(),
            display_name: display_name.into(),
        });
    }

    /// Search for participants matching a query (case-insensitive prefix match).
    pub fn search(&self, query: &str) -> Vec<&ChatParticipant> {
        let q = query.to_lowercase();
        self.participants.iter()
            .filter(|p| {
                p.display_name.to_lowercase().starts_with(&q)
                    || p.id.to_lowercase().starts_with(&q)
            })
            .collect()
    }

    /// Extract mention triggers from text (words starting with @).
    pub fn extract_mentions(text: &str) -> Vec<String> {
        text.split_whitespace()
            .filter(|w| w.starts_with('@') && w.len() > 1)
            .map(|w| w[1..].to_string())
            .collect()
    }

    /// Number of registered participants.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
}

// ---------------------------------------------------------------------------
// ChatViewTheme
// ---------------------------------------------------------------------------

/// Custom color theme for the chat UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatViewTheme {
    pub user_message_color: String,
    pub assistant_message_color: String,
    pub system_message_color: String,
    pub background_color: String,
    pub border_color: String,
}

impl ChatViewTheme {
    /// Dark theme defaults.
    pub fn default_dark() -> Self {
        Self {
            user_message_color: "#569cd6".to_string(),
            assistant_message_color: "#b5cea8".to_string(),
            system_message_color: "#808080".to_string(),
            background_color: "#1e1e1e".to_string(),
            border_color: "#333333".to_string(),
        }
    }

    /// Light theme defaults.
    pub fn default_light() -> Self {
        Self {
            user_message_color: "#0451a5".to_string(),
            assistant_message_color: "#098658".to_string(),
            system_message_color: "#6a6a6a".to_string(),
            background_color: "#ffffff".to_string(),
            border_color: "#cccccc".to_string(),
        }
    }

    /// Set the user message color (builder pattern).
    pub fn with_user_color(mut self, color: &str) -> Self {
        self.user_message_color = color.to_string();
        self
    }

    /// Set the assistant message color (builder pattern).
    pub fn with_assistant_color(mut self, color: &str) -> Self {
        self.assistant_message_color = color.to_string();
        self
    }

    /// Returns `true` if the background color looks dark (starts with low hex).
    pub fn is_dark(&self) -> bool {
        let bg = self.background_color.trim_start_matches('#');
        if bg.len() < 2 {
            return false;
        }
        let first_byte = u8::from_str_radix(&bg[..2], 16).unwrap_or(128);
        first_byte < 128
    }
}

impl Default for ChatViewTheme {
    fn default() -> Self {
        Self::default_dark()
    }
}

impl fmt::Display for ChatViewTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Theme(bg={}, border={}, user={}, assistant={}, system={})",
            self.background_color,
            self.border_color,
            self.user_message_color,
            self.assistant_message_color,
            self.system_message_color,
        )
    }
}

// ---------------------------------------------------------------------------
// ChatViewAccessibility
// ---------------------------------------------------------------------------

/// Screen reader and accessibility support for the chat view.
#[derive(Debug, Clone, Default)]
pub struct ChatViewAccessibility {
    announcements: Vec<String>,
    aria_labels: HashMap<String, String>,
}

impl ChatViewAccessibility {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a screen-reader announcement.
    pub fn announce(&mut self, text: &str) {
        if !text.is_empty() {
            self.announcements.push(text.to_string());
        }
    }

    /// All queued announcements.
    pub fn announcements(&self) -> &[String] {
        &self.announcements
    }

    /// Associate an ARIA label with a UI element.
    pub fn set_aria_label(&mut self, element: &str, label: &str) {
        self.aria_labels
            .insert(element.to_string(), label.to_string());
    }

    /// Retrieve the ARIA label for a UI element.
    pub fn get_aria_label(&self, element: &str) -> Option<&str> {
        self.aria_labels.get(element).map(|s| s.as_str())
    }

    /// All registered element-label pairs.
    pub fn labels(&self) -> Vec<(&str, &str)> {
        self.aria_labels
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    /// Discard all pending announcements.
    pub fn clear_announcements(&mut self) {
        self.announcements.clear();
    }
}

impl fmt::Display for ChatViewAccessibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Accessibility(announcements={}, labels={})",
            self.announcements.len(),
            self.aria_labels.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// ChatViewSearch
// ---------------------------------------------------------------------------

/// Result of searching within chat messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatViewSearchResult {
    pub message_id: usize,
    pub role: String,
    pub snippet: String,
    pub match_start: usize,
}

/// Indexed message stored for searching.
#[derive(Debug, Clone)]
struct SearchableMessage {
    id: usize,
    role: String,
    content: String,
}

/// Search within chat messages.
#[derive(Debug, Clone, Default)]
pub struct ChatViewSearch {
    messages: Vec<SearchableMessage>,
}

impl ChatViewSearch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Index a message for later searching.
    pub fn add_message(&mut self, id: usize, role: &str, content: &str) {
        self.messages.push(SearchableMessage {
            id,
            role: role.to_string(),
            content: content.to_string(),
        });
    }

    /// Find all messages containing `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<ChatViewSearchResult> {
        let q = query.to_lowercase();
        let mut results = Vec::new();
        for msg in &self.messages {
            let lower = msg.content.to_lowercase();
            if let Some(pos) = lower.find(&q) {
                let snippet_end = (pos + 60).min(msg.content.len());
                let snippet_start = pos;
                results.push(ChatViewSearchResult {
                    message_id: msg.id,
                    role: msg.role.clone(),
                    snippet: msg.content[snippet_start..snippet_end].to_string(),
                    match_start: pos,
                });
            }
        }
        results
    }

    /// Count total matches across all messages for `query`.
    pub fn search_count(&self, query: &str) -> usize {
        let q = query.to_lowercase();
        self.messages
            .iter()
            .filter(|m| m.content.to_lowercase().contains(&q))
            .count()
    }

    /// Number of indexed messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl fmt::Display for ChatViewSearch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChatViewSearch(messages={})", self.messages.len())
    }
}

// ---------------------------------------------------------------------------
// ChatViewExporter
// ---------------------------------------------------------------------------

/// Entry stored for export.
#[derive(Debug, Clone)]
struct ExportMessage {
    role: String,
    content: String,
    timestamp: String,
}

/// Export chat history to markdown or plain text.
#[derive(Debug, Clone, Default)]
pub struct ChatViewExporter {
    messages: Vec<ExportMessage>,
}

impl ChatViewExporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a message for export.
    pub fn add_message(&mut self, role: &str, content: &str, timestamp: &str) {
        self.messages.push(ExportMessage {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: timestamp.to_string(),
        });
    }

    /// Render the chat as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::from("# Chat Export\n\n");
        for msg in &self.messages {
            out.push_str(&format!("## {} ({})\n\n{}\n\n", msg.role, msg.timestamp, msg.content));
        }
        out
    }

    /// Render the chat as plain text.
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for msg in &self.messages {
            out.push_str(&format!("[{}] {}: {}\n", msg.timestamp, msg.role, msg.content));
        }
        out
    }

    /// Number of messages recorded for export.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl fmt::Display for ChatViewExporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChatViewExporter(messages={})", self.messages.len())
    }
}


// ---------------------------------------------------------------------------
// vsedit-chat-view: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatViewXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ChatViewXConfig {
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

impl std::fmt::Display for ChatViewXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ChatViewXRegistry {
    entries: Vec<ChatViewXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ChatViewXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ChatViewXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ChatViewXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ChatViewXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ChatViewXConfig> {
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

    pub fn active_entries(&self) -> Vec<&ChatViewXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ChatViewXConfig> {
        let mut sorted: Vec<&ChatViewXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ChatViewXConfig> {
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

    pub fn iter(&self) -> ChatViewXIterator<'_> {
        ChatViewXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ChatViewXIterator<'a> {
    inner: std::slice::Iter<'a, ChatViewXConfig>,
}

impl<'a> Iterator for ChatViewXIterator<'a> {
    type Item = &'a ChatViewXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ChatViewXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ChatViewXCache {
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
pub struct ChatViewXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ChatViewXFormatter {
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

    pub fn format_entry(&self, entry: &ChatViewXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ChatViewXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ChatViewXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ChatViewXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ChatViewXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ChatViewXValidator {
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

    pub fn validate(&self, entry: &ChatViewXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &ChatViewXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ChatViewXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// Chat panel message rendering — extended utilities (yl)
// ---------------------------------------------------------------------------

/// Metric accumulator for chat_view operations.
#[derive(Debug, Clone)]
pub struct YlMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YlMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for chat_view.
#[derive(Debug, Clone)]
pub struct YlRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YlRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for chat_view lookups.
#[derive(Debug, Clone)]
pub struct YlLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YlLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for chat_view
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaChatViewRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaChatViewRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaChatViewCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaChatViewCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaChatViewCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 12
// ---------------------------------------------------------------------------

/// Generic object pool `Xc12Pool<T>`.
pub struct Xc12Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc12Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc12PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc12Pool<T> {
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
    pub fn stats(&self) -> Xc12PoolStats {
        Xc12PoolStats {
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

impl<T> Default for Xc12Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc12Scheduler`.
pub struct Xc12Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc12Scheduler {
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

impl Default for Xc12Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_12 hash for the given byte slice.
pub fn xc_12_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_12 convention.
pub fn xc_12_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_91 deepening: state machine + event bus ---

/// States for the Xd91 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd91State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd91State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd91Transition {
    pub from: Xd91State,
    pub to: Xd91State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd91StateMachine {
    current: Xd91State,
    history: Vec<Xd91Transition>,
    step_counter: usize,
}

impl Xd91StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd91State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd91State {
        self.current
    }

    pub fn history(&self) -> &[Xd91Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd91State) -> Result<Xd91State, String> {
        let allowed = match (self.current, target) {
            (Xd91State::Idle, Xd91State::Running) => true,
            (Xd91State::Running, Xd91State::Paused) => true,
            (Xd91State::Running, Xd91State::Done) => true,
            (Xd91State::Paused, Xd91State::Running) => true,
            (Xd91State::Paused, Xd91State::Done) => true,
            (Xd91State::Done, Xd91State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_91: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd91Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd91SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd91State> {
        let prefix = "Xd91SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd91State::Idle),
            "Running" => Some(Xd91State::Running),
            "Paused" => Some(Xd91State::Paused),
            "Done" => Some(Xd91State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd91State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd91 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd91Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd91Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd91HandlerFn = Box<dyn Fn(&Xd91Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd91EventBus {
    handlers: Vec<(usize, Option<String>, Xd91HandlerFn)>,
    next_id: usize,
    published: Vec<Xd91Event>,
}

impl Xd91EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd91Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd91Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd91Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd91Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_8: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg8Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg8Graph {
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

impl Default for Xg8Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_8: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg8Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg8Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg8Heap<T>) {
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

impl<T: Ord> Default for Xg8Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 11).
pub struct Xh11SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh11SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 53 as u64,
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

/// A compact bit set supporting boolean operations (variant 11).
pub struct Xh11BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh11BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 11).
pub struct Xi11Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi11Deque<T> {
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
pub struct Xi11Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi11Interval {
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

/// A simple interval tree (variant 11).
pub struct Xi11IntervalTree {
    xi_intervals: Vec<Xi11Interval>,
}

impl Xi11IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi11Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi11Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi11Interval) -> Vec<&Xi11Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi11Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi11Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi11Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi11Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi11Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi11Interval> = Vec::new();
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

    // -- ChatMessageRenderer tests ------------------------------------------

    #[test]
    fn renderer_basic() {
        let renderer = ChatMessageRenderer::new();
        let msg = ChatMessage {
            id: 0,
            role: ChatRole::User,
            content: "Hello world".into(),
            timestamp: 1000,
            status: ChatMessageStatus::Complete,
        };
        let out = renderer.render_message(&msg);
        assert_eq!(out, "[1000] [user] Hello world");
    }

    #[test]
    fn renderer_no_timestamps() {
        let renderer = ChatMessageRenderer::new().with_timestamps(false);
        let msg = ChatMessage {
            id: 1,
            role: ChatRole::Assistant,
            content: "Hi there".into(),
            timestamp: 2000,
            status: ChatMessageStatus::Complete,
        };
        let out = renderer.render_message(&msg);
        assert_eq!(out, "[assistant] Hi there");
    }

    #[test]
    fn renderer_word_wrap() {
        let wrapped = ChatMessageRenderer::word_wrap("hello world foo bar baz", 11);
        // "hello world" is 11 chars, fits; "foo bar baz" on next line
        assert_eq!(wrapped, "hello world\nfoo bar baz");
    }

    #[test]
    fn renderer_session() {
        let renderer = ChatMessageRenderer::new()
            .with_timestamps(false);
        let mut session = ChatSession::new("s-render");
        session.add_message(ChatRole::User, "ping", 10);
        session.add_message(ChatRole::Assistant, "pong", 20);
        let out = renderer.render_session(&session);
        assert_eq!(out, "[user] ping\n[assistant] pong");
    }

    // -- ChatCodeBlock tests ------------------------------------------------

    #[test]
    fn code_block_extraction() {
        let content = "Some text\n```rust\nfn main() {}\n```\nMore text";
        let blocks = ChatCodeBlock::extract_code_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, "fn main() {}");
    }

    #[test]
    fn code_block_with_language() {
        let content = "```python\nprint('hi')\nprint('bye')\n```";
        let blocks = ChatCodeBlock::extract_code_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language.as_deref(), Some("python"));
        assert_eq!(blocks[0].line_count(), 2);
        assert_eq!(blocks[0].display_label(), "python (2 lines)");
    }

    #[test]
    fn code_block_no_language() {
        let content = "```\nsome code\n```";
        let blocks = ChatCodeBlock::extract_code_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].language.is_none());
        assert_eq!(blocks[0].display_label(), "code (1 lines)");
    }

    // -- chat_message_copy / export tests -----------------------------------

    #[test]
    fn chat_message_copy_with_role() {
        let msg = ChatMessage {
            id: 0,
            role: ChatRole::User,
            content: "test content".into(),
            timestamp: 500,
            status: ChatMessageStatus::Complete,
        };
        assert_eq!(chat_message_copy(&msg, true), "[user] test content");
    }

    #[test]
    fn chat_message_copy_without_role() {
        let msg = ChatMessage {
            id: 0,
            role: ChatRole::Assistant,
            content: "response text".into(),
            timestamp: 600,
            status: ChatMessageStatus::Complete,
        };
        assert_eq!(chat_message_copy(&msg, false), "response text");
    }

    #[test]
    fn session_export_markdown() {
        let mut session = ChatSession::new("md-export");
        session.set_title("Test Chat");
        session.add_message(ChatRole::User, "What is Rust?", 1);
        session.add_message(ChatRole::Assistant, "A systems language.", 2);

        let md = chat_session_export_markdown(&session).unwrap();
        assert!(md.starts_with("# Test Chat\n"));
        assert!(md.contains("## User\n\nWhat is Rust?\n"));
        assert!(md.contains("## Assistant\n\nA systems language.\n"));
    }

    #[test]
    fn session_export_markdown_empty() {
        let session = ChatSession::new("empty");
        let result = chat_session_export_markdown(&session);
        assert_eq!(result, Err(ChatError::EmptySession));
    }

    // -- existing tests below -----------------------------------------------

    #[test]
    fn add_and_retrieve_messages() {
        let mut session = ChatSession::new("s1");
        let id0 = session.add_message(ChatRole::User, "hello", 100);
        let id1 = session.add_message(ChatRole::Assistant, "hi", 101);
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(session.message_count(), 2);
        assert_eq!(session.get_messages()[0].content, "hello");
        assert_eq!(session.get_messages()[1].role, ChatRole::Assistant);
    }

    #[test]
    fn clear_messages() {
        let mut session = ChatSession::new("s2");
        session.add_message(ChatRole::User, "test", 100);
        session.clear();
        assert_eq!(session.message_count(), 0);
        assert!(session.last_message().is_none());
    }

    #[test]
    fn title_and_last_message() {
        let mut session = ChatSession::new("s3");
        assert!(session.title.is_none());
        session.set_title("My Chat");
        assert_eq!(session.title.as_deref(), Some("My Chat"));
        session.add_message(ChatRole::System, "welcome", 50);
        let last = session.last_message().unwrap();
        assert_eq!(last.role, ChatRole::System);
        assert_eq!(last.timestamp, 50);
    }

    #[test]
    fn streaming_message_lifecycle() {
        let mut session = ChatSession::new("s4");
        let id = session.add_streaming_message(ChatRole::Assistant, 200);
        let msg = session.get_message(id).unwrap();
        assert_eq!(msg.status, ChatMessageStatus::Pending);
        assert!(msg.content.is_empty());

        session.update_message_content(id, "partial response");
        let msg = session.get_message(id).unwrap();
        assert_eq!(msg.status, ChatMessageStatus::Streaming);
        assert_eq!(msg.content, "partial response");

        session.complete_message(id);
        let msg = session.get_message(id).unwrap();
        assert_eq!(msg.status, ChatMessageStatus::Complete);
    }

    #[test]
    fn fail_message_sets_error() {
        let mut session = ChatSession::new("s5");
        let id = session.add_streaming_message(ChatRole::Assistant, 300);
        session.fail_message(id, "connection lost");
        let msg = session.get_message(id).unwrap();
        assert_eq!(msg.status, ChatMessageStatus::Error);
        assert_eq!(msg.content, "connection lost");
    }

    #[test]
    fn delete_message_works() {
        let mut session = ChatSession::new("s6");
        let id0 = session.add_message(ChatRole::User, "a", 1);
        let id1 = session.add_message(ChatRole::Assistant, "b", 2);
        assert!(session.delete_message(id0));
        assert_eq!(session.message_count(), 1);
        assert!(session.get_message(id0).is_none());
        assert!(session.get_message(id1).is_some());
        assert!(!session.delete_message(999));
    }

    #[test]
    fn messages_by_role_filters() {
        let mut session = ChatSession::new("s7");
        session.add_message(ChatRole::User, "q1", 1);
        session.add_message(ChatRole::Assistant, "a1", 2);
        session.add_message(ChatRole::User, "q2", 3);
        session.add_message(ChatRole::System, "sys", 4);
        let user_msgs = session.messages_by_role(ChatRole::User);
        assert_eq!(user_msgs.len(), 2);
        assert_eq!(user_msgs[0].content, "q1");
        assert_eq!(user_msgs[1].content, "q2");
        assert_eq!(session.messages_by_role(ChatRole::System).len(), 1);
    }

    #[test]
    fn is_empty_and_created_at() {
        let session = ChatSession::new("s8");
        assert!(session.is_empty());
        assert_eq!(session.created_at, 0);
        let mut session = session;
        session.add_message(ChatRole::User, "hi", 1);
        assert!(!session.is_empty());
    }

    #[test]
    fn add_message_has_complete_status() {
        let mut session = ChatSession::new("s9");
        let id = session.add_message(ChatRole::User, "test", 100);
        let msg = session.get_message(id).unwrap();
        assert_eq!(msg.status, ChatMessageStatus::Complete);
    }

    #[test]
    fn try_new_rejects_empty_id() {
        let result = ChatSession::try_new("");
        assert_eq!(result.unwrap_err(), ChatError::InvalidSessionId);
    }

    #[test]
    fn try_new_accepts_valid_id() {
        let session = ChatSession::try_new("valid-id").unwrap();
        assert_eq!(session.id, "valid-id");
    }

    #[test]
    fn try_add_message_rejects_oversized_content() {
        let mut session = ChatSession::new("s10");
        let long = "x".repeat(MAX_CONTENT_LENGTH + 1);
        let err = session.try_add_message(ChatRole::User, long, 1).unwrap_err();
        assert_eq!(
            err,
            ChatError::ContentTooLong {
                max: MAX_CONTENT_LENGTH,
                actual: MAX_CONTENT_LENGTH + 1,
            }
        );
    }

    #[test]
    fn try_update_missing_message_returns_error() {
        let mut session = ChatSession::new("s11");
        let err = session.try_update_message_content(42, "data").unwrap_err();
        assert_eq!(err, ChatError::MessageNotFound(42));
    }

    #[test]
    fn try_complete_missing_message_returns_error() {
        let mut session = ChatSession::new("s12");
        let err = session.try_complete_message(99).unwrap_err();
        assert_eq!(err, ChatError::MessageNotFound(99));
    }

    #[test]
    fn export_transcript_empty_session() {
        let session = ChatSession::new("s13");
        assert_eq!(session.export_transcript().unwrap_err(), ChatError::EmptySession);
    }

    #[test]
    fn export_transcript_with_messages() {
        let mut session = ChatSession::new("s14");
        session.set_title("Demo");
        session.add_message(ChatRole::User, "hello", 1);
        session.add_message(ChatRole::Assistant, "hi there", 2);
        let transcript = session.export_transcript().unwrap();
        assert!(transcript.starts_with("# Demo\n"));
        assert!(transcript.contains("[user] hello\n"));
        assert!(transcript.contains("[assistant] hi there\n"));
    }

    #[test]
    fn total_content_length_and_error_count() {
        let mut session = ChatSession::new("s15");
        session.add_message(ChatRole::User, "abc", 1);
        session.add_message(ChatRole::User, "de", 2);
        assert_eq!(session.total_content_length(), 5);
        assert_eq!(session.error_count(), 0);
        let id = session.add_streaming_message(ChatRole::Assistant, 3);
        session.fail_message(id, "fail");
        assert_eq!(session.error_count(), 1);
    }

    #[test]
    fn timestamp_range_computation() {
        let mut session = ChatSession::new("s16");
        assert!(session.timestamp_range().is_none());
        session.add_message(ChatRole::User, "a", 50);
        session.add_message(ChatRole::User, "b", 10);
        session.add_message(ChatRole::User, "c", 100);
        assert_eq!(session.timestamp_range(), Some((10, 100)));
    }

    #[test]
    fn has_pending_work_detection() {
        let mut session = ChatSession::new("s17");
        session.add_message(ChatRole::User, "done", 1);
        assert!(!session.has_pending_work());
        let id = session.add_streaming_message(ChatRole::Assistant, 2);
        assert!(session.has_pending_work());
        session.complete_message(id);
        assert!(!session.has_pending_work());
    }

    #[test]
    fn builder_creates_session_with_system_prompt() {
        let session = ChatSessionBuilder::new("b1")
            .title("Builder Test")
            .created_at(999)
            .system_prompt("You are helpful.")
            .build()
            .unwrap();
        assert_eq!(session.id, "b1");
        assert_eq!(session.title.as_deref(), Some("Builder Test"));
        assert_eq!(session.created_at, 999);
        assert_eq!(session.message_count(), 1);
        let msg = session.get_message(0).unwrap();
        assert_eq!(msg.role, ChatRole::System);
        assert_eq!(msg.content, "You are helpful.");
    }

    #[test]
    fn builder_rejects_empty_id() {
        let result = ChatSessionBuilder::new("").build();
        assert_eq!(result.unwrap_err(), ChatError::InvalidSessionId);
    }

    #[test]
    fn display_impls_format_correctly() {
        assert_eq!(format!("{}", ChatRole::User), "user");
        assert_eq!(format!("{}", ChatMessageStatus::Streaming), "streaming");

        let mut session = ChatSession::new("x");
        session.set_title("T");
        session.add_message(ChatRole::User, "hi", 1);
        let display = format!("{session}");
        assert!(display.contains("x"));
        assert!(display.contains("T"));
        assert!(display.contains("1 messages"));

        let msg = session.get_message(0).unwrap();
        let msg_display = format!("{msg}");
        assert!(msg_display.contains("[complete]"));
        assert!(msg_display.contains("user"));
    }

    #[test]
    fn chat_error_display() {
        let e = ChatError::MessageNotFound(7);
        assert_eq!(format!("{e}"), "message not found: 7");
        let e = ChatError::ContentTooLong { max: 100, actual: 200 };
        assert!(format!("{e}").contains("200"));
    }

    #[test]
    fn clone_and_equality() {
        let mut session = ChatSession::new("eq");
        session.add_message(ChatRole::User, "hi", 1);
        let cloned = session.clone();
        assert_eq!(session, cloned);
        session.add_message(ChatRole::Assistant, "bye", 2);
        assert_ne!(session, cloned);
    }

    #[test]
    fn chat_view_stats_new_defaults() {
        let stats = ChatViewStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn chat_view_stats_record_success() {
        let mut stats = ChatViewStats::new();
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
    fn chat_view_stats_record_failure() {
        let mut stats = ChatViewStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn chat_view_stats_reset() {
        let mut stats = ChatViewStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn chat_view_stats_merge() {
        let mut a = ChatViewStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ChatViewStats::new();
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
    fn chat_view_stats_display() {
        let mut stats = ChatViewStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn chat_view_stats_default() {
        let stats = ChatViewStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn chat_view_validator_accepts_valid_name() {
        let v = ChatViewValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn chat_view_validator_rejects_empty() {
        let v = ChatViewValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn chat_view_validator_rejects_too_long() {
        let v = ChatViewValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn chat_view_validator_forbidden_prefix() {
        let v = ChatViewValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn chat_view_validator_allowed_chars() {
        let v = ChatViewValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn chat_view_validator_range() {
        let v = ChatViewValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn chat_view_sanitize_removes_control() {
        let result = ChatViewValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn chat_view_truncate_short_string() {
        assert_eq!(ChatViewValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn chat_view_truncate_long_string() {
        let result = ChatViewValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn chat_view_is_ascii_printable() {
        assert!(ChatViewValidator::is_ascii_printable("Hello World 123"));
        assert!(!ChatViewValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- new tests --

    fn make_session_with_messages() -> ChatSession {
        let mut s = ChatSession::new("test");
        s.add_message(ChatRole::User, "Hello, how are you?", 100);
        s.add_message(ChatRole::Assistant, "I'm doing well, thank you!", 101);
        s.add_message(ChatRole::User, "Tell me about Rust", 102);
        s.add_message(ChatRole::Assistant, "Rust is a systems programming language.", 103);
        s
    }

    #[test]
    fn search_messages_finds_matches() {
        let s = make_session_with_messages();
        let results = search_messages(&s, "Rust");
        assert_eq!(results.len(), 2); // "Rust" in user msg and assistant reply
        assert_eq!(results[0].snippet, "Rust");
    }

    #[test]
    fn search_messages_case_insensitive() {
        let s = make_session_with_messages();
        let results = search_messages(&s, "rust");
        assert!(!results.is_empty());
    }

    #[test]
    fn search_messages_no_match() {
        let s = make_session_with_messages();
        let results = search_messages(&s, "Python");
        assert!(results.is_empty());
    }

    #[test]
    fn filter_messages_by_role_user() {
        let s = make_session_with_messages();
        let user_msgs = filter_messages_by_role(&s, ChatRole::User);
        assert_eq!(user_msgs.len(), 2);
        assert!(user_msgs.iter().all(|m| m.role == ChatRole::User));
    }

    #[test]
    fn filter_messages_by_status_complete() {
        let s = make_session_with_messages();
        let complete = filter_messages_by_status(&s, ChatMessageStatus::Complete);
        assert_eq!(complete.len(), 4);
    }

    #[test]
    fn extract_threads_pairs_user_and_assistant() {
        let s = make_session_with_messages();
        let threads = extract_threads(&s);
        assert_eq!(threads.len(), 2);
        assert!(threads[0].assistant_reply.is_some());
        assert!(threads[1].assistant_reply.is_some());
        assert_eq!(threads[0].user_message.role, ChatRole::User);
    }

    #[test]
    fn extract_threads_user_without_reply() {
        let mut s = ChatSession::new("test2");
        s.add_message(ChatRole::User, "Hello?", 100);
        let threads = extract_threads(&s);
        assert_eq!(threads.len(), 1);
        assert!(threads[0].assistant_reply.is_none());
    }

    #[test]
    fn compute_message_statistics_basic() {
        let s = make_session_with_messages();
        let stats = compute_message_statistics(&s);
        assert_eq!(stats.total_messages, 4);
        assert_eq!(stats.user_messages, 2);
        assert_eq!(stats.assistant_messages, 2);
        assert_eq!(stats.system_messages, 0);
        assert!(stats.total_words > 0);
        assert!(stats.avg_message_length > 0.0);
    }

    #[test]
    fn compute_message_statistics_empty() {
        let s = ChatSession::new("empty");
        let stats = compute_message_statistics(&s);
        assert_eq!(stats.total_messages, 0);
        assert_eq!(stats.avg_message_length, 0.0);
    }

    #[test]
    fn count_code_blocks_counts_fenced() {
        assert_eq!(count_code_blocks("```rust\nfn main() {}\n```"), 1);
        assert_eq!(count_code_blocks("no code here"), 0);
        assert_eq!(count_code_blocks("```a```\n```b```"), 2);
    }

    #[test]
    fn average_message_length_computes() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "hi", 1);       // 2 chars
        session.add_message(ChatRole::Assistant, "hello there", 2); // 11 chars
        let avg = average_message_length(&session);
        // (2 + 11) / 2 = 6.5
        assert!((avg - 6.5).abs() < 0.01);
    }

    #[test]
    fn average_message_length_empty() {
        let session = ChatSession::new("s1");
        assert_eq!(average_message_length(&session), 0.0);
    }

    #[test]
    fn distinct_role_count_works() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "hi", 1);
        session.add_message(ChatRole::User, "again", 2);
        session.add_message(ChatRole::Assistant, "hello", 3);
        assert_eq!(distinct_role_count(&session), 2);
    }

    #[test]
    fn messages_with_code_finds_code() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "no code here", 1);
        session.add_message(ChatRole::Assistant, "```rust\nfn main() {}\n```", 2);
        let with_code = messages_with_code(&session);
        assert_eq!(with_code.len(), 1);
        assert!(with_code[0].content.contains("fn main"));
    }

    #[test]
    fn longest_message_finds_longest() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "short", 1);
        session.add_message(ChatRole::Assistant, "a much longer message", 2);
        let longest = longest_message(&session).unwrap();
        assert_eq!(longest.content, "a much longer message");
    }

    #[test]
    fn longest_message_empty_session() {
        let session = ChatSession::new("s1");
        assert!(longest_message(&session).is_none());
    }

    #[test]
    fn session_summary_format() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "hi", 1);
        session.add_message(ChatRole::Assistant, "hello", 2);
        let s = session_summary(&session);
        assert!(s.contains("2 messages"));
        assert!(s.contains("1 user"));
        assert!(s.contains("1 assistant"));
    }

    #[test]
    fn code_block_languages_extracts() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::Assistant, "```rust\ncode\n```\n```python\ncode\n```", 1);
        session.add_message(ChatRole::Assistant, "```rust\nmore\n```", 2);
        let langs = code_block_languages(&session);
        assert!(langs.contains(&"rust".to_string()));
        assert!(langs.contains(&"python".to_string()));
        assert_eq!(langs.len(), 2); // rust only counted once
    }

    #[test]
    fn count_by_status_tallies() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "hi", 1);
        session.add_message(ChatRole::Assistant, "hello", 2);
        let counts = count_by_status(&session);
        let complete = counts.iter().find(|(s, _)| *s == ChatMessageStatus::Complete);
        assert_eq!(complete.unwrap().1, 2);
    }

    #[test]
    fn total_content_length_sums() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "hi", 1);
        session.add_message(ChatRole::Assistant, "hello", 2);
        assert_eq!(total_content_length(&session), 7);
    }

    #[test]
    fn extract_exchanges_pairs() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "q1", 1);
        session.add_message(ChatRole::Assistant, "a1", 2);
        session.add_message(ChatRole::User, "q2", 3);
        session.add_message(ChatRole::Assistant, "a2", 4);
        let exchanges = extract_exchanges(&session);
        assert_eq!(exchanges.len(), 2);
        assert_eq!(exchanges[0].user_message.content, "q1");
    }

    #[test]
    fn extract_exchanges_skips_unpaired() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::System, "init", 0);
        session.add_message(ChatRole::User, "q", 1);
        session.add_message(ChatRole::Assistant, "a", 2);
        assert_eq!(extract_exchanges(&session).len(), 1);
    }

    #[test]
    fn response_ratio_computes() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "q", 1);
        session.add_message(ChatRole::Assistant, "a", 2);
        assert!((response_ratio(&session).unwrap() - 1.0).abs() < f64::EPSILON);
        assert!(response_ratio(&ChatSession::new("s2")).is_none());
    }

    #[test]
    fn avg_response_length_computes() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::Assistant, "short", 1);
        session.add_message(ChatRole::Assistant, "a longer response", 2);
        let avg = avg_response_length(&session).unwrap();
        assert!((avg - 11.0).abs() < f64::EPSILON);
    }

    #[test]
    fn find_message_containing_finds() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "How does Rust work?", 1);
        assert!(find_message_containing(&session, "rust").is_some());
        assert!(find_message_containing(&session, "python").is_none());
    }

    #[test]
    fn messages_longer_than_filters() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "hi", 1);
        session.add_message(ChatRole::Assistant, "a much longer response", 2);
        assert_eq!(messages_longer_than(&session, 10).len(), 1);
    }

    #[test]
    fn session_duration_computes() {
        let mut session = ChatSession::new("s1");
        session.add_message(ChatRole::User, "hi", 100);
        session.add_message(ChatRole::Assistant, "hello", 500);
        assert_eq!(session_duration(&session), 400);
        assert_eq!(session_duration(&ChatSession::new("s2")), 0);
    }

    // -- ChatViewScrollManager tests --

    #[test]
    fn scroll_manager_auto_scroll() {
        let mut sm = ChatViewScrollManager::new(10);
        sm.set_total_lines(20);
        assert!(sm.is_at_bottom());
        assert_eq!(sm.offset(), 10);
    }

    #[test]
    fn scroll_manager_scroll_up_disables_auto() {
        let mut sm = ChatViewScrollManager::new(10);
        sm.set_total_lines(30);
        sm.scroll_up(5);
        assert!(!sm.is_auto_scroll());
        assert_eq!(sm.offset(), 15);
    }

    #[test]
    fn scroll_manager_scroll_to_bottom_enables_auto() {
        let mut sm = ChatViewScrollManager::new(10);
        sm.set_total_lines(30);
        sm.scroll_up(5);
        sm.enable_auto_scroll();
        assert!(sm.is_auto_scroll());
        assert!(sm.is_at_bottom());
    }

    // -- ChatInputHistory tests --

    #[test]
    fn input_history_up_down() {
        let mut h = ChatInputHistory::new(100);
        h.push("hello");
        h.push("world");
        assert_eq!(h.up(), Some("world"));
        assert_eq!(h.up(), Some("hello"));
        assert_eq!(h.down(), Some("world"));
    }

    #[test]
    fn input_history_deduplicates() {
        let mut h = ChatInputHistory::new(100);
        h.push("hello");
        h.push("world");
        h.push("hello"); // duplicate
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn input_history_max_entries() {
        let mut h = ChatInputHistory::new(2);
        h.push("a");
        h.push("b");
        h.push("c");
        assert_eq!(h.len(), 2);
        assert_eq!(h.up(), Some("c"));
    }

    #[test]
    fn input_history_empty_ignored() {
        let mut h = ChatInputHistory::new(10);
        h.push("");
        assert!(h.is_empty());
    }

    // -- ChatMentionPicker tests --

    #[test]
    fn mention_picker_search() {
        let mut p = ChatMentionPicker::new();
        p.add_participant("alice", "Alice Smith");
        p.add_participant("bob", "Bob Jones");
        let results = p.search("ali");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "alice");
    }

    #[test]
    fn mention_picker_extract() {
        let mentions = ChatMentionPicker::extract_mentions("Hey @alice and @bob check this");
        assert_eq!(mentions, vec!["alice", "bob"]);
    }

    // -- ChatViewTheme tests ------------------------------------------------

    #[test]
    fn theme_dark_is_dark() {
        let theme = ChatViewTheme::default_dark();
        assert!(theme.is_dark());
        assert_eq!(theme.background_color, "#1e1e1e");
    }

    #[test]
    fn theme_light_is_not_dark() {
        let theme = ChatViewTheme::default_light();
        assert!(!theme.is_dark());
    }

    #[test]
    fn theme_builder_pattern() {
        let theme = ChatViewTheme::default_dark()
            .with_user_color("#ff0000")
            .with_assistant_color("#00ff00");
        assert_eq!(theme.user_message_color, "#ff0000");
        assert_eq!(theme.assistant_message_color, "#00ff00");
    }

    #[test]
    fn theme_default_is_dark() {
        let theme = ChatViewTheme::default();
        assert_eq!(theme, ChatViewTheme::default_dark());
    }

    #[test]
    fn theme_display() {
        let theme = ChatViewTheme::default_dark();
        let s = format!("{theme}");
        assert!(s.contains("#1e1e1e"));
    }

    // -- ChatViewAccessibility tests ----------------------------------------

    #[test]
    fn accessibility_announce_and_clear() {
        let mut a11y = ChatViewAccessibility::new();
        a11y.announce("New message received");
        a11y.announce("Typing indicator shown");
        assert_eq!(a11y.announcements().len(), 2);
        a11y.clear_announcements();
        assert_eq!(a11y.announcements().len(), 0);
    }

    #[test]
    fn accessibility_aria_labels() {
        let mut a11y = ChatViewAccessibility::new();
        a11y.set_aria_label("input", "Message input field");
        a11y.set_aria_label("send", "Send message button");
        assert_eq!(a11y.get_aria_label("input"), Some("Message input field"));
        assert_eq!(a11y.get_aria_label("missing"), None);
        assert_eq!(a11y.labels().len(), 2);
    }

    #[test]
    fn accessibility_empty_announce_ignored() {
        let mut a11y = ChatViewAccessibility::new();
        a11y.announce("");
        assert!(a11y.announcements().is_empty());
    }

    // -- ChatViewSearch tests -----------------------------------------------

    #[test]
    fn search_basic() {
        let mut search = ChatViewSearch::new();
        search.add_message(0, "user", "Hello world");
        search.add_message(1, "assistant", "World is great");
        let results = search.search("world");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].message_id, 0);
        assert_eq!(results[0].match_start, 6);
    }

    #[test]
    fn search_count_and_message_count() {
        let mut search = ChatViewSearch::new();
        search.add_message(0, "user", "Rust is fast");
        search.add_message(1, "assistant", "Rust is safe");
        search.add_message(2, "user", "Python is fun");
        assert_eq!(search.search_count("rust"), 2);
        assert_eq!(search.message_count(), 3);
    }

    #[test]
    fn search_no_match() {
        let search = ChatViewSearch::new();
        assert!(search.search("anything").is_empty());
    }

    // -- ChatViewExporter tests ---------------------------------------------

    #[test]
    fn exporter_markdown() {
        let mut exp = ChatViewExporter::new();
        exp.add_message("user", "Hi there", "2024-01-01T00:00:00Z");
        exp.add_message("assistant", "Hello!", "2024-01-01T00:00:01Z");
        let md = exp.to_markdown();
        assert!(md.starts_with("# Chat Export"));
        assert!(md.contains("## user"));
        assert!(md.contains("Hi there"));
        assert_eq!(exp.message_count(), 2);
    }

    #[test]
    fn exporter_plain_text() {
        let mut exp = ChatViewExporter::new();
        exp.add_message("user", "Hello", "10:00");
        let txt = exp.to_plain_text();
        assert!(txt.contains("[10:00] user: Hello"));
    }

    #[test]
    fn mention_picker_empty_search() {
        let p = ChatMentionPicker::new();
        assert_eq!(p.search("anything").len(), 0);
        assert_eq!(p.participant_count(), 0);
    }

    #[test]
    fn chatView_x_config_new() {
        let c = ChatViewXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn chatView_x_config_builder() {
        let c = ChatViewXConfig::new("k")
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
    fn chatView_x_config_display() {
        let c = ChatViewXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn chatView_x_registry_insert_get() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn chatView_x_registry_duplicate() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a")).unwrap();
        assert!(reg.insert(ChatViewXConfig::new("a")).is_err());
    }

    #[test]
    fn chatView_x_registry_remove() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a")).unwrap();
        reg.insert(ChatViewXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn chatView_x_registry_active_entries() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a")).unwrap();
        reg.insert(ChatViewXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn chatView_x_registry_by_weight() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ChatViewXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn chatView_x_registry_tags() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ChatViewXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn chatView_x_registry_total_weight() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ChatViewXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn chatView_x_registry_iterator() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a")).unwrap();
        reg.insert(ChatViewXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn chatView_x_cache_put_get() {
        let mut cache = ChatViewXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn chatView_x_cache_eviction() {
        let mut cache = ChatViewXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn chatView_x_cache_lru_order() {
        let mut cache = ChatViewXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn chatView_x_cache_most_least_recent() {
        let mut cache = ChatViewXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn chatView_x_formatter_entry() {
        let e = ChatViewXConfig::new("k").with_value("v");
        let fmt = ChatViewXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn chatView_x_formatter_summary() {
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ChatViewXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn chatView_x_validator_valid() {
        let v = ChatViewXValidator::new();
        let c = ChatViewXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn chatView_x_validator_empty_key() {
        let v = ChatViewXValidator::new();
        let c = ChatViewXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn chatView_x_validator_require_value() {
        let v = ChatViewXValidator::new().require_value(true);
        let c = ChatViewXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn chatView_x_validator_allowed_tags() {
        let v = ChatViewXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ChatViewXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn chatView_x_validator_validate_all() {
        let v = ChatViewXValidator::new();
        let mut reg = ChatViewXRegistry::new();
        reg.insert(ChatViewXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn yl_metrics_empty() {
        let m = YlMetrics::new("chat_view");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yl_metrics_record_and_mean() {
        let mut m = YlMetrics::new("chat_view");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yl_metrics_min_max() {
        let mut m = YlMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yl_metrics_variance_and_std() {
        let mut m = YlMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yl_metrics_percentile() {
        let mut m = YlMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yl_metrics_merge() {
        let mut a = YlMetrics::new("a");
        a.record(1.0);
        let mut b = YlMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yl_metrics_reset() {
        let mut m = YlMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yl_rate_window_empty() {
        let rw = YlRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yl_rate_window_tick_and_rate() {
        let mut rw = YlRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yl_lru_cache_basic() {
        let mut c = YlLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yl_lru_cache_contains_and_keys() {
        let mut c = YlLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yl_lru_cache_remove() {
        let mut c = YlLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yl_metrics_sum() {
        let mut m = YlMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yl_metrics_label() {
        let m = YlMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yl_lru_cache_clear() {
        let mut c = YlLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for chat_view
    #[test]
    fn xa_chat_view_ring_new() {
        let rb = super::XaChatViewRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_chat_view_ring_push_len() {
        let mut rb = super::XaChatViewRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_chat_view_ring_wrap() {
        let mut rb = super::XaChatViewRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_chat_view_ring_mean_empty() {
        let rb = super::XaChatViewRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_chat_view_ring_mean_values() {
        let mut rb = super::XaChatViewRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_chat_view_ring_min_max() {
        let mut rb = super::XaChatViewRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_chat_view_ring_iter() {
        let mut rb = super::XaChatViewRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_chat_view_counter_new() {
        let c = super::XaChatViewCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_chat_view_counter_inc() {
        let mut c = super::XaChatViewCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_chat_view_counter_inc_by() {
        let mut c = super::XaChatViewCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_chat_view_counter_reset() {
        let mut c = super::XaChatViewCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_chat_view_counter_clear() {
        let mut c = super::XaChatViewCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_chat_view_counter_default() {
        let c = super::XaChatViewCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 12 ----

    #[test]
    fn xc_12_pool_new_empty() {
        let pool: super::Xc12Pool<i32> = super::Xc12Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_12_pool_release_acquire() {
        let mut pool = super::Xc12Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_12_pool_acquire_empty() {
        let mut pool: super::Xc12Pool<i32> = super::Xc12Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_12_pool_full() {
        let mut pool = super::Xc12Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_12_pool_drain() {
        let mut pool = super::Xc12Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_12_pool_stats() {
        let mut pool = super::Xc12Pool::new(8);
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
    fn xc_12_pool_clear() {
        let mut pool = super::Xc12Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_12_pool_shrink() {
        let mut pool = super::Xc12Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_12_pool_default() {
        let pool: super::Xc12Pool<String> = super::Xc12Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_12_pool_extend() {
        let mut pool = super::Xc12Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_12_pool_retain() {
        let mut pool = super::Xc12Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_12_scheduler_round_robin() {
        let mut sched = super::Xc12Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_12_scheduler_empty() {
        let mut sched = super::Xc12Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_12_scheduler_reset() {
        let mut sched = super::Xc12Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_12_scheduler_add_remove() {
        let mut sched = super::Xc12Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_12_scheduler_targets() {
        let sched = super::Xc12Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_12_hash_empty() {
        assert_eq!(super::xc_12_hash(b""), 5381);
    }

    #[test]
    fn xc_12_hash_data() {
        let h = super::xc_12_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_12_hash(b"hello"), h);
    }

    #[test]
    fn xc_12_reverse_str() {
        assert_eq!(super::xc_12_reverse("abc"), "cba");
        assert_eq!(super::xc_12_reverse(""), "");
    }


    // --- xd_91 deepening tests ---

    #[test]
    fn xd_91_sm_initial_state() {
        let sm = Xd91StateMachine::new();
        assert_eq!(sm.current_state(), Xd91State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_91_sm_valid_idle_to_running() {
        let mut sm = Xd91StateMachine::new();
        assert!(sm.transition(Xd91State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd91State::Running);
    }

    #[test]
    fn xd_91_sm_valid_running_to_paused() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        assert!(sm.transition(Xd91State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd91State::Paused);
    }

    #[test]
    fn xd_91_sm_valid_running_to_done() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        assert!(sm.transition(Xd91State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd91State::Done);
    }

    #[test]
    fn xd_91_sm_valid_paused_to_running() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        sm.transition(Xd91State::Paused).unwrap();
        assert!(sm.transition(Xd91State::Running).is_ok());
    }

    #[test]
    fn xd_91_sm_valid_done_to_idle() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        sm.transition(Xd91State::Done).unwrap();
        assert!(sm.transition(Xd91State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd91State::Idle);
    }

    #[test]
    fn xd_91_sm_invalid_idle_to_done() {
        let mut sm = Xd91StateMachine::new();
        assert!(sm.transition(Xd91State::Done).is_err());
    }

    #[test]
    fn xd_91_sm_invalid_idle_to_paused() {
        let mut sm = Xd91StateMachine::new();
        assert!(sm.transition(Xd91State::Paused).is_err());
    }

    #[test]
    fn xd_91_sm_history_tracking() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        sm.transition(Xd91State::Paused).unwrap();
        sm.transition(Xd91State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd91State::Idle);
        assert_eq!(sm.history()[0].to, Xd91State::Running);
        assert_eq!(sm.history()[1].from, Xd91State::Running);
        assert_eq!(sm.history()[2].to, Xd91State::Done);
    }

    #[test]
    fn xd_91_sm_serialize_deserialize() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd91StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd91State::Running));
    }

    #[test]
    fn xd_91_sm_deserialize_invalid() {
        assert_eq!(Xd91StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_91_sm_reset() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd91State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_91_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd91EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd91Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_91_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd91EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd91Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd91Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_91_bus_unsubscribe() {
        let mut bus = Xd91EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_91_event_kind_and_payload() {
        let e = Xd91Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd91Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_91_bus_clear_history() {
        let mut bus = Xd91EventBus::new();
        bus.publish(Xd91Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_91_sm_step_counter_increments() {
        let mut sm = Xd91StateMachine::new();
        sm.transition(Xd91State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd91State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_8 graph tests ------------------------------------------------

    #[test]
    fn xg_8_graph_empty() {
        let g = super::Xg8Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_8_graph_add_node() {
        let mut g = super::Xg8Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_8_graph_add_edge() {
        let mut g = super::Xg8Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_8_graph_neighbors() {
        let mut g = super::Xg8Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_8_graph_has_path() {
        let mut g = super::Xg8Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_8_graph_self_path() {
        let g = super::Xg8Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_8_graph_topo_sort() {
        let mut g = super::Xg8Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_8_graph_cycle_detect_false() {
        let mut g = super::Xg8Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_8_graph_cycle_detect_true() {
        let mut g = super::Xg8Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_8 heap tests -------------------------------------------------

    #[test]
    fn xg_8_heap_empty() {
        let h: super::Xg8Heap<i32> = super::Xg8Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_8_heap_push_pop() {
        let mut h = super::Xg8Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_8_heap_peek() {
        let mut h = super::Xg8Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_8_heap_drain_sorted() {
        let mut h = super::Xg8Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_8_heap_merge() {
        let mut a = super::Xg8Heap::new();
        let mut b = super::Xg8Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_8_heap_default() {
        let h: super::Xg8Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_8_graph_default() {
        let g: super::Xg8Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh11_skip_insert_contains() {
        let mut sl = super::Xh11SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh11_skip_remove() {
        let mut sl = super::Xh11SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh11_skip_len() {
        let mut sl = super::Xh11SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh11_skip_range_query() {
        let mut sl = super::Xh11SkipList::xh_new(4);
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
    fn xh11_skip_floor_ceiling() {
        let mut sl = super::Xh11SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh11_skip_rank() {
        let mut sl = super::Xh11SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh11_skip_empty() {
        let sl = super::Xh11SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh11_skip_duplicates() {
        let mut sl = super::Xh11SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh11_bitset_set_test() {
        let mut bs = super::Xh11BitSet::xh_new(256);
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
    fn xh11_bitset_clear_count() {
        let mut bs = super::Xh11BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh11_bitset_and_or_xor() {
        let mut a = super::Xh11BitSet::xh_new(128);
        let mut b = super::Xh11BitSet::xh_new(128);
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
    fn xh11_bitset_iter_ones() {
        let mut bs = super::Xh11BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh11_bitset_first_last() {
        let mut bs = super::Xh11BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh11_bitset_empty() {
        let bs = super::Xh11BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi11_deque_push_pop_back() {
        let mut dq = super::Xi11Deque::xi_new(4);
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
    fn xi11_deque_push_pop_front() {
        let mut dq = super::Xi11Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi11_deque_mixed_ops() {
        let mut dq = super::Xi11Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi11_deque_get_and_split() {
        let mut dq = super::Xi11Deque::xi_new(8);
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
    fn xi11_deque_rotate_left() {
        let mut dq = super::Xi11Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi11_deque_rotate_right() {
        let mut dq = super::Xi11Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi11_deque_grow() {
        let mut dq = super::Xi11Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi11_deque_empty() {
        let dq = super::Xi11Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi11_interval_tree_insert_query() {
        let mut tree = super::Xi11IntervalTree::xi_new();
        tree.xi_insert(super::Xi11Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi11Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi11Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi11_interval_tree_overlap() {
        let mut tree = super::Xi11IntervalTree::xi_new();
        tree.xi_insert(super::Xi11Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi11Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi11Interval::xi_new(12, 20));
        let q = super::Xi11Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi11_interval_tree_remove() {
        let mut tree = super::Xi11IntervalTree::xi_new();
        tree.xi_insert(super::Xi11Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi11Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi11_interval_tree_gaps() {
        let mut tree = super::Xi11IntervalTree::xi_new();
        tree.xi_insert(super::Xi11Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi11Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi11Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi11Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi11Interval::xi_new(8, 10));
    }

    #[test]
    fn xi11_interval_tree_merge() {
        let mut tree = super::Xi11IntervalTree::xi_new();
        tree.xi_insert(super::Xi11Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi11Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi11Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi11Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi11Interval::xi_new(10, 15));
    }

    #[test]
    fn xi11_interval_tree_all() {
        let mut tree = super::Xi11IntervalTree::xi_new();
        tree.xi_insert(super::Xi11Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi11Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi11_interval_tree_empty() {
        let tree = super::Xi11IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi11_interval_tree_contains_point() {
        let iv = super::Xi11Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
