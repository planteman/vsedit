//! Inline chat widget.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineChatState {
    Idle,
    Waiting,
    Streaming,
    Done,
    Error,
}

impl fmt::Display for InlineChatState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Waiting => write!(f, "Waiting"),
            Self::Streaming => write!(f, "Streaming"),
            Self::Done => write!(f, "Done"),
            Self::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineChatError {
    NoActiveRequest,
    AlreadyStreaming,
    RequestCancelled,
}

impl fmt::Display for InlineChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoActiveRequest => write!(f, "no active request"),
            Self::AlreadyStreaming => write!(f, "already streaming"),
            Self::RequestCancelled => write!(f, "request cancelled"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InlineChatRequest {
    pub prompt: String,
    pub selection_start_line: u32,
    pub selection_end_line: u32,
    pub uri: String,
}

impl fmt::Display for InlineChatRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (lines {}-{})",
            self.prompt, self.selection_start_line, self.selection_end_line
        )
    }
}

#[derive(Debug, Clone)]
pub struct InlineChatEdit {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
}

#[derive(Debug, Clone)]
pub struct InlineChatResponse {
    pub text: String,
    pub edits: Vec<InlineChatEdit>,
}

/// A record of a completed inline chat interaction.
#[derive(Debug, Clone)]
pub struct InlineChatHistoryEntry {
    pub request: InlineChatRequest,
    pub response: InlineChatResponse,
}

/// Tracks past inline chat requests and responses.
#[derive(Debug, Clone, Default)]
pub struct InlineChatHistory {
    entries: Vec<InlineChatHistoryEntry>,
}

impl InlineChatHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, request: InlineChatRequest, response: InlineChatResponse) {
        self.entries.push(InlineChatHistoryEntry { request, response });
    }

    pub fn entries(&self) -> &[InlineChatHistoryEntry] {
        &self.entries
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
}

#[derive(Debug)]
pub struct InlineChatWidget {
    state: InlineChatState,
    request: Option<InlineChatRequest>,
    response: Option<InlineChatResponse>,
}

impl InlineChatWidget {
    pub fn new() -> Self {
        Self {
            state: InlineChatState::Idle,
            request: None,
            response: None,
        }
    }

    pub fn start_request(&mut self, req: InlineChatRequest) {
        self.request = Some(req);
        self.response = None;
        self.state = InlineChatState::Waiting;
    }

    pub fn set_response(&mut self, resp: InlineChatResponse) {
        self.response = Some(resp);
        self.state = InlineChatState::Done;
    }

    pub fn accept(&mut self) {
        self.state = InlineChatState::Idle;
        self.request = None;
        self.response = None;
    }

    pub fn reject(&mut self) {
        self.response = None;
        self.state = InlineChatState::Idle;
        self.request = None;
    }

    pub fn get_state(&self) -> &InlineChatState {
        &self.state
    }

    pub fn get_request(&self) -> Option<&InlineChatRequest> {
        self.request.as_ref()
    }

    pub fn get_response(&self) -> Option<&InlineChatResponse> {
        self.response.as_ref()
    }

    pub fn cancel(&mut self) {
        self.state = InlineChatState::Idle;
        self.request = None;
        self.response = None;
    }

    pub fn is_active(&self) -> bool {
        self.state != InlineChatState::Idle
    }

    /// Transition from Waiting to Streaming state.
    pub fn start_streaming(&mut self) -> Result<(), InlineChatError> {
        if self.request.is_none() {
            return Err(InlineChatError::NoActiveRequest);
        }
        if self.state == InlineChatState::Streaming {
            return Err(InlineChatError::AlreadyStreaming);
        }
        self.response = Some(InlineChatResponse {
            text: String::new(),
            edits: Vec::new(),
        });
        self.state = InlineChatState::Streaming;
        Ok(())
    }

    /// Append text to the response while streaming.
    pub fn append_streaming(&mut self, text: &str) -> Result<(), InlineChatError> {
        if self.state != InlineChatState::Streaming {
            return Err(InlineChatError::NoActiveRequest);
        }
        if let Some(ref mut resp) = self.response {
            resp.text.push_str(text);
            Ok(())
        } else {
            Err(InlineChatError::NoActiveRequest)
        }
    }

    /// Number of edits in the current response, or 0 if none.
    pub fn edit_count(&self) -> usize {
        self.response.as_ref().map_or(0, |r| r.edits.len())
    }
}

impl Default for InlineChatWidget {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional inline chat utilities
// ---------------------------------------------------------------------------

impl InlineChatEdit {
    /// Create a new edit spanning a range.
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32, new_text: impl Into<String>) -> Self {
        Self { start_line, start_col, end_line, end_col, new_text: new_text.into() }
    }

    /// The number of lines this edit spans in the original document.
    pub fn original_line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// The number of lines in the replacement text.
    pub fn new_text_line_count(&self) -> usize {
        if self.new_text.is_empty() { 0 } else { self.new_text.lines().count().max(1) }
    }

    /// Whether this edit is a pure insertion (start == end).
    pub fn is_insertion(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    /// Whether this edit is a pure deletion (empty new_text).
    pub fn is_deletion(&self) -> bool {
        self.new_text.is_empty() && !(self.start_line == self.end_line && self.start_col == self.end_col)
    }
}

impl PartialEq for InlineChatEdit {
    fn eq(&self, other: &Self) -> bool {
        self.start_line == other.start_line
            && self.start_col == other.start_col
            && self.end_line == other.end_line
            && self.end_col == other.end_col
            && self.new_text == other.new_text
    }
}

impl Eq for InlineChatEdit {}

impl PartialEq for InlineChatResponse {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text && self.edits == other.edits
    }
}

impl Eq for InlineChatResponse {}

impl InlineChatResponse {
    /// Create an empty response.
    pub fn empty() -> Self {
        Self { text: String::new(), edits: Vec::new() }
    }

    /// Total number of lines affected by all edits.
    pub fn total_lines_affected(&self) -> u32 {
        self.edits.iter().map(|e| e.original_line_span()).sum()
    }

    /// Whether the response contains any edits.
    pub fn has_edits(&self) -> bool {
        !self.edits.is_empty()
    }

    /// Word count of the response text.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

impl PartialEq for InlineChatHistoryEntry {
    fn eq(&self, other: &Self) -> bool {
        self.request.prompt == other.request.prompt && self.response.text == other.response.text
    }
}

impl InlineChatRequest {
    /// Number of lines in the selected range.
    pub fn selection_line_count(&self) -> u32 {
        self.selection_end_line.saturating_sub(self.selection_start_line) + 1
    }

    /// Whether the selection covers exactly one line.
    pub fn is_single_line(&self) -> bool {
        self.selection_start_line == self.selection_end_line
    }

    /// Word count of the prompt.
    pub fn prompt_word_count(&self) -> usize {
        self.prompt.split_whitespace().count()
    }
}

impl PartialEq for InlineChatRequest {
    fn eq(&self, other: &Self) -> bool {
        self.prompt == other.prompt
            && self.selection_start_line == other.selection_start_line
            && self.selection_end_line == other.selection_end_line
            && self.uri == other.uri
    }
}

impl Eq for InlineChatRequest {}

impl InlineChatWidget {
    /// Set the state to error.
    pub fn set_error(&mut self) {
        self.state = InlineChatState::Error;
    }

    /// Finish streaming and transition to Done.
    pub fn finish_streaming(&mut self) -> Result<(), InlineChatError> {
        if self.state != InlineChatState::Streaming {
            return Err(InlineChatError::NoActiveRequest);
        }
        self.state = InlineChatState::Done;
        Ok(())
    }

    /// Get a summary of the current widget state.
    pub fn summary(&self) -> String {
        let state = &self.state;
        let prompt = self.request.as_ref().map_or("none", |r| &r.prompt);
        let edit_count = self.edit_count();
        format!("state={state}, prompt=\"{prompt}\", edits={edit_count}")
    }

    /// Replace the current response entirely.
    pub fn replace_response(&mut self, resp: InlineChatResponse) {
        self.response = Some(resp);
    }
}

impl InlineChatHistory {
    /// Get the most recent entry.
    pub fn last(&self) -> Option<&InlineChatHistoryEntry> {
        self.entries.last()
    }

    /// Search history for entries whose prompt contains the query.
    pub fn search(&self, query: &str) -> Vec<&InlineChatHistoryEntry> {
        let q = query.to_lowercase();
        self.entries.iter().filter(|e| e.request.prompt.to_lowercase().contains(&q)).collect()
    }

    /// Total number of edits across all history entries.
    pub fn total_edits(&self) -> usize {
        self.entries.iter().map(|e| e.response.edits.len()).sum()
    }

    /// Get entry by index.
    pub fn get(&self, index: usize) -> Option<&InlineChatHistoryEntry> {
        self.entries.get(index)
    }
}

/// Tracks the state of a streaming response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingState {
    pub chunks_received: usize,
    pub total_bytes: usize,
    pub is_complete: bool,
}

impl StreamingState {
    pub fn new() -> Self {
        Self {
            chunks_received: 0,
            total_bytes: 0,
            is_complete: false,
        }
    }

    pub fn add_chunk(&mut self, bytes: usize) {
        self.chunks_received += 1;
        self.total_bytes += bytes;
    }

    pub fn complete(&mut self) {
        self.is_complete = true;
    }
}

impl Default for StreamingState {
    fn default() -> Self {
        Self::new()
    }
}

