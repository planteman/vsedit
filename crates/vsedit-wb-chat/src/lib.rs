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

// ---------------------------------------------------------------------------
// Threaded messages (reply chains)
// ---------------------------------------------------------------------------

/// A message that supports threading via reply chains.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadedMessage {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: u64,
}

/// A conversation that supports threaded reply chains.
pub struct ThreadedConversation {
    messages: Vec<ThreadedMessage>,
    next_id: u64,
}

impl ThreadedConversation {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            next_id: 1,
        }
    }

    /// Post a new top-level message. Returns its assigned ID.
    pub fn post(&mut self, role: MessageRole, content: impl Into<String>, timestamp: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.messages.push(ThreadedMessage {
            id,
            parent_id: None,
            role,
            content: content.into(),
            timestamp,
        });
        id
    }

    /// Reply to an existing message. Returns the new message ID, or
    /// `ChatError::ValidationError` if `parent_id` does not exist.
    pub fn reply(
        &mut self,
        parent_id: u64,
        role: MessageRole,
        content: impl Into<String>,
        timestamp: u64,
    ) -> Result<u64, ChatError> {
        if !self.messages.iter().any(|m| m.id == parent_id) {
            return Err(ChatError::ValidationError(format!(
                "parent message {parent_id} not found"
            )));
        }
        let id = self.next_id;
        self.next_id += 1;
        self.messages.push(ThreadedMessage {
            id,
            parent_id: Some(parent_id),
            role,
            content: content.into(),
            timestamp,
        });
        Ok(id)
    }

    /// Return all direct replies to the given message ID.
    pub fn replies_to(&self, parent_id: u64) -> Vec<&ThreadedMessage> {
        self.messages
            .iter()
            .filter(|m| m.parent_id == Some(parent_id))
            .collect()
    }

    /// Return the full thread starting from `root_id` (depth-first).
    pub fn thread_from(&self, root_id: u64) -> Vec<&ThreadedMessage> {
        let mut result = Vec::new();
        self.collect_thread(root_id, &mut result);
        result
    }

    fn collect_thread<'a>(&'a self, id: u64, out: &mut Vec<&'a ThreadedMessage>) {
        if let Some(msg) = self.messages.iter().find(|m| m.id == id) {
            out.push(msg);
            for child in self.replies_to(id) {
                self.collect_thread(child.id, out);
            }
        }
    }

    /// Return all top-level messages (those with no parent).
    pub fn top_level(&self) -> Vec<&ThreadedMessage> {
        self.messages
            .iter()
            .filter(|m| m.parent_id.is_none())
            .collect()
    }

    /// Return the thread depth of a message (0 for top-level).
    pub fn depth(&self, id: u64) -> usize {
        let mut d = 0;
        let mut current = id;
        while let Some(msg) = self.messages.iter().find(|m| m.id == current) {
            match msg.parent_id {
                Some(pid) => {
                    d += 1;
                    current = pid;
                }
                None => break,
            }
        }
        d
    }

    /// Total number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns `true` when no messages exist.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for ThreadedConversation {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Unread message tracker
// ---------------------------------------------------------------------------

/// Tracks which messages have been read by each participant.
pub struct UnreadTracker {
    /// Maps participant ID → set of read message indices.
    read_set: HashMap<String, std::collections::HashSet<usize>>,
    total_messages: usize,
}

impl UnreadTracker {
    pub fn new() -> Self {
        Self {
            read_set: HashMap::new(),
            total_messages: 0,
        }
    }

    /// Record that a new message was appended to the conversation.
    pub fn on_message_added(&mut self) {
        self.total_messages += 1;
    }

    /// Mark a specific message index as read for a participant.
    pub fn mark_read(&mut self, participant_id: &str, message_index: usize) {
        self.read_set
            .entry(participant_id.to_string())
            .or_default()
            .insert(message_index);
    }

    /// Mark all current messages as read for a participant.
    pub fn mark_all_read(&mut self, participant_id: &str) {
        let set = self
            .read_set
            .entry(participant_id.to_string())
            .or_default();
        for i in 0..self.total_messages {
            set.insert(i);
        }
    }

    /// Return the number of unread messages for a participant.
    pub fn unread_count(&self, participant_id: &str) -> usize {
        let read = self
            .read_set
            .get(participant_id)
            .map_or(0, |s| s.len().min(self.total_messages));
        self.total_messages.saturating_sub(read)
    }

    /// Return the indices of unread messages for a participant.
    pub fn unread_indices(&self, participant_id: &str) -> Vec<usize> {
        let read = self.read_set.get(participant_id);
        (0..self.total_messages)
            .filter(|i| !read.map_or(false, |s| s.contains(i)))
            .collect()
    }

