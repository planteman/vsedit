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


// ---------------------------------------------------------------------------
// inlinechat_view – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XInlinechatViewLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XInlinechatViewPanelState {
    pub region: XInlinechatViewLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XInlinechatViewPanelState {
    pub fn new(region: XInlinechatViewLayoutRegion, label: impl Into<String>) -> Self {
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
pub fn x_inlinechat_view_total_visible_area(panels: &[XInlinechatViewPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_inlinechat_view_count_in_region(
    panels: &[XInlinechatViewPanelState],
    region: XInlinechatViewLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_inlinechat_view_widest_panel(panels: &[XInlinechatViewPanelState]) -> Option<&XInlinechatViewPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_inlinechat_view_collapse_region(
    panels: &mut [XInlinechatViewPanelState],
    region: XInlinechatViewLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XInlinechatViewLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XInlinechatViewLayoutConstraint {
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
// inlinechat_view – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for inline chat widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YInlinechatViewInlineChatMode {
    Ask,
    Edit,
    Generate,
    Explain,
}

impl YInlinechatViewInlineChatMode {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Ask => 0,
            Self::Edit => 1,
            Self::Generate => 2,
            Self::Explain => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::Edit => "Edit",
            Self::Generate => "Generate",
            Self::Explain => "Explain",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YInlinechatViewInlineChatMode] {
        &[
            YInlinechatViewInlineChatMode::Ask,
            YInlinechatViewInlineChatMode::Edit,
            YInlinechatViewInlineChatMode::Generate,
            YInlinechatViewInlineChatMode::Explain,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YInlinechatViewInlineChatMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks chat history data.
#[derive(Debug, Clone)]
pub struct YInlinechatViewInlineChatHistory {
    pub messages: Vec<(bool, String)>,
    pub max_messages: usize,
    pub session_id: String,
}

impl YInlinechatViewInlineChatHistory {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_messages: 0,
            session_id: String::new(),
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
        format!("YInlinechatViewInlineChatHistory({}: {:?})", "messages", self.messages)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_inlinechat_view_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_inlinechat_view_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_inlinechat_view_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_inlinechat_view_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_inlinechat_view_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_inlinechat_view_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_inlinechat_view_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_inlinechat_view_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// inlinechat_view – Extended inline chat context helpers
// ---------------------------------------------------------------------------

/// Priority levels for inline chat context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZInlinechatViewPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZInlinechatViewPriority {
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
    pub fn all_asc() -> [ZInlinechatViewPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZInlinechatViewPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks inline chat context data.
#[derive(Debug, Clone)]
pub struct ZInlinechatViewInlineChatContext {
    pub context_lines: Vec<(u32, String)>,
    pub language_id: String,
    pub max_context: usize,
}

impl ZInlinechatViewInlineChatContext {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            context_lines: Vec::new(),
            language_id: String::new(),
            max_context: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.context_lines.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.context_lines.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.context_lines.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZInlinechatViewInlineChatContext[language_id={:?}, max_context={:?}]", self.language_id, self.max_context)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for inline chat context.
pub fn z_inlinechat_view_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_inlinechat_view_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_inlinechat_view_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_inlinechat_view_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_inlinechat_view_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_inlinechat_view_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_inlinechat_view_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 78
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer78 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer78 {
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
pub fn xb_fnv1a_78(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_78<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_78<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_78(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_78(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 95
// ---------------------------------------------------------------------------

/// Generic object pool `Xc95Pool<T>`.
pub struct Xc95Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc95Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc95PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc95Pool<T> {
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
    pub fn stats(&self) -> Xc95PoolStats {
        Xc95PoolStats {
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

impl<T> Default for Xc95Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc95Scheduler`.
pub struct Xc95Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc95Scheduler {
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

impl Default for Xc95Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_95 hash for the given byte slice.
pub fn xc_95_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_95 convention.
pub fn xc_95_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe91 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe91Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe91PipelineError {
    pub stage: Xe91Stage,
    pub message: String,
}

impl std::fmt::Display for Xe91PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe91Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe91Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError>>>,
    stage_names: Vec<Xe91Stage>,
}

impl Xe91Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe91Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe91Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe91Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe91Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> {
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

    pub fn compose(mut self, other: Xe91Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe91CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe91CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe91Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe91CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe91CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe91Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe91CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_91_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe91CacheEntry {
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

    fn xe_91_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe91CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_91_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> {
    Ok(data)
}

pub fn xe_91_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_91_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_91_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_91_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe91PipelineError> {
    Err(Xe91PipelineError {
        stage: Xe91Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_89: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg89Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg89Graph {
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

impl Default for Xg89Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_89: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg89Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg89Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg89Heap<T>) {
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

impl<T: Ord> Default for Xg89Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 94).
pub struct Xh94SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh94SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 136 as u64,
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

/// A compact bit set supporting boolean operations (variant 94).
pub struct Xh94BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh94BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 94).
pub struct Xi94Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi94Deque<T> {
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
pub struct Xi94Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi94Interval {
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

/// A simple interval tree (variant 94).
pub struct Xi94IntervalTree {
    xi_intervals: Vec<Xi94Interval>,
}

impl Xi94IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi94Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi94Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi94Interval) -> Vec<&Xi94Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi94Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi94Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi94Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi94Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi94Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi94Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 93) ---

/// Disjoint set / union-find for crate 93.
pub struct Xj93UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj93UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ93_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 93.
pub struct Xj93BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj93BTreeNode<K, V>>>,
    len: usize,
}

struct Xj93BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj93BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj93BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ93_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ93_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj93BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj93BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj93BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj93BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_94 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk94SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk94SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk94DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk94DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_93).
#[derive(Debug, Clone)]
pub struct Xl93Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl93Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_93).
#[derive(Debug, Clone)]
pub struct Xl93SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl93SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
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
    fn edit_count_works() {
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


    // -- inlinechat_view additional tests -------------------------------------------

    #[test]
    fn x_inlinechat_view_panel_state_new() {
        let p = XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XInlinechatViewLayoutRegion::Sidebar);
    }

    #[test]
    fn x_inlinechat_view_panel_area() {
        let p = XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_inlinechat_view_panel_toggle() {
        let mut p = XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_inlinechat_view_panel_resize() {
        let mut p = XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_inlinechat_view_panel_is_narrow() {
        let mut p = XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_inlinechat_view_total_visible_area_basic() {
        let panels = vec![
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "a"),
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_inlinechat_view_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_inlinechat_view_total_visible_area_hidden() {
        let mut panels = vec![
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "a"),
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_inlinechat_view_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_inlinechat_view_count_in_region_basic() {
        let panels = vec![
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "a"),
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "b"),
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_inlinechat_view_count_in_region(&panels, XInlinechatViewLayoutRegion::Sidebar), 2);
        assert_eq!(x_inlinechat_view_count_in_region(&panels, XInlinechatViewLayoutRegion::Editor), 1);
        assert_eq!(x_inlinechat_view_count_in_region(&panels, XInlinechatViewLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_inlinechat_view_widest_panel_basic() {
        let mut panels = vec![
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "narrow"),
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_inlinechat_view_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_inlinechat_view_collapse_region_basic() {
        let mut panels = vec![
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "a"),
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Sidebar, "b"),
            XInlinechatViewPanelState::new(XInlinechatViewLayoutRegion::Editor, "c"),
        ];
        x_inlinechat_view_collapse_region(&mut panels, XInlinechatViewLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_inlinechat_view_layout_constraint_clamp() {
        let lc = XInlinechatViewLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_inlinechat_view_layout_constraint_satisfied() {
        let lc = XInlinechatViewLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_inlinechat_view_widest_panel_empty() {
        let panels: Vec<XInlinechatViewPanelState> = vec![];
        assert!(x_inlinechat_view_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_inlinechat_view_layout_region_eq() {
        assert_eq!(XInlinechatViewLayoutRegion::Sidebar, XInlinechatViewLayoutRegion::Sidebar);
        assert_ne!(XInlinechatViewLayoutRegion::Sidebar, XInlinechatViewLayoutRegion::Panel);
    }


    // -- inlinechat_view extended domain tests ----------------------------------------

    #[test]
    fn y_inlinechat_view_enum_index() {
        assert_eq!(YInlinechatViewInlineChatMode::Ask.index(), 0);
        assert_eq!(YInlinechatViewInlineChatMode::Edit.index(), 1);
        assert_eq!(YInlinechatViewInlineChatMode::Generate.index(), 2);
        assert_eq!(YInlinechatViewInlineChatMode::Explain.index(), 3);
    }

    #[test]
    fn y_inlinechat_view_enum_label() {
        assert_eq!(YInlinechatViewInlineChatMode::Ask.label(), "Ask");
        assert_eq!(YInlinechatViewInlineChatMode::Edit.label(), "Edit");
        assert_eq!(YInlinechatViewInlineChatMode::Generate.label(), "Generate");
        assert_eq!(YInlinechatViewInlineChatMode::Explain.label(), "Explain");
    }

    #[test]
    fn y_inlinechat_view_enum_all() {
        let all = YInlinechatViewInlineChatMode::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_inlinechat_view_enum_is_default() {
        assert!(YInlinechatViewInlineChatMode::Ask.is_default());
        assert!(!YInlinechatViewInlineChatMode::Explain.is_default());
    }

    #[test]
    fn y_inlinechat_view_enum_display() {
        assert_eq!(format!("{}", YInlinechatViewInlineChatMode::Ask), "Ask");
    }

    #[test]
    fn y_inlinechat_view_struct_new() {
        let s = YInlinechatViewInlineChatHistory::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_inlinechat_view_struct_clear() {
        let mut s = YInlinechatViewInlineChatHistory::new();
        s.messages.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_inlinechat_view_fingerprint_deterministic() {
        let h1 = y_inlinechat_view_fingerprint("hello");
        let h2 = y_inlinechat_view_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_inlinechat_view_fingerprint("a"), y_inlinechat_view_fingerprint("b"));
    }

    #[test]
    fn y_inlinechat_view_truncate_short() {
        assert_eq!(y_inlinechat_view_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_inlinechat_view_truncate_long() {
        let r = y_inlinechat_view_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_inlinechat_view_normalize_key_basic() {
        assert_eq!(y_inlinechat_view_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_inlinechat_view_split_path_basic() {
        let parts = y_inlinechat_view_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_inlinechat_view_count_occurrences_basic() {
        assert_eq!(y_inlinechat_view_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_inlinechat_view_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_inlinechat_view_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_inlinechat_view_in_range_basic() {
        assert!(y_inlinechat_view_in_range(5, 1, 10));
        assert!(y_inlinechat_view_in_range(1, 1, 10));
        assert!(y_inlinechat_view_in_range(10, 1, 10));
        assert!(!y_inlinechat_view_in_range(0, 1, 10));
        assert!(!y_inlinechat_view_in_range(11, 1, 10));
    }

    #[test]
    fn y_inlinechat_view_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_inlinechat_view_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_inlinechat_view_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_inlinechat_view_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- inlinechat_view Z-extended tests -----------------------------------------------

    #[test]
    fn z_inlinechat_view_priority_weight() {
        assert_eq!(ZInlinechatViewPriority::Idle.weight(), 0);
        assert_eq!(ZInlinechatViewPriority::Normal.weight(), 2);
        assert_eq!(ZInlinechatViewPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_inlinechat_view_priority_label() {
        assert_eq!(ZInlinechatViewPriority::Low.label(), "low");
        assert_eq!(ZInlinechatViewPriority::High.label(), "high");
    }

    #[test]
    fn z_inlinechat_view_priority_is_elevated() {
        assert!(!ZInlinechatViewPriority::Normal.is_elevated());
        assert!(ZInlinechatViewPriority::High.is_elevated());
        assert!(ZInlinechatViewPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_inlinechat_view_priority_display() {
        assert_eq!(format!("{}", ZInlinechatViewPriority::Idle), "idle");
    }

    #[test]
    fn z_inlinechat_view_priority_all_asc() {
        let all = ZInlinechatViewPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZInlinechatViewPriority::Idle);
        assert_eq!(all[4], ZInlinechatViewPriority::Realtime);
    }

    #[test]
    fn z_inlinechat_view_struct_new() {
        let s = ZInlinechatViewInlineChatContext::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_inlinechat_view_struct_toggled_clone() {
        let s = ZInlinechatViewInlineChatContext::new();
        let t = s.toggled_clone();
        let _ = t.max_context;
    }

    #[test]
    fn z_inlinechat_view_rolling_hash_deterministic() {
        let h1 = z_inlinechat_view_rolling_hash(b"test");
        let h2 = z_inlinechat_view_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_inlinechat_view_rolling_hash(b"a"), z_inlinechat_view_rolling_hash(b"b"));
    }

    #[test]
    fn z_inlinechat_view_pad_to_basic() {
        assert_eq!(z_inlinechat_view_pad_to("hi", 5), "hi   ");
        assert_eq!(z_inlinechat_view_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_inlinechat_view_is_identifier_basic() {
        assert!(z_inlinechat_view_is_identifier("foo_bar"));
        assert!(z_inlinechat_view_is_identifier("abc123"));
        assert!(!z_inlinechat_view_is_identifier(""));
        assert!(!z_inlinechat_view_is_identifier("has space"));
    }

    #[test]
    fn z_inlinechat_view_levenshtein_basic() {
        assert_eq!(z_inlinechat_view_levenshtein("", ""), 0);
        assert_eq!(z_inlinechat_view_levenshtein("abc", "abc"), 0);
        assert_eq!(z_inlinechat_view_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_inlinechat_view_unique_words_basic() {
        let w = z_inlinechat_view_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_inlinechat_view_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_inlinechat_view_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_inlinechat_view_common_prefix_basic() {
        assert_eq!(z_inlinechat_view_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_inlinechat_view_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_inlinechat_view_struct_clear() {
        let mut s = ZInlinechatViewInlineChatContext::new();
        s.context_lines.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_inlinechat_view_rolling_hash_empty() {
        let h = z_inlinechat_view_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_78_push_and_len() {
        let mut rb = super::XbRingBuffer78::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_78_overwrite() {
        let mut rb = super::XbRingBuffer78::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_78_get_out_of_bounds() {
        let rb = super::XbRingBuffer78::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_78_drain_all() {
        let mut rb = super::XbRingBuffer78::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_78_peek_front_back() {
        let mut rb = super::XbRingBuffer78::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_78_clear() {
        let mut rb = super::XbRingBuffer78::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_78_capacity() {
        let rb = super::XbRingBuffer78::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_78_basic() {
        let h = super::xb_fnv1a_78(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_78(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_78_different_inputs() {
        let h1 = super::xb_fnv1a_78(b"abc");
        let h2 = super::xb_fnv1a_78(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_78_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_78(&data);
        let dec = super::xb_rle_decode_78(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_78_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_78(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_78(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_78_values() {
        assert!((super::xb_clamp_78(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_78(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_78(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_78_values() {
        assert!((super::xb_lerp_78(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_78(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_78(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_78_wrap_around_twice() {
        let mut rb = super::XbRingBuffer78::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 95 ----

    #[test]
    fn xc_95_pool_new_empty() {
        let pool: super::Xc95Pool<i32> = super::Xc95Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_95_pool_release_acquire() {
        let mut pool = super::Xc95Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_95_pool_acquire_empty() {
        let mut pool: super::Xc95Pool<i32> = super::Xc95Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_95_pool_full() {
        let mut pool = super::Xc95Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_95_pool_drain() {
        let mut pool = super::Xc95Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_95_pool_stats() {
        let mut pool = super::Xc95Pool::new(8);
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
    fn xc_95_pool_clear() {
        let mut pool = super::Xc95Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_95_pool_shrink() {
        let mut pool = super::Xc95Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_95_pool_default() {
        let pool: super::Xc95Pool<String> = super::Xc95Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_95_pool_extend() {
        let mut pool = super::Xc95Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_95_pool_retain() {
        let mut pool = super::Xc95Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_95_scheduler_round_robin() {
        let mut sched = super::Xc95Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_95_scheduler_empty() {
        let mut sched = super::Xc95Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_95_scheduler_reset() {
        let mut sched = super::Xc95Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_95_scheduler_add_remove() {
        let mut sched = super::Xc95Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_95_scheduler_targets() {
        let sched = super::Xc95Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_95_hash_empty() {
        assert_eq!(super::xc_95_hash(b""), 5381);
    }

    #[test]
    fn xc_95_hash_data() {
        let h = super::xc_95_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_95_hash(b"hello"), h);
    }

    #[test]
    fn xc_95_reverse_str() {
        assert_eq!(super::xc_95_reverse("abc"), "cba");
        assert_eq!(super::xc_95_reverse(""), "");
    }


    #[test]
    fn xe_91_pipeline_empty() {
        let p = super::Xe91Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_91_pipeline_parse_stage() {
        let p = super::Xe91Pipeline::new()
            .add_parse(super::xe_91_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_91_pipeline_transform_double() {
        let p = super::Xe91Pipeline::new()
            .add_transform(super::xe_91_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_91_pipeline_validate_reverse() {
        let p = super::Xe91Pipeline::new()
            .add_validate(super::xe_91_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_91_pipeline_emit_filter() {
        let p = super::Xe91Pipeline::new()
            .add_emit(super::xe_91_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_91_pipeline_multi_stage() {
        let p = super::Xe91Pipeline::new()
            .add_parse(super::xe_91_pipeline_identity)
            .add_transform(super::xe_91_pipeline_double)
            .add_validate(super::xe_91_pipeline_reverse)
            .add_emit(super::xe_91_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_91_pipeline_error_propagation() {
        let p = super::Xe91Pipeline::new()
            .add_parse(super::xe_91_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe91Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_91_pipeline_compose() {
        let p1 = super::Xe91Pipeline::new()
            .add_parse(super::xe_91_pipeline_identity);
        let p2 = super::Xe91Pipeline::new()
            .add_transform(super::xe_91_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_91_pipeline_error_display() {
        let e = super::Xe91PipelineError {
            stage: super::Xe91Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_91_cache_put_get() {
        let mut c = super::Xe91Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_91_cache_miss() {
        let mut c: super::Xe91Cache<&str, i32> = super::Xe91Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_91_cache_ttl_expiry() {
        let mut c = super::Xe91Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_91_cache_evict() {
        let mut c = super::Xe91Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_91_cache_capacity() {
        let mut c = super::Xe91Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_91_cache_stats() {
        let mut c = super::Xe91Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_91_cache_clear() {
        let mut c = super::Xe91Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_89 graph tests ------------------------------------------------

    #[test]
    fn xg_89_graph_empty() {
        let g = super::Xg89Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_89_graph_add_node() {
        let mut g = super::Xg89Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_89_graph_add_edge() {
        let mut g = super::Xg89Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_89_graph_neighbors() {
        let mut g = super::Xg89Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_89_graph_has_path() {
        let mut g = super::Xg89Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_89_graph_self_path() {
        let g = super::Xg89Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_89_graph_topo_sort() {
        let mut g = super::Xg89Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_89_graph_cycle_detect_false() {
        let mut g = super::Xg89Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_89_graph_cycle_detect_true() {
        let mut g = super::Xg89Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_89 heap tests -------------------------------------------------

    #[test]
    fn xg_89_heap_empty() {
        let h: super::Xg89Heap<i32> = super::Xg89Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_89_heap_push_pop() {
        let mut h = super::Xg89Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_89_heap_peek() {
        let mut h = super::Xg89Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_89_heap_drain_sorted() {
        let mut h = super::Xg89Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_89_heap_merge() {
        let mut a = super::Xg89Heap::new();
        let mut b = super::Xg89Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_89_heap_default() {
        let h: super::Xg89Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_89_graph_default() {
        let g: super::Xg89Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh94_skip_insert_contains() {
        let mut sl = super::Xh94SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh94_skip_remove() {
        let mut sl = super::Xh94SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh94_skip_len() {
        let mut sl = super::Xh94SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh94_skip_range_query() {
        let mut sl = super::Xh94SkipList::xh_new(4);
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
    fn xh94_skip_floor_ceiling() {
        let mut sl = super::Xh94SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh94_skip_rank() {
        let mut sl = super::Xh94SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh94_skip_empty() {
        let sl = super::Xh94SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh94_skip_duplicates() {
        let mut sl = super::Xh94SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh94_bitset_set_test() {
        let mut bs = super::Xh94BitSet::xh_new(256);
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
    fn xh94_bitset_clear_count() {
        let mut bs = super::Xh94BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh94_bitset_and_or_xor() {
        let mut a = super::Xh94BitSet::xh_new(128);
        let mut b = super::Xh94BitSet::xh_new(128);
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
    fn xh94_bitset_iter_ones() {
        let mut bs = super::Xh94BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh94_bitset_first_last() {
        let mut bs = super::Xh94BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh94_bitset_empty() {
        let bs = super::Xh94BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi94_deque_push_pop_back() {
        let mut dq = super::Xi94Deque::xi_new(4);
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
    fn xi94_deque_push_pop_front() {
        let mut dq = super::Xi94Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi94_deque_mixed_ops() {
        let mut dq = super::Xi94Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi94_deque_get_and_split() {
        let mut dq = super::Xi94Deque::xi_new(8);
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
    fn xi94_deque_rotate_left() {
        let mut dq = super::Xi94Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi94_deque_rotate_right() {
        let mut dq = super::Xi94Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi94_deque_grow() {
        let mut dq = super::Xi94Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi94_deque_empty() {
        let dq = super::Xi94Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi94_interval_tree_insert_query() {
        let mut tree = super::Xi94IntervalTree::xi_new();
        tree.xi_insert(super::Xi94Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi94Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi94Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi94_interval_tree_overlap() {
        let mut tree = super::Xi94IntervalTree::xi_new();
        tree.xi_insert(super::Xi94Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi94Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi94Interval::xi_new(12, 20));
        let q = super::Xi94Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi94_interval_tree_remove() {
        let mut tree = super::Xi94IntervalTree::xi_new();
        tree.xi_insert(super::Xi94Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi94Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi94_interval_tree_gaps() {
        let mut tree = super::Xi94IntervalTree::xi_new();
        tree.xi_insert(super::Xi94Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi94Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi94Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi94Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi94Interval::xi_new(8, 10));
    }

    #[test]
    fn xi94_interval_tree_merge() {
        let mut tree = super::Xi94IntervalTree::xi_new();
        tree.xi_insert(super::Xi94Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi94Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi94Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi94Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi94Interval::xi_new(10, 15));
    }

    #[test]
    fn xi94_interval_tree_all() {
        let mut tree = super::Xi94IntervalTree::xi_new();
        tree.xi_insert(super::Xi94Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi94Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi94_interval_tree_empty() {
        let tree = super::Xi94IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi94_interval_tree_contains_point() {
        let iv = super::Xi94Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 93) ---

    #[test]
    fn xj_93_uf_make_and_find() {
        let mut uf = super::Xj93UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_93_uf_union_connected() {
        let mut uf = super::Xj93UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_93_uf_component_count() {
        let mut uf = super::Xj93UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_93_uf_component_size() {
        let mut uf = super::Xj93UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_93_uf_largest_component() {
        let mut uf = super::Xj93UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_93_uf_many_elements() {
        let mut uf = super::Xj93UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_93_uf_separate_components() {
        let mut uf = super::Xj93UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_93_uf_path_compression() {
        let mut uf = super::Xj93UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_93_bt_insert_get() {
        let mut bt = super::Xj93BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_93_bt_contains_len() {
        let mut bt = super::Xj93BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_93_bt_replace() {
        let mut bt = super::Xj93BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_93_bt_remove() {
        let mut bt = super::Xj93BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_93_bt_keys_values() {
        let mut bt = super::Xj93BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_93_bt_range() {
        let mut bt = super::Xj93BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_93_bt_min_max() {
        let mut bt = super::Xj93BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_93_bt_many_inserts() {
        let mut bt = super::Xj93BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_94 segment tree tests ---

    #[test]
    fn xk_94_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk94SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_94_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk94SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_94_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk94SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_94_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk94SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_94_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk94SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_94_st_single_element() {
        let data = vec![42];
        let st = super::Xk94SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_94_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk94SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_94_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk94SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_94 disjoint intervals tests ---

    #[test]
    fn xk_94_di_add_and_count() {
        let mut di = super::Xk94DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_94_di_merge_overlap() {
        let mut di = super::Xk94DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_94_di_contains() {
        let mut di = super::Xk94DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_94_di_remove() {
        let mut di = super::Xk94DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_94_di_covered_length() {
        let mut di = super::Xk94DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_94_di_gaps() {
        let mut di = super::Xk94DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_94_di_merge_adjacent() {
        let mut di = super::Xk94DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_94_di_empty() {
        let di = super::Xk94DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_93_rope_new_empty() {
        let rope = super::Xl93Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_93_rope_from_str() {
        let rope = super::Xl93Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_93_rope_insert_at() {
        let mut rope = super::Xl93Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_93_rope_delete_range() {
        let mut rope = super::Xl93Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_93_rope_char_at() {
        let rope = super::Xl93Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_93_rope_split_concat() {
        let rope = super::Xl93Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_93_rope_line_count() {
        let rope = super::Xl93Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_93_rope_line_at() {
        let rope = super::Xl93Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_93_sa_build_and_search() {
        let sa = super::Xl93SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_93_sa_count() {
        let sa = super::Xl93SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_93_sa_longest_repeated() {
        let sa = super::Xl93SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_93_sa_all_positions() {
        let sa = super::Xl93SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_93_sa_len() {
        let sa = super::Xl93SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_93_sa_empty() {
        let sa = super::Xl93SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_93_rope_slice() {
        let rope = super::Xl93Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_93_sa_search_start() {
        let sa = super::Xl93SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
