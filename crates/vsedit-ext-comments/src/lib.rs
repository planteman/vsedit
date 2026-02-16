//! Ext API: Comments.
//!
//! RPC bridge between the extension host and the main thread for code comments.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_comments";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CommentMessage {
    CreateThread {
        uri: String,
        range_start_line: u32,
        range_end_line: u32,
    },
    DeleteThread {
        thread_id: String,
    },
    AddComment {
        thread_id: String,
        body: String,
        author: String,
    },
    DeleteComment {
        thread_id: String,
        comment_id: String,
    },
    RegisterController {
        id: String,
        label: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub author: CommentAuthor,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommentAuthor {
    pub name: String,
    pub icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommentThread {
    pub id: String,
    pub uri: String,
    pub range_start_line: u32,
    pub range_end_line: u32,
    pub comments: Vec<Comment>,
    pub is_collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommentController {
    pub id: String,
    pub label: String,
}

// ── Bridge ──

// ── Error Types ──

/// Errors that can occur when operating on comments and threads.
#[derive(Debug, Clone, PartialEq)]
pub enum CommentError {
    /// The referenced thread does not exist.
    ThreadNotFound(String),
    /// The referenced comment does not exist.
    CommentNotFound { thread_id: String, comment_id: String },
    /// A controller with the given id is already registered.
    DuplicateController(String),
    /// The provided range is invalid (start > end).
    InvalidRange { start: u32, end: u32 },
    /// A required field was empty.
    EmptyField(&'static str),
}

impl fmt::Display for CommentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThreadNotFound(id) => write!(f, "thread not found: {id}"),
            Self::CommentNotFound { thread_id, comment_id } => {
                write!(f, "comment {comment_id} not found in thread {thread_id}")
            }
            Self::DuplicateController(id) => write!(f, "controller already registered: {id}"),
            Self::InvalidRange { start, end } => {
                write!(f, "invalid range: start ({start}) > end ({end})")
            }
            Self::EmptyField(name) => write!(f, "field '{name}' must not be empty"),
        }
    }
}

impl std::error::Error for CommentError {}

// ── Builder ──

/// Builder for constructing a [`CommentThread`] with validation.
#[derive(Debug, Default)]
pub struct CommentThreadBuilder {
    id: Option<String>,
    uri: Option<String>,
    range_start_line: Option<u32>,
    range_end_line: Option<u32>,
    comments: Vec<Comment>,
    is_collapsed: bool,
}

impl CommentThreadBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    pub fn range(mut self, start: u32, end: u32) -> Self {
        self.range_start_line = Some(start);
        self.range_end_line = Some(end);
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.is_collapsed = collapsed;
        self
    }

    pub fn comment(mut self, comment: Comment) -> Self {
        self.comments.push(comment);
        self
    }

    /// Build the thread, returning an error if required fields are missing or invalid.
    pub fn build(self) -> Result<CommentThread, CommentError> {
        let id = self.id.ok_or(CommentError::EmptyField("id"))?;
        if id.is_empty() {
            return Err(CommentError::EmptyField("id"));
        }
        let uri = self.uri.ok_or(CommentError::EmptyField("uri"))?;
        if uri.is_empty() {
            return Err(CommentError::EmptyField("uri"));
        }
        let start = self.range_start_line.ok_or(CommentError::EmptyField("range_start_line"))?;
        let end = self.range_end_line.ok_or(CommentError::EmptyField("range_end_line"))?;
        if start > end {
            return Err(CommentError::InvalidRange { start, end });
        }
        Ok(CommentThread {
            id,
            uri,
            range_start_line: start,
            range_end_line: end,
            comments: self.comments,
            is_collapsed: self.is_collapsed,
        })
    }
}

// ── Helper methods on core types ──

impl CommentThread {
    /// Returns the number of lines this thread spans.
    pub fn line_span(&self) -> u32 {
        self.range_end_line.saturating_sub(self.range_start_line) + 1
    }

    /// Returns `true` if the thread has no comments.
    pub fn is_empty(&self) -> bool {
        self.comments.is_empty()
    }

    /// Find a comment by id within this thread.
    pub fn find_comment(&self, comment_id: &str) -> Option<&Comment> {
        self.comments.iter().find(|c| c.id == comment_id)
    }

