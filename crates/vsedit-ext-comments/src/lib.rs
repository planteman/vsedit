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
}
