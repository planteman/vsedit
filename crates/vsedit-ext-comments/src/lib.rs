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

}