    /// Returns the set of unique author names in this thread.
    pub fn authors(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.comments.iter().map(|c| c.author.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

impl Comment {
    /// Create a new comment with the given id, body, and author name.
    pub fn new(id: impl Into<String>, body: impl Into<String>, author_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            body: body.into(),
            author: CommentAuthor {
                name: author_name.into(),
                icon_path: None,
            },
            timestamp: None,
        }
    }

    /// Set the timestamp on this comment, returning self for chaining.
    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = Some(ts);
        self
    }
}

impl fmt::Display for Comment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.id, self.author.name, self.body)
    }
}

impl fmt::Display for CommentAuthor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// ── Bridge ──

pub struct CommentBridge {
    controllers: Vec<CommentController>,
    threads: Vec<CommentThread>,
    next_id: u64,
}

impl CommentBridge {
    pub fn new() -> Self {
        Self {
            controllers: Vec::new(),
            threads: Vec::new(),
            next_id: 1,
        }
    }

    pub fn register_controller(&mut self, controller: CommentController) {
        self.controllers.push(controller);
    }

    pub fn create_thread(&mut self, uri: &str, start: u32, end: u32) -> String {
        let id = format!("thread-{}", self.next_id);
        self.next_id += 1;
        self.threads.push(CommentThread {
            id: id.clone(),
            uri: uri.to_string(),
            range_start_line: start,
            range_end_line: end,
            comments: Vec::new(),
            is_collapsed: false,
        });
        id
    }

    pub fn delete_thread(&mut self, thread_id: &str) -> bool {
        let before = self.threads.len();
        self.threads.retain(|t| t.id != thread_id);
        self.threads.len() < before
    }

    pub fn get_thread(&self, thread_id: &str) -> Option<&CommentThread> {
        self.threads.iter().find(|t| t.id == thread_id)
    }

    pub fn handle_message(&mut self, msg: &CommentMessage) -> serde_json::Value {
        match msg {
            CommentMessage::CreateThread {
                uri,
                range_start_line,
                range_end_line,
            } => {
                let id = self.create_thread(uri, *range_start_line, *range_end_line);
                serde_json::json!({"threadId": id})
            }
            CommentMessage::DeleteThread { thread_id } => {
                let ok = self.delete_thread(thread_id);
                serde_json::json!({"deleted": ok})
            }
            CommentMessage::AddComment {
                thread_id,
                body,
                author,
            } => {
                if let Some(thread) = self.threads.iter_mut().find(|t| t.id == *thread_id) {
                    let cid = format!("comment-{}", thread.comments.len() + 1);
                    thread.comments.push(Comment {
                        id: cid.clone(),
                        body: body.clone(),
                        author: CommentAuthor {
                            name: author.clone(),
                            icon_path: None,
                        },
                        timestamp: None,
                    });
                    serde_json::json!({"commentId": cid})
                } else {
                    serde_json::json!({"error": "thread not found"})
                }
            }
            CommentMessage::DeleteComment {
                thread_id,
                comment_id,
            } => {
                if let Some(thread) = self.threads.iter_mut().find(|t| t.id == *thread_id) {
                    thread.comments.retain(|c| c.id != *comment_id);
                    serde_json::json!({"deleted": true})
                } else {
                    serde_json::json!({"error": "thread not found"})
                }
            }
            CommentMessage::RegisterController { id, label } => {
                self.register_controller(CommentController {
                    id: id.clone(),
                    label: label.clone(),
                });
                serde_json::json!({"registered": true})
            }
        }
    }
}

impl Default for CommentBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl CommentBridge {
    /// Register a controller, returning an error if one with the same id exists.
    pub fn try_register_controller(
        &mut self,
        controller: CommentController,
    ) -> Result<(), CommentError> {
        if self.controllers.iter().any(|c| c.id == controller.id) {
            return Err(CommentError::DuplicateController(controller.id));
        }
        self.controllers.push(controller);
        Ok(())
    }

    /// Create a thread with range validation.
    pub fn try_create_thread(
        &mut self,
        uri: &str,
        start: u32,
        end: u32,
    ) -> Result<String, CommentError> {
        if uri.is_empty() {
            return Err(CommentError::EmptyField("uri"));
        }
        if start > end {
            return Err(CommentError::InvalidRange { start, end });
        }
        Ok(self.create_thread(uri, start, end))
    }