    /// Returns `true` if the participant has unread messages.
    pub fn has_unread(&self, participant_id: &str) -> bool {
        self.unread_count(participant_id) > 0
    }
}

impl Default for UnreadTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Participant presence tracking
// ---------------------------------------------------------------------------

/// Online status of a chat participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceStatus {
    Online,
    Away,
    DoNotDisturb,
    Offline,
}

impl fmt::Display for PresenceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresenceStatus::Online => write!(f, "online"),
            PresenceStatus::Away => write!(f, "away"),
            PresenceStatus::DoNotDisturb => write!(f, "do not disturb"),
            PresenceStatus::Offline => write!(f, "offline"),
        }
    }
}

/// Entry for one participant's presence information.
#[derive(Debug, Clone)]
pub struct PresenceEntry {
    pub participant_id: String,
    pub status: PresenceStatus,
    /// Timestamp when the status was last updated.
    pub last_updated: u64,
}

/// Tracks presence/status for all chat participants.
pub struct PresenceTracker {
    entries: HashMap<String, PresenceEntry>,
}

impl PresenceTracker {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Update the presence status for a participant.
    pub fn set_status(&mut self, participant_id: &str, status: PresenceStatus, timestamp: u64) {
        self.entries.insert(
            participant_id.to_string(),
            PresenceEntry {
                participant_id: participant_id.to_string(),
                status,
                last_updated: timestamp,
            },
        );
    }

    /// Get the current status of a participant (defaults to `Offline`).
    pub fn get_status(&self, participant_id: &str) -> PresenceStatus {
        self.entries
            .get(participant_id)
            .map_or(PresenceStatus::Offline, |e| e.status)
    }

    /// Return all participants currently online.
    pub fn online_participants(&self) -> Vec<&str> {
        self.entries
            .values()
            .filter(|e| e.status == PresenceStatus::Online)
            .map(|e| e.participant_id.as_str())
            .collect()
    }

    /// Return all participants with the given status.
    pub fn participants_with_status(&self, status: PresenceStatus) -> Vec<&str> {
        self.entries
            .values()
            .filter(|e| e.status == status)
            .map(|e| e.participant_id.as_str())
            .collect()
    }

    /// Number of tracked participants.
    pub fn tracked_count(&self) -> usize {
        self.entries.len()
    }

    /// Mark all participants whose `last_updated` is older than `threshold` as
    /// `Offline`.
    pub fn expire_stale(&mut self, threshold: u64) {
        for entry in self.entries.values_mut() {
            if entry.last_updated < threshold && entry.status != PresenceStatus::Offline {
                entry.status = PresenceStatus::Offline;
            }
        }
    }
}

impl Default for PresenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Chat session export to Markdown with metadata
// ---------------------------------------------------------------------------

/// Extended exporter that includes participant and session metadata.
pub struct ChatSessionExporter<'a> {
    title: &'a str,
    messages: &'a [ChatMessage],
    participants: &'a [ChatParticipant],
}

impl<'a> ChatSessionExporter<'a> {
    pub fn new(
        title: &'a str,
        messages: &'a [ChatMessage],
        participants: &'a [ChatParticipant],
    ) -> Self {
        Self {
            title,
            messages,
            participants,
        }
    }

    /// Export to Markdown with a header, participant list, and messages.
    pub fn to_markdown(&self) -> String {
        let mut buf = format!("# {}\n\n", self.title);

        if !self.participants.is_empty() {
            buf.push_str("## Participants\n\n");
            for p in self.participants {
                let desc = p.description.as_deref().unwrap_or("(no description)");
                buf.push_str(&format!("- **{}** (`{}`): {}\n", p.name, p.id, desc));
            }
            buf.push('\n');
        }

        buf.push_str("## Messages\n\n");
        for (i, msg) in self.messages.iter().enumerate() {
            buf.push_str(&format!(
                "{}. **{}** _(t={})_: {}\n",
                i + 1,
                msg.role,
                msg.timestamp,
                msg.content
            ));
        }
        buf
    }

    /// Export to JSON-like structured text (no serde dependency).
    pub fn to_json_lines(&self) -> String {
        let mut buf = String::new();
        for msg in self.messages {
            buf.push_str(&format!(
                "{{\"role\":\"{}\",\"content\":\"{}\",\"timestamp\":{}}}\n",
                msg.role,
                msg.content.replace('\\', "\\\\").replace('"', "\\\""),
                msg.timestamp,
            ));
        }
        buf
    }
}

// ---------------------------------------------------------------------------
// ChatWelcomeView – quick start actions for new sessions
// ---------------------------------------------------------------------------

