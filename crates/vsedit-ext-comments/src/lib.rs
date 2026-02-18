//! Ext API: Comments.
//!
//! RPC bridge between the extension host and the main thread for code comments.

use std::collections::HashMap;
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

// ── Reactions ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentReaction {
    pub emoji: String,
    pub count: u32,
    pub reacted_by: Vec<String>,
}

impl CommentReaction {
    pub fn new(emoji: impl Into<String>) -> Self {
        Self {
            emoji: emoji.into(),
            count: 0,
            reacted_by: Vec::new(),
        }
    }

    pub fn add_reaction(&mut self, author: impl Into<String>) {
        let author = author.into();
        if !self.reacted_by.contains(&author) {
            self.reacted_by.push(author);
            self.count += 1;
        }
    }

    pub fn remove_reaction(&mut self, author: &str) -> bool {
        if let Some(pos) = self.reacted_by.iter().position(|a| a == author) {
            self.reacted_by.remove(pos);
            self.count -= 1;
            true
        } else {
            false
        }
    }

    pub fn has_reacted(&self, author: &str) -> bool {
        self.reacted_by.iter().any(|a| a == author)
    }
}

// ── Markdown Rendering ──

/// Basic markdown to plain-text rendering for display.
pub fn comment_markdown_render(body: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        // Blockquote
        if trimmed.starts_with("> ") {
            let rest = &trimmed[2..];
            lines.push(format!("│ {rest}"));
            continue;
        }
        // Heading
        if trimmed.starts_with("# ") {
            let rest = &trimmed[2..];
            lines.push(rest.to_uppercase());
            continue;
        }
        // Bullet lists
        if trimmed.starts_with("- ") {
            let rest = &trimmed[2..];
            lines.push(format!("• {rest}"));
            continue;
        }
        if trimmed.starts_with("* ") {
            let rest = &trimmed[2..];
            lines.push(format!("• {rest}"));
            continue;
        }
        // Inline formatting
        let mut s = line.to_string();
        // Bold: **text** or __text__
        while let Some(start) = s.find("**") {
            if let Some(end) = s[start + 2..].find("**") {
                let inner = s[start + 2..start + 2 + end].to_string();
                s = format!("{}{}{}", &s[..start], inner, &s[start + 2 + end + 2..]);
            } else {
                break;
            }
        }
        while let Some(start) = s.find("__") {
            if let Some(end) = s[start + 2..].find("__") {
                let inner = s[start + 2..start + 2 + end].to_string();
                s = format!("{}{}{}", &s[..start], inner, &s[start + 2 + end + 2..]);
            } else {
                break;
            }
        }
        // Italic: *text* or _text_
        while let Some(start) = s.find('*') {
            if let Some(end) = s[start + 1..].find('*') {
                let inner = s[start + 1..start + 1 + end].to_string();
                s = format!("{}{}{}", &s[..start], inner, &s[start + 1 + end + 1..]);
            } else {
                break;
            }
        }
        while let Some(start) = s.find('_') {
            if let Some(end) = s[start + 1..].find('_') {
                let inner = s[start + 1..start + 1 + end].to_string();
                s = format!("{}{}{}", &s[..start], inner, &s[start + 1 + end + 1..]);
            } else {
                break;
            }
        }
        // Inline code: `code`
        while let Some(start) = s.find('`') {
            if let Some(end) = s[start + 1..].find('`') {
                let inner = s[start + 1..start + 1 + end].to_string();
                s = format!("{}{}{}", &s[..start], inner, &s[start + 1 + end + 1..]);
            } else {
                break;
            }
        }
        // Links: [text](url)
        while let Some(start) = s.find('[') {
            if let Some(end_bracket) = s[start + 1..].find("](") {
                let text = &s[start + 1..start + 1 + end_bracket];
                let url_start = start + 1 + end_bracket + 2;
                if let Some(end_paren) = s[url_start..].find(')') {
                    let url = &s[url_start..url_start + end_paren];
                    let replacement = format!("{text} ({url})");
                    s = format!("{}{replacement}{}", &s[..start], &s[url_start + end_paren + 1..]);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        lines.push(s);
    }
    lines.join("\n")
}

// ── Draft ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentDraft {
    pub thread_id: Option<String>,
    pub body: String,
    pub author: String,
    pub uri: Option<String>,
    pub line: Option<u32>,
    pub created_at: u64,
}

impl CommentDraft {
    pub fn new(author: impl Into<String>) -> Self {
        Self {
            thread_id: None,
            body: String::new(),
            author: author.into(),
            uri: None,
            line: None,
            created_at: 0,
        }
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn with_thread_id(mut self, id: impl Into<String>) -> Self {
        self.thread_id = Some(id.into());
        self
    }

    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }

    pub fn to_comment(&self, id: impl Into<String>) -> Comment {
        Comment::new(id, &self.body, &self.author)
    }

    pub fn is_reply(&self) -> bool {
        self.thread_id.is_some()
    }
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

// ---------------------------------------------------------------------------
// ThreadResolutionStatus
// ---------------------------------------------------------------------------

/// The resolution status of a comment thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadResolutionStatus {
    Open,
    Resolved,
    WontFix,
    Outdated,
}

impl ThreadResolutionStatus {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            ThreadResolutionStatus::Open => "Open",
            ThreadResolutionStatus::Resolved => "Resolved",
            ThreadResolutionStatus::WontFix => "Won't Fix",
            ThreadResolutionStatus::Outdated => "Outdated",
        }
    }

    /// Whether the status is considered "closed" (not open).
    pub fn is_closed(&self) -> bool {
        !matches!(self, ThreadResolutionStatus::Open)
    }
}

impl fmt::Display for ThreadResolutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ---------------------------------------------------------------------------
// ThreadResolutionWorkflow
// ---------------------------------------------------------------------------

/// Manages the resolution workflow for comment threads.
pub struct ThreadResolutionWorkflow {
    statuses: std::collections::HashMap<String, ThreadResolutionStatus>,
    history: Vec<ThreadResolutionChange>,
}

/// A recorded resolution status change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadResolutionChange {
    pub thread_id: String,
    pub old_status: ThreadResolutionStatus,
    pub new_status: ThreadResolutionStatus,
    pub changed_by: String,
}

impl ThreadResolutionWorkflow {
    pub fn new() -> Self {
        Self {
            statuses: std::collections::HashMap::new(),
            history: Vec::new(),
        }
    }

    /// Register a thread as open.
    pub fn register_thread(&mut self, thread_id: impl Into<String>) {
        self.statuses.insert(thread_id.into(), ThreadResolutionStatus::Open);
    }

    /// Get the current status of a thread.
    pub fn status(&self, thread_id: &str) -> Option<ThreadResolutionStatus> {
        self.statuses.get(thread_id).copied()
    }

    /// Resolve a thread.
    pub fn resolve(&mut self, thread_id: &str, by: &str) -> bool {
        self.transition(thread_id, ThreadResolutionStatus::Resolved, by)
    }

    /// Reopen a thread.
    pub fn reopen(&mut self, thread_id: &str, by: &str) -> bool {
        self.transition(thread_id, ThreadResolutionStatus::Open, by)
    }

    /// Mark a thread as won't fix.
    pub fn wont_fix(&mut self, thread_id: &str, by: &str) -> bool {
        self.transition(thread_id, ThreadResolutionStatus::WontFix, by)
    }

    /// Mark a thread as outdated.
    pub fn mark_outdated(&mut self, thread_id: &str, by: &str) -> bool {
        self.transition(thread_id, ThreadResolutionStatus::Outdated, by)
    }