/// A thread in a conversation, grouping related messages.
#[derive(Debug, Clone)]
pub struct ConversationThread {
    pub thread_id: String,
    pub parent_prompt: String,
    pub follow_ups: Vec<InlineChatRequest>,
}

impl ConversationThread {
    pub fn new(thread_id: impl Into<String>, parent_prompt: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            parent_prompt: parent_prompt.into(),
            follow_ups: Vec::new(),
        }
    }

    pub fn add_follow_up(&mut self, request: InlineChatRequest) {
        self.follow_ups.push(request);
    }

    pub fn follow_up_count(&self) -> usize {
        self.follow_ups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.follow_ups.is_empty()
    }
}

/// Search results from chat history.
#[derive(Debug, Clone)]
pub struct ChatSearchResult<'a> {
    pub index: usize,
    pub entry: &'a InlineChatHistoryEntry,
}

/// Search chat history entries whose prompt or response text contains the query.
pub fn search_history_entries<'a>(
    history: &'a InlineChatHistory,
    query: &str,
) -> Vec<ChatSearchResult<'a>> {
    let q = query.to_lowercase();
    history
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.request.prompt.to_lowercase().contains(&q)
                || e.response.text.to_lowercase().contains(&q)
        })
        .map(|(index, entry)| ChatSearchResult { index, entry })
        .collect()
}

// ---------------------------------------------------------------------------
// Inline chat diff
// ---------------------------------------------------------------------------

/// Character-level diff statistics between original text and new text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineChatDiff {
    /// Number of characters present in new_text but not in original.
    pub added_chars: usize,
    /// Number of characters present in original but not in new_text.
    pub removed_chars: usize,
    /// Number of characters unchanged between original and new_text.
    pub unchanged_chars: usize,
}

impl InlineChatDiff {
    /// Compute a simple character-level diff between `original` and `new_text`.
    ///
    /// Uses a longest-common-subsequence inspired scan: walks both strings
    /// left-to-right and counts matching vs differing characters.
    pub fn compute(original: &str, new_text: &str) -> Self {
        let orig_chars: Vec<char> = original.chars().collect();
        let new_chars: Vec<char> = new_text.chars().collect();

        let mut unchanged = 0usize;
        let mut oi = 0usize;
        let mut ni = 0usize;

        while oi < orig_chars.len() && ni < new_chars.len() {
            if orig_chars[oi] == new_chars[ni] {
                unchanged += 1;
                oi += 1;
                ni += 1;
            } else {
                // advance whichever pointer has more remaining
                if (orig_chars.len() - oi) > (new_chars.len() - ni) {
                    oi += 1;
                } else {
                    ni += 1;
                }
            }
        }

        let removed = orig_chars.len() - unchanged;
        let added = new_chars.len() - unchanged;

        Self {
            added_chars: added,
            removed_chars: removed,
            unchanged_chars: unchanged,
        }
    }

    /// Returns `true` if the diff has no changes at all.
    pub fn is_identical(&self) -> bool {
        self.added_chars == 0 && self.removed_chars == 0
    }

    /// Total number of changed characters (added + removed).
    pub fn total_changes(&self) -> usize {
        self.added_chars + self.removed_chars
    }
}

// ---------------------------------------------------------------------------
// Inline chat session
// ---------------------------------------------------------------------------

/// A session wrapping widget + history with prompt/response tracking.
#[derive(Debug)]
pub struct InlineChatSession {
    pub session_id: String,
    pub widget: InlineChatWidget,
    pub history: InlineChatHistory,
    pub created_prompts: Vec<String>,
}

impl InlineChatSession {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            widget: InlineChatWidget::new(),
            history: InlineChatHistory::new(),
            created_prompts: Vec::new(),
        }
    }

    /// Start a request and record the prompt.
    pub fn submit_prompt(&mut self, request: InlineChatRequest) {
        self.created_prompts.push(request.prompt.clone());
        self.widget.start_request(request);
    }

    /// Set the response and push the completed interaction to history.
    pub fn complete(&mut self, response: InlineChatResponse) {
        self.widget.set_response(response.clone());
        if let Some(req) = self.widget.get_request().cloned() {
            self.history.push(req, response);
        }
    }

    /// Accept the current response, resetting the widget to idle.
    pub fn accept(&mut self) {
        self.widget.accept();
    }

    /// Reject the current response, resetting the widget to idle.
    pub fn reject(&mut self) {
        self.widget.reject();
    }

    /// Re-submit the last prompt as a new request.
    pub fn retry(&mut self) {
        if let Some(prompt) = self.created_prompts.last().cloned() {
            if let Some(req) = self.widget.get_request().cloned() {
                let new_req = InlineChatRequest {
                    prompt,
                    selection_start_line: req.selection_start_line,
                    selection_end_line: req.selection_end_line,
                    uri: req.uri,
                };
                self.widget.start_request(new_req);
            }
        }
    }

    pub fn prompt_count(&self) -> usize {
        self.created_prompts.len()
    }

    pub fn is_idle(&self) -> bool {
        *self.widget.get_state() == InlineChatState::Idle
    }

    pub fn current_prompt(&self) -> Option<&str> {
        self.widget.get_request().map(|r| r.prompt.as_str())
    }
}

// ---------------------------------------------------------------------------
// Inline diff preview
// ---------------------------------------------------------------------------

/// A single hunk of differences between original and proposed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub start_line: usize,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

/// Shows proposed changes inline with hunk-based diffing.
#[derive(Debug, Clone)]
pub struct InlineDiffPreview {
    pub original_lines: Vec<String>,
    pub proposed_lines: Vec<String>,
    pub diff_hunks: Vec<DiffHunk>,
}

impl InlineDiffPreview {
    /// Compute a simple line-by-line diff between original and proposed text.
    pub fn compute(original: &str, proposed: &str) -> Self {
        let original_lines: Vec<String> = original.lines().map(String::from).collect();
        let proposed_lines: Vec<String> = proposed.lines().map(String::from).collect();
        let mut hunks = Vec::new();

        let max_len = original_lines.len().max(proposed_lines.len());
        let mut i = 0;
        while i < max_len {
            let orig = original_lines.get(i).map(String::as_str);
            let prop = proposed_lines.get(i).map(String::as_str);

            if orig != prop {
                let start = i;
                let mut removed = Vec::new();
                let mut added = Vec::new();

                // Collect consecutive differing lines.
                while i < max_len {
                    let o = original_lines.get(i).map(String::as_str);
                    let p = proposed_lines.get(i).map(String::as_str);
                    if o == p {
                        break;
                    }
                    if let Some(line) = o {
                        removed.push(line.to_string());
                    }
                    if let Some(line) = p {
                        added.push(line.to_string());
                    }
                    i += 1;
                }

                hunks.push(DiffHunk { start_line: start, removed, added });
            } else {
                i += 1;
            }
        }

        Self { original_lines, proposed_lines, diff_hunks: hunks }
    }

    pub fn hunk_count(&self) -> usize {
        self.diff_hunks.len()
    }

    pub fn total_additions(&self) -> usize {
        self.diff_hunks.iter().map(|h| h.added.len()).sum()
    }

    pub fn total_removals(&self) -> usize {
        self.diff_hunks.iter().map(|h| h.removed.len()).sum()
    }

    pub fn has_changes(&self) -> bool {
        !self.diff_hunks.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "{} hunk(s), +{} -{} lines",
            self.hunk_count(),
            self.total_additions(),
            self.total_removals(),
        )
    }
}

// ---------------------------------------------------------------------------
// Inline chat action
// ---------------------------------------------------------------------------

/// Actions a user can take on an inline chat response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineChatAction {
    Accept,
    Reject,
    Retry,
    Edit(String),
    AcceptPartial { hunk_index: usize },
}

impl InlineChatAction {
    /// Whether this action is terminal (Accept or Reject).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Accept | Self::Reject)
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Accept => "Accept the proposed changes",
            Self::Reject => "Reject the proposed changes",
            Self::Retry => "Retry the request",
            Self::Edit(_) => "Edit the prompt and resubmit",
            Self::AcceptPartial { .. } => "Accept a specific hunk",
        }
    }
}

impl fmt::Display for InlineChatAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accept => write!(f, "Accept"),
            Self::Reject => write!(f, "Reject"),
            Self::Retry => write!(f, "Retry"),
            Self::Edit(prompt) => write!(f, "Edit({prompt})"),
            Self::AcceptPartial { hunk_index } => write!(f, "AcceptPartial(hunk {hunk_index})"),
        }
    }
}

// ---------------------------------------------------------------------------
// ChatHistory — full conversation tracking
// ---------------------------------------------------------------------------

/// Role in a chat conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

impl fmt::Display for ChatRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::System => write!(f, "system"),
        }
    }
}

/// A single message in a chat conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub timestamp_ms: u64,
}

impl ChatMessage {
    /// Create a new chat message.
    pub fn new(role: ChatRole, content: impl Into<String>, timestamp_ms: u64) -> Self {
        Self {
            role,
            content: content.into(),
            timestamp_ms,
        }
    }

    /// Word count of the message content.
    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    /// Character count of the message content.
    pub fn char_count(&self) -> usize {
        self.content.len()
    }
}

/// Tracks a full chat conversation with multiple turns.
#[derive(Debug, Clone)]
pub struct ChatHistory {
    messages: Vec<ChatMessage>,
    max_messages: Option<usize>,
}

