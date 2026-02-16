//! Ext API: Documents.
//!
//! RPC bridge between the extension host and the main thread for documents.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_documents";

// ── RPC message types ──

/// Messages exchanged for the `TextDocument` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DocumentMessage {
    Open { uri: String, language_id: String, version: u32, content: String },
    Close { uri: String },
    Change { uri: String, version: u32, changes: Vec<TextEdit>, sync_kind: DocumentSyncKind },
    Save { uri: String },
    GetContent { uri: String },
    GetLanguage { uri: String },
    GetUri { uri: String },
}

/// How the extension host synchronises document content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentSyncKind {
    Full,
    Incremental,
}

/// A single text edit within a document change event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub text: String,
}

/// Notification sent when a document changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentChangeEvent {
    pub uri: String,
    pub changes: Vec<TextEdit>,
    pub version: u32,
}

/// Response payload returned by document queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DocumentResponse {
    Content { text: String },
    Language { language_id: String },
    Uri { uri: String },
    Ok,
}

// ── Bridge ──

/// Tracks open documents and processes sync messages from the extension host.
#[derive(Debug, Default)]
pub struct DocumentBridge {
    /// URI → current content.
    documents: HashMap<String, DocumentState>,
}

#[derive(Debug, Clone)]
struct DocumentState {
    content: String,
    language_id: String,
    version: u32,
}

impl DocumentBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an incoming document message and return a response.
    pub fn handle(&mut self, msg: DocumentMessage) -> DocumentResponse {
        match msg {
            DocumentMessage::Open { uri, language_id, version, content } => {
                self.documents.insert(uri, DocumentState { content, language_id, version });
                DocumentResponse::Ok
            }
            DocumentMessage::Close { uri } => {
                self.documents.remove(&uri);
                DocumentResponse::Ok
            }
            DocumentMessage::Change { uri, version, changes, sync_kind } => {
                if let Some(state) = self.documents.get_mut(&uri) {
                    state.version = version;
                    match sync_kind {
                        DocumentSyncKind::Full => {
                            if let Some(edit) = changes.first() {
                                state.content = edit.text.clone();
                            }
                        }
                        DocumentSyncKind::Incremental => {
                            // Incremental edits would be applied via rope/offset logic;
                            // for the bridge layer we accept the last edit text as content.
                            if let Some(edit) = changes.last() {
                                state.content = edit.text.clone();
                            }
                        }
                    }
                }
                DocumentResponse::Ok
            }
            DocumentMessage::Save { .. } => DocumentResponse::Ok,
            DocumentMessage::GetContent { uri } => {
                self.documents.get(&uri).map_or(
                    DocumentResponse::Content { text: String::new() },
                    |s| DocumentResponse::Content { text: s.content.clone() },
                )
            }
            DocumentMessage::GetLanguage { uri } => {
                self.documents.get(&uri).map_or(
                    DocumentResponse::Language { language_id: String::new() },
                    |s| DocumentResponse::Language { language_id: s.language_id.clone() },
                )
            }
            DocumentMessage::GetUri { uri } => DocumentResponse::Uri { uri },
        }
    }

    /// Number of currently tracked documents.
    pub fn open_count(&self) -> usize {
        self.documents.len()
    }
}

// ── Error types ──

/// Errors that can occur when processing document operations.
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentError {
    /// The document was not found in the bridge.
    NotFound { uri: String },
    /// A version mismatch was detected.
    VersionMismatch { uri: String, expected: u32, actual: u32 },
    /// The URI is invalid or empty.
    InvalidUri(String),
    /// An edit range is out of bounds.
    EditOutOfBounds { line: u32, col: u32 },
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::NotFound { uri } => write!(f, "document not found: {uri}"),
            DocumentError::VersionMismatch { uri, expected, actual } => {
                write!(f, "version mismatch for {uri}: expected {expected}, got {actual}")
            }
            DocumentError::InvalidUri(reason) => write!(f, "invalid URI: {reason}"),
            DocumentError::EditOutOfBounds { line, col } => {
                write!(f, "edit out of bounds at line {line}, col {col}")
            }
        }
    }
}

impl std::error::Error for DocumentError {}

// ── TextEdit helpers ──