    fn transition(&mut self, thread_id: &str, new_status: ThreadResolutionStatus, by: &str) -> bool {
        if let Some(old_status) = self.statuses.get(thread_id).copied() {
            if old_status == new_status {
                return false;
            }
            self.statuses.insert(thread_id.to_string(), new_status);
            self.history.push(ThreadResolutionChange {
                thread_id: thread_id.to_string(),
                old_status,
                new_status,
                changed_by: by.to_string(),
            });
            true
        } else {
            false
        }
    }

    /// Get all open threads.
    pub fn open_threads(&self) -> Vec<&str> {
        self.statuses
            .iter()
            .filter(|(_, s)| **s == ThreadResolutionStatus::Open)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Get all resolved threads.
    pub fn resolved_threads(&self) -> Vec<&str> {
        self.statuses
            .iter()
            .filter(|(_, s)| s.is_closed())
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Get the resolution change history.
    pub fn history(&self) -> &[ThreadResolutionChange] {
        &self.history
    }

    /// Number of tracked threads.
    pub fn thread_count(&self) -> usize {
        self.statuses.len()
    }
}

impl Default for ThreadResolutionWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ReactionKind
// ---------------------------------------------------------------------------

/// Standard reaction types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReactionKind {
    ThumbsUp,
    ThumbsDown,
    Laugh,
    Heart,
    Confused,
    Rocket,
}

impl ReactionKind {
    /// Emoji representation.
    pub fn emoji(&self) -> &'static str {
        match self {
            ReactionKind::ThumbsUp => "👍",
            ReactionKind::ThumbsDown => "👎",
            ReactionKind::Laugh => "😄",
            ReactionKind::Heart => "❤️",
            ReactionKind::Confused => "😕",
            ReactionKind::Rocket => "🚀",
        }
    }

    /// All available reaction kinds.
    pub fn all() -> &'static [ReactionKind] {
        &[
            ReactionKind::ThumbsUp,
            ReactionKind::ThumbsDown,
            ReactionKind::Laugh,
            ReactionKind::Heart,
            ReactionKind::Confused,
            ReactionKind::Rocket,
        ]
    }
}

impl fmt::Display for ReactionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.emoji())
    }
}

/// Manages reactions on a comment.
#[derive(Debug, Clone)]
pub struct CommentReactions {
    reactions: std::collections::HashMap<ReactionKind, Vec<String>>,
}

impl CommentReactions {
    pub fn new() -> Self {
        Self {
            reactions: std::collections::HashMap::new(),
        }
    }

    /// Toggle a reaction for an author. Returns true if added, false if removed.
    pub fn toggle(&mut self, kind: ReactionKind, author: &str) -> bool {
        let users = self.reactions.entry(kind).or_default();
        if let Some(pos) = users.iter().position(|a| a == author) {
            users.remove(pos);
            false
        } else {
            users.push(author.to_string());
            true
        }
    }

    /// Get the count for a specific reaction kind.
    pub fn count(&self, kind: ReactionKind) -> usize {
        self.reactions.get(&kind).map_or(0, |v| v.len())
    }

    /// Get all reactions with their counts (non-zero only).
    pub fn summary(&self) -> Vec<(ReactionKind, usize)> {
        self.reactions
            .iter()
            .filter(|(_, users)| !users.is_empty())
            .map(|(kind, users)| (*kind, users.len()))
            .collect()
    }

    /// Total number of reactions across all kinds.
    pub fn total(&self) -> usize {
        self.reactions.values().map(|v| v.len()).sum()
    }
}

impl Default for CommentReactions {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommentSearch
// ---------------------------------------------------------------------------

/// Search filter criteria for comments.
#[derive(Debug, Clone)]
pub struct CommentSearchFilter {
    pub text: Option<String>,
    pub author: Option<String>,
    pub uri: Option<String>,
    pub include_resolved: bool,
}

impl CommentSearchFilter {
    /// Create a filter that matches everything.
    pub fn all() -> Self {
        Self {
            text: None,
            author: None,
            uri: None,
            include_resolved: true,
        }
    }

    /// Filter by text content.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Filter by author name.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Filter by file URI.
    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Exclude resolved threads.
    pub fn exclude_resolved(mut self) -> Self {
        self.include_resolved = false;
        self
    }
}

/// Searches comment threads using filters.
pub struct CommentSearchEngine;

impl CommentSearchEngine {
    /// Search threads matching the given filter.
    pub fn search<'a>(
        threads: &'a [CommentThread],
        filter: &CommentSearchFilter,
    ) -> Vec<&'a CommentThread> {
        threads
            .iter()
            .filter(|thread| {
                if let Some(ref uri) = filter.uri {
                    if &thread.uri != uri {
                        return false;
                    }
                }
                if let Some(ref text) = filter.text {
                    let text_lower = text.to_lowercase();
                    let has_match = thread.comments.iter().any(|c| {
                        c.body.to_lowercase().contains(&text_lower)
                    });
                    if !has_match {
                        return false;
                    }
                }
                if let Some(ref author) = filter.author {
                    let has_author = thread.comments.iter().any(|c| {
                        c.author.name == *author
                    });
                    if !has_author {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Count threads matching a filter.
    pub fn count(threads: &[CommentThread], filter: &CommentSearchFilter) -> usize {
        Self::search(threads, filter).len()
    }

    /// Get unique authors across all matching threads.
    pub fn unique_authors(threads: &[CommentThread], filter: &CommentSearchFilter) -> Vec<String> {
        let mut authors: Vec<String> = Self::search(threads, filter)
            .iter()
            .flat_map(|t| t.comments.iter().map(|c| c.author.name.clone()))
            .collect();
        authors.sort();
        authors.dedup();
        authors
    }
}

// ── Thread statistics ──

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadStats {
    pub thread_count: usize,
    pub comment_count: usize,
    pub collapsed_count: usize,
    pub empty_thread_count: usize,
    pub uri_count: usize,
}

pub fn compute_thread_stats(threads: &[CommentThread]) -> ThreadStats {
    let mut uris = Vec::new();
    let mut comment_count = 0;
    let mut collapsed_count = 0;
    let mut empty_thread_count = 0;
    for t in threads {
        comment_count += t.comments.len();
        if t.is_collapsed { collapsed_count += 1; }
        if t.comments.is_empty() { empty_thread_count += 1; }
        if !uris.contains(&t.uri) { uris.push(t.uri.clone()); }
    }
    ThreadStats { thread_count: threads.len(), comment_count, collapsed_count, empty_thread_count, uri_count: uris.len() }
}

pub fn threads_by_uri<'a>(threads: &'a [CommentThread]) -> std::collections::HashMap<&'a str, Vec<&'a CommentThread>> {
    let mut map: std::collections::HashMap<&str, Vec<&CommentThread>> = std::collections::HashMap::new();
    for t in threads { map.entry(t.uri.as_str()).or_default().push(t); }
    map
}

pub fn newest_comment<'a>(threads: &'a [CommentThread]) -> Option<&'a Comment> {
    threads.iter().flat_map(|t| t.comments.iter()).filter(|c| c.timestamp.is_some()).max_by_key(|c| c.timestamp.unwrap_or(0))
}

pub fn threads_overlapping_range<'a>(threads: &'a [CommentThread], uri: &str, start_line: u32, end_line: u32) -> Vec<&'a CommentThread> {
    threads.iter().filter(|t| t.uri == uri && t.range_start_line <= end_line && t.range_end_line >= start_line).collect()
}

pub fn comment_count_by_author(threads: &[CommentThread], author: &str) -> usize {
    threads.iter().flat_map(|t| t.comments.iter()).filter(|c| c.author.name == author).count()
}