/// An action displayed on the chat welcome screen.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatWelcomeAction {
    pub id: String,
    pub label: String,
    pub description: String,
    pub icon: String,
}

impl ChatWelcomeAction {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        icon: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            icon: icon.into(),
        }
    }
}

impl fmt::Display for ChatWelcomeAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.icon, self.label)
    }
}

/// The welcome view displayed when no chat session is active.
#[derive(Debug, Clone)]
pub struct ChatWelcomeView {
    pub title: String,
    pub subtitle: String,
    pub actions: Vec<ChatWelcomeAction>,
}

impl ChatWelcomeView {
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            actions: Vec::new(),
        }
    }

    pub fn add_action(&mut self, action: ChatWelcomeAction) {
        self.actions.push(action);
    }

    /// Build a default welcome view.
    pub fn default_view() -> Self {
        let mut v = Self::new("Welcome to Chat", "How can I help you today?");
        v.add_action(ChatWelcomeAction::new("explain", "Explain Code", "Get an explanation of selected code", "💡"));
        v.add_action(ChatWelcomeAction::new("generate", "Generate Code", "Generate code from a description", "⚡"));
        v.add_action(ChatWelcomeAction::new("fix", "Fix Issues", "Find and fix problems in code", "🔧"));
        v
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }
}

// ---------------------------------------------------------------------------
// ChatCodeBlock – syntax highlighted code in chat
// ---------------------------------------------------------------------------

/// A code block displayed in a chat message.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatCodeBlock {
    pub code: String,
    pub language: String,
    pub line_count: usize,
    pub copyable: bool,
    pub insertable: bool,
}

impl ChatCodeBlock {
    pub fn new(code: impl Into<String>, language: impl Into<String>) -> Self {
        let code = code.into();
        let line_count = code.lines().count().max(1);
        Self {
            code,
            language: language.into(),
            line_count,
            copyable: true,
            insertable: true,
        }
    }

    /// Format as a markdown fenced code block.
    pub fn to_markdown(&self) -> String {
        format!("```{}\n{}\n```", self.language, self.code)
    }

    /// Extract from a markdown code block string.
    pub fn from_markdown(markdown: &str) -> Option<Self> {
        let trimmed = markdown.trim();
        if !trimmed.starts_with("```") || !trimmed.ends_with("```") {
            return None;
        }
        let inner = &trimmed[3..trimmed.len() - 3];
        let (lang, code) = match inner.find('\n') {
            Some(pos) => (inner[..pos].trim(), inner[pos + 1..].trim()),
            None => ("", inner),
        };
        Some(Self::new(code, lang))
    }
}

impl fmt::Display for ChatCodeBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_markdown())
    }
}

// ---------------------------------------------------------------------------
// ChatUserInputHandler – slash command parsing
// ---------------------------------------------------------------------------

/// A parsed user input with optional slash command.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedChatInput {
    pub command: Option<String>,
    pub argument: String,
    pub mentions: Vec<String>,
}

/// Parses user input text, extracting slash commands and @mentions.
pub struct ChatUserInputHandler;

impl ChatUserInputHandler {
    /// Parse user input text.
    pub fn parse(input: &str) -> ParsedChatInput {
        let trimmed = input.trim();
        let (command, rest) = if trimmed.starts_with('/') {
            match trimmed.find(' ') {
                Some(pos) => (Some(trimmed[1..pos].to_string()), trimmed[pos + 1..].trim()),
                None => (Some(trimmed[1..].to_string()), ""),
            }
        } else {
            (None, trimmed)
        };

        let mentions: Vec<String> = rest
            .split_whitespace()
            .filter(|w| w.starts_with('@'))
            .map(|w| w[1..].to_string())
            .collect();

        ParsedChatInput {
            command,
            argument: rest.to_string(),
            mentions,
        }
    }

    /// Check if input starts with a slash command.
    pub fn is_command(input: &str) -> bool {
        input.trim().starts_with('/')
    }

    /// Extract just the command name from input, if present.
    pub fn command_name(input: &str) -> Option<String> {
        let parsed = Self::parse(input);
        parsed.command
    }
}

// ---------------------------------------------------------------------------
// ChatSessionExportFormat – session export in multiple formats
// ---------------------------------------------------------------------------

/// Format for exporting a chat session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatExportFormat {
    Markdown,
    JsonLines,
    PlainText,
}

impl fmt::Display for ChatExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markdown => write!(f, "Markdown"),
            Self::JsonLines => write!(f, "JSON Lines"),
            Self::PlainText => write!(f, "Plain Text"),
        }
    }
}

