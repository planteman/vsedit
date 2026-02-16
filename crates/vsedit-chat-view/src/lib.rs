//! Chat view panel.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