pub fn all_comment_bodies(threads: &[CommentThread]) -> Vec<&str> {
    threads.iter().flat_map(|t| t.comments.iter()).map(|c| c.body.as_str()).collect()
}

pub fn collapse_empty_threads(threads: &mut [CommentThread]) {
    for t in threads.iter_mut() {
        if t.comments.is_empty() { t.is_collapsed = true; }
    }
}

// ── Thread Filtering ──

/// Utility for filtering collections of `CommentThread`.
pub struct CommentThreadFilter;

impl CommentThreadFilter {
    /// Return threads that are collapsed (resolved).
    pub fn by_collapsed(threads: &[CommentThread]) -> Vec<&CommentThread> {
        threads.iter().filter(|t| t.is_collapsed).collect()
    }

    /// Return threads that are not collapsed (unresolved).
    pub fn by_uncollapsed(threads: &[CommentThread]) -> Vec<&CommentThread> {
        threads.iter().filter(|t| !t.is_collapsed).collect()
    }

    /// Return threads matching the given URI.
    pub fn by_uri<'a>(threads: &'a [CommentThread], uri: &str) -> Vec<&'a CommentThread> {
        threads.iter().filter(|t| t.uri == uri).collect()
    }

    /// Return threads whose first comment was written by `author`.
    pub fn by_author<'a>(threads: &'a [CommentThread], author: &str) -> Vec<&'a CommentThread> {
        threads
            .iter()
            .filter(|t| {
                t.comments
                    .first()
                    .map_or(false, |c| c.author.name == author)
            })
            .collect()
    }

    /// Return threads with at least `min` comments.
    pub fn with_min_comments(threads: &[CommentThread], min: usize) -> Vec<&CommentThread> {
        threads
            .iter()
            .filter(|t| t.comments.len() >= min)
            .collect()
    }
}

// ── Reaction Aggregation ──

/// Aggregates reaction counts by label.
#[derive(Debug, Clone)]
pub struct CommentReactionAggregator {
    reactions_map: HashMap<String, u32>,
}

impl CommentReactionAggregator {
    pub fn new() -> Self {
        Self {
            reactions_map: HashMap::new(),
        }
    }

    /// Add `count` occurrences of the given reaction label.
    pub fn add_reaction(&mut self, kind_label: &str, count: u32) {
        *self.reactions_map.entry(kind_label.to_string()).or_insert(0) += count;
    }

    /// Total number of reactions across all labels.
    pub fn total(&self) -> u32 {
        self.reactions_map.values().sum()
    }

    /// Return the label with the highest count, if any.
    pub fn most_popular(&self) -> Option<(String, u32)> {
        self.reactions_map
            .iter()
            .max_by_key(|(_, v)| **v)
            .map(|(k, v)| (k.clone(), *v))
    }

    /// Merge counts from another aggregator into this one.
    pub fn merge(&mut self, other: &Self) {
        for (k, v) in &other.reactions_map {
            *self.reactions_map.entry(k.clone()).or_insert(0) += *v;
        }
    }

    /// Remove all tracked reactions.
    pub fn clear(&mut self) {
        self.reactions_map.clear();
    }
}

impl Default for CommentReactionAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CommentReactionAggregator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} reactions", self.total())
    }
}

// ── Draft Management ──

/// Manages in-progress comment drafts keyed by thread id.
#[derive(Debug, Clone)]
pub struct CommentDraftManager {
    drafts: HashMap<String, CommentDraft>,
}

impl CommentDraftManager {
    pub fn new() -> Self {
        Self {
            drafts: HashMap::new(),
        }
    }

    /// Save or overwrite a draft for the given thread.
    pub fn save_draft(&mut self, thread_id: impl Into<String>, body: impl Into<String>, timestamp: u64) {
        let tid = thread_id.into();
        let draft = CommentDraft {
            thread_id: Some(tid.clone()),
            body: body.into(),
            author: String::new(),
            uri: None,
            line: None,
            created_at: timestamp,
        };
        self.drafts.insert(tid, draft);
    }

    pub fn get_draft(&self, thread_id: &str) -> Option<&CommentDraft> {
        self.drafts.get(thread_id)
    }

    pub fn remove_draft(&mut self, thread_id: &str) -> Option<CommentDraft> {
        self.drafts.remove(thread_id)
    }

    pub fn has_draft(&self, thread_id: &str) -> bool {
        self.drafts.contains_key(thread_id)
    }

    pub fn draft_count(&self) -> usize {
        self.drafts.len()
    }

    /// Return all drafts sorted by `created_at` ascending.
    pub fn all_drafts(&self) -> Vec<&CommentDraft> {
        let mut v: Vec<_> = self.drafts.values().collect();
        v.sort_by_key(|d| d.created_at);
        v
    }

    pub fn clear_all(&mut self) {
        self.drafts.clear();
    }
}

impl Default for CommentDraftManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Range Highlight Tracking ──

/// Tracks which line ranges are highlighted for comment threads.
#[derive(Debug, Clone)]
pub struct CommentRangeHighlightTracker {
    highlights: HashMap<String, (u32, u32)>,
}

impl CommentRangeHighlightTracker {
    pub fn new() -> Self {
        Self {
            highlights: HashMap::new(),
        }
    }

    /// Start tracking a highlight range for the given thread.
    pub fn track(&mut self, thread_id: impl Into<String>, start: u32, end: u32) {
        self.highlights.insert(thread_id.into(), (start, end));
    }

    /// Stop tracking highlights for the given thread.
    pub fn untrack(&mut self, thread_id: &str) -> bool {
        self.highlights.remove(thread_id).is_some()
    }

    pub fn is_tracked(&self, thread_id: &str) -> bool {
        self.highlights.contains_key(thread_id)
    }

    pub fn lines_for(&self, thread_id: &str) -> Option<(u32, u32)> {
        self.highlights.get(thread_id).copied()
    }