    /// Add a comment to a thread, returning the comment id or an error.
    pub fn try_add_comment(
        &mut self,
        thread_id: &str,
        body: &str,
        author: &str,
    ) -> Result<String, CommentError> {
        if body.is_empty() {
            return Err(CommentError::EmptyField("body"));
        }
        if author.is_empty() {
            return Err(CommentError::EmptyField("author"));
        }
        let thread = self
            .threads
            .iter_mut()
            .find(|t| t.id == thread_id)
            .ok_or_else(|| CommentError::ThreadNotFound(thread_id.to_string()))?;
        let cid = format!("comment-{}", thread.comments.len() + 1);
        thread.comments.push(Comment::new(cid.clone(), body, author));
        Ok(cid)
    }

    /// Delete a specific comment from a thread.
    pub fn try_delete_comment(
        &mut self,
        thread_id: &str,
        comment_id: &str,
    ) -> Result<(), CommentError> {
        let thread = self
            .threads
            .iter_mut()
            .find(|t| t.id == thread_id)
            .ok_or_else(|| CommentError::ThreadNotFound(thread_id.to_string()))?;
        let before = thread.comments.len();
        thread.comments.retain(|c| c.id != comment_id);
        if thread.comments.len() == before {
            return Err(CommentError::CommentNotFound {
                thread_id: thread_id.to_string(),
                comment_id: comment_id.to_string(),
            });
        }
        Ok(())
    }

    /// Returns all threads associated with a given URI.
    pub fn threads_for_uri(&self, uri: &str) -> Vec<&CommentThread> {
        self.threads.iter().filter(|t| t.uri == uri).collect()
    }

    /// Returns the total number of comments across all threads.
    pub fn total_comment_count(&self) -> usize {
        self.threads.iter().map(|t| t.comments.len()).sum()
    }

    /// Returns a reference to all registered controllers.
    pub fn controllers(&self) -> &[CommentController] {
        &self.controllers
    }

    /// Returns a reference to all threads.
    pub fn threads(&self) -> &[CommentThread] {
        &self.threads
    }
}