impl TextEdit {
    /// Create a new `TextEdit`.
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32, text: impl Into<String>) -> Self {
        Self { start_line, start_col, end_line, end_col, text: text.into() }
    }

    /// Returns true if this edit inserts text without replacing anything.
    pub fn is_insert(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    /// Returns true if the replacement text is empty (a deletion).
    pub fn is_delete(&self) -> bool {
        self.text.is_empty() && !self.is_insert()
    }

    /// Validate that the edit range is well-formed.
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.start_line > self.end_line
            || (self.start_line == self.end_line && self.start_col > self.end_col)
        {
            return Err(DocumentError::EditOutOfBounds {
                line: self.start_line,
                col: self.start_col,
            });
        }
        Ok(())
    }

    /// Number of lines spanned by this edit's range.
    pub fn span_lines(&self) -> u32 {
        self.end_line - self.start_line + 1
    }
}

// ── TextEditBuilder ──

/// Builder for constructing `TextEdit` values ergonomically.
#[derive(Debug, Clone, Default)]
pub struct TextEditBuilder {
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    text: String,
}

impl TextEditBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(mut self, line: u32, col: u32) -> Self {
        self.start_line = line;
        self.start_col = col;
        self
    }

    pub fn end(mut self, line: u32, col: u32) -> Self {
        self.end_line = line;
        self.end_col = col;
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Build and validate the `TextEdit`.
    pub fn build(self) -> Result<TextEdit, DocumentError> {
        let edit = TextEdit {
            start_line: self.start_line,
            start_col: self.start_col,
            end_line: self.end_line,
            end_col: self.end_col,
            text: self.text,
        };
        edit.validate()?;
        Ok(edit)
    }
}

// ── DocumentState Display ──

impl fmt::Display for DocumentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}@v{}] {} bytes", self.language_id, self.version, self.content.len())
    }
}

impl PartialEq for DocumentState {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content
            && self.language_id == other.language_id
            && self.version == other.version
    }
}

// ── DocumentBridge extended API ──

impl DocumentBridge {
    /// Process an incoming message with error handling.
    pub fn try_handle(&mut self, msg: DocumentMessage) -> Result<DocumentResponse, DocumentError> {
        match &msg {
            DocumentMessage::Open { uri, .. }
            | DocumentMessage::Close { uri }
            | DocumentMessage::Change { uri, .. }
            | DocumentMessage::Save { uri }
            | DocumentMessage::GetContent { uri }
            | DocumentMessage::GetLanguage { uri }
            | DocumentMessage::GetUri { uri } => {
                if uri.is_empty() {
                    return Err(DocumentError::InvalidUri("URI must not be empty".into()));
                }
            }
        }

        match &msg {
            DocumentMessage::GetContent { uri }
            | DocumentMessage::GetLanguage { uri }
            | DocumentMessage::Change { uri, .. } => {
                if !matches!(&msg, DocumentMessage::Open { .. }) && !self.documents.contains_key(uri.as_str()) {
                    return Err(DocumentError::NotFound { uri: uri.clone() });
                }
            }
            _ => {}
        }

        Ok(self.handle(msg))
    }

    /// Check whether a document is currently open.
    pub fn is_open(&self, uri: &str) -> bool {
        self.documents.contains_key(uri)
    }

    /// Get the version of an open document.
    pub fn version(&self, uri: &str) -> Option<u32> {
        self.documents.get(uri).map(|s| s.version)
    }

    /// Get the byte length of an open document's content.
    pub fn content_len(&self, uri: &str) -> Option<usize> {
        self.documents.get(uri).map(|s| s.content.len())
    }

    /// Return an iterator over all currently open URIs.
    pub fn open_uris(&self) -> impl Iterator<Item = &str> {
        self.documents.keys().map(|s| s.as_str())
    }

    /// Count the number of lines in a document's content.
    pub fn line_count(&self, uri: &str) -> Option<usize> {
        self.documents.get(uri).map(|s| s.content.lines().count().max(1))
    }

    /// Validate that a change applies to the expected version.
    pub fn validate_version(&self, uri: &str, expected: u32) -> Result<(), DocumentError> {
        match self.documents.get(uri) {
            None => Err(DocumentError::NotFound { uri: uri.to_string() }),
            Some(state) if state.version != expected => Err(DocumentError::VersionMismatch {
                uri: uri.to_string(),
                expected,
                actual: state.version,
            }),
            _ => Ok(()),
        }
    }
}

impl fmt::Display for DocumentBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DocumentBridge({} open)", self.documents.len())
    }
}

impl Clone for DocumentBridge {
    fn clone(&self) -> Self {
        Self { documents: self.documents.clone() }
    }
}