/// Export chat messages in plain text format.
pub fn export_plain_text(messages: &[ChatMessage]) -> String {
    let mut buf = String::new();
    for msg in messages {
        buf.push_str(&format!("[{}] {}\n", msg.role, msg.content));
    }
    buf
}


// === Chat Code Block Executor ===

/// Chat Code Block Executor implementation.
#[derive(Debug, Clone)]
pub struct ChatCodeBlockExecutor {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: ChatCodeBlockExecutorStats,
}

/// Statistics for ChatCodeBlockExecutor.
#[derive(Debug, Clone, Default)]
pub struct ChatCodeBlockExecutorStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl ChatCodeBlockExecutorStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl ChatCodeBlockExecutor {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: ChatCodeBlockExecutorStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &ChatCodeBlockExecutorStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for ChatCodeBlockExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// === Chat Message Search ===

/// Priority level for ChatMessageSearch items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChatMessageSearchPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ChatMessageSearchPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for ChatMessageSearchPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Chat Message Search implementation.
#[derive(Debug, Clone)]
pub struct ChatMessageSearch {
    items: Vec<ChatMessageSearchItem>,
    max_items: usize,
    default_priority: ChatMessageSearchPriority,
}

/// A single item in ChatMessageSearch.
#[derive(Debug, Clone)]
pub struct ChatMessageSearchItem {
    pub id: String,
    pub label: String,
    pub priority: ChatMessageSearchPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl ChatMessageSearchItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: ChatMessageSearchPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: ChatMessageSearchPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl ChatMessageSearch {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: ChatMessageSearchPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: ChatMessageSearchItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<ChatMessageSearchItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&ChatMessageSearchItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: ChatMessageSearchPriority) -> Vec<&ChatMessageSearchItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ChatMessageSearchItem> {
        let mut sorted: Vec<&ChatMessageSearchItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&ChatMessageSearchItem> {
        let mut sorted: Vec<&ChatMessageSearchItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&ChatMessageSearchItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: ChatMessageSearchPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> ChatMessageSearchPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &ChatMessageSearchItem> {
        self.items.iter()
    }
}

impl Default for ChatMessageSearch {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// wb_chat – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbChatLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbChatPanelState {
    pub region: XWbChatLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbChatPanelState {
    pub fn new(region: XWbChatLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_chat_total_visible_area(panels: &[XWbChatPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_chat_count_in_region(
    panels: &[XWbChatPanelState],
    region: XWbChatLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_chat_widest_panel(panels: &[XWbChatPanelState]) -> Option<&XWbChatPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_chat_collapse_region(
    panels: &mut [XWbChatPanelState],
    region: XWbChatLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbChatLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbChatLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// wb_chat – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench chat panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbChatChatMessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl YWbChatChatMessageRole {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::User => 0,
            Self::Assistant => 1,
            Self::System => 2,
            Self::Tool => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::User => "User",
            Self::Assistant => "Assistant",
            Self::System => "System",
            Self::Tool => "Tool",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbChatChatMessageRole] {
        &[
            YWbChatChatMessageRole::User,
            YWbChatChatMessageRole::Assistant,
            YWbChatChatMessageRole::System,
            YWbChatChatMessageRole::Tool,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbChatChatMessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks chat session data.
#[derive(Debug, Clone)]
pub struct YWbChatChatSession {
    pub messages: Vec<(String, String)>,
    pub session_id: String,
    pub token_count: u64,
}

impl YWbChatChatSession {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            session_id: String::new(),
            token_count: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbChatChatSession({}: {:?})", "messages", self.messages)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_chat_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_chat_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_chat_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_chat_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_chat_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_chat_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_chat_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_chat_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_chat – Extended chat token counter helpers
// ---------------------------------------------------------------------------

/// Priority levels for chat token counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbChatPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbChatPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZWbChatPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbChatPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks chat token counter data.
#[derive(Debug, Clone)]
pub struct ZWbChatChatTokenCounter {
    pub segment_counts: Vec<(String, usize)>,
    pub total_tokens: usize,
    pub limit: usize,
}

impl ZWbChatChatTokenCounter {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            segment_counts: Vec::new(),
            total_tokens: 0,
            limit: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.segment_counts.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.segment_counts.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.segment_counts.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbChatChatTokenCounter[total_tokens={:?}, limit={:?}]", self.total_tokens, self.limit)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for chat token counter.
pub fn z_wb_chat_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_chat_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_chat_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_chat_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_wb_chat_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_chat_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_chat_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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
    fn get_default_participant_works() {
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
    fn unregister_variable_works() {
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

    // ── ThreadedConversation tests ──

    #[test]
    fn threaded_post_and_reply() {
        let mut tc = ThreadedConversation::new();
        let root = tc.post(MessageRole::User, "Hello", 100);
        assert_eq!(root, 1);
        let reply_id = tc.reply(root, MessageRole::Assistant, "Hi back", 101).unwrap();
        assert_eq!(reply_id, 2);
        assert_eq!(tc.len(), 2);

        let replies = tc.replies_to(root);
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].content, "Hi back");
    }

    #[test]
    fn threaded_reply_to_nonexistent_fails() {
        let mut tc = ThreadedConversation::new();
        let err = tc.reply(999, MessageRole::User, "orphan", 0).unwrap_err();
        assert!(matches!(err, ChatError::ValidationError(_)));
    }

    #[test]
    fn threaded_depth_and_thread_from() {
        let mut tc = ThreadedConversation::new();
        let a = tc.post(MessageRole::User, "root", 1);
        let b = tc.reply(a, MessageRole::Assistant, "depth-1", 2).unwrap();
        let c = tc.reply(b, MessageRole::User, "depth-2", 3).unwrap();
        assert_eq!(tc.depth(a), 0);
        assert_eq!(tc.depth(b), 1);
        assert_eq!(tc.depth(c), 2);

        let thread = tc.thread_from(a);
        assert_eq!(thread.len(), 3);
        assert_eq!(thread[0].id, a);
        assert_eq!(thread[2].id, c);

        assert_eq!(tc.top_level().len(), 1);
    }

    // ── UnreadTracker tests ──

    #[test]
    fn unread_tracker_basic() {
        let mut tracker = UnreadTracker::new();
        tracker.on_message_added();
        tracker.on_message_added();
        tracker.on_message_added();

        assert_eq!(tracker.unread_count("alice"), 3);
        assert!(tracker.has_unread("alice"));

        tracker.mark_read("alice", 0);
        tracker.mark_read("alice", 1);
        assert_eq!(tracker.unread_count("alice"), 1);
        assert_eq!(tracker.unread_indices("alice"), vec![2]);

        tracker.mark_all_read("alice");
        assert_eq!(tracker.unread_count("alice"), 0);
        assert!(!tracker.has_unread("alice"));
    }

    // ── PresenceTracker tests ──

    #[test]
    fn presence_tracker_set_and_query() {
        let mut pt = PresenceTracker::new();
        assert_eq!(pt.get_status("copilot"), PresenceStatus::Offline);

        pt.set_status("copilot", PresenceStatus::Online, 100);
        pt.set_status("user1", PresenceStatus::Away, 100);
        pt.set_status("user2", PresenceStatus::Online, 100);
        assert_eq!(pt.get_status("copilot"), PresenceStatus::Online);
        assert_eq!(pt.tracked_count(), 3);

        let mut online = pt.online_participants();
        online.sort();
        assert_eq!(online, vec!["copilot", "user2"]);

        assert_eq!(pt.participants_with_status(PresenceStatus::Away), vec!["user1"]);
    }

    #[test]
    fn presence_tracker_expire_stale() {
        let mut pt = PresenceTracker::new();
        pt.set_status("a", PresenceStatus::Online, 50);
        pt.set_status("b", PresenceStatus::Online, 200);
        pt.expire_stale(100);
        assert_eq!(pt.get_status("a"), PresenceStatus::Offline);
        assert_eq!(pt.get_status("b"), PresenceStatus::Online);
    }

    // ── ChatSessionExporter tests ──

    #[test]
    fn session_exporter_markdown() {
        let participants = vec![make_participant("copilot", true)];
        let messages = vec![
            ChatMessage { role: MessageRole::User, content: "Hi".into(), timestamp: 1 },
            ChatMessage { role: MessageRole::Assistant, content: "Hello!".into(), timestamp: 2 },
        ];
        let md = ChatSessionExporter::new("Test Chat", &messages, &participants).to_markdown();
        assert!(md.starts_with("# Test Chat"));
        assert!(md.contains("## Participants"));
        assert!(md.contains("`copilot`"));
        assert!(md.contains("## Messages"));
        assert!(md.contains("1. **user**"));
    }

    #[test]
    fn session_exporter_json_lines() {
        let messages = vec![
            ChatMessage { role: MessageRole::User, content: "Say \"hi\"".into(), timestamp: 5 },
        ];
        let jl = ChatSessionExporter::new("t", &messages, &[]).to_json_lines();
        assert!(jl.contains(r#"\"hi\""#));
        assert!(jl.contains("\"timestamp\":5"));
    }

    // -- ChatWelcomeView tests --

    #[test]
    fn welcome_view_default() {
        let wv = ChatWelcomeView::default_view();
        assert_eq!(wv.title, "Welcome to Chat");
        assert!(wv.action_count() >= 3);
    }

    #[test]
    fn welcome_action_display() {
        let action = ChatWelcomeAction::new("fix", "Fix", "Fix stuff", "🔧");
        assert_eq!(format!("{}", action), "🔧 Fix");
    }

    #[test]
    fn welcome_view_custom() {
        let mut wv = ChatWelcomeView::new("Hello", "Start chatting");
        wv.add_action(ChatWelcomeAction::new("a", "Action", "Do stuff", "⚡"));
        assert_eq!(wv.action_count(), 1);
    }

    // -- ChatCodeBlock tests --

    #[test]
    fn code_block_to_markdown() {
        let cb = ChatCodeBlock::new("let x = 42;", "rust");
        let md = cb.to_markdown();
        assert!(md.starts_with("```rust"));
        assert!(md.contains("let x = 42;"));
        assert!(md.ends_with("```"));
    }

    #[test]
    fn code_block_from_markdown() {
        let md = "```python\nprint('hello')\n```";
        let cb = ChatCodeBlock::from_markdown(md).unwrap();
        assert_eq!(cb.language, "python");
        assert!(cb.code.contains("print"));
    }

    #[test]
    fn code_block_from_markdown_invalid() {
        assert!(ChatCodeBlock::from_markdown("not a code block").is_none());
    }

    #[test]
    fn code_block_line_count() {
        let cb = ChatCodeBlock::new("a\nb\nc", "txt");
        assert_eq!(cb.line_count, 3);
    }

    #[test]
    fn code_block_display() {
        let cb = ChatCodeBlock::new("x", "js");
        let s = format!("{}", cb);
        assert!(s.contains("```js"));
    }

    // -- ChatUserInputHandler tests --

    #[test]
    fn parse_slash_command() {
        let parsed = ChatUserInputHandler::parse("/explain this code");
        assert_eq!(parsed.command, Some("explain".into()));
        assert_eq!(parsed.argument, "this code");
    }

    #[test]
    fn parse_no_command() {
        let parsed = ChatUserInputHandler::parse("just a question");
        assert!(parsed.command.is_none());
        assert_eq!(parsed.argument, "just a question");
    }

    #[test]
    fn parse_mentions() {
        let parsed = ChatUserInputHandler::parse("@copilot what does @workspace do?");
        assert_eq!(parsed.mentions, vec!["copilot", "workspace"]);
    }

    #[test]
    fn is_command_check() {
        assert!(ChatUserInputHandler::is_command("/fix"));
        assert!(!ChatUserInputHandler::is_command("hello"));
    }

    #[test]
    fn command_name_extraction() {
        assert_eq!(ChatUserInputHandler::command_name("/help"), Some("help".into()));
        assert_eq!(ChatUserInputHandler::command_name("no command"), None);
    }

    // -- Export tests --

    #[test]
    fn export_format_display() {
        assert_eq!(format!("{}", ChatExportFormat::Markdown), "Markdown");
        assert_eq!(format!("{}", ChatExportFormat::PlainText), "Plain Text");
    }

    #[test]
    fn export_plain_text_format() {
        let messages = vec![
            ChatMessage { role: MessageRole::User, content: "hello".into(), timestamp: 1 },
            ChatMessage { role: MessageRole::Assistant, content: "hi!".into(), timestamp: 2 },
        ];
        let text = export_plain_text(&messages);
        assert!(text.contains("[user]"));
        assert!(text.contains("[assistant]"));
        assert!(text.contains("hello"));
        assert!(text.contains("hi!"));
    }

    #[test]
    fn chatCodeBlockExecutor_new() {
        let s = ChatCodeBlockExecutor::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn chatCodeBlockExecutor_add_contains() {
        let mut s = ChatCodeBlockExecutor::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn chatCodeBlockExecutor_add_duplicate() {
        let mut s = ChatCodeBlockExecutor::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn chatCodeBlockExecutor_remove() {
        let mut s = ChatCodeBlockExecutor::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn chatCodeBlockExecutor_capacity() {
        let s = ChatCodeBlockExecutor::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn chatCodeBlockExecutor_search() {
        let mut s = ChatCodeBlockExecutor::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn chatCodeBlockExecutor_stats() {
        let mut s = ChatCodeBlockExecutor::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn chatMessageSearch_new() {
        let m = ChatMessageSearch::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn chatMessageSearch_add_find() {
        let mut m = ChatMessageSearch::new();
        m.add(ChatMessageSearchItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn chatMessageSearch_priority_filter() {
        let mut m = ChatMessageSearch::new();
        m.add(ChatMessageSearchItem::new("a", "A").with_priority(ChatMessageSearchPriority::High));
        m.add(ChatMessageSearchItem::new("b", "B").with_priority(ChatMessageSearchPriority::Low));
        m.add(ChatMessageSearchItem::new("c", "C").with_priority(ChatMessageSearchPriority::High));
        assert_eq!(m.by_priority(ChatMessageSearchPriority::High).len(), 2);
    }

    #[test]
    fn chatMessageSearch_remove() {
        let mut m = ChatMessageSearch::new();
        m.add(ChatMessageSearchItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn chatMessageSearch_search() {
        let mut m = ChatMessageSearch::new();
        m.add(ChatMessageSearchItem::new("id1", "Hello World"));
        m.add(ChatMessageSearchItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn chatMessageSearch_total_weight() {
        let mut m = ChatMessageSearch::new();
        m.add(ChatMessageSearchItem::new("a", "A").with_priority(ChatMessageSearchPriority::Critical));
        m.add(ChatMessageSearchItem::new("b", "B").with_priority(ChatMessageSearchPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn chatMessageSearch_capacity_limit() {
        let mut m = ChatMessageSearch::new().with_max_items(2);
        m.add(ChatMessageSearchItem::new("1", "one"));
        m.add(ChatMessageSearchItem::new("2", "two"));
        assert!(!m.add(ChatMessageSearchItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn chatMessageSearch_sorted_by_priority() {
        let mut m = ChatMessageSearch::new();
        m.add(ChatMessageSearchItem::new("lo", "Low").with_priority(ChatMessageSearchPriority::Low));
        m.add(ChatMessageSearchItem::new("hi", "High").with_priority(ChatMessageSearchPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn chatMessageSearch_item_metadata() {
        let mut item = ChatMessageSearchItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn chatCodeBlockExecutor_enabled_toggle() {
        let mut s = ChatCodeBlockExecutor::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn chatMessageSearch_priority_display() {
        assert_eq!(format!("{}", ChatMessageSearchPriority::High), "high");
        assert_eq!(format!("{}", ChatMessageSearchPriority::Low), "low");
    }


    // -- wb_chat additional tests -------------------------------------------

    #[test]
    fn x_wb_chat_panel_state_new() {
        let p = XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbChatLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_chat_panel_area() {
        let p = XWbChatPanelState::new(XWbChatLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_chat_panel_toggle() {
        let mut p = XWbChatPanelState::new(XWbChatLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_chat_panel_resize() {
        let mut p = XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_chat_panel_is_narrow() {
        let mut p = XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_chat_total_visible_area_basic() {
        let panels = vec![
            XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "a"),
            XWbChatPanelState::new(XWbChatLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_chat_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_chat_total_visible_area_hidden() {
        let mut panels = vec![
            XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "a"),
            XWbChatPanelState::new(XWbChatLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_chat_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_chat_count_in_region_basic() {
        let panels = vec![
            XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "a"),
            XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "b"),
            XWbChatPanelState::new(XWbChatLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_chat_count_in_region(&panels, XWbChatLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_chat_count_in_region(&panels, XWbChatLayoutRegion::Editor), 1);
        assert_eq!(x_wb_chat_count_in_region(&panels, XWbChatLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_chat_widest_panel_basic() {
        let mut panels = vec![
            XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "narrow"),
            XWbChatPanelState::new(XWbChatLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_chat_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_chat_collapse_region_basic() {
        let mut panels = vec![
            XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "a"),
            XWbChatPanelState::new(XWbChatLayoutRegion::Sidebar, "b"),
            XWbChatPanelState::new(XWbChatLayoutRegion::Editor, "c"),
        ];
        x_wb_chat_collapse_region(&mut panels, XWbChatLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_chat_layout_constraint_clamp() {
        let lc = XWbChatLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_chat_layout_constraint_satisfied() {
        let lc = XWbChatLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_chat_widest_panel_empty() {
        let panels: Vec<XWbChatPanelState> = vec![];
        assert!(x_wb_chat_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_chat_layout_region_eq() {
        assert_eq!(XWbChatLayoutRegion::Sidebar, XWbChatLayoutRegion::Sidebar);
        assert_ne!(XWbChatLayoutRegion::Sidebar, XWbChatLayoutRegion::Panel);
    }


    // -- wb_chat extended domain tests ----------------------------------------

    #[test]
    fn y_wb_chat_enum_index() {
        assert_eq!(YWbChatChatMessageRole::User.index(), 0);
        assert_eq!(YWbChatChatMessageRole::Assistant.index(), 1);
        assert_eq!(YWbChatChatMessageRole::System.index(), 2);
        assert_eq!(YWbChatChatMessageRole::Tool.index(), 3);
    }

    #[test]
    fn y_wb_chat_enum_label() {
        assert_eq!(YWbChatChatMessageRole::User.label(), "User");
        assert_eq!(YWbChatChatMessageRole::Assistant.label(), "Assistant");
        assert_eq!(YWbChatChatMessageRole::System.label(), "System");
        assert_eq!(YWbChatChatMessageRole::Tool.label(), "Tool");
    }

    #[test]
    fn y_wb_chat_enum_all() {
        let all = YWbChatChatMessageRole::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_chat_enum_is_default() {
        assert!(YWbChatChatMessageRole::User.is_default());
        assert!(!YWbChatChatMessageRole::Tool.is_default());
    }

    #[test]
    fn y_wb_chat_enum_display() {
        assert_eq!(format!("{}", YWbChatChatMessageRole::User), "User");
    }

    #[test]
    fn y_wb_chat_struct_new() {
        let s = YWbChatChatSession::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_wb_chat_struct_clear() {
        let mut s = YWbChatChatSession::new();
        s.messages.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_wb_chat_fingerprint_deterministic() {
        let h1 = y_wb_chat_fingerprint("hello");
        let h2 = y_wb_chat_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_chat_fingerprint("a"), y_wb_chat_fingerprint("b"));
    }

    #[test]
    fn y_wb_chat_truncate_short() {
        assert_eq!(y_wb_chat_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_chat_truncate_long() {
        let r = y_wb_chat_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_chat_normalize_key_basic() {
        assert_eq!(y_wb_chat_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_chat_split_path_basic() {
        let parts = y_wb_chat_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_chat_count_occurrences_basic() {
        assert_eq!(y_wb_chat_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_chat_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_chat_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_chat_in_range_basic() {
        assert!(y_wb_chat_in_range(5, 1, 10));
        assert!(y_wb_chat_in_range(1, 1, 10));
        assert!(y_wb_chat_in_range(10, 1, 10));
        assert!(!y_wb_chat_in_range(0, 1, 10));
        assert!(!y_wb_chat_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_chat_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_chat_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_chat_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_chat_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_chat Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_chat_priority_weight() {
        assert_eq!(ZWbChatPriority::Idle.weight(), 0);
        assert_eq!(ZWbChatPriority::Normal.weight(), 2);
        assert_eq!(ZWbChatPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_chat_priority_label() {
        assert_eq!(ZWbChatPriority::Low.label(), "low");
        assert_eq!(ZWbChatPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_chat_priority_is_elevated() {
        assert!(!ZWbChatPriority::Normal.is_elevated());
        assert!(ZWbChatPriority::High.is_elevated());
        assert!(ZWbChatPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_chat_priority_display() {
        assert_eq!(format!("{}", ZWbChatPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_chat_priority_all_asc() {
        let all = ZWbChatPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbChatPriority::Idle);
        assert_eq!(all[4], ZWbChatPriority::Realtime);
    }

    #[test]
    fn z_wb_chat_struct_new() {
        let s = ZWbChatChatTokenCounter::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_chat_struct_toggled_clone() {
        let s = ZWbChatChatTokenCounter::new();
        let t = s.toggled_clone();
        let _ = t.limit;
    }

    #[test]
    fn z_wb_chat_rolling_hash_deterministic() {
        let h1 = z_wb_chat_rolling_hash(b"test");
        let h2 = z_wb_chat_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_chat_rolling_hash(b"a"), z_wb_chat_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_chat_pad_to_basic() {
        assert_eq!(z_wb_chat_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_chat_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_chat_is_identifier_basic() {
        assert!(z_wb_chat_is_identifier("foo_bar"));
        assert!(z_wb_chat_is_identifier("abc123"));
        assert!(!z_wb_chat_is_identifier(""));
        assert!(!z_wb_chat_is_identifier("has space"));
    }

    #[test]
    fn z_wb_chat_levenshtein_basic() {
        assert_eq!(z_wb_chat_levenshtein("", ""), 0);
        assert_eq!(z_wb_chat_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_chat_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_chat_unique_words_basic() {
        let w = z_wb_chat_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_chat_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_chat_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_chat_common_prefix_basic() {
        assert_eq!(z_wb_chat_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_chat_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_chat_struct_clear() {
        let mut s = ZWbChatChatTokenCounter::new();
        s.segment_counts.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_chat_rolling_hash_empty() {
        let h = z_wb_chat_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}