impl ChatHistory {
    /// Create a new empty chat history with no limit.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_messages: None,
        }
    }

    /// Create a chat history with a maximum number of messages.
    pub fn with_max(max: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages: Some(max),
        }
    }

    /// Add a message to the history.
    pub fn push(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
        if let Some(max) = self.max_messages {
            while self.messages.len() > max {
                self.messages.remove(0);
            }
        }
    }

    /// Number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get all messages.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get messages by role.
    pub fn messages_by_role(&self, role: &ChatRole) -> Vec<&ChatMessage> {
        self.messages.iter().filter(|m| &m.role == role).collect()
    }

    /// Get the last message.
    pub fn last(&self) -> Option<&ChatMessage> {
        self.messages.last()
    }

    /// Total word count across all messages.
    pub fn total_word_count(&self) -> usize {
        self.messages.iter().map(|m| m.word_count()).sum()
    }

    /// Clear the history.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Number of conversation turns (user+assistant pairs).
    pub fn turn_count(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.role == ChatRole::User)
            .count()
    }
}

// ---------------------------------------------------------------------------
// ChatMessageFormatter — format messages for display
// ---------------------------------------------------------------------------

/// Formats chat messages for terminal display.
#[derive(Debug, Clone)]
pub struct ChatMessageFormatter {
    /// Maximum width in characters for wrapping.
    pub max_width: usize,
    /// Whether to show timestamps.
    pub show_timestamps: bool,
}

impl ChatMessageFormatter {
    /// Create a new formatter with defaults.
    pub fn new(max_width: usize) -> Self {
        Self {
            max_width,
            show_timestamps: false,
        }
    }

    /// Format a single message for display.
    pub fn format(&self, msg: &ChatMessage) -> String {
        let role_prefix = match msg.role {
            ChatRole::User => "You",
            ChatRole::Assistant => "AI",
            ChatRole::System => "System",
        };
        let mut out = String::new();
        if self.show_timestamps {
            out.push_str(&format!("[{}ms] ", msg.timestamp_ms));
        }
        out.push_str(&format!("{}: ", role_prefix));

        // Word-wrap the content
        let mut line_len = out.len();
        for word in msg.content.split_whitespace() {
            if line_len + word.len() + 1 > self.max_width && line_len > 0 {
                out.push('\n');
                line_len = 0;
            }
            if line_len > 0 {
                out.push(' ');
                line_len += 1;
            }
            out.push_str(word);
            line_len += word.len();
        }
        out
    }

    /// Format a full history for display.
    pub fn format_history(&self, history: &ChatHistory) -> String {
        history
            .messages()
            .iter()
            .map(|m| self.format(m))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// StreamSimulator — simulate streaming responses in chunks
// ---------------------------------------------------------------------------

/// Simulates a streaming chat response broken into chunks.
#[derive(Debug, Clone)]
pub struct StreamSimulator {
    full_text: String,
    chunk_size: usize,
    position: usize,
}

impl StreamSimulator {
    /// Create a new stream simulator.
    pub fn new(text: impl Into<String>, chunk_size: usize) -> Self {
        Self {
            full_text: text.into(),
            chunk_size: chunk_size.max(1),
            position: 0,
        }
    }

    /// Get the next chunk of the stream, or `None` if finished.
    pub fn next_chunk(&mut self) -> Option<&str> {
        if self.position >= self.full_text.len() {
            return None;
        }
        let end = (self.position + self.chunk_size).min(self.full_text.len());
        let chunk = &self.full_text[self.position..end];
        self.position = end;
        Some(chunk)
    }

    /// Whether the stream is finished.
    pub fn is_done(&self) -> bool {
        self.position >= self.full_text.len()
    }

    /// Reset the stream to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// How much of the text has been consumed (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.full_text.is_empty() {
            return 1.0;
        }
        self.position as f64 / self.full_text.len() as f64
    }

    /// Collect all remaining chunks into a string.
    pub fn collect_remaining(&mut self) -> String {
        let mut out = String::new();
        while let Some(chunk) = self.next_chunk() {
            out.push_str(chunk);
        }
        out
    }

    /// Total length of the full text.
    pub fn total_len(&self) -> usize {
        self.full_text.len()
    }

    /// Number of bytes consumed so far.
    pub fn consumed(&self) -> usize {
        self.position
    }
}

// ── Inline chat utilities ───────────────────────────────────────────────

/// Count the total number of edits across all responses in a history.
pub fn total_history_edits(history: &InlineChatHistory) -> usize {
    history.entries().iter().map(|e| e.response.edits.len()).sum()
}

/// Compute the total word count of all response texts in a history.
pub fn total_history_words(history: &InlineChatHistory) -> usize {
    history
        .entries()
        .iter()
        .map(|e| e.response.word_count())
        .sum()
}

/// Find history entries whose prompts contain a given substring (case-insensitive).
pub fn search_history_prompts<'a>(history: &'a InlineChatHistory, query: &str) -> Vec<&'a InlineChatHistoryEntry> {
    let query_lower = query.to_lowercase();
    history
        .entries()
        .iter()
        .filter(|e| e.request.prompt.to_lowercase().contains(&query_lower))
        .collect()
}

/// Return the average number of edits per response in a history.
pub fn average_edits_per_response(history: &InlineChatHistory) -> f64 {
    if history.is_empty() {
        return 0.0;
    }
    total_history_edits(history) as f64 / history.len() as f64
}

/// Collect all unique URIs referenced in history requests.
pub fn unique_history_uris(history: &InlineChatHistory) -> Vec<String> {
    let mut uris: Vec<String> = history
        .entries()
        .iter()
        .map(|e| e.request.uri.clone())
        .collect();
    uris.sort();
    uris.dedup();
    uris
}

/// Compute the total lines affected across all edits in a response.
pub fn response_total_lines(response: &InlineChatResponse) -> u32 {
    response
        .edits
        .iter()
        .map(|e| e.original_line_span())
        .sum()
}

/// Check if an edit range overlaps with a given line range.
pub fn edit_overlaps_range(edit: &InlineChatEdit, start_line: u32, end_line: u32) -> bool {
    edit.start_line <= end_line && edit.end_line >= start_line
}

/// Filter edits in a response to only those overlapping a given selection.
pub fn edits_in_selection(response: &InlineChatResponse, start_line: u32, end_line: u32) -> Vec<&InlineChatEdit> {
    response
        .edits
        .iter()
        .filter(|e| edit_overlaps_range(e, start_line, end_line))
        .collect()
}

// ---------------------------------------------------------------------------
// Code block extraction from chat messages
// ---------------------------------------------------------------------------

/// A code block extracted from a chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// Optional language identifier (e.g. "rust", "python").
    pub language: Option<String>,
    /// The code content without the fence markers.
    pub code: String,
}

/// Extract fenced code blocks (``` delimited) from a message string.
pub fn extract_code_blocks(text: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            let lang_tag = trimmed.trim_start_matches('`').trim();
            let language = if lang_tag.is_empty() {
                None
            } else {
                Some(lang_tag.to_string())
            };

            let mut code_lines: Vec<&str> = Vec::new();
            for inner in lines.by_ref() {
                if inner.trim().starts_with("```") {
                    break;
                }
                code_lines.push(inner);
            }
            blocks.push(CodeBlock {
                language,
                code: code_lines.join("\n"),
            });
        }
    }
    blocks
}

// ---------------------------------------------------------------------------
// Suggestion accept/reject tracking
// ---------------------------------------------------------------------------

/// Outcome of a suggestion review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionOutcome {
    Accepted,
    Rejected,
    Partial,
}

/// Tracks accept/reject statistics for inline chat suggestions.
#[derive(Debug, Clone, Default)]
pub struct SuggestionTracker {
    outcomes: Vec<SuggestionOutcome>,
}

impl SuggestionTracker {
    pub fn new() -> Self {
        Self { outcomes: Vec::new() }
    }

    pub fn record(&mut self, outcome: SuggestionOutcome) {
        self.outcomes.push(outcome);
    }

    pub fn total(&self) -> usize {
        self.outcomes.len()
    }

    pub fn count(&self, outcome: SuggestionOutcome) -> usize {
        self.outcomes.iter().filter(|o| **o == outcome).count()
    }

    /// Acceptance rate as a value between 0.0 and 1.0.
    pub fn acceptance_rate(&self) -> f64 {
        if self.outcomes.is_empty() {
            return 0.0;
        }
        self.count(SuggestionOutcome::Accepted) as f64 / self.outcomes.len() as f64
    }

    pub fn clear(&mut self) {
        self.outcomes.clear();
    }
}

// ---------------------------------------------------------------------------
// Typing indicator state
// ---------------------------------------------------------------------------

/// State of a typing/thinking indicator shown while waiting for a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypingIndicatorState {
    Hidden,
    Thinking,
    Generating,
}

impl fmt::Display for TypingIndicatorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hidden => write!(f, ""),
            Self::Thinking => write!(f, "Thinking..."),
            Self::Generating => write!(f, "Generating..."),
        }
    }
}

/// Manages the typing indicator lifecycle.
#[derive(Debug, Clone)]
pub struct TypingIndicator {
    state: TypingIndicatorState,
    elapsed_ms: u64,
}

impl TypingIndicator {
    pub fn new() -> Self {
        Self {
            state: TypingIndicatorState::Hidden,
            elapsed_ms: 0,
        }
    }