impl PartialEq for DocumentBridge {
    fn eq(&self, other: &Self) -> bool {
        self.documents == other.documents
    }
}

/// Validate a URI string has a scheme prefix.
pub fn validate_uri(uri: &str) -> Result<(), DocumentError> {
    if uri.is_empty() {
        return Err(DocumentError::InvalidUri("empty URI".into()));
    }
    if !uri.contains("://") {
        return Err(DocumentError::InvalidUri(format!("missing scheme in '{uri}'")));
    }
    Ok(())
}

/// Initialize the documents extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-documents operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtDocumentsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtDocumentsStats {
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
    pub fn merge(&mut self, other: &ExtDocumentsStats) {
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

impl Default for ExtDocumentsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtDocumentsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtDocumentsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-documents.
#[derive(Debug, Clone)]
pub struct ExtDocumentsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtDocumentsValidator {
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

impl Default for ExtDocumentsValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A content provider for virtual documents with a URI scheme.
#[derive(Debug, Clone)]
pub struct DocumentContentProvider {
    pub scheme: String,
    contents: HashMap<String, String>,
}

impl DocumentContentProvider {
    pub fn new(scheme: impl Into<String>) -> Self {
        Self { scheme: scheme.into(), contents: HashMap::new() }
    }

    /// Register content for a virtual URI.
    pub fn provide_content(&mut self, uri: &str, content: impl Into<String>) {
        self.contents.insert(uri.to_string(), content.into());
    }

    /// Retrieve content for a virtual URI.
    pub fn get_content(&self, uri: &str) -> Option<&str> {
        self.contents.get(uri).map(|s| s.as_str())
    }

    /// Remove a virtual document.
    pub fn remove(&mut self, uri: &str) -> bool {
        self.contents.remove(uri).is_some()
    }

    /// Check if a URI belongs to this provider's scheme.
    pub fn handles_uri(&self, uri: &str) -> bool {
        uri.starts_with(&format!("{}://", self.scheme))
    }

    /// Number of virtual documents registered.
    pub fn count(&self) -> usize {
        self.contents.len()
    }

    /// List all registered URIs.
    pub fn uris(&self) -> Vec<&str> {
        self.contents.keys().map(|s| s.as_str()).collect()
    }
}

impl fmt::Display for DocumentContentProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DocumentContentProvider(scheme={}, count={})", self.scheme, self.contents.len())
    }
}

/// A single diff hunk between two document versions.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub content: String,
}

impl fmt::Display for DiffHunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@@ -{},{} +{},{} @@", self.old_start, self.old_count, self.new_start, self.new_count)
    }
}

/// Result of a document diff operation.
#[derive(Debug, Clone)]
pub struct DocumentDiffResult {
    pub hunks: Vec<DiffHunk>,
    pub has_changes: bool,
    pub additions: usize,
    pub deletions: usize,
}

impl DocumentDiffResult {
    pub fn no_changes() -> Self {
        Self { hunks: Vec::new(), has_changes: false, additions: 0, deletions: 0 }
    }
}

/// Compare two document contents line-by-line, returning diff hunks.
pub fn document_diff(old: &str, new: &str) -> DocumentDiffResult {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    if old_lines == new_lines {
        return DocumentDiffResult::no_changes();
    }

    let mut hunks = Vec::new();
    let mut additions = 0usize;
    let mut deletions = 0usize;
    let max_len = old_lines.len().max(new_lines.len());
    let mut i = 0;
    while i < max_len {
        let old_line = old_lines.get(i).copied();
        let new_line = new_lines.get(i).copied();
        if old_line != new_line {
            let hunk_start = i;
            let mut hunk_content = String::new();
            let mut old_count = 0;
            let mut new_count = 0;
            while i < max_len && old_lines.get(i).copied() != new_lines.get(i).copied() {
                if let Some(ol) = old_lines.get(i) {
                    hunk_content.push_str(&format!("-{}\n", ol));
                    old_count += 1;
                    deletions += 1;
                }
                if let Some(nl) = new_lines.get(i) {
                    hunk_content.push_str(&format!("+{}\n", nl));
                    new_count += 1;
                    additions += 1;
                }
                i += 1;
            }
            hunks.push(DiffHunk {
                old_start: hunk_start + 1,
                old_count,
                new_start: hunk_start + 1,
                new_count,
                content: hunk_content,
            });
        } else {
            i += 1;
        }
    }

    DocumentDiffResult { hunks, has_changes: true, additions, deletions }
}