/// Initialize the comments extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
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
        let msg = CommentMessage::CreateThread {
            uri: "file:///a.rs".into(),
            range_start_line: 1,
            range_end_line: 5,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: CommentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn thread_serialization() {
        let thread = CommentThread {
            id: "t1".into(),
            uri: "file:///a.rs".into(),
            range_start_line: 1,
            range_end_line: 5,
            comments: vec![],
            is_collapsed: false,
        };
        let json = serde_json::to_string(&thread).unwrap();
        let back: CommentThread = serde_json::from_str(&json).unwrap();
        assert_eq!(thread, back);
    }

    #[test]
    fn bridge_create_and_delete_thread() {
        let mut bridge = CommentBridge::new();
        let id = bridge.create_thread("file:///a.rs", 1, 10);
        assert!(bridge.get_thread(&id).is_some());
        assert!(bridge.delete_thread(&id));
        assert!(bridge.get_thread(&id).is_none());
    }

    #[test]
    fn bridge_add_comment_to_thread() {
        let mut bridge = CommentBridge::new();
        let tid = bridge.create_thread("file:///a.rs", 1, 10);
        let msg = CommentMessage::AddComment {
            thread_id: tid.clone(),
            body: "Fix this".into(),
            author: "alice".into(),
        };
        bridge.handle_message(&msg);
        let thread = bridge.get_thread(&tid).unwrap();
        assert_eq!(thread.comments.len(), 1);
        assert_eq!(thread.comments[0].body, "Fix this");
    }

    #[test]
    fn bridge_delete_nonexistent_thread() {
        let mut bridge = CommentBridge::new();
        assert!(!bridge.delete_thread("nope"));
    }

    #[test]
    fn builder_valid_thread() {
        let thread = CommentThreadBuilder::new()
            .id("t1")
            .uri("file:///b.rs")
            .range(5, 10)
            .collapsed(true)
            .build()
            .unwrap();
        assert_eq!(thread.id, "t1");
        assert_eq!(thread.line_span(), 6);
        assert!(thread.is_collapsed);
    }

    #[test]
    fn builder_rejects_invalid_range() {
        let err = CommentThreadBuilder::new()
            .id("t1")
            .uri("file:///b.rs")
            .range(10, 5)
            .build()
            .unwrap_err();
        assert_eq!(err, CommentError::InvalidRange { start: 10, end: 5 });
    }

    #[test]
    fn builder_rejects_missing_fields() {
        let err = CommentThreadBuilder::new().build().unwrap_err();
        assert_eq!(err, CommentError::EmptyField("id"));
    }

    #[test]
    fn try_create_thread_validates_range() {
        let mut bridge = CommentBridge::new();
        let err = bridge.try_create_thread("file:///a.rs", 20, 10).unwrap_err();
        assert!(matches!(err, CommentError::InvalidRange { .. }));
    }

    #[test]
    fn try_add_comment_validates_fields() {
        let mut bridge = CommentBridge::new();
        let tid = bridge.create_thread("file:///a.rs", 1, 5);
        assert!(bridge.try_add_comment(&tid, "", "alice").is_err());
        assert!(bridge.try_add_comment(&tid, "body", "").is_err());
        assert!(bridge.try_add_comment("no-such-thread", "body", "alice").is_err());
    }

    #[test]
    fn try_delete_comment_errors() {
        let mut bridge = CommentBridge::new();
        let tid = bridge.create_thread("file:///a.rs", 1, 5);
        bridge.try_add_comment(&tid, "hello", "alice").unwrap();
        // deleting nonexistent comment
        let err = bridge.try_delete_comment(&tid, "no-comment").unwrap_err();
        assert!(matches!(err, CommentError::CommentNotFound { .. }));
        // deleting from nonexistent thread
        assert!(bridge.try_delete_comment("bad-thread", "c1").is_err());
    }

    #[test]
    fn duplicate_controller_rejected() {
        let mut bridge = CommentBridge::new();
        let ctrl = CommentController { id: "c1".into(), label: "Review".into() };
        bridge.try_register_controller(ctrl.clone()).unwrap();
        let err = bridge.try_register_controller(ctrl).unwrap_err();
        assert!(matches!(err, CommentError::DuplicateController(_)));
    }

    #[test]
    fn threads_for_uri_filters() {
        let mut bridge = CommentBridge::new();
        bridge.create_thread("file:///a.rs", 1, 5);
        bridge.create_thread("file:///b.rs", 1, 3);
        bridge.create_thread("file:///a.rs", 10, 20);
        assert_eq!(bridge.threads_for_uri("file:///a.rs").len(), 2);
        assert_eq!(bridge.threads_for_uri("file:///b.rs").len(), 1);
        assert_eq!(bridge.threads_for_uri("file:///c.rs").len(), 0);
    }

    #[test]
    fn total_comment_count() {
        let mut bridge = CommentBridge::new();
        let t1 = bridge.create_thread("file:///a.rs", 1, 5);
        let t2 = bridge.create_thread("file:///a.rs", 6, 10);
        bridge.try_add_comment(&t1, "one", "alice").unwrap();
        bridge.try_add_comment(&t1, "two", "bob").unwrap();
        bridge.try_add_comment(&t2, "three", "alice").unwrap();
        assert_eq!(bridge.total_comment_count(), 3);
    }

    #[test]
    fn thread_authors_deduplication() {
        let mut bridge = CommentBridge::new();
        let tid = bridge.create_thread("file:///a.rs", 1, 5);
        bridge.try_add_comment(&tid, "a", "alice").unwrap();
        bridge.try_add_comment(&tid, "b", "bob").unwrap();
        bridge.try_add_comment(&tid, "c", "alice").unwrap();
        let thread = bridge.get_thread(&tid).unwrap();
        let authors = thread.authors();
        assert_eq!(authors, vec!["alice", "bob"]);
    }

    #[test]
    fn comment_display() {
        let c = Comment::new("c1", "Fix this", "alice").with_timestamp(1000);
        assert_eq!(format!("{c}"), "[c1] alice: Fix this");
        assert_eq!(c.timestamp, Some(1000));
    }

    #[test]
    fn error_display_messages() {
        let e = CommentError::ThreadNotFound("t1".into());
        assert_eq!(e.to_string(), "thread not found: t1");
        let e = CommentError::InvalidRange { start: 10, end: 5 };
        assert_eq!(e.to_string(), "invalid range: start (10) > end (5)");
        let e = CommentError::EmptyField("body");
        assert_eq!(e.to_string(), "field 'body' must not be empty");
    }
}