    pub fn show_thinking(&mut self) {
        self.state = TypingIndicatorState::Thinking;
        self.elapsed_ms = 0;
    }

    pub fn show_generating(&mut self) {
        self.state = TypingIndicatorState::Generating;
    }

    pub fn hide(&mut self) {
        self.state = TypingIndicatorState::Hidden;
        self.elapsed_ms = 0;
    }

    pub fn tick(&mut self, delta_ms: u64) {
        self.elapsed_ms += delta_ms;
    }

    pub fn state(&self) -> &TypingIndicatorState {
        &self.state
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    pub fn is_visible(&self) -> bool {
        self.state != TypingIndicatorState::Hidden
    }

    /// Render a simple animation frame based on elapsed time.
    pub fn render_frame(&self) -> &str {
        match self.state {
            TypingIndicatorState::Hidden => "",
            TypingIndicatorState::Thinking | TypingIndicatorState::Generating => {
                let dots = (self.elapsed_ms / 500) % 4;
                match dots {
                    0 => "",
                    1 => ".",
                    2 => "..",
                    _ => "...",
                }
            }
        }
    }
}

impl Default for TypingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Message filtering
// ---------------------------------------------------------------------------

/// Filter criteria for searching chat messages.
#[derive(Debug, Clone)]
pub struct MessageFilter {
    pub role: Option<ChatRole>,
    pub keyword: Option<String>,
    pub min_word_count: Option<usize>,
}

impl MessageFilter {
    pub fn new() -> Self {
        Self {
            role: None,
            keyword: None,
            min_word_count: None,
        }
    }

    pub fn with_role(mut self, role: ChatRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn with_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keyword = Some(keyword.into());
        self
    }

    pub fn with_min_words(mut self, min: usize) -> Self {
        self.min_word_count = Some(min);
        self
    }

    /// Test whether a message matches this filter.
    pub fn matches(&self, msg: &ChatMessage) -> bool {
        if let Some(ref role) = self.role {
            if &msg.role != role {
                return false;
            }
        }
        if let Some(ref kw) = self.keyword {
            if !msg.content.to_lowercase().contains(&kw.to_lowercase()) {
                return false;
            }
        }
        if let Some(min) = self.min_word_count {
            if msg.word_count() < min {
                return false;
            }
        }
        true
    }
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply a filter to a chat history and return matching messages.
pub fn filter_messages<'a>(history: &'a ChatHistory, filter: &MessageFilter) -> Vec<&'a ChatMessage> {
    history.messages().iter().filter(|m| filter.matches(m)).collect()
}


// ---------------------------------------------------------------------------
// InlineChatSuggestionApply — apply/preview inline chat suggestions
// ---------------------------------------------------------------------------

/// Describes how a suggestion was applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// The suggestion was fully applied.
    Applied,
    /// The suggestion was partially applied (some edits conflicted).
    PartiallyApplied { applied: usize, skipped: usize },
    /// The suggestion could not be applied.
    Failed,
}

impl fmt::Display for ApplyOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Applied => write!(f, "applied"),
            Self::PartiallyApplied { applied, skipped } => {
                write!(f, "partially applied ({applied} ok, {skipped} skipped)")
            }
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Tracks the application of an inline-chat suggestion to the document.
#[derive(Debug, Clone)]
pub struct InlineChatSuggestionApply {
    pub edits: Vec<InlineChatEdit>,
    pub outcome: ApplyOutcome,
    pub original_lines: Vec<String>,
}

impl InlineChatSuggestionApply {
    /// Create a new apply record.
    pub fn new(edits: Vec<InlineChatEdit>, original_lines: Vec<String>) -> Self {
        Self {
            edits,
            outcome: ApplyOutcome::Applied,
            original_lines,
        }
    }

    /// Simulate applying edits to a flat text buffer.
    /// Returns the resulting text if all edits are non-overlapping insertions.
    pub fn simulate_apply(&self, source: &str) -> Result<String, InlineChatError> {
        let src_lines: Vec<&str> = source.lines().collect();
        let mut result_lines: Vec<String> = src_lines.iter().map(|s| s.to_string()).collect();

        // Apply edits in reverse line order to avoid index shifting
        let mut sorted_edits = self.edits.clone();
        sorted_edits.sort_by(|a, b| b.start_line.cmp(&a.start_line));

        for edit in &sorted_edits {
            let start = edit.start_line as usize;
            let end = edit.end_line as usize;
            if end >= result_lines.len() {
                return Err(InlineChatError::NoActiveRequest);
            }
            let new_lines: Vec<String> = if edit.new_text.is_empty() {
                Vec::new()
            } else {
                edit.new_text.lines().map(|l| l.to_string()).collect()
            };
            let range_len = end - start + 1;
            result_lines.splice(start..start + range_len, new_lines);
        }

        Ok(result_lines.join("
"))
    }

    /// The number of edits that were applied.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    /// Whether the outcome was successful (fully or partially applied).
    pub fn is_success(&self) -> bool {
        !matches!(self.outcome, ApplyOutcome::Failed)
    }

    /// Total number of new-text lines across all edits.
    pub fn total_new_lines(&self) -> usize {
        self.edits.iter().map(|e| e.new_text_line_count()).sum()
    }
}

// ---------------------------------------------------------------------------
// InlineChatUndoStack — undo/redo for inline chat changes
// ---------------------------------------------------------------------------

/// A single undo entry representing the state before an inline chat change.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub description: String,
    pub original_text: String,
    pub applied_text: String,
    pub edit_count: usize,
}

/// An undo/redo stack for inline chat operations.
#[derive(Debug, Clone)]
pub struct InlineChatUndoStack {
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    max_depth: usize,
}

impl InlineChatUndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth,
        }
    }

    /// Push a new entry onto the undo stack, clearing the redo stack.
    pub fn push(&mut self, entry: UndoEntry) {
        if self.undo_stack.len() >= self.max_depth {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(entry);
        self.redo_stack.clear();
    }

    /// Undo the last operation and return the entry.
    pub fn undo(&mut self) -> Option<UndoEntry> {
        if let Some(entry) = self.undo_stack.pop() {
            self.redo_stack.push(entry.clone());
            Some(entry)
        } else {
            None
        }
    }

    /// Redo the last undone operation.
    pub fn redo(&mut self) -> Option<UndoEntry> {
        if let Some(entry) = self.redo_stack.pop() {
            self.undo_stack.push(entry.clone());
            Some(entry)
        } else {
            None
        }
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of entries on the undo stack.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of entries on the redo stack.
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear both stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for InlineChatUndoStack {
    fn default() -> Self {
        Self::new(50)
    }
}

// ---------------------------------------------------------------------------
// InlineChatDiffPreview — show a diff between original and suggestion
// ---------------------------------------------------------------------------

/// A single line in a diff view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Unchanged context line.
    Context,
    /// Line was added by the suggestion.
    Added,
    /// Line was removed by the suggestion.
    Removed,
}

/// A line in the diff preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub line_number: Option<usize>,
}

/// A computed diff between original and suggested text.
#[derive(Debug, Clone)]
pub struct InlineChatDiffPreview {
    pub lines: Vec<DiffLine>,
}

impl InlineChatDiffPreview {
    /// Compute a simple line-by-line diff between two texts.
    pub fn compute(original: &str, suggested: &str) -> Self {
        let orig_lines: Vec<&str> = original.lines().collect();
        let sugg_lines: Vec<&str> = suggested.lines().collect();
        let mut diff_lines = Vec::new();

        let max_len = orig_lines.len().max(sugg_lines.len());
        for i in 0..max_len {
            match (orig_lines.get(i), sugg_lines.get(i)) {
                (Some(o), Some(s)) if *o == *s => {
                    diff_lines.push(DiffLine {
                        kind: DiffLineKind::Context,
                        content: o.to_string(),
                        line_number: Some(i + 1),
                    });
                }
                (Some(o), Some(s)) => {
                    diff_lines.push(DiffLine {
                        kind: DiffLineKind::Removed,
                        content: o.to_string(),
                        line_number: Some(i + 1),
                    });
                    diff_lines.push(DiffLine {
                        kind: DiffLineKind::Added,
                        content: s.to_string(),
                        line_number: Some(i + 1),
                    });
                }
                (Some(o), None) => {
                    diff_lines.push(DiffLine {
                        kind: DiffLineKind::Removed,
                        content: o.to_string(),
                        line_number: Some(i + 1),
                    });
                }
                (None, Some(s)) => {
                    diff_lines.push(DiffLine {
                        kind: DiffLineKind::Added,
                        content: s.to_string(),
                        line_number: None,
                    });
                }
                (None, None) => {}
            }
        }

        Self { lines: diff_lines }
    }

    /// Number of added lines.
    pub fn additions(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffLineKind::Added).count()
    }

    /// Number of removed lines.
    pub fn deletions(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffLineKind::Removed).count()
    }

    /// Number of context (unchanged) lines.
    pub fn context_lines(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffLineKind::Context).count()
    }

    /// Whether the diff contains any changes.
    pub fn has_changes(&self) -> bool {
        self.additions() > 0 || self.deletions() > 0
    }

    /// Total number of lines in the diff.
    pub fn total_lines(&self) -> usize {
        self.lines.len()
    }
}

// ---------------------------------------------------------------------------
// InlineChatAcceptRejectTracker — track accept/reject decisions
// ---------------------------------------------------------------------------

/// The decision made about an inline chat suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionDecision {
    Accepted,
    Rejected,
    Cancelled,
}

