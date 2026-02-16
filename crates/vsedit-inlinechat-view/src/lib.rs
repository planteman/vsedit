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
}
