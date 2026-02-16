//! AI chat service.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur within the chat service.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatError {
    /// A participant with this ID already exists.
    DuplicateParticipant(String),
    /// A variable with this name already exists.
    DuplicateVariable(String),
    /// The referenced participant was not found.
    ParticipantNotFound(String),
    /// A required field was empty or invalid.
    ValidationError(String),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::DuplicateParticipant(id) => {
                write!(f, "participant already registered: {id}")
            }
            ChatError::DuplicateVariable(name) => {
                write!(f, "variable already registered: {name}")
            }
            ChatError::ParticipantNotFound(id) => {
                write!(f, "participant not found: {id}")
            }
            ChatError::ValidationError(msg) => {
                write!(f, "validation error: {msg}")
            }
        }
    }
}

impl std::error::Error for ChatError {}

/// A participant that can respond in the chat.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatParticipant {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

impl ChatParticipant {
    /// Returns the display name of the participant.
    pub fn display_name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for ChatParticipant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

/// Builder for constructing a `ChatParticipant` with validation.
#[derive(Debug, Default)]
pub struct ChatParticipantBuilder {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    is_default: bool,
}

impl ChatParticipantBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn is_default(mut self, val: bool) -> Self {
        self.is_default = val;
        self
    }

    /// Build the participant, returning a `ChatError::ValidationError` if
    /// required fields are missing or empty.
    pub fn build(self) -> Result<ChatParticipant, ChatError> {
        let id = self
            .id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ChatError::ValidationError("id is required".into()))?;
        let name = self
            .name
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ChatError::ValidationError("name is required".into()))?;
        Ok(ChatParticipant {
            id,
            name,
            description: self.description,
            is_default: self.is_default,
        })
    }
}

/// A slash command available within a chat participant.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSlashCommand {
    pub name: String,
    pub description: String,
    pub participant_id: String,
}

impl fmt::Display for ChatSlashCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{} ({})", self.name, self.participant_id)
    }
}

/// A variable that can be referenced in chat messages.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatVariable {
    pub name: String,
    pub description: String,
    pub value: String,
}

impl fmt::Display for ChatVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${{{}}}", self.name)
    }
}

/// Service for managing chat participants and commands.
pub struct ChatWorkbenchService {
    participants: Vec<ChatParticipant>,
    commands: Vec<ChatSlashCommand>,
    variables: Vec<ChatVariable>,
}

impl ChatWorkbenchService {
    pub fn new() -> Self {
        Self {
            participants: Vec::new(),
            commands: Vec::new(),
            variables: Vec::new(),
        }
    }

    pub fn register_participant(&mut self, participant: ChatParticipant) {
        self.participants.push(participant);
    }

    pub fn register_command(&mut self, command: ChatSlashCommand) {
        self.commands.push(command);
    }

    pub fn get_participant(&self, id: &str) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.id == id)
    }

    pub fn get_default_participant(&self) -> Option<&ChatParticipant> {
        self.participants.iter().find(|p| p.is_default)
    }

    pub fn get_commands_for(&self, participant_id: &str) -> Vec<&ChatSlashCommand> {
        self.commands
            .iter()
            .filter(|c| c.participant_id == participant_id)
            .collect()
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    pub fn register_variable(&mut self, variable: ChatVariable) {
        self.variables.push(variable);
    }

    pub fn get_variable(&self, name: &str) -> Option<&ChatVariable> {
        self.variables.iter().find(|v| v.name == name)
    }

    pub fn get_all_variables(&self) -> &[ChatVariable] {
        &self.variables
    }

    pub fn unregister_participant(&mut self, id: &str) -> bool {
        let before = self.participants.len();
        self.participants.retain(|p| p.id != id);
        self.participants.len() < before
    }

    pub fn unregister_command(&mut self, name: &str, participant_id: &str) -> bool {
        let before = self.commands.len();
        self.commands
            .retain(|c| !(c.name == name && c.participant_id == participant_id));
        self.commands.len() < before
    }

    pub fn get_all_commands(&self) -> &[ChatSlashCommand] {
        &self.commands
    }

    pub fn find_command(&self, name: &str) -> Option<&ChatSlashCommand> {
        self.commands.iter().find(|c| c.name == name)
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Register a participant, rejecting duplicates.
    pub fn try_register_participant(
        &mut self,
        participant: ChatParticipant,
    ) -> Result<(), ChatError> {
        if self.participants.iter().any(|p| p.id == participant.id) {
            return Err(ChatError::DuplicateParticipant(participant.id));
        }
        self.participants.push(participant);
        Ok(())
    }

    /// Register a variable, rejecting duplicates.
    pub fn try_register_variable(
        &mut self,
        variable: ChatVariable,
    ) -> Result<(), ChatError> {
        if self.variables.iter().any(|v| v.name == variable.name) {
            return Err(ChatError::DuplicateVariable(variable.name));
        }
        self.variables.push(variable);
        Ok(())
    }

    /// Get all participant IDs as a collected vector.
    pub fn participant_ids(&self) -> Vec<&str> {
        self.participants.iter().map(|p| p.id.as_str()).collect()
    }

    /// Resolve `${variable}` placeholders in a template string using
    /// registered variables. Unknown variables are left as-is.
    pub fn resolve_variables(&self, template: &str) -> String {
        let mut result = template.to_string();
        for var in &self.variables {
            let placeholder = format!("${{{}}}", var.name);
            result = result.replace(&placeholder, &var.value);
        }
        result
    }

    /// Returns true when `name` looks like a valid slash-command reference
    /// (non-empty, ASCII alphanumeric or hyphen, no leading hyphen).
    pub fn is_valid_command_name(name: &str) -> bool {
        !name.is_empty()
            && !name.starts_with('-')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    }

    /// Unregister a variable by name, returning whether it was present.
    pub fn unregister_variable(&mut self, name: &str) -> bool {
        let before = self.variables.len();
        self.variables.retain(|v| v.name != name);
        self.variables.len() < before
    }

    /// Clear every registration (participants, commands, variables).
    pub fn clear(&mut self) {
        self.participants.clear();
        self.commands.clear();
        self.variables.clear();
    }
}

impl Default for ChatWorkbenchService {
    fn default() -> Self {
        Self::new()
    }
}

/// Role of a message sender in a chat conversation.
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::System => write!(f, "system"),
        }
    }
}