impl fmt::Display for SuggestionDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected => write!(f, "rejected"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A record of a decision made about a suggestion.
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    pub prompt: String,
    pub decision: SuggestionDecision,
    pub edit_count: usize,
}

/// Tracks accept/reject decisions for inline chat suggestions.
#[derive(Debug, Clone, Default)]
pub struct AcceptRejectTracker {
    records: Vec<DecisionRecord>,
}

impl AcceptRejectTracker {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    /// Record a decision.
    pub fn record(&mut self, prompt: String, decision: SuggestionDecision, edit_count: usize) {
        self.records.push(DecisionRecord { prompt, decision, edit_count });
    }

    /// Total number of decisions.
    pub fn total(&self) -> usize {
        self.records.len()
    }

    /// Number of accepted suggestions.
    pub fn accepted_count(&self) -> usize {
        self.records.iter().filter(|r| r.decision == SuggestionDecision::Accepted).count()
    }

    /// Number of rejected suggestions.
    pub fn rejected_count(&self) -> usize {
        self.records.iter().filter(|r| r.decision == SuggestionDecision::Rejected).count()
    }

    /// Number of cancelled suggestions.
    pub fn cancelled_count(&self) -> usize {
        self.records.iter().filter(|r| r.decision == SuggestionDecision::Cancelled).count()
    }

    /// Acceptance rate as a percentage (0.0 – 100.0).
    pub fn acceptance_rate(&self) -> f64 {
        let decided = self.accepted_count() + self.rejected_count();
        if decided == 0 {
            return 0.0;
        }
        (self.accepted_count() as f64 / decided as f64) * 100.0
    }

    /// All records.
    pub fn records(&self) -> &[DecisionRecord] {
        &self.records
    }

