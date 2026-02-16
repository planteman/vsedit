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

/// Accumulated statistics for ext-comments operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtCommentsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtCommentsStats {
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
    pub fn merge(&mut self, other: &ExtCommentsStats) {
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

impl Default for ExtCommentsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtCommentsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtCommentsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-comments.
#[derive(Debug, Clone)]
pub struct ExtCommentsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtCommentsValidator {
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

impl Default for ExtCommentsValidator {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn ext_comments_stats_new_defaults() {
        let stats = ExtCommentsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_comments_stats_record_success() {
        let mut stats = ExtCommentsStats::new();
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
    fn ext_comments_stats_record_failure() {
        let mut stats = ExtCommentsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_comments_stats_reset() {
        let mut stats = ExtCommentsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_comments_stats_merge() {
        let mut a = ExtCommentsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtCommentsStats::new();
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
    fn ext_comments_stats_display() {
        let mut stats = ExtCommentsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_comments_stats_default() {
        let stats = ExtCommentsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_comments_validator_accepts_valid_name() {
        let v = ExtCommentsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_comments_validator_rejects_empty() {
        let v = ExtCommentsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_comments_validator_rejects_too_long() {
        let v = ExtCommentsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_comments_validator_forbidden_prefix() {
        let v = ExtCommentsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_comments_validator_allowed_chars() {
        let v = ExtCommentsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_comments_validator_range() {
        let v = ExtCommentsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_comments_sanitize_removes_control() {
        let result = ExtCommentsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_comments_truncate_short_string() {
        assert_eq!(ExtCommentsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_comments_truncate_long_string() {
        let result = ExtCommentsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_comments_is_ascii_printable() {
        assert!(ExtCommentsValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtCommentsValidator::is_ascii_printable("Hello\x00World"));
    }
}