/// A single message in a chat conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    /// Timestamp as seconds since an arbitrary epoch (e.g. `UNIX_EPOCH`).
    pub timestamp: u64,
}

/// Aggregated statistics for a chat conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatStats {
    pub total_messages: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    /// Estimated token count (word count × 1.3, rounded down).
    pub total_tokens_estimate: usize,
    /// Average content length in bytes across all messages (0 when empty).
    pub avg_message_length: usize,
}

/// Compute statistics over a slice of chat messages.
pub fn compute_chat_stats(messages: &[ChatMessage]) -> ChatStats {
    let total_messages = messages.len();
    let user_messages = messages.iter().filter(|m| m.role == MessageRole::User).count();
    let assistant_messages = messages
        .iter()
        .filter(|m| m.role == MessageRole::Assistant)
        .count();

    let total_words: usize = messages
        .iter()
        .map(|m| m.content.split_whitespace().count())
        .sum();
    let total_tokens_estimate = (total_words as f64 * 1.3) as usize;

    let total_len: usize = messages.iter().map(|m| m.content.len()).sum();
    let avg_message_length = if total_messages > 0 {
        total_len / total_messages
    } else {
        0
    };

    ChatStats {
        total_messages,
        user_messages,
        assistant_messages,
        total_tokens_estimate,
        avg_message_length,
    }
}

/// Exports a chat history to human-readable text formats.
pub struct ChatExporter<'a> {
    messages: &'a [ChatMessage],
}

impl<'a> ChatExporter<'a> {
    pub fn new(messages: &'a [ChatMessage]) -> Self {
        Self { messages }
    }

    /// Render the conversation as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut buf = String::from("# Chat Export\n\n");
        for msg in self.messages {
            buf.push_str(&format!("**{}**: {}\n\n", msg.role, msg.content));
        }
        buf
    }

    /// Render the conversation as plain text.
    pub fn to_plain_text(&self) -> String {
        let mut buf = String::new();
        for msg in self.messages {
            buf.push_str(&format!("[{}] {}\n", msg.role, msg.content));
        }
        buf
    }
}

/// Filter criteria for selecting a subset of chat messages.
pub struct MessageFilter {
    /// If set, only include messages with this role.
    pub role: Option<MessageRole>,
    /// If set, only include messages whose content contains this substring
    /// (case-insensitive).
    pub keyword: Option<String>,
    /// If set, only include messages at or after this timestamp.
    pub time_start: Option<u64>,
    /// If set, only include messages at or before this timestamp.
    pub time_end: Option<u64>,
}

impl MessageFilter {
    pub fn new() -> Self {
        Self {
            role: None,
            keyword: None,
            time_start: None,
            time_end: None,
        }
    }