/// Accumulates multiple text edits before applying them as a batch.
#[derive(Debug, Clone, Default)]
pub struct DocumentChangeAccumulator {
    uri: String,
    edits: Vec<TextEdit>,
}

impl DocumentChangeAccumulator {
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into(), edits: Vec::new() }
    }

    /// Add an edit to the batch.
    pub fn add_edit(&mut self, edit: TextEdit) {
        self.edits.push(edit);
    }

    /// Add an insertion at a specific position.
    pub fn insert(&mut self, line: u32, col: u32, text: impl Into<String>) {
        self.edits.push(TextEdit::new(line, col, line, col, text));
    }

    /// Add a deletion of a range.
    pub fn delete(&mut self, start_line: u32, start_col: u32, end_line: u32, end_col: u32) {
        self.edits.push(TextEdit::new(start_line, start_col, end_line, end_col, ""));
    }

    /// Number of accumulated edits.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    /// Build a DocumentMessage::Change from the accumulated edits.
    pub fn to_change_message(&self, version: u32) -> DocumentMessage {
        DocumentMessage::Change {
            uri: self.uri.clone(),
            version,
            changes: self.edits.clone(),
            sync_kind: DocumentSyncKind::Incremental,
        }
    }

    /// Clear all accumulated edits.
    pub fn clear(&mut self) {
        self.edits.clear();
    }

    /// Get the target URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Check if there are any accumulated edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Validate all accumulated edits.
    pub fn validate_all(&self) -> Result<(), DocumentError> {
        for edit in &self.edits {
            edit.validate()?;
        }
        Ok(())
    }
}