    /// Clear all records.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> InlineChatRequest {
        InlineChatRequest {
            prompt: "refactor this".into(),
            selection_start_line: 10,
            selection_end_line: 20,
            uri: "main.rs".into(),
        }
    }

    fn sample_response() -> InlineChatResponse {
        InlineChatResponse {
            text: "refactored".into(),
            edits: vec![InlineChatEdit {
                start_line: 10,
                start_col: 0,
                end_line: 20,
                end_col: 0,
                new_text: "new code".into(),
            }],
        }
    }

    #[test]
    fn request_response_flow() {
        let mut w = InlineChatWidget::new();
        assert_eq!(*w.get_state(), InlineChatState::Idle);
        w.start_request(sample_request());
        assert_eq!(*w.get_state(), InlineChatState::Waiting);
        w.set_response(sample_response());
        assert_eq!(*w.get_state(), InlineChatState::Done);
    }

    #[test]
    fn accept_resets() {
        let mut w = InlineChatWidget::new();
        w.start_request(sample_request());
        w.set_response(sample_response());
        w.accept();
        assert_eq!(*w.get_state(), InlineChatState::Idle);
    }

    #[test]
    fn reject_resets() {
        let mut w = InlineChatWidget::new();
        w.start_request(sample_request());
        w.set_response(sample_response());
        w.reject();
        assert_eq!(*w.get_state(), InlineChatState::Idle);
    }

    #[test]
    fn display_state() {
        assert_eq!(InlineChatState::Idle.to_string(), "Idle");
        assert_eq!(InlineChatState::Waiting.to_string(), "Waiting");
        assert_eq!(InlineChatState::Streaming.to_string(), "Streaming");
        assert_eq!(InlineChatState::Done.to_string(), "Done");
        assert_eq!(InlineChatState::Error.to_string(), "Error");
    }

    #[test]
    fn display_request() {
        let r = sample_request();
        assert_eq!(r.to_string(), "refactor this (lines 10-20)");
    }

    #[test]
    fn display_error() {
        assert_eq!(InlineChatError::NoActiveRequest.to_string(), "no active request");
        assert_eq!(InlineChatError::AlreadyStreaming.to_string(), "already streaming");
        assert_eq!(InlineChatError::RequestCancelled.to_string(), "request cancelled");
    }

    #[test]
    fn get_request_and_response() {
        let mut w = InlineChatWidget::new();
        assert!(w.get_request().is_none());
        assert!(w.get_response().is_none());
        w.start_request(sample_request());
        assert!(w.get_request().is_some());
        assert_eq!(w.get_request().unwrap().prompt, "refactor this");
        w.set_response(sample_response());
        assert!(w.get_response().is_some());
        assert_eq!(w.get_response().unwrap().text, "refactored");
    }

    #[test]
    fn cancel_resets_to_idle() {
        let mut w = InlineChatWidget::new();
        w.start_request(sample_request());
        assert!(w.is_active());
        w.cancel();
        assert_eq!(*w.get_state(), InlineChatState::Idle);
        assert!(!w.is_active());
        assert!(w.get_request().is_none());
    }

    #[test]
    fn is_active_reflects_state() {
        let mut w = InlineChatWidget::new();
        assert!(!w.is_active());
        w.start_request(sample_request());
        assert!(w.is_active());
        w.set_response(sample_response());
        assert!(w.is_active());
        w.accept();
        assert!(!w.is_active());
    }

    #[test]
    fn streaming_flow() {
        let mut w = InlineChatWidget::new();
        w.start_request(sample_request());
        assert!(w.start_streaming().is_ok());
        assert_eq!(*w.get_state(), InlineChatState::Streaming);
        assert!(w.append_streaming("hello ").is_ok());
        assert!(w.append_streaming("world").is_ok());
        assert_eq!(w.get_response().unwrap().text, "hello world");
    }

    #[test]
    fn start_streaming_errors() {
        let mut w = InlineChatWidget::new();
        // No request active
        assert_eq!(w.start_streaming(), Err(InlineChatError::NoActiveRequest));
        w.start_request(sample_request());
        assert!(w.start_streaming().is_ok());
        // Already streaming
        assert_eq!(w.start_streaming(), Err(InlineChatError::AlreadyStreaming));
    }

    #[test]
    fn append_streaming_without_streaming_errors() {
        let mut w = InlineChatWidget::new();
        assert_eq!(w.append_streaming("text"), Err(InlineChatError::NoActiveRequest));
    }

    #[test]
    fn edit_count() {
        let mut w = InlineChatWidget::new();
        assert_eq!(w.edit_count(), 0);
        w.start_request(sample_request());
        w.set_response(sample_response());
        assert_eq!(w.edit_count(), 1);
    }

    #[test]
    fn history_tracking() {
        let mut h = InlineChatHistory::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        h.push(sample_request(), sample_response());
        assert_eq!(h.len(), 1);
        assert!(!h.is_empty());
        assert_eq!(h.entries()[0].request.prompt, "refactor this");
        h.push(sample_request(), sample_response());
        assert_eq!(h.len(), 2);
        h.clear();
        assert!(h.is_empty());
    }

    #[test]
    fn default_widget() {
        let w = InlineChatWidget::default();
        assert_eq!(*w.get_state(), InlineChatState::Idle);
        assert!(!w.is_active());
        assert_eq!(w.edit_count(), 0);
    }

    #[test]
    fn inline_chat_edit_new() {
        let e = InlineChatEdit::new(1, 0, 5, 10, "replacement");
        assert_eq!(e.start_line, 1);
        assert_eq!(e.end_col, 10);
        assert_eq!(e.new_text, "replacement");
    }

    #[test]
    fn edit_original_line_span() {
        let e = InlineChatEdit::new(3, 0, 7, 0, "x");
        assert_eq!(e.original_line_span(), 5);
    }

    #[test]
    fn edit_new_text_line_count() {
        let e = InlineChatEdit::new(0, 0, 0, 0, "a\nb\nc");
        assert_eq!(e.new_text_line_count(), 3);
        let e2 = InlineChatEdit::new(0, 0, 0, 5, "");
        assert_eq!(e2.new_text_line_count(), 0);
    }

    #[test]
    fn edit_is_insertion_deletion() {
        let ins = InlineChatEdit::new(1, 5, 1, 5, "text");
        assert!(ins.is_insertion());
        assert!(!ins.is_deletion());
        let del = InlineChatEdit::new(1, 0, 1, 5, "");
        assert!(del.is_deletion());
        assert!(!del.is_insertion());
    }

    #[test]
    fn response_empty() {
        let r = InlineChatResponse::empty();
        assert!(!r.has_edits());
        assert_eq!(r.word_count(), 0);
    }

    #[test]
    fn response_total_lines_affected() {
        let r = InlineChatResponse {
            text: "done".into(),
            edits: vec![
                InlineChatEdit::new(1, 0, 3, 0, "x"),
                InlineChatEdit::new(5, 0, 5, 0, "y"),
            ],
        };
        assert_eq!(r.total_lines_affected(), 4);
    }

    #[test]
    fn response_word_count() {
        let r = InlineChatResponse {
            text: "hello world foo bar".into(),
            edits: Vec::new(),
        };
        assert_eq!(r.word_count(), 4);
    }

    #[test]
    fn request_selection_line_count() {
        let r = sample_request();
        assert_eq!(r.selection_line_count(), 11);
    }

    #[test]
    fn request_is_single_line() {
        let r = InlineChatRequest {
            prompt: "fix".into(),
            selection_start_line: 5,
            selection_end_line: 5,
            uri: "f.rs".into(),
        };
        assert!(r.is_single_line());
        assert!(!sample_request().is_single_line());
    }

    #[test]
    fn request_prompt_word_count() {
        let r = sample_request();
        assert_eq!(r.prompt_word_count(), 2);
    }

    #[test]
    fn widget_set_error() {
        let mut w = InlineChatWidget::new();
        w.start_request(sample_request());
        w.set_error();
        assert_eq!(*w.get_state(), InlineChatState::Error);
    }

    #[test]
    fn widget_finish_streaming() {
        let mut w = InlineChatWidget::new();
        w.start_request(sample_request());
        w.start_streaming().unwrap();
        w.append_streaming("data").unwrap();
        assert!(w.finish_streaming().is_ok());
        assert_eq!(*w.get_state(), InlineChatState::Done);
    }

    #[test]
    fn widget_finish_streaming_error() {
        let mut w = InlineChatWidget::new();
        assert!(w.finish_streaming().is_err());
    }

    #[test]
    fn widget_summary() {
        let mut w = InlineChatWidget::new();
        w.start_request(sample_request());
        let s = w.summary();
        assert!(s.contains("Waiting"));
        assert!(s.contains("refactor this"));
    }

    #[test]
    fn history_last() {
        let mut h = InlineChatHistory::new();
        assert!(h.last().is_none());
        h.push(sample_request(), sample_response());
        assert!(h.last().is_some());
    }

    #[test]
    fn history_search() {
        let mut h = InlineChatHistory::new();
        h.push(sample_request(), sample_response());
        h.push(
            InlineChatRequest { prompt: "optimize query".into(), selection_start_line: 1, selection_end_line: 5, uri: "q.rs".into() },
            InlineChatResponse::empty(),
        );
        assert_eq!(h.search("refactor").len(), 1);
        assert_eq!(h.search("optimize").len(), 1);
        assert_eq!(h.search("nothing").len(), 0);
    }

    #[test]
    fn history_total_edits() {
        let mut h = InlineChatHistory::new();
        h.push(sample_request(), sample_response());
        h.push(sample_request(), InlineChatResponse::empty());
        assert_eq!(h.total_edits(), 1);
    }

    #[test]
    fn history_get_by_index() {
        let mut h = InlineChatHistory::new();
        h.push(sample_request(), sample_response());
        assert!(h.get(0).is_some());
        assert!(h.get(1).is_none());
    }

    #[test]
    fn inline_chat_edit_equality() {
        let a = InlineChatEdit::new(1, 0, 5, 10, "x");
        let b = InlineChatEdit::new(1, 0, 5, 10, "x");
        let c = InlineChatEdit::new(1, 0, 5, 10, "y");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn request_equality() {
        let a = sample_request();
        let b = sample_request();
        assert_eq!(a, b);
    }

    #[test]
    fn streaming_state_tracking() {
        let mut ss = StreamingState::new();
        assert_eq!(ss.chunks_received, 0);
        assert_eq!(ss.total_bytes, 0);
        assert!(!ss.is_complete);
        ss.add_chunk(100);
        ss.add_chunk(200);
        assert_eq!(ss.chunks_received, 2);
        assert_eq!(ss.total_bytes, 300);
        ss.complete();
        assert!(ss.is_complete);
    }

    #[test]
    fn conversation_thread_management() {
        let mut thread = ConversationThread::new("t1", "initial prompt");
        assert!(thread.is_empty());
        assert_eq!(thread.follow_up_count(), 0);
        thread.add_follow_up(sample_request());
        assert_eq!(thread.follow_up_count(), 1);
        assert!(!thread.is_empty());
        assert_eq!(thread.thread_id, "t1");
        assert_eq!(thread.parent_prompt, "initial prompt");
    }

    #[test]
    fn search_history_entries_by_response() {
        let mut h = InlineChatHistory::new();
        h.push(
            sample_request(),
            InlineChatResponse { text: "optimized result".into(), edits: Vec::new() },
        );
        h.push(
            InlineChatRequest { prompt: "other".into(), selection_start_line: 1, selection_end_line: 2, uri: "x.rs".into() },
            InlineChatResponse::empty(),
        );
        let results = search_history_entries(&h, "optimized");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].index, 0);
    }

    #[test]
    fn search_history_entries_no_match() {
        let mut h = InlineChatHistory::new();
        h.push(sample_request(), sample_response());
        let results = search_history_entries(&h, "nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn streaming_state_default() {
        let ss = StreamingState::default();
        assert_eq!(ss.chunks_received, 0);
        assert!(!ss.is_complete);
    }

    // -- InlineChatDiff tests ---------------------------------------------

    #[test]
    fn diff_identical_strings() {
        let diff = InlineChatDiff::compute("hello world", "hello world");
        assert!(diff.is_identical());
        assert_eq!(diff.unchanged_chars, 11);
    }

    #[test]
    fn diff_completely_different() {
        let diff = InlineChatDiff::compute("aaa", "bbb");
        assert_eq!(diff.unchanged_chars, 0);
        assert_eq!(diff.removed_chars, 3);
        assert_eq!(diff.added_chars, 3);
    }

    #[test]
    fn diff_insertion_only() {
        let diff = InlineChatDiff::compute("ab", "axb");
        assert!(diff.added_chars >= 1);
        assert_eq!(diff.unchanged_chars, 2);
        assert_eq!(diff.removed_chars, 0);
    }

    #[test]
    fn diff_deletion_only() {
        let diff = InlineChatDiff::compute("abc", "ac");
        assert!(diff.removed_chars >= 1);
        assert_eq!(diff.added_chars, 0);
    }

    #[test]
    fn diff_total_changes() {
        let diff = InlineChatDiff::compute("hello", "help");
        assert!(diff.total_changes() > 0);
    }

    #[test]
    fn diff_empty_strings() {
        let diff = InlineChatDiff::compute("", "");
        assert!(diff.is_identical());
        assert_eq!(diff.total_changes(), 0);
    }

    // -----------------------------------------------------------------------
    // InlineChatSession tests
    // -----------------------------------------------------------------------

    #[test]
    fn session_new_is_idle() {
        let session = InlineChatSession::new("s1");
        assert_eq!(session.session_id, "s1");
        assert!(session.is_idle());
        assert_eq!(session.prompt_count(), 0);
        assert!(session.current_prompt().is_none());
    }

    #[test]
    fn session_submit_and_complete() {
        let mut session = InlineChatSession::new("s2");
        session.submit_prompt(sample_request());
        assert!(!session.is_idle());
        assert_eq!(session.current_prompt(), Some("refactor this"));
        assert_eq!(session.prompt_count(), 1);

        session.complete(sample_response());
        assert_eq!(session.history.len(), 1);
        assert_eq!(
            session.history.last().unwrap().response.text,
            "refactored"
        );
    }

    #[test]
    fn session_accept_and_reject() {
        let mut session = InlineChatSession::new("s3");
        session.submit_prompt(sample_request());
        session.complete(sample_response());
        session.accept();
        assert!(session.is_idle());

        session.submit_prompt(sample_request());
        session.complete(sample_response());
        session.reject();
        assert!(session.is_idle());
    }

    #[test]
    fn session_retry_resubmits_last_prompt() {
        let mut session = InlineChatSession::new("s4");
        session.submit_prompt(sample_request());
        session.retry();
        // Widget should still be active (re-submitted).
        assert!(!session.is_idle());
        assert_eq!(session.current_prompt(), Some("refactor this"));
    }

    // -----------------------------------------------------------------------
    // InlineDiffPreview tests
    // -----------------------------------------------------------------------

    #[test]
    fn diff_preview_identical() {
        let preview = InlineDiffPreview::compute("aaa\nbbb\nccc", "aaa\nbbb\nccc");
        assert!(!preview.has_changes());
        assert_eq!(preview.hunk_count(), 0);
        assert_eq!(preview.total_additions(), 0);
        assert_eq!(preview.total_removals(), 0);
    }

    #[test]
    fn diff_preview_single_line_change() {
        let preview = InlineDiffPreview::compute("aaa\nbbb\nccc", "aaa\nBBB\nccc");
        assert!(preview.has_changes());
        assert_eq!(preview.hunk_count(), 1);
        assert_eq!(preview.total_additions(), 1);
        assert_eq!(preview.total_removals(), 1);
        assert_eq!(preview.diff_hunks[0].start_line, 1);
        assert_eq!(preview.diff_hunks[0].removed, vec!["bbb"]);
        assert_eq!(preview.diff_hunks[0].added, vec!["BBB"]);
    }

    #[test]
    fn diff_preview_summary() {
        let preview = InlineDiffPreview::compute("a\nb\nc", "a\nX\nY\nc");
        let s = preview.summary();
        assert!(s.contains("hunk(s)"));
        assert!(s.contains('+'));
        assert!(s.contains('-'));
    }

    #[test]
    fn diff_preview_addition_only() {
        let preview = InlineDiffPreview::compute("a\nb", "a\nb\nc");
        assert!(preview.has_changes());
        assert!(preview.total_additions() >= 1);
    }

    // -----------------------------------------------------------------------
    // InlineChatAction tests
    // -----------------------------------------------------------------------

    #[test]
    fn action_is_terminal() {
        assert!(InlineChatAction::Accept.is_terminal());
        assert!(InlineChatAction::Reject.is_terminal());
        assert!(!InlineChatAction::Retry.is_terminal());
        assert!(!InlineChatAction::Edit("fix".into()).is_terminal());
        assert!(!InlineChatAction::AcceptPartial { hunk_index: 0 }.is_terminal());
    }

    #[test]
    fn action_display() {
        assert_eq!(format!("{}", InlineChatAction::Accept), "Accept");
        assert_eq!(format!("{}", InlineChatAction::Reject), "Reject");
        assert_eq!(format!("{}", InlineChatAction::Retry), "Retry");
        assert_eq!(
            format!("{}", InlineChatAction::Edit("new prompt".into())),
            "Edit(new prompt)"
        );
        assert_eq!(
            format!("{}", InlineChatAction::AcceptPartial { hunk_index: 2 }),
            "AcceptPartial(hunk 2)"
        );
    }

    #[test]
    fn action_description() {
        assert!(!InlineChatAction::Accept.description().is_empty());
        assert!(!InlineChatAction::Retry.description().is_empty());
        assert!(!InlineChatAction::AcceptPartial { hunk_index: 0 }
            .description()
            .is_empty());
    }

    // -- ChatHistory tests ---------------------------------------------------

    #[test]
    fn chat_history_push_and_query() {
        let mut history = ChatHistory::new();
        history.push(ChatMessage::new(ChatRole::User, "hello", 100));
        history.push(ChatMessage::new(ChatRole::Assistant, "hi there", 200));
        assert_eq!(history.len(), 2);
        assert_eq!(history.turn_count(), 1);
        assert_eq!(history.messages_by_role(&ChatRole::User).len(), 1);
        assert_eq!(history.last().unwrap().role, ChatRole::Assistant);
    }

    #[test]
    fn chat_history_with_max_evicts_old() {
        let mut history = ChatHistory::with_max(2);
        history.push(ChatMessage::new(ChatRole::User, "a", 1));
        history.push(ChatMessage::new(ChatRole::User, "b", 2));
        history.push(ChatMessage::new(ChatRole::User, "c", 3));
        assert_eq!(history.len(), 2);
        assert_eq!(history.messages()[0].content, "b");
    }

    // -- ChatMessageFormatter tests ------------------------------------------

    #[test]
    fn formatter_formats_message() {
        let formatter = ChatMessageFormatter::new(80);
        let msg = ChatMessage::new(ChatRole::User, "hello world", 0);
        let formatted = formatter.format(&msg);
        assert!(formatted.contains("You:"));
        assert!(formatted.contains("hello"));
    }

    #[test]
    fn formatter_with_timestamps() {
        let mut formatter = ChatMessageFormatter::new(80);
        formatter.show_timestamps = true;
        let msg = ChatMessage::new(ChatRole::System, "init", 42);
        let formatted = formatter.format(&msg);
        assert!(formatted.contains("[42ms]"));
        assert!(formatted.contains("System:"));
    }

    // -- StreamSimulator tests -----------------------------------------------

    #[test]
    fn stream_chunks_and_progress() {
        let mut stream = StreamSimulator::new("hello world", 5);
        assert_eq!(stream.next_chunk(), Some("hello"));
        assert!(!stream.is_done());
        assert!((stream.progress() - 5.0 / 11.0).abs() < 0.01);
        assert_eq!(stream.next_chunk(), Some(" worl"));
        assert_eq!(stream.next_chunk(), Some("d"));
        assert!(stream.is_done());
        assert_eq!(stream.next_chunk(), None);
    }

    #[test]
    fn stream_collect_remaining() {
        let mut stream = StreamSimulator::new("abcdef", 2);
        let _ = stream.next_chunk(); // consume "ab"
        let rest = stream.collect_remaining();
        assert_eq!(rest, "cdef");
        assert!(stream.is_done());
    }

    #[test]
    fn total_history_edits_sums() {
        let mut history = InlineChatHistory::new();
        history.push(
            sample_request(),
            InlineChatResponse {
                text: "r1".into(),
                edits: vec![InlineChatEdit::new(0, 0, 1, 0, "a")],
            },
        );
        history.push(
            sample_request(),
            InlineChatResponse {
                text: "r2".into(),
                edits: vec![InlineChatEdit::new(0, 0, 0, 5, "b"), InlineChatEdit::new(1, 0, 2, 0, "c")],
            },
        );
        assert_eq!(total_history_edits(&history), 3);
    }

    #[test]
    fn total_history_edits_empty() {
        let history = InlineChatHistory::new();
        assert_eq!(total_history_edits(&history), 0);
    }

    #[test]
    fn total_history_words_sums() {
        let mut history = InlineChatHistory::new();
        history.push(
            sample_request(),
            InlineChatResponse { text: "one two three".into(), edits: vec![] },
        );
        history.push(
            sample_request(),
            InlineChatResponse { text: "four five".into(), edits: vec![] },
        );
        assert_eq!(total_history_words(&history), 5);
    }

    #[test]
    fn search_history_prompts_finds() {
        let mut history = InlineChatHistory::new();
        let req1 = InlineChatRequest {
            prompt: "Refactor this function".into(),
            selection_start_line: 0,
            selection_end_line: 5,
            uri: "file:///a.rs".into(),
        };
        let req2 = InlineChatRequest {
            prompt: "Add tests".into(),
            selection_start_line: 0,
            selection_end_line: 10,
            uri: "file:///b.rs".into(),
        };
        history.push(req1, sample_response());
        history.push(req2, sample_response());
        assert_eq!(search_history_prompts(&history, "refactor").len(), 1);
        assert_eq!(search_history_prompts(&history, "TESTS").len(), 1);
        assert_eq!(search_history_prompts(&history, "delete").len(), 0);
    }

    #[test]
    fn average_edits_per_response_computes() {
        let history = InlineChatHistory::new();
        assert_eq!(average_edits_per_response(&history), 0.0);

        let mut h2 = InlineChatHistory::new();
        h2.push(sample_request(), InlineChatResponse { text: "r".into(), edits: vec![InlineChatEdit::new(0, 0, 1, 0, "a"), InlineChatEdit::new(2, 0, 3, 0, "b")] });
        h2.push(sample_request(), InlineChatResponse { text: "r".into(), edits: vec![] });
        assert!((average_edits_per_response(&h2) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unique_history_uris_deduplicates() {
        let mut history = InlineChatHistory::new();
        let req_a = InlineChatRequest { prompt: "p".into(), selection_start_line: 0, selection_end_line: 1, uri: "file:///a.rs".into() };
        let req_b = InlineChatRequest { prompt: "p".into(), selection_start_line: 0, selection_end_line: 1, uri: "file:///b.rs".into() };
        let req_a2 = InlineChatRequest { prompt: "p".into(), selection_start_line: 0, selection_end_line: 1, uri: "file:///a.rs".into() };
        history.push(req_a, sample_response());
        history.push(req_b, sample_response());
        history.push(req_a2, sample_response());
        let uris = unique_history_uris(&history);
        assert_eq!(uris.len(), 2);
    }

    #[test]
    fn edit_overlaps_range_check() {
        let edit = InlineChatEdit::new(5, 0, 10, 0, "x");
        assert!(edit_overlaps_range(&edit, 3, 6));
        assert!(edit_overlaps_range(&edit, 10, 15));
        assert!(!edit_overlaps_range(&edit, 11, 15));
        assert!(!edit_overlaps_range(&edit, 0, 4));
    }

    #[test]
    fn edits_in_selection_filters() {
        let response = InlineChatResponse {
            text: "resp".into(),
            edits: vec![
                InlineChatEdit::new(1, 0, 3, 0, "a"),
                InlineChatEdit::new(5, 0, 7, 0, "b"),
                InlineChatEdit::new(10, 0, 12, 0, "c"),
            ],
        };
        assert_eq!(edits_in_selection(&response, 4, 8).len(), 1);
        assert_eq!(edits_in_selection(&response, 0, 100).len(), 3);
        assert_eq!(edits_in_selection(&response, 13, 20).len(), 0);
    }

    // -----------------------------------------------------------------------
    // Code block extraction tests
    // -----------------------------------------------------------------------

    #[test]
    fn extract_code_blocks_single() {
        let text = "Here is code:\n```rust\nfn main() {}\n```\nDone.";
        let blocks = extract_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("rust".to_string()));
        assert_eq!(blocks[0].code, "fn main() {}");
    }

    #[test]
    fn extract_code_blocks_multiple() {
        let text = "```python\nprint('hi')\n```\ntext\n```\nplain\n```";
        let blocks = extract_code_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].language, Some("python".to_string()));
        assert_eq!(blocks[0].code, "print('hi')");
        assert_eq!(blocks[1].language, None);
        assert_eq!(blocks[1].code, "plain");
    }

    #[test]
    fn extract_code_blocks_none() {
        let text = "No code blocks here.";
        let blocks = extract_code_blocks(text);
        assert!(blocks.is_empty());
    }

    // -----------------------------------------------------------------------
    // Suggestion tracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn suggestion_tracker_records_and_counts() {
        let mut tracker = SuggestionTracker::new();
        tracker.record(SuggestionOutcome::Accepted);
        tracker.record(SuggestionOutcome::Rejected);
        tracker.record(SuggestionOutcome::Accepted);
        tracker.record(SuggestionOutcome::Partial);
        assert_eq!(tracker.total(), 4);
        assert_eq!(tracker.count(SuggestionOutcome::Accepted), 2);
        assert_eq!(tracker.count(SuggestionOutcome::Rejected), 1);
        assert_eq!(tracker.count(SuggestionOutcome::Partial), 1);
        assert!((tracker.acceptance_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn suggestion_tracker_empty_rate() {
        let tracker = SuggestionTracker::new();
        assert_eq!(tracker.acceptance_rate(), 0.0);
    }

    // -----------------------------------------------------------------------
    // Typing indicator tests
    // -----------------------------------------------------------------------

    #[test]
    fn typing_indicator_lifecycle() {
        let mut indicator = TypingIndicator::new();
        assert!(!indicator.is_visible());
        assert_eq!(*indicator.state(), TypingIndicatorState::Hidden);

        indicator.show_thinking();
        assert!(indicator.is_visible());
        assert_eq!(*indicator.state(), TypingIndicatorState::Thinking);
        assert_eq!(indicator.elapsed_ms(), 0);

        indicator.tick(600);
        assert_eq!(indicator.elapsed_ms(), 600);

        indicator.show_generating();
        assert_eq!(*indicator.state(), TypingIndicatorState::Generating);

        indicator.hide();
        assert!(!indicator.is_visible());
        assert_eq!(indicator.elapsed_ms(), 0);
    }

    #[test]
    fn typing_indicator_render_frames() {
        let mut indicator = TypingIndicator::new();
        indicator.show_thinking();
        assert_eq!(indicator.render_frame(), "");
        indicator.tick(500);
        assert_eq!(indicator.render_frame(), ".");
        indicator.tick(500);
        assert_eq!(indicator.render_frame(), "..");
        indicator.tick(500);
        assert_eq!(indicator.render_frame(), "...");
    }

    #[test]
    fn typing_indicator_display() {
        assert_eq!(TypingIndicatorState::Hidden.to_string(), "");
        assert_eq!(TypingIndicatorState::Thinking.to_string(), "Thinking...");
        assert_eq!(TypingIndicatorState::Generating.to_string(), "Generating...");
    }

    // -----------------------------------------------------------------------
    // Message filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn message_filter_by_role() {
        let mut history = ChatHistory::new();
        history.push(ChatMessage::new(ChatRole::User, "hello world", 100));
        history.push(ChatMessage::new(ChatRole::Assistant, "hi there friend", 200));
        history.push(ChatMessage::new(ChatRole::User, "goodbye world", 300));

        let filter = MessageFilter::new().with_role(ChatRole::User);
        let results = filter_messages(&history, &filter);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content, "hello world");
    }

    #[test]
    fn message_filter_by_keyword() {
        let mut history = ChatHistory::new();
        history.push(ChatMessage::new(ChatRole::User, "fix the bug", 100));
        history.push(ChatMessage::new(ChatRole::Assistant, "done with feature", 200));
        history.push(ChatMessage::new(ChatRole::User, "another bug report", 300));

        let filter = MessageFilter::new().with_keyword("bug");
        let results = filter_messages(&history, &filter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn message_filter_combined() {
        let mut history = ChatHistory::new();
        history.push(ChatMessage::new(ChatRole::User, "short", 100));
        history.push(ChatMessage::new(ChatRole::User, "a longer message with many words", 200));
        history.push(ChatMessage::new(ChatRole::Assistant, "a longer response with many words too", 300));

        let filter = MessageFilter::new()
            .with_role(ChatRole::User)
            .with_min_words(4);
        let results = filter_messages(&history, &filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "a longer message with many words");
    }

    // --- InlineChatSuggestionApply tests ------------------------------------

    #[test]
    fn apply_outcome_display() {
        assert_eq!(format!("{}", ApplyOutcome::Applied), "applied");
        assert_eq!(format!("{}", ApplyOutcome::Failed), "failed");
        let partial = ApplyOutcome::PartiallyApplied { applied: 2, skipped: 1 };
        assert!(format!("{}", partial).contains("2 ok"));
    }

    #[test]
    fn suggestion_apply_basic() {
        let edit = InlineChatEdit::new(0, 0, 0, 5, "replaced");
        let apply = InlineChatSuggestionApply::new(
            vec![edit],
            vec!["hello world".into()],
        );
        assert!(apply.is_success());
        assert_eq!(apply.edit_count(), 1);
    }

    #[test]
    fn suggestion_apply_simulate() {
        let edit = InlineChatEdit::new(1, 0, 1, 0, "new line 2");
        let apply = InlineChatSuggestionApply::new(vec![edit], vec![]);
        let result = apply.simulate_apply("line 1
line 2
line 3");
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("new line 2"));
    }

    #[test]
    fn suggestion_apply_failed_outcome() {
        let apply = InlineChatSuggestionApply {
            edits: vec![],
            outcome: ApplyOutcome::Failed,
            original_lines: vec![],
        };
        assert!(!apply.is_success());
    }

    // --- InlineChatUndoStack tests ------------------------------------------

    #[test]
    fn undo_stack_push_and_undo() {
        let mut stack = InlineChatUndoStack::new(10);
        assert!(!stack.can_undo());
        stack.push(UndoEntry {
            description: "edit 1".into(),
            original_text: "before".into(),
            applied_text: "after".into(),
            edit_count: 1,
        });
        assert!(stack.can_undo());
        assert_eq!(stack.undo_depth(), 1);
        let entry = stack.undo().unwrap();
        assert_eq!(entry.description, "edit 1");
        assert!(!stack.can_undo());
        assert!(stack.can_redo());
    }

    #[test]
    fn undo_stack_redo() {
        let mut stack = InlineChatUndoStack::new(10);
        stack.push(UndoEntry {
            description: "e1".into(),
            original_text: "a".into(),
            applied_text: "b".into(),
            edit_count: 1,
        });
        stack.undo();
        let re = stack.redo().unwrap();
        assert_eq!(re.description, "e1");
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_max_depth() {
        let mut stack = InlineChatUndoStack::new(2);
        for i in 0..5 {
            stack.push(UndoEntry {
                description: format!("e{i}"),
                original_text: String::new(),
                applied_text: String::new(),
                edit_count: 0,
            });
        }
        assert_eq!(stack.undo_depth(), 2);
    }

    #[test]
    fn undo_stack_push_clears_redo() {
        let mut stack = InlineChatUndoStack::new(10);
        stack.push(UndoEntry { description: "a".into(), original_text: String::new(), applied_text: String::new(), edit_count: 0 });
        stack.undo();
        assert!(stack.can_redo());
        stack.push(UndoEntry { description: "b".into(), original_text: String::new(), applied_text: String::new(), edit_count: 0 });
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_clear() {
        let mut stack = InlineChatUndoStack::default();
        stack.push(UndoEntry { description: "x".into(), original_text: String::new(), applied_text: String::new(), edit_count: 0 });
        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    // --- InlineChatDiffPreview tests ----------------------------------------

    #[test]
    fn diff_identical() {
        let diff = InlineChatDiffPreview::compute("hello\nworld", "hello\nworld");
        assert!(!diff.has_changes());
        assert_eq!(diff.context_lines(), 2);
    }

    #[test]
    fn diff_addition() {
        let diff = InlineChatDiffPreview::compute("a", "a\nb");
        assert!(diff.has_changes());
        assert_eq!(diff.additions(), 1);
        assert_eq!(diff.deletions(), 0);
    }

    #[test]
    fn diff_removal() {
        let diff = InlineChatDiffPreview::compute("a\nb", "a");
        assert_eq!(diff.deletions(), 1);
    }

    #[test]
    fn diff_modification() {
        let diff = InlineChatDiffPreview::compute("old line", "new line");
        assert_eq!(diff.additions(), 1);
        assert_eq!(diff.deletions(), 1);
        assert_eq!(diff.context_lines(), 0);
    }

    // --- AcceptRejectTracker tests ------------------------------------------

    #[test]
    fn tracker_record_and_count() {
        let mut tracker = AcceptRejectTracker::new();
        tracker.record("fix bug".into(), SuggestionDecision::Accepted, 2);
        tracker.record("refactor".into(), SuggestionDecision::Rejected, 1);
        assert_eq!(tracker.total(), 2);
        assert_eq!(tracker.accepted_count(), 1);
        assert_eq!(tracker.rejected_count(), 1);
    }

    #[test]
    fn tracker_acceptance_rate() {
        let mut tracker = AcceptRejectTracker::new();
        tracker.record("a".into(), SuggestionDecision::Accepted, 1);
        tracker.record("b".into(), SuggestionDecision::Accepted, 1);
        tracker.record("c".into(), SuggestionDecision::Rejected, 1);
        let rate = tracker.acceptance_rate();
        assert!((rate - 66.666).abs() < 1.0);
    }

    #[test]
    fn tracker_empty_rate() {
        let tracker = AcceptRejectTracker::new();
        assert!((tracker.acceptance_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tracker_cancelled() {
        let mut tracker = AcceptRejectTracker::new();
        tracker.record("x".into(), SuggestionDecision::Cancelled, 0);
        assert_eq!(tracker.cancelled_count(), 1);
        assert_eq!(tracker.accepted_count(), 0);
    }

    #[test]
    fn tracker_clear() {
        let mut tracker = AcceptRejectTracker::new();
        tracker.record("a".into(), SuggestionDecision::Accepted, 1);
        tracker.clear();
        assert_eq!(tracker.total(), 0);
    }

    #[test]
    fn suggestion_decision_display() {
        assert_eq!(SuggestionDecision::Accepted.to_string(), "accepted");
        assert_eq!(SuggestionDecision::Rejected.to_string(), "rejected");
        assert_eq!(SuggestionDecision::Cancelled.to_string(), "cancelled");
    }

}