    pub fn with_role(mut self, role: MessageRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn with_keyword(mut self, kw: impl Into<String>) -> Self {
        self.keyword = Some(kw.into());
        self
    }

    pub fn with_time_range(mut self, start: u64, end: u64) -> Self {
        self.time_start = Some(start);
        self.time_end = Some(end);
        self
    }

    /// Apply this filter to a slice of messages, returning only those that
    /// match all specified criteria.
    pub fn apply<'a>(&self, messages: &'a [ChatMessage]) -> Vec<&'a ChatMessage> {
        messages
            .iter()
            .filter(|m| {
                if let Some(ref role) = self.role {
                    if m.role != *role {
                        return false;
                    }
                }
                if let Some(ref kw) = self.keyword {
                    let lower_content = m.content.to_ascii_lowercase();
                    let lower_kw = kw.to_ascii_lowercase();
                    if !lower_content.contains(&lower_kw) {
                        return false;
                    }
                }
                if let Some(start) = self.time_start {
                    if m.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = self.time_end {
                    if m.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks a sequence of chat messages forming a conversation.
pub struct ChatConversation {
    messages: Vec<ChatMessage>,
    next_id: u64,
}

impl ChatConversation {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a message to the conversation.
    pub fn add_message(&mut self, role: MessageRole, content: impl Into<String>, timestamp: u64) {
        self.messages.push(ChatMessage {
            role,
            content: content.into(),
            timestamp,
        });
        self.next_id += 1;
    }

    /// Number of messages in the conversation.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Get the most recent message, if any.
    pub fn last_message(&self) -> Option<&ChatMessage> {
        self.messages.last()
    }

    /// Return all messages whose content contains `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&ChatMessage> {
        let q = query.to_ascii_lowercase();
        self.messages
            .iter()
            .filter(|m| m.content.to_ascii_lowercase().contains(&q))
            .collect()
    }

    /// Return all messages as a slice.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }
}

impl Default for ChatConversation {
    fn default() -> Self {
        Self::new()
    }
}

// ── ChatHistory ──

/// Persistent conversation history with querying capabilities.
pub struct ChatHistory {
    messages: Vec<ChatMessage>,
}

impl ChatHistory {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Append a message to the history.
    pub fn add_message(&mut self, role: MessageRole, content: impl Into<String>, timestamp: u64) {
        self.messages.push(ChatMessage {
            role,
            content: content.into(),
            timestamp,
        });
    }

    /// Return all messages as a slice.
    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Remove all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Return messages matching the given role.
    pub fn get_messages_by_role(&self, role: &MessageRole) -> Vec<&ChatMessage> {
        self.messages.iter().filter(|m| m.role == *role).collect()
    }

    /// Total number of stored messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Return the last `n` messages (or fewer if the history is shorter).
    pub fn last_n_messages(&self, n: usize) -> &[ChatMessage] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }

    /// Return messages with `timestamp >= since`.
    pub fn messages_since(&self, since: u64) -> Vec<&ChatMessage> {
        self.messages.iter().filter(|m| m.timestamp >= since).collect()
    }

    /// Convert the history into a `ChatConversation`.
    pub fn to_conversation(&self) -> ChatConversation {
        let mut conv = ChatConversation::new();
        for msg in &self.messages {
            conv.add_message(msg.role.clone(), msg.content.clone(), msg.timestamp);
        }
        conv
    }
}

impl Default for ChatHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ── ChatPromptTemplate ──

/// A template that produces a `ChatMessage` by replacing `{variable}` placeholders.
#[derive(Debug, Clone)]
pub struct ChatPromptTemplate {
    template: String,
    role: MessageRole,
}

impl ChatPromptTemplate {
    pub fn new(template: impl Into<String>, role: MessageRole) -> Self {
        Self {
            template: template.into(),
            role,
        }
    }

    /// Render the template by substituting `{key}` with values from `vars`.
    pub fn render(&self, vars: &HashMap<String, String>) -> ChatMessage {
        let mut result = self.template.clone();
        for (key, value) in vars {
            let placeholder = format!("{{{key}}}");
            result = result.replace(&placeholder, value);
        }
        ChatMessage {
            role: self.role.clone(),
            content: result,
            timestamp: 0,
        }
    }

    /// Return a copy of this template with a different role.
    pub fn with_role(mut self, role: MessageRole) -> Self {
        self.role = role;
        self
    }
}

/// Chain of prompt templates rendered in sequence.
pub struct ChatPromptChain {
    templates: Vec<ChatPromptTemplate>,
}

impl ChatPromptChain {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    /// Append a template to the chain.
    pub fn push(mut self, template: ChatPromptTemplate) -> Self {
        self.templates.push(template);
        self
    }

    /// Render all templates with the provided variables.
    pub fn render(&self, vars: &HashMap<String, String>) -> Vec<ChatMessage> {
        self.templates.iter().map(|t| t.render(vars)).collect()
    }
}

impl Default for ChatPromptChain {
    fn default() -> Self {
        Self::new()
    }
}

// ── ChatTokenCounter ──

/// Approximate token counting utilities using a word-based heuristic.
pub struct ChatTokenCounter;

impl ChatTokenCounter {
    /// Estimate token count for a string (words × 1.3, rounded down).
    pub fn count_tokens(text: &str) -> usize {
        let words = text.split_whitespace().count();
        (words as f64 * 1.3) as usize
    }

    /// Estimate token count for a single message.
    pub fn count_message_tokens(msg: &ChatMessage) -> usize {
        Self::count_tokens(&msg.content)
    }

    /// Estimate total token count across a slice of messages.
    pub fn count_conversation_tokens(msgs: &[ChatMessage]) -> usize {
        msgs.iter().map(|m| Self::count_message_tokens(m)).sum()
    }

    /// Returns `true` if the messages fit within `max_tokens`.
    pub fn fits_in_context(msgs: &[ChatMessage], max_tokens: usize) -> bool {
        Self::count_conversation_tokens(msgs) <= max_tokens
    }

    /// Return the longest prefix of `msgs` whose total tokens ≤ `max_tokens`.
    pub fn truncate_to_fit(msgs: &[ChatMessage], max_tokens: usize) -> Vec<ChatMessage> {
        let mut total: usize = 0;
        let mut result = Vec::new();
        for msg in msgs {
            let t = Self::count_message_tokens(msg);
            if total + t > max_tokens {
                break;
            }
            total += t;
            result.push(msg.clone());
        }
        result
    }
}


// ---------------------------------------------------------------------------
// ChatRole helpers
// ---------------------------------------------------------------------------

impl MessageRole {
    /// Returns all role variants.
    pub fn all() -> &'static [MessageRole] {
        &[MessageRole::User, MessageRole::Assistant, MessageRole::System]
    }

    /// Parse from a string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "user" => Some(Self::User),
            "assistant" | "ai" | "bot" => Some(Self::Assistant),
            "system" => Some(Self::System),
            _ => None,
        }
    }