impl fmt::Display for DocumentChangeAccumulator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DocumentChangeAccumulator(uri={}, edits={})", self.uri, self.edits.len())
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
    fn open_and_get_content() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///a.rs".into(),
            language_id: "rust".into(),
            version: 1,
            content: "fn main() {}".into(),
        });
        assert_eq!(bridge.open_count(), 1);
        let resp = bridge.handle(DocumentMessage::GetContent { uri: "file:///a.rs".into() });
        assert_eq!(resp, DocumentResponse::Content { text: "fn main() {}".into() });
    }

    #[test]
    fn close_removes_document() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///b.rs".into(),
            language_id: "rust".into(),
            version: 1,
            content: "".into(),
        });
        assert_eq!(bridge.open_count(), 1);
        bridge.handle(DocumentMessage::Close { uri: "file:///b.rs".into() });
        assert_eq!(bridge.open_count(), 0);
    }

    #[test]
    fn full_sync_replaces_content() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///c.rs".into(),
            language_id: "rust".into(),
            version: 1,
            content: "old".into(),
        });
        bridge.handle(DocumentMessage::Change {
            uri: "file:///c.rs".into(),
            version: 2,
            changes: vec![TextEdit {
                start_line: 0, start_col: 0, end_line: 0, end_col: 3,
                text: "new".into(),
            }],
            sync_kind: DocumentSyncKind::Full,
        });
        let resp = bridge.handle(DocumentMessage::GetContent { uri: "file:///c.rs".into() });
        assert_eq!(resp, DocumentResponse::Content { text: "new".into() });
    }

    #[test]
    fn get_language() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///d.py".into(),
            language_id: "python".into(),
            version: 1,
            content: "".into(),
        });
        let resp = bridge.handle(DocumentMessage::GetLanguage { uri: "file:///d.py".into() });
        assert_eq!(resp, DocumentResponse::Language { language_id: "python".into() });
    }

    #[test]
    fn serde_round_trip() {
        let msg = DocumentMessage::Open {
            uri: "file:///e.ts".into(),
            language_id: "typescript".into(),
            version: 1,
            content: "export {}".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: DocumentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn text_edit_is_insert() {
        let edit = TextEdit::new(5, 3, 5, 3, "hello");
        assert!(edit.is_insert());
        assert!(!edit.is_delete());
    }

    #[test]
    fn text_edit_is_delete() {
        let edit = TextEdit::new(0, 0, 0, 5, "");
        assert!(edit.is_delete());
        assert!(!edit.is_insert());
    }

    #[test]
    fn text_edit_validate_ok() {
        let edit = TextEdit::new(1, 0, 2, 5, "x");
        assert!(edit.validate().is_ok());
    }

    #[test]
    fn text_edit_validate_bad_range() {
        let edit = TextEdit::new(5, 0, 3, 0, "x");
        assert!(matches!(edit.validate(), Err(DocumentError::EditOutOfBounds { .. })));
    }

    #[test]
    fn text_edit_span_lines() {
        let edit = TextEdit::new(2, 0, 7, 0, "");
        assert_eq!(edit.span_lines(), 6);
    }

    #[test]
    fn builder_pattern() {
        let edit = TextEditBuilder::new()
            .start(1, 0)
            .end(1, 5)
            .text("replacement")
            .build()
            .unwrap();
        assert_eq!(edit.start_line, 1);
        assert_eq!(edit.end_col, 5);
        assert_eq!(edit.text, "replacement");
    }

    #[test]
    fn builder_rejects_invalid_range() {
        let result = TextEditBuilder::new()
            .start(10, 0)
            .end(5, 0)
            .text("bad")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn try_handle_empty_uri_error() {
        let mut bridge = DocumentBridge::new();
        let result = bridge.try_handle(DocumentMessage::GetContent { uri: "".into() });
        assert!(matches!(result, Err(DocumentError::InvalidUri(_))));
    }

    #[test]
    fn try_handle_not_found_error() {
        let mut bridge = DocumentBridge::new();
        let result = bridge.try_handle(DocumentMessage::GetContent { uri: "file:///missing.rs".into() });
        assert!(matches!(result, Err(DocumentError::NotFound { .. })));
    }

    #[test]
    fn bridge_is_open_and_version() {
        let mut bridge = DocumentBridge::new();
        assert!(!bridge.is_open("file:///f.rs"));
        bridge.handle(DocumentMessage::Open {
            uri: "file:///f.rs".into(),
            language_id: "rust".into(),
            version: 42,
            content: "let x = 1;".into(),
        });
        assert!(bridge.is_open("file:///f.rs"));
        assert_eq!(bridge.version("file:///f.rs"), Some(42));
        assert_eq!(bridge.content_len("file:///f.rs"), Some(10));
    }

    #[test]
    fn bridge_line_count() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///g.rs".into(),
            language_id: "rust".into(),
            version: 1,
            content: "line1\nline2\nline3".into(),
        });
        assert_eq!(bridge.line_count("file:///g.rs"), Some(3));
    }

    #[test]
    fn validate_version_ok_and_mismatch() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///h.rs".into(),
            language_id: "rust".into(),
            version: 5,
            content: "".into(),
        });
        assert!(bridge.validate_version("file:///h.rs", 5).is_ok());
        let err = bridge.validate_version("file:///h.rs", 3).unwrap_err();
        assert!(matches!(err, DocumentError::VersionMismatch { expected: 3, actual: 5, .. }));
    }

    #[test]
    fn validate_uri_helper() {
        assert!(validate_uri("file:///ok.rs").is_ok());
        assert!(validate_uri("").is_err());
        assert!(validate_uri("no-scheme").is_err());
    }

    #[test]
    fn bridge_display_and_clone() {
        let mut bridge = DocumentBridge::new();
        assert_eq!(format!("{bridge}"), "DocumentBridge(0 open)");
        bridge.handle(DocumentMessage::Open {
            uri: "file:///i.rs".into(),
            language_id: "rust".into(),
            version: 1,
            content: "code".into(),
        });
        let cloned = bridge.clone();
        assert_eq!(bridge, cloned);
        assert_eq!(format!("{bridge}"), "DocumentBridge(1 open)");
    }

    #[test]
    fn document_error_display() {
        let err = DocumentError::NotFound { uri: "file:///z.rs".into() };
        assert_eq!(format!("{err}"), "document not found: file:///z.rs");
        let err2 = DocumentError::VersionMismatch { uri: "x".into(), expected: 1, actual: 2 };
        assert!(format!("{err2}").contains("mismatch"));
    }

    #[test]
    fn open_uris_iterator() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///j.rs".into(),
            language_id: "rust".into(),
            version: 1,
            content: "".into(),
        });
        bridge.handle(DocumentMessage::Open {
            uri: "file:///k.rs".into(),
            language_id: "rust".into(),
            version: 1,
            content: "".into(),
        });
        let uris: Vec<&str> = bridge.open_uris().collect();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&"file:///j.rs"));
        assert!(uris.contains(&"file:///k.rs"));
    }

    #[test]
    fn ext_documents_stats_new_defaults() {
        let stats = ExtDocumentsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_documents_stats_record_success() {
        let mut stats = ExtDocumentsStats::new();
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
    fn ext_documents_stats_record_failure() {
        let mut stats = ExtDocumentsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_documents_stats_reset() {
        let mut stats = ExtDocumentsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_documents_stats_merge() {
        let mut a = ExtDocumentsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtDocumentsStats::new();
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
    fn ext_documents_stats_display() {
        let mut stats = ExtDocumentsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_documents_stats_default() {
        let stats = ExtDocumentsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_documents_validator_accepts_valid_name() {
        let v = ExtDocumentsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_documents_validator_rejects_empty() {
        let v = ExtDocumentsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_documents_validator_rejects_too_long() {
        let v = ExtDocumentsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_documents_validator_forbidden_prefix() {
        let v = ExtDocumentsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_documents_validator_allowed_chars() {
        let v = ExtDocumentsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_documents_validator_range() {
        let v = ExtDocumentsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_documents_sanitize_removes_control() {
        let result = ExtDocumentsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_documents_truncate_short_string() {
        assert_eq!(ExtDocumentsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_documents_truncate_long_string() {
        let result = ExtDocumentsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_documents_is_ascii_printable() {
        assert!(ExtDocumentsValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtDocumentsValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn content_provider_handles_scheme() {
        let mut p = DocumentContentProvider::new("git");
        p.provide_content("git://HEAD/main.rs", "fn main() {}");
        assert!(p.handles_uri("git://HEAD/main.rs"));
        assert!(!p.handles_uri("file:///tmp/foo.rs"));
        assert_eq!(p.get_content("git://HEAD/main.rs"), Some("fn main() {}"));
    }

    #[test]
    fn content_provider_remove() {
        let mut p = DocumentContentProvider::new("test");
        p.provide_content("test://doc1", "hello");
        assert!(p.remove("test://doc1"));
        assert!(!p.remove("test://doc1"));
        assert_eq!(p.count(), 0);
    }

    #[test]
    fn content_provider_uris() {
        let mut p = DocumentContentProvider::new("mem");
        p.provide_content("mem://a", "a");
        p.provide_content("mem://b", "b");
        assert_eq!(p.count(), 2);
    }

    #[test]
    fn document_diff_identical() {
        let result = document_diff("hello\nworld", "hello\nworld");
        assert!(!result.has_changes);
        assert!(result.hunks.is_empty());
    }

    #[test]
    fn document_diff_single_line_change() {
        let result = document_diff("hello\nworld", "hello\nearth");
        assert!(result.has_changes);
        assert_eq!(result.additions, 1);
        assert_eq!(result.deletions, 1);
        assert_eq!(result.hunks.len(), 1);
    }

    #[test]
    fn document_diff_added_lines() {
        let result = document_diff("line1", "line1\nline2\nline3");
        assert!(result.has_changes);
        assert!(result.additions > 0);
    }

    #[test]
    fn diff_hunk_display() {
        let h = DiffHunk { old_start: 1, old_count: 2, new_start: 1, new_count: 3, content: String::new() };
        let s = format!("{}", h);
        assert!(s.contains("@@ -1,2 +1,3 @@"));
    }

    #[test]
    fn change_accumulator_basic() {
        let mut acc = DocumentChangeAccumulator::new("file:///test.rs");
        assert!(acc.is_empty());
        acc.insert(0, 0, "hello");
        acc.delete(1, 0, 1, 5);
        assert_eq!(acc.edit_count(), 2);
        assert_eq!(acc.uri(), "file:///test.rs");
    }

    #[test]
    fn change_accumulator_to_message() {
        let mut acc = DocumentChangeAccumulator::new("file:///foo.rs");
        acc.insert(0, 0, "test");
        let msg = acc.to_change_message(5);
        match msg {
            DocumentMessage::Change { version, uri, .. } => {
                assert_eq!(version, 5);
                assert_eq!(uri, "file:///foo.rs");
            }
            _ => panic!("expected Change message"),
        }
    }

    #[test]
    fn change_accumulator_validate_all() {
        let mut acc = DocumentChangeAccumulator::new("file:///x.rs");
        acc.insert(0, 0, "ok");
        assert!(acc.validate_all().is_ok());
    }

    #[test]
    fn change_accumulator_clear() {
        let mut acc = DocumentChangeAccumulator::new("file:///x.rs");
        acc.insert(0, 0, "text");
        acc.clear();
        assert!(acc.is_empty());
    }

    #[test]
    fn content_provider_display() {
        let p = DocumentContentProvider::new("vscode");
        let s = format!("{}", p);
        assert!(s.contains("vscode"));
    }
}