    /// Return thread ids whose tracked range contains the given line.
    pub fn overlapping(&self, line: u32) -> Vec<String> {
        self.highlights
            .iter()
            .filter(|(_, (s, e))| *s <= line && line <= *e)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn count(&self) -> usize {
        self.highlights.len()
    }
}

impl Default for CommentRangeHighlightTracker {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// CommentDraftAutoSave
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentDraftAutoSave {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl CommentDraftAutoSave {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for CommentDraftAutoSave {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for CommentDraftAutoSave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "CommentDraftAutoSave({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// CommentMentionResolver
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentMentionResolver {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl CommentMentionResolver {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for CommentMentionResolver {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for CommentMentionResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "CommentMentionResolver({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// CommentDraftAutoSaveSnapshot — point-in-time snapshot of CommentDraftAutoSave state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentDraftAutoSaveSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl CommentDraftAutoSaveSnapshot {
    pub fn capture(source: &CommentDraftAutoSave, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for CommentDraftAutoSaveSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// CommentMentionResolverStats — aggregate statistics for CommentMentionResolver
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CommentMentionResolverStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl CommentMentionResolverStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for CommentMentionResolverStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// CommentDraftAutoSaveConfig — configuration for CommentDraftAutoSave
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommentDraftAutoSaveConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl CommentDraftAutoSaveConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for CommentDraftAutoSaveConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for CommentDraftAutoSaveConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}


// ---------------------------------------------------------------------------
// vsedit-ext-comments: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtCommentsXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ExtCommentsXConfig {
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

impl std::fmt::Display for ExtCommentsXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ExtCommentsXRegistry {
    entries: Vec<ExtCommentsXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ExtCommentsXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ExtCommentsXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ExtCommentsXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ExtCommentsXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ExtCommentsXConfig> {
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

    pub fn active_entries(&self) -> Vec<&ExtCommentsXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ExtCommentsXConfig> {
        let mut sorted: Vec<&ExtCommentsXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ExtCommentsXConfig> {
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

    pub fn iter(&self) -> ExtCommentsXIterator<'_> {
        ExtCommentsXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ExtCommentsXIterator<'a> {
    inner: std::slice::Iter<'a, ExtCommentsXConfig>,
}

impl<'a> Iterator for ExtCommentsXIterator<'a> {
    type Item = &'a ExtCommentsXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ExtCommentsXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ExtCommentsXCache {
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
pub struct ExtCommentsXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ExtCommentsXFormatter {
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

    pub fn format_entry(&self, entry: &ExtCommentsXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ExtCommentsXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ExtCommentsXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ExtCommentsXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ExtCommentsXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ExtCommentsXValidator {
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

    pub fn validate(&self, entry: &ExtCommentsXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &ExtCommentsXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ExtCommentsXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// Comment thread API for extensions — extended utilities (yo)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_cmt operations.
#[derive(Debug, Clone)]
pub struct YoMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YoMetrics {
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

/// Sliding-window rate counter for ext_cmt.
#[derive(Debug, Clone)]
pub struct YoRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YoRateWindow {
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

/// A small LRU-style cache for ext_cmt lookups.
#[derive(Debug, Clone)]
pub struct YoLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YoLruCache {
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
// xa_ extended helpers for ext_comments
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtCommentsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtCommentsRingBuf {
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
pub struct XaExtCommentsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtCommentsCounter {
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

impl Default for XaExtCommentsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 53
// ---------------------------------------------------------------------------

/// Generic object pool `Xc53Pool<T>`.
pub struct Xc53Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc53Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc53PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc53Pool<T> {
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
    pub fn stats(&self) -> Xc53PoolStats {
        Xc53PoolStats {
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

impl<T> Default for Xc53Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc53Scheduler`.
pub struct Xc53Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc53Scheduler {
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

impl Default for Xc53Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_53 hash for the given byte slice.
pub fn xc_53_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_53 convention.
pub fn xc_53_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_98 deepening: state machine + event bus ---

/// States for the Xd98 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd98State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd98State {
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
pub struct Xd98Transition {
    pub from: Xd98State,
    pub to: Xd98State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd98StateMachine {
    current: Xd98State,
    history: Vec<Xd98Transition>,
    step_counter: usize,
}

impl Xd98StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd98State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd98State {
        self.current
    }

    pub fn history(&self) -> &[Xd98Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd98State) -> Result<Xd98State, String> {
        let allowed = match (self.current, target) {
            (Xd98State::Idle, Xd98State::Running) => true,
            (Xd98State::Running, Xd98State::Paused) => true,
            (Xd98State::Running, Xd98State::Done) => true,
            (Xd98State::Paused, Xd98State::Running) => true,
            (Xd98State::Paused, Xd98State::Done) => true,
            (Xd98State::Done, Xd98State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_98: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd98Transition {
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
            "Xd98SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd98State> {
        let prefix = "Xd98SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd98State::Idle),
            "Running" => Some(Xd98State::Running),
            "Paused" => Some(Xd98State::Paused),
            "Done" => Some(Xd98State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd98State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd98 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd98Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd98Event {
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

type Xd98HandlerFn = Box<dyn Fn(&Xd98Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd98EventBus {
    handlers: Vec<(usize, Option<String>, Xd98HandlerFn)>,
    next_id: usize,
    published: Vec<Xd98Event>,
}

impl Xd98EventBus {
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
        F: Fn(&Xd98Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd98Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd98Event) {
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

    pub fn published_events(&self) -> &[Xd98Event] {
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
// xg_22: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg22Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg22Graph {
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

impl Default for Xg22Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_22: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg22Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg22Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg22Heap<T>) {
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

impl<T: Ord> Default for Xg22Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 52).
pub struct Xh52SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh52SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 94 as u64,
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

/// A compact bit set supporting boolean operations (variant 52).
pub struct Xh52BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh52BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 52).
pub struct Xi52Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi52Deque<T> {
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
pub struct Xi52Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi52Interval {
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

/// A simple interval tree (variant 52).
pub struct Xi52IntervalTree {
    xi_intervals: Vec<Xi52Interval>,
}

impl Xi52IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi52Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi52Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi52Interval) -> Vec<&Xi52Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi52Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi52Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi52Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi52Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi52Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi52Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 52) ---

/// Disjoint set / union-find for crate 52.
pub struct Xj52UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj52UnionFind {
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

const XJ52_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 52.
pub struct Xj52BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj52BTreeNode<K, V>>>,
    len: usize,
}

struct Xj52BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj52BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj52BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ52_BTREE_ORDER - 1
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
        let mid = XJ52_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj52BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj52BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj52BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj52BTreeNode::xj_new_leaf();
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


// --- xk_52 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk52SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk52SegmentTree {
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
pub struct Xk52DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk52DisjointIntervals {
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

    #[test]
    fn test_reaction_new() {
        let r = CommentReaction::new("👍");
        assert_eq!(r.emoji, "👍");
        assert_eq!(r.count, 0);
        assert!(r.reacted_by.is_empty());
    }

    #[test]
    fn test_reaction_add_remove() {
        let mut r = CommentReaction::new("👍");
        r.add_reaction("alice");
        assert_eq!(r.count, 1);
        assert!(r.remove_reaction("alice"));
        assert_eq!(r.count, 0);
        assert!(!r.remove_reaction("alice"));
    }

    #[test]
    fn test_reaction_has_reacted() {
        let mut r = CommentReaction::new("👍");
        r.add_reaction("bob");
        assert!(r.has_reacted("bob"));
        assert!(!r.has_reacted("alice"));
    }

    #[test]
    fn test_reaction_no_duplicate() {
        let mut r = CommentReaction::new("👍");
        r.add_reaction("alice");
        r.add_reaction("alice");
        assert_eq!(r.count, 1);
        assert_eq!(r.reacted_by.len(), 1);
    }

    #[test]
    fn test_markdown_bold() {
        assert_eq!(comment_markdown_render("**bold**"), "bold");
        assert_eq!(comment_markdown_render("__bold__"), "bold");
    }

    #[test]
    fn test_markdown_italic() {
        assert_eq!(comment_markdown_render("*italic*"), "italic");
        assert_eq!(comment_markdown_render("_italic_"), "italic");
    }

    #[test]
    fn test_markdown_code() {
        assert_eq!(comment_markdown_render("`code`"), "code");
    }

    #[test]
    fn test_markdown_heading() {
        assert_eq!(comment_markdown_render("# Heading"), "HEADING");
    }

    #[test]
    fn test_markdown_bullet() {
        assert_eq!(comment_markdown_render("- item"), "• item");
        assert_eq!(comment_markdown_render("* item"), "• item");
    }

    #[test]
    fn test_markdown_link() {
        assert_eq!(
            comment_markdown_render("[text](https://example.com)"),
            "text (https://example.com)"
        );
    }

    #[test]
    fn test_markdown_blockquote() {
        assert_eq!(comment_markdown_render("> text"), "│ text");
    }

    #[test]
    fn test_draft_new() {
        let d = CommentDraft::new("alice");
        assert_eq!(d.author, "alice");
        assert!(d.body.is_empty());
        assert!(d.thread_id.is_none());
    }

    #[test]
    fn test_draft_to_comment() {
        let d = CommentDraft::new("alice").with_body("hello");
        let c = d.to_comment("c1");
        assert_eq!(c.id, "c1");
        assert_eq!(c.body, "hello");
        assert_eq!(c.author.name, "alice");
    }

    #[test]
    fn test_draft_is_reply() {
        let d = CommentDraft::new("alice");
        assert!(!d.is_reply());
        let d = d.with_thread_id("t1");
        assert!(d.is_reply());
    }

    #[test]
    fn test_draft_is_empty() {
        let d = CommentDraft::new("alice");
        assert!(d.is_empty());
        let d = d.with_body("hello");
        assert!(!d.is_empty());
    }

    // ── ThreadResolution / Reactions / Search tests ──

    #[test]
    fn thread_resolution_workflow() {
        let mut wf = ThreadResolutionWorkflow::new();
        wf.register_thread("t1");
        wf.register_thread("t2");
        assert_eq!(wf.status("t1"), Some(ThreadResolutionStatus::Open));
        assert!(wf.resolve("t1", "alice"));
        assert_eq!(wf.status("t1"), Some(ThreadResolutionStatus::Resolved));
        assert!(!wf.resolve("t1", "bob")); // already resolved
        assert!(wf.reopen("t1", "bob"));
        assert_eq!(wf.status("t1"), Some(ThreadResolutionStatus::Open));
        assert_eq!(wf.history().len(), 2);
    }

    #[test]
    fn thread_resolution_open_resolved_lists() {
        let mut wf = ThreadResolutionWorkflow::new();
        wf.register_thread("a");
        wf.register_thread("b");
        wf.register_thread("c");
        wf.resolve("b", "alice");
        wf.wont_fix("c", "bob");
        let open = wf.open_threads();
        assert_eq!(open.len(), 1);
        let resolved = wf.resolved_threads();
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn comment_reactions_toggle() {
        let mut rx = CommentReactions::new();
        assert!(rx.toggle(ReactionKind::ThumbsUp, "alice")); // added
        assert!(rx.toggle(ReactionKind::ThumbsUp, "bob"));
        assert_eq!(rx.count(ReactionKind::ThumbsUp), 2);
        assert!(!rx.toggle(ReactionKind::ThumbsUp, "alice")); // removed
        assert_eq!(rx.count(ReactionKind::ThumbsUp), 1);
        assert_eq!(rx.total(), 1);
    }

    #[test]
    fn reaction_kind_emoji() {
        assert_eq!(ReactionKind::ThumbsUp.emoji(), "👍");
        assert_eq!(ReactionKind::all().len(), 6);
    }

    #[test]
    fn comment_search_by_text() {
        let threads = vec![
            CommentThread {
                id: "t1".into(),
                uri: "file:///a.rs".into(),
                range_start_line: 1,
                range_end_line: 5,
                comments: vec![Comment::new("c1", "fix the bug", "alice")],
                is_collapsed: false,
            },
            CommentThread {
                id: "t2".into(),
                uri: "file:///b.rs".into(),
                range_start_line: 10,
                range_end_line: 12,
                comments: vec![Comment::new("c2", "looks good", "bob")],
                is_collapsed: false,
            },
        ];
        let filter = CommentSearchFilter::all().with_text("bug");
        let results = CommentSearchEngine::search(&threads, &filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "t1");
    }

    #[test]
    fn comment_search_by_author() {
        let threads = vec![
            CommentThread {
                id: "t1".into(),
                uri: "file:///a.rs".into(),
                range_start_line: 1,
                range_end_line: 5,
                comments: vec![Comment::new("c1", "hello", "alice")],
                is_collapsed: false,
            },
        ];
        let filter = CommentSearchFilter::all().with_author("bob");
        assert_eq!(CommentSearchEngine::count(&threads, &filter), 0);
        let filter = CommentSearchFilter::all().with_author("alice");
        assert_eq!(CommentSearchEngine::count(&threads, &filter), 1);
    }

    #[test]
    fn comment_search_unique_authors() {
        let threads = vec![
            CommentThread {
                id: "t1".into(),
                uri: "file:///a.rs".into(),
                range_start_line: 1,
                range_end_line: 5,
                comments: vec![
                    Comment::new("c1", "hi", "alice"),
                    Comment::new("c2", "hey", "bob"),
                ],
                is_collapsed: false,
            },
        ];
        let authors = CommentSearchEngine::unique_authors(&threads, &CommentSearchFilter::all());
        assert_eq!(authors, vec!["alice", "bob"]);
    }

    #[test]
    fn compute_thread_stats_basic() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5, comments: vec![Comment::new("c1", "hi", "alice")], is_collapsed: false },
            CommentThread { id: "t2".into(), uri: "file:///b.rs".into(), range_start_line: 10, range_end_line: 15, comments: vec![], is_collapsed: true },
        ];
        let stats = compute_thread_stats(&threads);
        assert_eq!(stats.thread_count, 2);
        assert_eq!(stats.comment_count, 1);
        assert_eq!(stats.collapsed_count, 1);
        assert_eq!(stats.empty_thread_count, 1);
        assert_eq!(stats.uri_count, 2);
    }

    #[test]
    fn threads_by_uri_groups_correctly() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5, comments: vec![], is_collapsed: false },
            CommentThread { id: "t2".into(), uri: "file:///a.rs".into(), range_start_line: 10, range_end_line: 15, comments: vec![], is_collapsed: false },
            CommentThread { id: "t3".into(), uri: "file:///b.rs".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: false },
        ];
        let grouped = threads_by_uri(&threads);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["file:///a.rs"].len(), 2);
        assert_eq!(grouped["file:///b.rs"].len(), 1);
    }

    #[test]
    fn newest_comment_finds_latest() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5,
                comments: vec![Comment::new("c1", "old", "alice").with_timestamp(100), Comment::new("c2", "new", "bob").with_timestamp(200)],
                is_collapsed: false },
        ];
        assert_eq!(newest_comment(&threads).unwrap().id, "c2");
    }

    #[test]
    fn newest_comment_none_when_no_timestamps() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5, comments: vec![Comment::new("c1", "hi", "alice")], is_collapsed: false },
        ];
        assert!(newest_comment(&threads).is_none());
    }

    #[test]
    fn threads_overlapping_range_filters() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5, comments: vec![], is_collapsed: false },
            CommentThread { id: "t2".into(), uri: "file:///a.rs".into(), range_start_line: 10, range_end_line: 20, comments: vec![], is_collapsed: false },
        ];
        assert_eq!(threads_overlapping_range(&threads, "file:///a.rs", 3, 12).len(), 2);
        assert_eq!(threads_overlapping_range(&threads, "file:///a.rs", 6, 9).len(), 0);
    }

    #[test]
    fn comment_count_by_author_counts() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5,
                comments: vec![Comment::new("c1", "hi", "alice"), Comment::new("c2", "hey", "bob"), Comment::new("c3", "yo", "alice")],
                is_collapsed: false },
        ];
        assert_eq!(comment_count_by_author(&threads, "alice"), 2);
        assert_eq!(comment_count_by_author(&threads, "charlie"), 0);
    }

    #[test]
    fn all_comment_bodies_collects() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5,
                comments: vec![Comment::new("c1", "hello", "alice"), Comment::new("c2", "world", "bob")],
                is_collapsed: false },
        ];
        assert_eq!(all_comment_bodies(&threads), vec!["hello", "world"]);
    }

    #[test]
    fn collapse_empty_threads_collapses() {
        let mut threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 5, comments: vec![Comment::new("c1", "hi", "alice")], is_collapsed: false },
            CommentThread { id: "t2".into(), uri: "file:///b.rs".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: false },
        ];
        collapse_empty_threads(&mut threads);
        assert!(!threads[0].is_collapsed);
        assert!(threads[1].is_collapsed);
    }

    // ── CommentThreadFilter tests ──

    #[test]
    fn filter_by_collapsed_and_uncollapsed() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: true },
            CommentThread { id: "t2".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: false },
            CommentThread { id: "t3".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: true },
        ];
        assert_eq!(CommentThreadFilter::by_collapsed(&threads).len(), 2);
        assert_eq!(CommentThreadFilter::by_uncollapsed(&threads).len(), 1);
    }

    #[test]
    fn filter_by_uri() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "file:///a.rs".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: false },
            CommentThread { id: "t2".into(), uri: "file:///b.rs".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: false },
        ];
        assert_eq!(CommentThreadFilter::by_uri(&threads, "file:///a.rs").len(), 1);
        assert!(CommentThreadFilter::by_uri(&threads, "file:///c.rs").is_empty());
    }

    #[test]
    fn filter_by_author() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![Comment::new("c1", "hi", "alice")], is_collapsed: false },
            CommentThread { id: "t2".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![Comment::new("c2", "yo", "bob")], is_collapsed: false },
            CommentThread { id: "t3".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![], is_collapsed: false },
        ];
        assert_eq!(CommentThreadFilter::by_author(&threads, "alice").len(), 1);
        assert_eq!(CommentThreadFilter::by_author(&threads, "charlie").len(), 0);
    }

    #[test]
    fn filter_with_min_comments() {
        let threads = vec![
            CommentThread { id: "t1".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![Comment::new("c1", "a", "x"), Comment::new("c2", "b", "y")], is_collapsed: false },
            CommentThread { id: "t2".into(), uri: "u".into(), range_start_line: 1, range_end_line: 2, comments: vec![Comment::new("c3", "c", "z")], is_collapsed: false },
        ];
        assert_eq!(CommentThreadFilter::with_min_comments(&threads, 2).len(), 1);
        assert_eq!(CommentThreadFilter::with_min_comments(&threads, 1).len(), 2);
    }

    // ── CommentReactionAggregator tests ──

    #[test]
    fn reaction_aggregator_add_and_total() {
        let mut agg = CommentReactionAggregator::new();
        agg.add_reaction("thumbsup", 3);
        agg.add_reaction("heart", 2);
        agg.add_reaction("thumbsup", 1);
        assert_eq!(agg.total(), 6);
    }

    #[test]
    fn reaction_aggregator_most_popular() {
        let mut agg = CommentReactionAggregator::new();
        agg.add_reaction("heart", 5);
        agg.add_reaction("laugh", 2);
        let (label, count) = agg.most_popular().unwrap();
        assert_eq!(label, "heart");
        assert_eq!(count, 5);
    }

    #[test]
    fn reaction_aggregator_merge_and_clear() {
        let mut a = CommentReactionAggregator::new();
        a.add_reaction("rocket", 1);
        let mut b = CommentReactionAggregator::new();
        b.add_reaction("rocket", 4);
        b.add_reaction("eyes", 2);
        a.merge(&b);
        assert_eq!(a.total(), 7);
        a.clear();
        assert_eq!(a.total(), 0);
        assert!(a.most_popular().is_none());
    }

    #[test]
    fn reaction_aggregator_display() {
        let mut agg = CommentReactionAggregator::new();
        agg.add_reaction("x", 3);
        assert_eq!(format!("{agg}"), "3 reactions");
    }

    // ── CommentDraftManager tests ──

    #[test]
    fn draft_manager_save_get_remove() {
        let mut mgr = CommentDraftManager::new();
        mgr.save_draft("t1", "wip text", 100);
        assert!(mgr.has_draft("t1"));
        assert_eq!(mgr.draft_count(), 1);
        let d = mgr.get_draft("t1").unwrap();
        assert_eq!(d.body, "wip text");
        assert_eq!(d.created_at, 100);
        let removed = mgr.remove_draft("t1").unwrap();
        assert_eq!(removed.body, "wip text");
        assert!(!mgr.has_draft("t1"));
    }

    #[test]
    fn draft_manager_all_drafts_sorted() {
        let mut mgr = CommentDraftManager::new();
        mgr.save_draft("t2", "second", 200);
        mgr.save_draft("t1", "first", 100);
        mgr.save_draft("t3", "third", 300);
        let all = mgr.all_drafts();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].created_at, 100);
        assert_eq!(all[2].created_at, 300);
        mgr.clear_all();
        assert_eq!(mgr.draft_count(), 0);
    }

    // ── CommentRangeHighlightTracker tests ──

    #[test]
    fn highlight_tracker_track_and_query() {
        let mut tracker = CommentRangeHighlightTracker::new();
        tracker.track("t1", 10, 20);
        tracker.track("t2", 15, 25);
        assert!(tracker.is_tracked("t1"));
        assert!(!tracker.is_tracked("t3"));
        assert_eq!(tracker.lines_for("t1"), Some((10, 20)));
        assert_eq!(tracker.count(), 2);
    }

    #[test]
    fn highlight_tracker_overlapping() {
        let mut tracker = CommentRangeHighlightTracker::new();
        tracker.track("t1", 10, 20);
        tracker.track("t2", 15, 25);
        tracker.track("t3", 30, 40);
        let mut ids = tracker.overlapping(17);
        ids.sort();
        assert_eq!(ids, vec!["t1", "t2"]);
        assert!(tracker.overlapping(28).is_empty());
    }

    #[test]
    fn highlight_tracker_untrack() {
        let mut tracker = CommentRangeHighlightTracker::new();
        tracker.track("t1", 1, 5);
        assert!(tracker.untrack("t1"));
        assert!(!tracker.untrack("t1"));
        assert_eq!(tracker.count(), 0);
    }

    #[test] fn commentDraftAutoSave_new() { let s = CommentDraftAutoSave::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn commentDraftAutoSave_add() { let mut s = CommentDraftAutoSave::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn commentDraftAutoSave_remove() { let mut s = CommentDraftAutoSave::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn commentDraftAutoSave_config() { let mut s = CommentDraftAutoSave::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn commentDraftAutoSave_nav() { let mut s = CommentDraftAutoSave::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn commentDraftAutoSave_filter() { let mut s = CommentDraftAutoSave::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn commentDraftAutoSave_display() { assert!(format!("{}", CommentDraftAutoSave::new()).contains("CommentDraftAutoSave")); }
    #[test] fn commentMentionResolver_new() { let s = CommentMentionResolver::new(); assert!(s.is_empty()); }
    #[test] fn commentMentionResolver_add() { let mut s = CommentMentionResolver::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn commentMentionResolver_active() { let mut s = CommentMentionResolver::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn commentMentionResolver_error() { let mut s = CommentMentionResolver::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn commentMentionResolver_rm_group() { let mut s = CommentMentionResolver::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn commentMentionResolver_display() { assert!(format!("{}", CommentMentionResolver::new()).contains("CommentMentionResolver")); }


    #[test] fn commentDraftAutoSave_snap_capture() {
        let s = CommentDraftAutoSave::new();
        let snap = CommentDraftAutoSaveSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn commentDraftAutoSave_snap_stale() {
        let s = CommentDraftAutoSave::new();
        let snap = CommentDraftAutoSaveSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn commentDraftAutoSave_snap_diff() {
        let s = CommentDraftAutoSave::new();
        let s1v = CommentDraftAutoSaveSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn commentDraftAutoSave_snap_display() {
        let s = CommentDraftAutoSave::new();
        let snap = CommentDraftAutoSaveSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn commentMentionResolver_stats_record() {
        let mut st = CommentMentionResolverStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn commentMentionResolver_stats_hit_ratio() {
        let mut st = CommentMentionResolverStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn commentMentionResolver_stats_merge() {
        let mut a = CommentMentionResolverStats::new();
        a.total_adds = 5;
        let mut b = CommentMentionResolverStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn commentMentionResolver_stats_display() {
        let st = CommentMentionResolverStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn commentDraftAutoSave_config_default() {
        let c = CommentDraftAutoSaveConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn commentDraftAutoSave_config_builder() {
        let c = CommentDraftAutoSaveConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn commentDraftAutoSave_config_labels() {
        let mut c = CommentDraftAutoSaveConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn commentDraftAutoSave_config_cleanup_threshold() {
        let c = CommentDraftAutoSaveConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn commentDraftAutoSave_config_display() {
        assert!(format!("{}", CommentDraftAutoSaveConfig::new()).contains("Config"));
    }
    #[test] fn commentMentionResolver_stats_peaks() {
        let mut st = CommentMentionResolverStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }


    #[test]
    fn extComments_x_config_new() {
        let c = ExtCommentsXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn extComments_x_config_builder() {
        let c = ExtCommentsXConfig::new("k")
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
    fn extComments_x_config_display() {
        let c = ExtCommentsXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn extComments_x_registry_insert_get() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extComments_x_registry_duplicate() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a")).unwrap();
        assert!(reg.insert(ExtCommentsXConfig::new("a")).is_err());
    }

    #[test]
    fn extComments_x_registry_remove() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a")).unwrap();
        reg.insert(ExtCommentsXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extComments_x_registry_active_entries() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a")).unwrap();
        reg.insert(ExtCommentsXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn extComments_x_registry_by_weight() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ExtCommentsXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn extComments_x_registry_tags() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ExtCommentsXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn extComments_x_registry_total_weight() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ExtCommentsXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn extComments_x_registry_iterator() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a")).unwrap();
        reg.insert(ExtCommentsXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn extComments_x_cache_put_get() {
        let mut cache = ExtCommentsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn extComments_x_cache_eviction() {
        let mut cache = ExtCommentsXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn extComments_x_cache_lru_order() {
        let mut cache = ExtCommentsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn extComments_x_cache_most_least_recent() {
        let mut cache = ExtCommentsXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn extComments_x_formatter_entry() {
        let e = ExtCommentsXConfig::new("k").with_value("v");
        let fmt = ExtCommentsXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn extComments_x_formatter_summary() {
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ExtCommentsXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn extComments_x_validator_valid() {
        let v = ExtCommentsXValidator::new();
        let c = ExtCommentsXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn extComments_x_validator_empty_key() {
        let v = ExtCommentsXValidator::new();
        let c = ExtCommentsXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extComments_x_validator_require_value() {
        let v = ExtCommentsXValidator::new().require_value(true);
        let c = ExtCommentsXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extComments_x_validator_allowed_tags() {
        let v = ExtCommentsXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ExtCommentsXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extComments_x_validator_validate_all() {
        let v = ExtCommentsXValidator::new();
        let mut reg = ExtCommentsXRegistry::new();
        reg.insert(ExtCommentsXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn yo_metrics_empty() {
        let m = YoMetrics::new("ext_cmt");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yo_metrics_record_and_mean() {
        let mut m = YoMetrics::new("ext_cmt");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yo_metrics_min_max() {
        let mut m = YoMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yo_metrics_variance_and_std() {
        let mut m = YoMetrics::new("v");
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
    fn yo_metrics_percentile() {
        let mut m = YoMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yo_metrics_merge() {
        let mut a = YoMetrics::new("a");
        a.record(1.0);
        let mut b = YoMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yo_metrics_reset() {
        let mut m = YoMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yo_rate_window_empty() {
        let rw = YoRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yo_rate_window_tick_and_rate() {
        let mut rw = YoRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yo_lru_cache_basic() {
        let mut c = YoLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yo_lru_cache_contains_and_keys() {
        let mut c = YoLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yo_lru_cache_remove() {
        let mut c = YoLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yo_metrics_sum() {
        let mut m = YoMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yo_metrics_label() {
        let m = YoMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yo_lru_cache_clear() {
        let mut c = YoLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_comments
    #[test]
    fn xa_ext_comments_ring_new() {
        let rb = super::XaExtCommentsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_comments_ring_push_len() {
        let mut rb = super::XaExtCommentsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_comments_ring_wrap() {
        let mut rb = super::XaExtCommentsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_comments_ring_mean_empty() {
        let rb = super::XaExtCommentsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_comments_ring_mean_values() {
        let mut rb = super::XaExtCommentsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_comments_ring_min_max() {
        let mut rb = super::XaExtCommentsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_comments_ring_iter() {
        let mut rb = super::XaExtCommentsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_comments_counter_new() {
        let c = super::XaExtCommentsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_comments_counter_inc() {
        let mut c = super::XaExtCommentsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_comments_counter_inc_by() {
        let mut c = super::XaExtCommentsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_comments_counter_reset() {
        let mut c = super::XaExtCommentsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_comments_counter_clear() {
        let mut c = super::XaExtCommentsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_comments_counter_default() {
        let c = super::XaExtCommentsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 53 ----

    #[test]
    fn xc_53_pool_new_empty() {
        let pool: super::Xc53Pool<i32> = super::Xc53Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_53_pool_release_acquire() {
        let mut pool = super::Xc53Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_53_pool_acquire_empty() {
        let mut pool: super::Xc53Pool<i32> = super::Xc53Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_53_pool_full() {
        let mut pool = super::Xc53Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_53_pool_drain() {
        let mut pool = super::Xc53Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_53_pool_stats() {
        let mut pool = super::Xc53Pool::new(8);
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
    fn xc_53_pool_clear() {
        let mut pool = super::Xc53Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_53_pool_shrink() {
        let mut pool = super::Xc53Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_53_pool_default() {
        let pool: super::Xc53Pool<String> = super::Xc53Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_53_pool_extend() {
        let mut pool = super::Xc53Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_53_pool_retain() {
        let mut pool = super::Xc53Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_53_scheduler_round_robin() {
        let mut sched = super::Xc53Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_53_scheduler_empty() {
        let mut sched = super::Xc53Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_53_scheduler_reset() {
        let mut sched = super::Xc53Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_53_scheduler_add_remove() {
        let mut sched = super::Xc53Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_53_scheduler_targets() {
        let sched = super::Xc53Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_53_hash_empty() {
        assert_eq!(super::xc_53_hash(b""), 5381);
    }

    #[test]
    fn xc_53_hash_data() {
        let h = super::xc_53_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_53_hash(b"hello"), h);
    }

    #[test]
    fn xc_53_reverse_str() {
        assert_eq!(super::xc_53_reverse("abc"), "cba");
        assert_eq!(super::xc_53_reverse(""), "");
    }


    // --- xd_98 deepening tests ---

    #[test]
    fn xd_98_sm_initial_state() {
        let sm = Xd98StateMachine::new();
        assert_eq!(sm.current_state(), Xd98State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_98_sm_valid_idle_to_running() {
        let mut sm = Xd98StateMachine::new();
        assert!(sm.transition(Xd98State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd98State::Running);
    }

    #[test]
    fn xd_98_sm_valid_running_to_paused() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        assert!(sm.transition(Xd98State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd98State::Paused);
    }

    #[test]
    fn xd_98_sm_valid_running_to_done() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        assert!(sm.transition(Xd98State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd98State::Done);
    }

    #[test]
    fn xd_98_sm_valid_paused_to_running() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        sm.transition(Xd98State::Paused).unwrap();
        assert!(sm.transition(Xd98State::Running).is_ok());
    }

    #[test]
    fn xd_98_sm_valid_done_to_idle() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        sm.transition(Xd98State::Done).unwrap();
        assert!(sm.transition(Xd98State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd98State::Idle);
    }

    #[test]
    fn xd_98_sm_invalid_idle_to_done() {
        let mut sm = Xd98StateMachine::new();
        assert!(sm.transition(Xd98State::Done).is_err());
    }

    #[test]
    fn xd_98_sm_invalid_idle_to_paused() {
        let mut sm = Xd98StateMachine::new();
        assert!(sm.transition(Xd98State::Paused).is_err());
    }

    #[test]
    fn xd_98_sm_history_tracking() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        sm.transition(Xd98State::Paused).unwrap();
        sm.transition(Xd98State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd98State::Idle);
        assert_eq!(sm.history()[0].to, Xd98State::Running);
        assert_eq!(sm.history()[1].from, Xd98State::Running);
        assert_eq!(sm.history()[2].to, Xd98State::Done);
    }

    #[test]
    fn xd_98_sm_serialize_deserialize() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd98StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd98State::Running));
    }

    #[test]
    fn xd_98_sm_deserialize_invalid() {
        assert_eq!(Xd98StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_98_sm_reset() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd98State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_98_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd98EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd98Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_98_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd98EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd98Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd98Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_98_bus_unsubscribe() {
        let mut bus = Xd98EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_98_event_kind_and_payload() {
        let e = Xd98Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd98Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_98_bus_clear_history() {
        let mut bus = Xd98EventBus::new();
        bus.publish(Xd98Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_98_sm_step_counter_increments() {
        let mut sm = Xd98StateMachine::new();
        sm.transition(Xd98State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd98State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_22 graph tests ------------------------------------------------

    #[test]
    fn xg_22_graph_empty() {
        let g = super::Xg22Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_22_graph_add_node() {
        let mut g = super::Xg22Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_22_graph_add_edge() {
        let mut g = super::Xg22Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_22_graph_neighbors() {
        let mut g = super::Xg22Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_22_graph_has_path() {
        let mut g = super::Xg22Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_22_graph_self_path() {
        let g = super::Xg22Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_22_graph_topo_sort() {
        let mut g = super::Xg22Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_22_graph_cycle_detect_false() {
        let mut g = super::Xg22Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_22_graph_cycle_detect_true() {
        let mut g = super::Xg22Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_22 heap tests -------------------------------------------------

    #[test]
    fn xg_22_heap_empty() {
        let h: super::Xg22Heap<i32> = super::Xg22Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_22_heap_push_pop() {
        let mut h = super::Xg22Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_22_heap_peek() {
        let mut h = super::Xg22Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_22_heap_drain_sorted() {
        let mut h = super::Xg22Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_22_heap_merge() {
        let mut a = super::Xg22Heap::new();
        let mut b = super::Xg22Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_22_heap_default() {
        let h: super::Xg22Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_22_graph_default() {
        let g: super::Xg22Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh52_skip_insert_contains() {
        let mut sl = super::Xh52SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh52_skip_remove() {
        let mut sl = super::Xh52SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh52_skip_len() {
        let mut sl = super::Xh52SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh52_skip_range_query() {
        let mut sl = super::Xh52SkipList::xh_new(4);
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
    fn xh52_skip_floor_ceiling() {
        let mut sl = super::Xh52SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh52_skip_rank() {
        let mut sl = super::Xh52SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh52_skip_empty() {
        let sl = super::Xh52SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh52_skip_duplicates() {
        let mut sl = super::Xh52SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh52_bitset_set_test() {
        let mut bs = super::Xh52BitSet::xh_new(256);
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
    fn xh52_bitset_clear_count() {
        let mut bs = super::Xh52BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh52_bitset_and_or_xor() {
        let mut a = super::Xh52BitSet::xh_new(128);
        let mut b = super::Xh52BitSet::xh_new(128);
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
    fn xh52_bitset_iter_ones() {
        let mut bs = super::Xh52BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh52_bitset_first_last() {
        let mut bs = super::Xh52BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh52_bitset_empty() {
        let bs = super::Xh52BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi52_deque_push_pop_back() {
        let mut dq = super::Xi52Deque::xi_new(4);
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
    fn xi52_deque_push_pop_front() {
        let mut dq = super::Xi52Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi52_deque_mixed_ops() {
        let mut dq = super::Xi52Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi52_deque_get_and_split() {
        let mut dq = super::Xi52Deque::xi_new(8);
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
    fn xi52_deque_rotate_left() {
        let mut dq = super::Xi52Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi52_deque_rotate_right() {
        let mut dq = super::Xi52Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi52_deque_grow() {
        let mut dq = super::Xi52Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi52_deque_empty() {
        let dq = super::Xi52Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi52_interval_tree_insert_query() {
        let mut tree = super::Xi52IntervalTree::xi_new();
        tree.xi_insert(super::Xi52Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi52Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi52Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi52_interval_tree_overlap() {
        let mut tree = super::Xi52IntervalTree::xi_new();
        tree.xi_insert(super::Xi52Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi52Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi52Interval::xi_new(12, 20));
        let q = super::Xi52Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi52_interval_tree_remove() {
        let mut tree = super::Xi52IntervalTree::xi_new();
        tree.xi_insert(super::Xi52Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi52Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi52_interval_tree_gaps() {
        let mut tree = super::Xi52IntervalTree::xi_new();
        tree.xi_insert(super::Xi52Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi52Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi52Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi52Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi52Interval::xi_new(8, 10));
    }

    #[test]
    fn xi52_interval_tree_merge() {
        let mut tree = super::Xi52IntervalTree::xi_new();
        tree.xi_insert(super::Xi52Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi52Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi52Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi52Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi52Interval::xi_new(10, 15));
    }

    #[test]
    fn xi52_interval_tree_all() {
        let mut tree = super::Xi52IntervalTree::xi_new();
        tree.xi_insert(super::Xi52Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi52Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi52_interval_tree_empty() {
        let tree = super::Xi52IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi52_interval_tree_contains_point() {
        let iv = super::Xi52Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 52) ---

    #[test]
    fn xj_52_uf_make_and_find() {
        let mut uf = super::Xj52UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_52_uf_union_connected() {
        let mut uf = super::Xj52UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_52_uf_component_count() {
        let mut uf = super::Xj52UnionFind::xj_new();
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
    fn xj_52_uf_component_size() {
        let mut uf = super::Xj52UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_52_uf_largest_component() {
        let mut uf = super::Xj52UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_52_uf_many_elements() {
        let mut uf = super::Xj52UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_52_uf_separate_components() {
        let mut uf = super::Xj52UnionFind::xj_new();
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
    fn xj_52_uf_path_compression() {
        let mut uf = super::Xj52UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_52_bt_insert_get() {
        let mut bt = super::Xj52BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_52_bt_contains_len() {
        let mut bt = super::Xj52BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_52_bt_replace() {
        let mut bt = super::Xj52BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_52_bt_remove() {
        let mut bt = super::Xj52BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_52_bt_keys_values() {
        let mut bt = super::Xj52BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_52_bt_range() {
        let mut bt = super::Xj52BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_52_bt_min_max() {
        let mut bt = super::Xj52BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_52_bt_many_inserts() {
        let mut bt = super::Xj52BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_52 segment tree tests ---

    #[test]
    fn xk_52_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk52SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_52_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk52SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_52_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk52SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_52_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk52SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_52_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk52SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_52_st_single_element() {
        let data = vec![42];
        let st = super::Xk52SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_52_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk52SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_52_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk52SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_52 disjoint intervals tests ---

    #[test]
    fn xk_52_di_add_and_count() {
        let mut di = super::Xk52DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_52_di_merge_overlap() {
        let mut di = super::Xk52DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_52_di_contains() {
        let mut di = super::Xk52DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_52_di_remove() {
        let mut di = super::Xk52DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_52_di_covered_length() {
        let mut di = super::Xk52DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_52_di_gaps() {
        let mut di = super::Xk52DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_52_di_merge_adjacent() {
        let mut di = super::Xk52DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_52_di_empty() {
        let di = super::Xk52DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