    /// Returns the role name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        }
    }

    /// Returns an icon character for this role.
    pub fn icon(&self) -> char {
        match self {
            MessageRole::User => '👤',
            MessageRole::Assistant => '🤖',
            MessageRole::System => '⚙',
        }
    }

    /// Returns true if this is a user message.
    pub fn is_user(&self) -> bool {
        matches!(self, MessageRole::User)
    }

    /// Returns true if this is an assistant message.
    pub fn is_assistant(&self) -> bool {
        matches!(self, MessageRole::Assistant)
    }
}

impl Default for MessageRole {
    fn default() -> Self {
        MessageRole::User
    }
}

// ---------------------------------------------------------------------------
// ChatMessage helpers
// ---------------------------------------------------------------------------

impl ChatMessage {
    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            timestamp: 0,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: 0,
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            timestamp: 0,
        }
    }

    /// Returns the word count of the content.
    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    /// Returns a truncated preview of the content.
    pub fn preview(&self, max_len: usize) -> String {
        if self.content.len() <= max_len {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_len.saturating_sub(3)])
        }
    }

    /// Returns the character count.
    pub fn char_count(&self) -> usize {
        self.content.len()
    }
}

impl fmt::Display for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.role.name(), self.preview(80))
    }
}

// ---------------------------------------------------------------------------
// Chat analysis helpers
// ---------------------------------------------------------------------------

/// Summary of a chat conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub total_messages: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub system_messages: usize,
    pub total_words: usize,
}

impl ChatSummary {
    /// Generate a summary from messages.
    pub fn from_messages(messages: &[ChatMessage]) -> Self {
        Self {
            total_messages: messages.len(),
            user_messages: messages.iter().filter(|m| m.role.is_user()).count(),
            assistant_messages: messages.iter().filter(|m| m.role.is_assistant()).count(),
            system_messages: messages.iter().filter(|m| matches!(m.role, MessageRole::System)).count(),
            total_words: messages.iter().map(|m| m.word_count()).sum(),
        }
    }

    /// Returns the average message length in words.
    pub fn avg_words(&self) -> f64 {
        if self.total_messages == 0 {
            0.0
        } else {
            self.total_words as f64 / self.total_messages as f64
        }
    }
}

impl fmt::Display for ChatSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} messages ({} user, {} assistant, {} system), {} words",
            self.total_messages, self.user_messages, self.assistant_messages,
            self.system_messages, self.total_words
        )
    }
}

