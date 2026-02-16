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
}