/// Extract all content from messages with a specific role.
pub fn extract_role_content(messages: &[ChatMessage], role: MessageRole) -> Vec<String> {
    messages.iter()
        .filter(|m| m.role == role)
        .map(|m| m.content.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_participant(id: &str, is_default: bool) -> ChatParticipant {
        ChatParticipant {
            id: id.to_string(),
            name: format!("Participant {id}"),
            description: None,
            is_default,
        }
    }

    fn make_command(name: &str, participant_id: &str) -> ChatSlashCommand {
        ChatSlashCommand {
            name: name.to_string(),
            description: format!("Command {name}"),
            participant_id: participant_id.to_string(),
        }
    }

    #[test]
    fn register_and_query_participants() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_participant(make_participant("workspace", false));
        assert_eq!(svc.participant_count(), 2);
        assert!(svc.get_participant("copilot").is_some());
        assert!(svc.get_participant("missing").is_none());
    }

    #[test]
    fn get_default_participant() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_participant(make_participant("workspace", false));
        let default = svc.get_default_participant().unwrap();
        assert_eq!(default.id, "copilot");
    }

    #[test]
    fn commands_for_participant() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_command(make_command("explain", "copilot"));
        svc.register_command(make_command("fix", "copilot"));
        svc.register_command(make_command("test", "workspace"));
        assert_eq!(svc.get_commands_for("copilot").len(), 2);
        assert_eq!(svc.get_commands_for("workspace").len(), 1);
    }

    fn make_variable(name: &str, value: &str) -> ChatVariable {
        ChatVariable {
            name: name.to_string(),
            description: format!("Variable {name}"),
            value: value.to_string(),
        }
    }

    #[test]
    fn register_and_query_variables() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_variable(make_variable("file", "main.rs"));
        svc.register_variable(make_variable("selection", "fn main()"));
        assert_eq!(svc.get_all_variables().len(), 2);
        let v = svc.get_variable("file").unwrap();
        assert_eq!(v.value, "main.rs");
        assert!(svc.get_variable("missing").is_none());
    }

    #[test]
    fn unregister_participant_removes_entry() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_participant(make_participant("workspace", false));
        assert!(svc.unregister_participant("copilot"));
        assert_eq!(svc.participant_count(), 1);
        assert!(svc.get_participant("copilot").is_none());
        assert!(!svc.unregister_participant("copilot"));
    }

    #[test]
    fn unregister_command_removes_entry() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_command(make_command("explain", "copilot"));
        svc.register_command(make_command("fix", "copilot"));
        assert!(svc.unregister_command("explain", "copilot"));
        assert_eq!(svc.command_count(), 1);
        assert!(!svc.unregister_command("explain", "copilot"));
    }

    #[test]
    fn find_command_and_get_all() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_command(make_command("explain", "copilot"));
        svc.register_command(make_command("fix", "copilot"));
        assert_eq!(svc.get_all_commands().len(), 2);
        let cmd = svc.find_command("fix").unwrap();
        assert_eq!(cmd.participant_id, "copilot");
        assert!(svc.find_command("missing").is_none());
    }

    #[test]
    fn participant_display_name() {
        let p = make_participant("copilot", true);
        assert_eq!(p.display_name(), "Participant copilot");
    }

    #[test]
    fn participant_display_trait() {
        let p = make_participant("copilot", true);
        assert_eq!(format!("{p}"), "Participant copilot (copilot)");
    }

    #[test]
    fn slash_command_display() {
        let cmd = make_command("explain", "copilot");
        assert_eq!(format!("{cmd}"), "/explain (copilot)");
    }

    #[test]
    fn variable_display() {
        let v = make_variable("file", "main.rs");
        assert_eq!(format!("{v}"), "${file}");
    }

    #[test]
    fn builder_creates_participant() {
        let p = ChatParticipantBuilder::new()
            .id("copilot")
            .name("Copilot")
            .description("AI pair programmer")
            .is_default(true)
            .build()
            .unwrap();
        assert_eq!(p.id, "copilot");
        assert_eq!(p.name, "Copilot");
        assert_eq!(p.description.as_deref(), Some("AI pair programmer"));
        assert!(p.is_default);
    }

    #[test]
    fn builder_rejects_missing_id() {
        let res = ChatParticipantBuilder::new().name("Copilot").build();
        assert_eq!(
            res,
            Err(ChatError::ValidationError("id is required".into()))
        );
    }

    #[test]
    fn builder_rejects_empty_name() {
        let res = ChatParticipantBuilder::new().id("x").name("").build();
        assert_eq!(
            res,
            Err(ChatError::ValidationError("name is required".into()))
        );
    }

    #[test]
    fn try_register_duplicate_participant() {
        let mut svc = ChatWorkbenchService::new();
        svc.try_register_participant(make_participant("copilot", true))
            .unwrap();
        let err = svc
            .try_register_participant(make_participant("copilot", false))
            .unwrap_err();
        assert_eq!(err, ChatError::DuplicateParticipant("copilot".into()));
    }

    #[test]
    fn try_register_duplicate_variable() {
        let mut svc = ChatWorkbenchService::new();
        svc.try_register_variable(make_variable("file", "a.rs"))
            .unwrap();
        let err = svc
            .try_register_variable(make_variable("file", "b.rs"))
            .unwrap_err();
        assert_eq!(err, ChatError::DuplicateVariable("file".into()));
    }

    #[test]
    fn resolve_variables_in_template() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_variable(make_variable("file", "main.rs"));
        svc.register_variable(make_variable("lang", "Rust"));
        let result = svc.resolve_variables("Open ${file} in ${lang}, ${unknown} stays");
        assert_eq!(result, "Open main.rs in Rust, ${unknown} stays");
    }

    #[test]
    fn is_valid_command_name_checks() {
        assert!(ChatWorkbenchService::is_valid_command_name("explain"));
        assert!(ChatWorkbenchService::is_valid_command_name("my-cmd"));
        assert!(!ChatWorkbenchService::is_valid_command_name(""));
        assert!(!ChatWorkbenchService::is_valid_command_name("-bad"));
        assert!(!ChatWorkbenchService::is_valid_command_name("no spaces"));
    }

    #[test]
    fn participant_ids_returns_all() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("a", false));
        svc.register_participant(make_participant("b", false));
        let ids = svc.participant_ids();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn clear_removes_everything() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_participant(make_participant("copilot", true));
        svc.register_command(make_command("fix", "copilot"));
        svc.register_variable(make_variable("file", "x.rs"));
        svc.clear();
        assert_eq!(svc.participant_count(), 0);
        assert_eq!(svc.command_count(), 0);
        assert_eq!(svc.get_all_variables().len(), 0);
    }

    #[test]
    fn unregister_variable() {
        let mut svc = ChatWorkbenchService::new();
        svc.register_variable(make_variable("file", "a.rs"));
        assert!(svc.unregister_variable("file"));
        assert!(!svc.unregister_variable("file"));
        assert_eq!(svc.get_all_variables().len(), 0);
    }

    #[test]
    fn chat_error_display() {
        let e = ChatError::ParticipantNotFound("abc".into());
        assert_eq!(e.to_string(), "participant not found: abc");
        let e2 = ChatError::ValidationError("bad".into());
        assert_eq!(e2.to_string(), "validation error: bad");
    }

    #[test]
    fn participant_equality() {
        let a = make_participant("copilot", true);
        let b = make_participant("copilot", true);
        assert_eq!(a, b);
        let c = make_participant("other", true);
        assert_ne!(a, c);
    }

    // --- helpers for new feature tests ---

    fn sample_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: MessageRole::User,
                content: "Hello, can you help me?".into(),
                timestamp: 1000,
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: "Sure! What do you need?".into(),
                timestamp: 1001,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Explain the borrow checker in Rust.".into(),
                timestamp: 1002,
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: "The borrow checker ensures memory safety.".into(),
                timestamp: 1003,
            },
            ChatMessage {
                role: MessageRole::System,
                content: "Session started.".into(),
                timestamp: 999,
            },
        ]
    }

    #[test]
    fn compute_stats_counts_roles() {
        let msgs = sample_messages();
        let stats = compute_chat_stats(&msgs);
        assert_eq!(stats.total_messages, 5);
        assert_eq!(stats.user_messages, 2);
        assert_eq!(stats.assistant_messages, 2);
    }

    #[test]
    fn compute_stats_tokens_and_avg_length() {
        let msgs = sample_messages();
        let stats = compute_chat_stats(&msgs);
        // Total words: 5 + 5 + 6 + 6 + 2 = 24, tokens = (24 * 1.3) = 31
        assert_eq!(stats.total_tokens_estimate, 31);
        let total_len: usize = msgs.iter().map(|m| m.content.len()).sum();
        assert_eq!(stats.avg_message_length, total_len / msgs.len());
    }

    #[test]
    fn compute_stats_empty() {
        let stats = compute_chat_stats(&[]);
        assert_eq!(stats.total_messages, 0);
        assert_eq!(stats.total_tokens_estimate, 0);
        assert_eq!(stats.avg_message_length, 0);
    }

    #[test]
    fn exporter_to_markdown() {
        let msgs = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hi".into(),
            timestamp: 0,
        }];
        let md = ChatExporter::new(&msgs).to_markdown();
        assert!(md.starts_with("# Chat Export"));
        assert!(md.contains("**user**: Hi"));
    }

    #[test]
    fn exporter_to_plain_text() {
        let msgs = vec![
            ChatMessage { role: MessageRole::User, content: "Hi".into(), timestamp: 0 },
            ChatMessage { role: MessageRole::Assistant, content: "Hello".into(), timestamp: 1 },
        ];
        let txt = ChatExporter::new(&msgs).to_plain_text();
        assert!(txt.contains("[user] Hi"));
        assert!(txt.contains("[assistant] Hello"));
    }

    #[test]
    fn filter_by_role() {
        let msgs = sample_messages();
        let filtered = MessageFilter::new()
            .with_role(MessageRole::User)
            .apply(&msgs);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|m| m.role == MessageRole::User));
    }

    #[test]
    fn filter_by_keyword_case_insensitive() {
        let msgs = sample_messages();
        let filtered = MessageFilter::new()
            .with_keyword("MEMORY SAFETY")
            .apply(&msgs);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].content.contains("memory safety"));
    }

    #[test]
    fn filter_by_time_range() {
        let msgs = sample_messages();
        let filtered = MessageFilter::new()
            .with_time_range(1001, 1002)
            .apply(&msgs);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|m| m.timestamp >= 1001 && m.timestamp <= 1002));
    }

    #[test]
    fn filter_combined_role_and_keyword() {
        let msgs = sample_messages();
        let filtered = MessageFilter::new()
            .with_role(MessageRole::Assistant)
            .with_keyword("memory")
            .apply(&msgs);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].content.contains("memory safety"));
    }

    #[test]
    fn message_role_display() {
        assert_eq!(format!("{}", MessageRole::User), "user");
        assert_eq!(format!("{}", MessageRole::Assistant), "assistant");
        assert_eq!(format!("{}", MessageRole::System), "system");
    }

    // ── ChatConversation tests ──

    #[test]
    fn conversation_add_and_count() {
        let mut conv = ChatConversation::new();
        assert_eq!(conv.message_count(), 0);
        conv.add_message(MessageRole::User, "Hello", 100);
        conv.add_message(MessageRole::Assistant, "Hi there", 101);
        assert_eq!(conv.message_count(), 2);
    }

    #[test]
    fn conversation_last_message() {
        let mut conv = ChatConversation::new();
        assert!(conv.last_message().is_none());
        conv.add_message(MessageRole::User, "first", 1);
        conv.add_message(MessageRole::Assistant, "second", 2);
        assert_eq!(conv.last_message().unwrap().content, "second");
    }

    #[test]
    fn conversation_search() {
        let mut conv = ChatConversation::new();
        conv.add_message(MessageRole::User, "Explain borrow checker", 1);
        conv.add_message(MessageRole::Assistant, "The borrow checker ensures safety", 2);
        conv.add_message(MessageRole::User, "What about lifetimes?", 3);
        let results = conv.search("borrow");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn conversation_search_case_insensitive() {
        let mut conv = ChatConversation::new();
        conv.add_message(MessageRole::User, "HELLO World", 1);
        let results = conv.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn conversation_messages_slice() {
        let mut conv = ChatConversation::new();
        conv.add_message(MessageRole::System, "init", 0);
        assert_eq!(conv.messages().len(), 1);
        assert_eq!(conv.messages()[0].role, MessageRole::System);
    }

    // ── ChatHistory tests ──

    #[test]
    fn history_add_and_get() {
        let mut h = ChatHistory::new();
        h.add_message(MessageRole::User, "hello", 1);
        h.add_message(MessageRole::Assistant, "hi", 2);
        assert_eq!(h.message_count(), 2);
        assert_eq!(h.get_messages().len(), 2);
    }

    #[test]
    fn history_clear() {
        let mut h = ChatHistory::new();
        h.add_message(MessageRole::User, "msg", 1);
        h.clear();
        assert_eq!(h.message_count(), 0);
    }

    #[test]
    fn history_get_by_role() {
        let mut h = ChatHistory::new();
        h.add_message(MessageRole::User, "a", 1);
        h.add_message(MessageRole::Assistant, "b", 2);
        h.add_message(MessageRole::User, "c", 3);
        assert_eq!(h.get_messages_by_role(&MessageRole::User).len(), 2);
        assert_eq!(h.get_messages_by_role(&MessageRole::Assistant).len(), 1);
    }

    #[test]
    fn history_last_n_and_since() {
        let mut h = ChatHistory::new();
        for i in 0..5 {
            h.add_message(MessageRole::User, format!("m{i}"), i * 10);
        }
        assert_eq!(h.last_n_messages(3).len(), 3);
        assert_eq!(h.last_n_messages(100).len(), 5);
        assert_eq!(h.messages_since(20).len(), 3);
    }

    #[test]
    fn history_to_conversation() {
        let mut h = ChatHistory::new();
        h.add_message(MessageRole::System, "sys", 0);
        h.add_message(MessageRole::User, "hi", 1);
        let conv = h.to_conversation();
        assert_eq!(conv.message_count(), 2);
    }

    // ── ChatPromptTemplate tests ──

    #[test]
    fn template_render_basic() {
        let tpl = ChatPromptTemplate::new("Hello {name}, welcome to {place}!", MessageRole::System);
        let mut vars = HashMap::new();
        vars.insert("name".into(), "Alice".into());
        vars.insert("place".into(), "Wonderland".into());
        let msg = tpl.render(&vars);
        assert_eq!(msg.content, "Hello Alice, welcome to Wonderland!");
        assert_eq!(msg.role, MessageRole::System);
    }

    #[test]
    fn template_with_role() {
        let tpl = ChatPromptTemplate::new("test", MessageRole::User).with_role(MessageRole::Assistant);
        let msg = tpl.render(&HashMap::new());
        assert_eq!(msg.role, MessageRole::Assistant);
    }

    #[test]
    fn prompt_chain_render() {
        let chain = ChatPromptChain::new()
            .push(ChatPromptTemplate::new("You are {role}", MessageRole::System))
            .push(ChatPromptTemplate::new("Hello {role}", MessageRole::User));
        let mut vars = HashMap::new();
        vars.insert("role".into(), "an assistant".into());
        let msgs = chain.render(&vars);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "You are an assistant");
        assert_eq!(msgs[1].role, MessageRole::User);
    }

    // ── ChatTokenCounter tests ──

    #[test]
    fn token_count_basic() {
        // 4 words × 1.3 = 5.2 → 5
        assert_eq!(ChatTokenCounter::count_tokens("one two three four"), 5);
        assert_eq!(ChatTokenCounter::count_tokens(""), 0);
    }

    #[test]
    fn token_count_message_and_conversation() {
        let m1 = ChatMessage { role: MessageRole::User, content: "one two".into(), timestamp: 0 };
        let m2 = ChatMessage { role: MessageRole::Assistant, content: "three four five".into(), timestamp: 1 };
        assert_eq!(ChatTokenCounter::count_message_tokens(&m1), 2); // 2 × 1.3 = 2.6 → 2
        let total = ChatTokenCounter::count_conversation_tokens(&[m1.clone(), m2.clone()]);
        assert_eq!(total, 2 + 3); // 2 + (3×1.3=3.9→3)
    }

    #[test]
    fn token_fits_and_truncate() {
        let msgs: Vec<ChatMessage> = (0..5)
            .map(|i| ChatMessage {
                role: MessageRole::User,
                content: format!("word{i} extra"),
                timestamp: i,
            })
            .collect();
        // each message: 2 words × 1.3 = 2.6 → 2 tokens
        assert!(ChatTokenCounter::fits_in_context(&msgs, 100));
        assert!(!ChatTokenCounter::fits_in_context(&msgs, 3));
        let truncated = ChatTokenCounter::truncate_to_fit(&msgs, 5);
        assert!(truncated.len() < msgs.len());
        assert!(ChatTokenCounter::fits_in_context(&truncated, 5));
    }

    #[test]
    fn test_chat_role_all() {
        assert_eq!(MessageRole::all().len(), 3);
    }

    #[test]
    fn test_chat_role_from_name() {
        assert_eq!(MessageRole::from_name("user"), Some(MessageRole::User));
        assert_eq!(MessageRole::from_name("AI"), Some(MessageRole::Assistant));
        assert_eq!(MessageRole::from_name("bogus"), None);
    }

    #[test]
    fn test_chat_role_name_and_icon() {
        assert_eq!(MessageRole::User.name(), "user");
        assert_eq!(MessageRole::Assistant.icon(), '🤖');
    }

    #[test]
    fn test_chat_role_is_checks() {
        assert!(MessageRole::User.is_user());
        assert!(!MessageRole::User.is_assistant());
        assert!(MessageRole::Assistant.is_assistant());
        assert_eq!(MessageRole::default(), MessageRole::User);
    }

    #[test]
    fn test_chat_message_constructors() {
        let u = ChatMessage::user("Hello");
        assert_eq!(u.role, MessageRole::User);
        assert_eq!(u.content, "Hello");
        let a = ChatMessage::assistant("Hi there");
        assert_eq!(a.role, MessageRole::Assistant);
        let s = ChatMessage::system("You are helpful");
        assert_eq!(s.role, MessageRole::System);
    }

    #[test]
    fn test_chat_message_word_count() {
        let m = ChatMessage::user("hello world foo bar");
        assert_eq!(m.word_count(), 4);
        assert_eq!(m.char_count(), 19);
    }

    #[test]
    fn test_chat_message_preview() {
        let m = ChatMessage::user("a".repeat(100));
        let p = m.preview(20);
        assert!(p.len() <= 20);
        assert!(p.ends_with("..."));
    }

    #[test]
    fn test_chat_message_display() {
        let m = ChatMessage::user("Hello world");
        let s = format!("{m}");
        assert!(s.contains("user"));
        assert!(s.contains("Hello"));
    }

    #[test]
    fn test_chat_summary() {
        let messages = vec![
            ChatMessage::system("Be helpful"),
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi there how are you"),
            ChatMessage::user("Fine thanks"),
        ];
        let summary = ChatSummary::from_messages(&messages);
        assert_eq!(summary.total_messages, 4);
        assert_eq!(summary.user_messages, 2);
        assert_eq!(summary.assistant_messages, 1);
        assert_eq!(summary.system_messages, 1);
        assert!(summary.avg_words() > 0.0);
        assert!(format!("{summary}").contains("4 messages"));
    }

    #[test]
    fn test_extract_role_content() {
        let messages = vec![
            ChatMessage::user("Q1"),
            ChatMessage::assistant("A1"),
            ChatMessage::user("Q2"),
        ];
        let user_content = extract_role_content(&messages, MessageRole::User);
        assert_eq!(user_content, vec!["Q1", "Q2"]);
    }
}
