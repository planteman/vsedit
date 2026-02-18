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

// ---------------------------------------------------------------------------
// DocumentVersion – version tracking with diff
// ---------------------------------------------------------------------------

/// Tracks document versions and their content snapshots.
#[derive(Debug, Clone)]
pub struct DocumentVersionEntry {
    pub version: u32,
    pub content: String,
}

/// Document version tracker with content diffing.
#[derive(Debug, Clone)]
pub struct DocumentVersionTracker {
    uri: String,
    versions: Vec<DocumentVersionEntry>,
}

impl DocumentVersionTracker {
    pub fn new(uri: impl Into<String>, initial_content: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            versions: vec![DocumentVersionEntry {
                version: 1,
                content: initial_content.into(),
            }],
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn current_version(&self) -> u32 {
        self.versions.last().map(|e| e.version).unwrap_or(0)
    }

    pub fn current_content(&self) -> &str {
        self.versions.last().map(|e| e.content.as_str()).unwrap_or("")
    }

    /// Record a new version with full content.
    pub fn push_version(&mut self, content: impl Into<String>) -> u32 {
        let v = self.current_version() + 1;
        self.versions.push(DocumentVersionEntry {
            version: v,
            content: content.into(),
        });
        v
    }

    pub fn version_count(&self) -> usize {
        self.versions.len()
    }

    /// Get content at a specific version.
    pub fn content_at(&self, version: u32) -> Option<&str> {
        self.versions
            .iter()
            .find(|e| e.version == version)
            .map(|e| e.content.as_str())
    }

    /// Compute a simple line-based diff between two versions.
    /// Returns lines prefixed with '+' (added) or '-' (removed).
    pub fn diff(&self, from_version: u32, to_version: u32) -> Option<Vec<String>> {
        let from = self.content_at(from_version)?;
        let to = self.content_at(to_version)?;
        let from_lines: Vec<&str> = from.lines().collect();
        let to_lines: Vec<&str> = to.lines().collect();
        let mut result = Vec::new();

        let mut i = 0;
        let mut j = 0;
        while i < from_lines.len() && j < to_lines.len() {
            if from_lines[i] == to_lines[j] {
                i += 1;
                j += 1;
            } else {
                result.push(format!("-{}", from_lines[i]));
                result.push(format!("+{}", to_lines[j]));
                i += 1;
                j += 1;
            }
        }
        while i < from_lines.len() {
            result.push(format!("-{}", from_lines[i]));
            i += 1;
        }
        while j < to_lines.len() {
            result.push(format!("+{}", to_lines[j]));
            j += 1;
        }
        Some(result)
    }

    /// Check if content changed between two versions.
    pub fn has_changed(&self, from_version: u32, to_version: u32) -> bool {
        match (self.content_at(from_version), self.content_at(to_version)) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        }
    }
}

impl fmt::Display for DocumentVersionTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DocumentVersionTracker(uri={}, versions={})",
            self.uri,
            self.versions.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Document encoding detection
// ---------------------------------------------------------------------------

/// Detected encoding of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Ascii,
    Latin1,
    Unknown,
}

impl fmt::Display for DocumentEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Utf8 => "utf-8",
            Self::Utf8Bom => "utf-8-bom",
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::Ascii => "ascii",
            Self::Latin1 => "latin1",
            Self::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// Detect encoding from raw bytes by inspecting BOM and content.
pub fn detect_encoding(bytes: &[u8]) -> DocumentEncoding {
    // BOM detection
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return DocumentEncoding::Utf8Bom;
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return DocumentEncoding::Utf16Le;
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return DocumentEncoding::Utf16Be;
    }

    // Check if pure ASCII
    if bytes.iter().all(|&b| b < 128) {
        return DocumentEncoding::Ascii;
    }

    // Check valid UTF-8
    if std::str::from_utf8(bytes).is_ok() {
        return DocumentEncoding::Utf8;
    }

    // Check if it looks like Latin-1 (no null bytes, high bytes present)
    if bytes.iter().all(|&b| b != 0) {
        return DocumentEncoding::Latin1;
    }

    DocumentEncoding::Unknown
}

// ---------------------------------------------------------------------------
// Line ending normalization
// ---------------------------------------------------------------------------

/// Line ending style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineEnding {
    Lf,
    CrLf,
    Cr,
    Mixed,
}

impl fmt::Display for LineEnding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lf => write!(f, "LF"),
            Self::CrLf => write!(f, "CRLF"),
            Self::Cr => write!(f, "CR"),
            Self::Mixed => write!(f, "Mixed"),
        }
    }
}

/// Detect the dominant line ending style in text.
pub fn detect_line_ending(text: &str) -> LineEnding {
    let crlf_count = text.matches("\r\n").count();
    // Count standalone \r (not part of \r\n)
    let cr_only = text.chars().enumerate().filter(|&(i, c)| {
        c == '\r' && text.as_bytes().get(i + 1) != Some(&b'\n')
    }).count();
    // Count standalone \n (not part of \r\n)
    let lf_only = text.chars().enumerate().filter(|&(i, c)| {
        c == '\n' && (i == 0 || text.as_bytes().get(i - 1) != Some(&b'\r'))
    }).count();

    let styles = [crlf_count > 0, lf_only > 0, cr_only > 0];
    let distinct: usize = styles.iter().filter(|&&b| b).count();

    if distinct > 1 {
        return LineEnding::Mixed;
    }
    if crlf_count > 0 {
        return LineEnding::CrLf;
    }
    if cr_only > 0 {
        return LineEnding::Cr;
    }
    LineEnding::Lf
}

/// Normalize all line endings in text to the target style.
pub fn normalize_line_endings(text: &str, target: LineEnding) -> String {
    let target_str = match target {
        LineEnding::Lf => "\n",
        LineEnding::CrLf => "\r\n",
        LineEnding::Cr => "\r",
        LineEnding::Mixed => "\n", // default to LF for mixed
    };
    // First normalize all to \n, then convert to target
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if target_str == "\n" {
        normalized
    } else {
        normalized.replace('\n', target_str)
    }
}

/// Count lines in text (handling all line ending styles).
pub fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    // Normalize to \n then count
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized.lines().count()
}

// ── Document query utilities ────────────────────────────────────────────

/// Extract the file extension from a URI (e.g., "file:///foo.rs" → "rs").
pub fn uri_extension(uri: &str) -> Option<&str> {
    let path = uri.rsplit('/').next()?;
    let dot_pos = path.rfind('.')?;
    Some(&path[dot_pos + 1..])
}

/// Check if a URI matches a given language based on common extension mappings.
pub fn uri_matches_language(uri: &str, language_id: &str) -> bool {
    let ext = match uri_extension(uri) {
        Some(e) => e,
        None => return false,
    };
    match language_id {
        "rust" => ext == "rs",
        "javascript" => ext == "js" || ext == "mjs" || ext == "cjs",
        "typescript" => ext == "ts" || ext == "mts" || ext == "cts",
        "python" => ext == "py",
        "go" => ext == "go",
        "c" => ext == "c" || ext == "h",
        "cpp" => ext == "cpp" || ext == "hpp" || ext == "cc",
        "markdown" => ext == "md",
        _ => false,
    }
}

/// Count the number of edits that are pure insertions.
pub fn count_insertions(edits: &[TextEdit]) -> usize {
    edits.iter().filter(|e| e.is_insert()).count()
}

/// Count the number of edits that are pure deletions.
pub fn count_deletions(edits: &[TextEdit]) -> usize {
    edits.iter().filter(|e| e.is_delete()).count()
}

/// Return the total number of characters inserted across all edits.
pub fn total_inserted_chars(edits: &[TextEdit]) -> usize {
    edits.iter().map(|e| e.text.len()).sum()
}

/// Return the maximum line span across a set of edits.
pub fn max_edit_span(edits: &[TextEdit]) -> u32 {
    edits.iter().map(|e| e.span_lines()).max().unwrap_or(0)
}

/// Filter edits to only those affecting a specific line.
pub fn edits_on_line(edits: &[TextEdit], line: u32) -> Vec<&TextEdit> {
    edits
        .iter()
        .filter(|e| e.start_line <= line && e.end_line >= line)
        .collect()
}

/// Validate all edits in a batch, returning the first error found.
pub fn validate_edit_batch(edits: &[TextEdit]) -> Result<(), DocumentError> {
    for edit in edits {
        edit.validate()?;
    }
    Ok(())
}

/// Sort edits by their start position (line first, then column).
pub fn sort_edits_by_position(edits: &mut [TextEdit]) {
    edits.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.start_col.cmp(&b.start_col))
    });
}

// ---------------------------------------------------------------------------
// Document lifecycle state machine
// ---------------------------------------------------------------------------

/// Lifecycle state of a document in the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentLifecycleState {
    /// Document has just been opened and has no unsaved changes.
    Pristine,
    /// Document has been modified since the last save.
    Modified,
    /// Document has been saved (transitions back from Modified).
    Saved,
    /// Document is in the process of being saved.
    Saving,
    /// Document has been closed.
    Closed,
}

impl fmt::Display for DocumentLifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pristine => "pristine",
            Self::Modified => "modified",
            Self::Saved => "saved",
            Self::Saving => "saving",
            Self::Closed => "closed",
        };
        f.write_str(s)
    }
}

/// Tracks the lifecycle state of documents with valid state transitions.
#[derive(Debug, Clone)]
pub struct DocumentLifecycleTracker {
    states: HashMap<String, DocumentLifecycleState>,
}

impl DocumentLifecycleTracker {
    pub fn new() -> Self {
        Self { states: HashMap::new() }
    }

    /// Register a document as opened (Pristine).
    pub fn open(&mut self, uri: impl Into<String>) {
        self.states.insert(uri.into(), DocumentLifecycleState::Pristine);
    }

    /// Get the current lifecycle state of a document.
    pub fn state(&self, uri: &str) -> Option<DocumentLifecycleState> {
        self.states.get(uri).copied()
    }

    /// Attempt a state transition; returns the new state or an error if invalid.
    pub fn transition(
        &mut self,
        uri: &str,
        to: DocumentLifecycleState,
    ) -> Result<DocumentLifecycleState, DocumentError> {
        let current = self
            .states
            .get(uri)
            .copied()
            .ok_or_else(|| DocumentError::NotFound { uri: uri.to_string() })?;

        if !Self::is_valid_transition(current, to) {
            return Err(DocumentError::InvalidUri(format!(
                "invalid transition from {} to {} for {}",
                current, to, uri
            )));
        }

        if to == DocumentLifecycleState::Closed {
            self.states.remove(uri);
        } else {
            self.states.insert(uri.to_string(), to);
        }
        Ok(to)
    }

    /// Check whether a state transition is valid.
    pub fn is_valid_transition(from: DocumentLifecycleState, to: DocumentLifecycleState) -> bool {
        use DocumentLifecycleState::*;
        matches!(
            (from, to),
            (Pristine, Modified)
                | (Pristine, Closed)
                | (Modified, Saving)
                | (Modified, Closed)
                | (Saving, Saved)
                | (Saving, Modified) // save failed or concurrent edit
                | (Saved, Modified)
                | (Saved, Closed)
        )
    }

    /// Return all URIs currently in the Modified state.
    pub fn modified_uris(&self) -> Vec<&str> {
        self.states
            .iter()
            .filter(|(_, s)| **s == DocumentLifecycleState::Modified)
            .map(|(uri, _)| uri.as_str())
            .collect()
    }

    /// Return the count of documents in each state.
    pub fn state_counts(&self) -> HashMap<DocumentLifecycleState, usize> {
        let mut counts = HashMap::new();
        for &state in self.states.values() {
            *counts.entry(state).or_insert(0) += 1;
        }
        counts
    }

    /// True if any document is in the Modified state.
    pub fn has_unsaved_changes(&self) -> bool {
        self.states.values().any(|s| *s == DocumentLifecycleState::Modified)
    }

    /// Number of tracked documents (excludes Closed).
    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }
}

impl Default for DocumentLifecycleTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Document metadata
// ---------------------------------------------------------------------------

/// Rich metadata about an open document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMetadata {
    pub uri: String,
    pub language_id: String,
    pub version: u32,
    pub line_count: usize,
    pub byte_size: usize,
    pub encoding: String,
    pub is_untitled: bool,
    pub scheme: String,
}

impl DocumentMetadata {
    /// Build metadata from a URI and content.
    pub fn from_content(
        uri: &str,
        language_id: &str,
        version: u32,
        content: &str,
    ) -> Self {
        let scheme = uri
            .find("://")
            .map(|i| &uri[..i])
            .unwrap_or("file")
            .to_string();
        let is_untitled = scheme == "untitled";
        let line_count = if content.is_empty() { 0 } else { content.lines().count().max(1) };

        Self {
            uri: uri.to_string(),
            language_id: language_id.to_string(),
            version,
            line_count,
            byte_size: content.len(),
            encoding: "utf-8".to_string(),
            is_untitled,
            scheme,
        }
    }
}

impl fmt::Display for DocumentMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] v{} ({} lines, {} bytes)",
            self.uri, self.language_id, self.version, self.line_count, self.byte_size
        )
    }
}

// ---------------------------------------------------------------------------
// Document change event log
// ---------------------------------------------------------------------------

/// A recorded change event for auditing / undo history.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeRecord {
    pub uri: String,
    pub from_version: u32,
    pub to_version: u32,
    pub edit_count: usize,
    pub chars_added: usize,
    pub chars_deleted: usize,
}

/// Append-only log of document change events.
#[derive(Debug, Clone, Default)]
pub struct ChangeEventLog {
    records: Vec<ChangeRecord>,
}

impl ChangeEventLog {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    /// Record a change event.
    pub fn record(
        &mut self,
        uri: &str,
        from_version: u32,
        to_version: u32,
        edits: &[TextEdit],
    ) {
        let chars_added: usize = edits.iter().map(|e| e.text.len()).sum();
        let chars_deleted: usize = edits
            .iter()
            .filter(|e| e.is_delete())
            .map(|e| ((e.end_line - e.start_line) as usize + 1) * 80) // estimate
            .sum();

        self.records.push(ChangeRecord {
            uri: uri.to_string(),
            from_version,
            to_version,
            edit_count: edits.len(),
            chars_added,
            chars_deleted,
        });
    }

    /// Get all records for a specific URI.
    pub fn records_for(&self, uri: &str) -> Vec<&ChangeRecord> {
        self.records.iter().filter(|r| r.uri == uri).collect()
    }

    /// Total number of recorded change events.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// True if no change events have been recorded.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Get the most recent change record, if any.
    pub fn last(&self) -> Option<&ChangeRecord> {
        self.records.last()
    }

    /// Sum of all chars added across all records.
    pub fn total_chars_added(&self) -> usize {
        self.records.iter().map(|r| r.chars_added).sum()
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

// ---------------------------------------------------------------------------
// DocumentBridge filtering and bulk operations
// ---------------------------------------------------------------------------

impl DocumentBridge {
    /// Filter open documents by language ID, returning matching URIs.
    pub fn filter_by_language(&self, language_id: &str) -> Vec<&str> {
        self.documents
            .iter()
            .filter(|(_, state)| state.language_id == language_id)
            .map(|(uri, _)| uri.as_str())
            .collect()
    }

    /// Filter open documents by URI scheme (e.g., "file", "untitled").
    pub fn filter_by_scheme(&self, scheme: &str) -> Vec<&str> {
        let prefix = format!("{}://", scheme);
        self.documents
            .keys()
            .filter(|uri| uri.starts_with(&prefix))
            .map(|uri| uri.as_str())
            .collect()
    }

    /// Close all documents matching a predicate on URI.
    pub fn close_matching(&mut self, predicate: impl Fn(&str) -> bool) -> usize {
        let to_remove: Vec<String> = self
            .documents
            .keys()
            .filter(|uri| predicate(uri.as_str()))
            .cloned()
            .collect();
        let count = to_remove.len();
        for uri in to_remove {
            self.documents.remove(&uri);
        }
        count
    }

    /// Collect metadata for all open documents.
    pub fn all_metadata(&self) -> Vec<DocumentMetadata> {
        self.documents
            .iter()
            .map(|(uri, state)| {
                DocumentMetadata::from_content(uri, &state.language_id, state.version, &state.content)
            })
            .collect()
    }

    /// Get the content of a document, if open.
    pub fn get_content(&self, uri: &str) -> Option<&str> {
        self.documents.get(uri).map(|s| s.content.as_str())
    }

    /// Get the language ID of a document, if open.
    pub fn get_language_id(&self, uri: &str) -> Option<&str> {
        self.documents.get(uri).map(|s| s.language_id.as_str())
    }

    /// Find all documents whose content contains the given substring.
    pub fn search_content(&self, needle: &str) -> Vec<&str> {
        self.documents
            .iter()
            .filter(|(_, state)| state.content.contains(needle))
            .map(|(uri, _)| uri.as_str())
            .collect()
    }

    /// Get the URI of the document with the highest version number.
    pub fn newest_document(&self) -> Option<&str> {
        self.documents
            .iter()
            .max_by_key(|(_, state)| state.version)
            .map(|(uri, _)| uri.as_str())
    }

    /// Compute total byte size across all open documents.
    pub fn total_content_size(&self) -> usize {
        self.documents.values().map(|s| s.content.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// URI resolution utilities
// ---------------------------------------------------------------------------

/// Extract the scheme portion of a URI (e.g., "file" from "file:///foo").
pub fn uri_scheme(uri: &str) -> Option<&str> {
    uri.find("://").map(|i| &uri[..i])
}

/// Extract the path portion of a URI (after the scheme + authority).
pub fn uri_path(uri: &str) -> Option<&str> {
    let rest = uri.find("://").map(|i| &uri[i + 3..])?;
    // For file:///path, rest is "/path"; for scheme://host/path, find first /
    Some(rest)
}

/// Get the filename component from a URI.
pub fn uri_filename(uri: &str) -> Option<&str> {
    uri.rsplit('/').next().filter(|s| !s.is_empty())
}

/// Guess a language ID from a file extension.
pub fn language_id_from_extension(ext: &str) -> &str {
    match ext {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "py" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" => "cpp",
        "md" => "markdown",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "html" | "htm" => "html",
        "css" => "css",
        "sh" | "bash" => "shellscript",
        _ => "plaintext",
    }
}

/// Infer language ID from a full URI by extracting its extension.
pub fn language_id_from_uri(uri: &str) -> &str {
    match uri_extension(uri) {
        Some(ext) => language_id_from_extension(ext),
        None => "plaintext",
    }
}

// ---------------------------------------------------------------------------
// DocVersionManager - document version manager
// ---------------------------------------------------------------------------

/// Severity level for document version manager issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DocVersionManagerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for DocVersionManagerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [DocVersionManager].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocVersionManagerEntry {
    pub id: String,
    pub label: String,
    pub severity: DocVersionManagerSeverity,
    pub detail: Option<String>,
    pub version: usize,
    enabled: bool,
}

impl DocVersionManagerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: DocVersionManagerSeverity::Low,
            detail: None,
            version: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: DocVersionManagerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_version(mut self, val: usize) -> Self {
        self.version = val;
        self
    }

    pub fn is_dirty(&self) -> bool {
        self.enabled && self.severity >= DocVersionManagerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.version, det)
    }
}

impl fmt::Display for DocVersionManagerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [DocVersionManagerEntry] items.
#[derive(Debug, Clone)]
pub struct DocVersionManager {
    entries: Vec<DocVersionManagerEntry>,
    name: String,
    capacity: usize,
}

impl DocVersionManager {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: DocVersionManagerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<DocVersionManagerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&DocVersionManagerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn version(&self) -> usize { self.entries.len() }

    pub fn is_dirty(&self) -> bool {
        self.entries.iter().any(|e| e.is_dirty())
    }

    pub fn entries_by_severity(&self, severity: DocVersionManagerSeverity) -> Vec<&DocVersionManagerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= DocVersionManagerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&DocVersionManagerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&DocVersionManagerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// DocLanguageMapper - document language mapper
// ---------------------------------------------------------------------------

/// Configuration for [DocLanguageMapper].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocLanguageMapperConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub language_count: usize,
}

impl DocLanguageMapperConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, language_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_language_count(mut self, val: usize) -> Self { self.language_count = val; self }
}

impl Default for DocLanguageMapperConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [DocLanguageMapper].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocLanguageMapperItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl DocLanguageMapperItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_language(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for DocLanguageMapperItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [DocLanguageMapperItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct DocLanguageMapper {
    config: DocLanguageMapperConfig,
    items: Vec<DocLanguageMapperItem>,
}

impl DocLanguageMapper {
    pub fn new(config: DocLanguageMapperConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: DocLanguageMapperItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<DocLanguageMapperItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&DocLanguageMapperItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn language_count(&self) -> usize { self.items.len() }

    pub fn has_language(&self) -> bool {
        self.items.iter().any(|i| i.has_language())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&DocLanguageMapperItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&DocLanguageMapperItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &DocLanguageMapperConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Configuration manager for ext_documents functionality.
pub struct ExtDocumentsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtDocumentsConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &ExtDocumentsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_documents operations.
pub struct ExtDocumentsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtDocumentsRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for ext_documents.
pub struct ExtDocumentsValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtDocumentsValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &ExtDocumentsValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Document model for extensions — extended utilities (zz)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_docs operations.
#[derive(Debug, Clone)]
pub struct ZzMetrics {
    samples: Vec<f64>,
    label: String,
}

impl ZzMetrics {
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

/// Sliding-window rate counter for ext_docs.
#[derive(Debug, Clone)]
pub struct ZzRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl ZzRateWindow {
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

/// A small LRU-style cache for ext_docs lookups.
#[derive(Debug, Clone)]
pub struct ZzLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZzLruCache {
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
// xa_ extended helpers for ext_documents
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtDocumentsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtDocumentsRingBuf {
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
pub struct XaExtDocumentsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtDocumentsCounter {
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

impl Default for XaExtDocumentsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 57
// ---------------------------------------------------------------------------

/// Generic object pool `Xc57Pool<T>`.
pub struct Xc57Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc57Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc57PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc57Pool<T> {
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
    pub fn stats(&self) -> Xc57PoolStats {
        Xc57PoolStats {
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

impl<T> Default for Xc57Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc57Scheduler`.
pub struct Xc57Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc57Scheduler {
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

impl Default for Xc57Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_57 hash for the given byte slice.
pub fn xc_57_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_57 convention.
pub fn xc_57_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_46 deepening: state machine + event bus ---

/// States for the Xd46 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd46State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd46State {
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
pub struct Xd46Transition {
    pub from: Xd46State,
    pub to: Xd46State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd46StateMachine {
    current: Xd46State,
    history: Vec<Xd46Transition>,
    step_counter: usize,
}

impl Xd46StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd46State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd46State {
        self.current
    }

    pub fn history(&self) -> &[Xd46Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd46State) -> Result<Xd46State, String> {
        let allowed = match (self.current, target) {
            (Xd46State::Idle, Xd46State::Running) => true,
            (Xd46State::Running, Xd46State::Paused) => true,
            (Xd46State::Running, Xd46State::Done) => true,
            (Xd46State::Paused, Xd46State::Running) => true,
            (Xd46State::Paused, Xd46State::Done) => true,
            (Xd46State::Done, Xd46State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_46: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd46Transition {
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
            "Xd46SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd46State> {
        let prefix = "Xd46SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd46State::Idle),
            "Running" => Some(Xd46State::Running),
            "Paused" => Some(Xd46State::Paused),
            "Done" => Some(Xd46State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd46State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd46 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd46Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd46Event {
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

type Xd46HandlerFn = Box<dyn Fn(&Xd46Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd46EventBus {
    handlers: Vec<(usize, Option<String>, Xd46HandlerFn)>,
    next_id: usize,
    published: Vec<Xd46Event>,
}

impl Xd46EventBus {
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
        F: Fn(&Xd46Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd46Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd46Event) {
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

    pub fn published_events(&self) -> &[Xd46Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #44
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf44Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf44TrieNode {
    children: std::collections::HashMap<char, Xf44TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf44Trie {
    root: Xf44TrieNode,
    count: usize,
}

impl Xf44Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf44TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf44TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf44TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf44BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf44BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 56).
pub struct Xh56SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh56SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 98 as u64,
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

/// A compact bit set supporting boolean operations (variant 56).
pub struct Xh56BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh56BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 56).
pub struct Xi56Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi56Deque<T> {
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
pub struct Xi56Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi56Interval {
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

/// A simple interval tree (variant 56).
pub struct Xi56IntervalTree {
    xi_intervals: Vec<Xi56Interval>,
}

impl Xi56IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi56Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi56Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi56Interval) -> Vec<&Xi56Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi56Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi56Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi56Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi56Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi56Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi56Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 56) ---

/// Disjoint set / union-find for crate 56.
pub struct Xj56UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj56UnionFind {
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

const XJ56_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 56.
pub struct Xj56BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj56BTreeNode<K, V>>>,
    len: usize,
}

struct Xj56BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj56BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj56BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ56_BTREE_ORDER - 1
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
        let mid = XJ56_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj56BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj56BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj56BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj56BTreeNode::xj_new_leaf();
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


// --- xk_56 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk56SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk56SegmentTree {
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
pub struct Xk56DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk56DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_56).
#[derive(Debug, Clone)]
pub struct Xl56Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl56Rope {
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

/// Suffix array for efficient string searching (xl_56).
#[derive(Debug, Clone)]
pub struct Xl56SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl56SuffixArray {
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


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm56MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm56MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm56Tokenizer {
    text: String,
}

impl Xm56Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 56.
pub struct Xn56Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn56Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 56 -----

#[derive(Debug, Clone)]
struct Xn56AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn56AvlNode<K, V>>>,
    right: Option<Box<Xn56AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 56.
#[derive(Debug, Clone)]
pub struct Xn56AVL<K, V> {
    root: Option<Box<Xn56AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn56AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn56AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn56AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn56AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn56AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn56AvlNode<K, V>>) -> Box<Xn56AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn56AvlNode<K, V>>) -> Box<Xn56AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn56AvlNode<K, V>>) -> Box<Xn56AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn56AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn56AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn56AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn56AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn56AvlNode<K, V>>) -> &Xn56AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn56AvlNode<K, V>>) -> (Box<Xn56AvlNode<K, V>>, Option<Box<Xn56AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn56AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn56AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn56AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn56AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn56AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn56AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn56AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo56RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo56Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo56RBNode<K, V> {
    key: K,
    value: V,
    color: Xo56Color,
    left: Option<Box<Xo56RBNode<K, V>>>,
    right: Option<Box<Xo56RBNode<K, V>>>,
}

/// A red-black tree map for crate 56.
#[derive(Debug, Clone)]
pub struct Xo56RedBlack<K, V> {
    root: Option<Box<Xo56RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo56RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo56Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo56RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo56RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo56RBNode {
                    key, value, color: Xo56Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo56RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo56Color::Red)
    }

    fn xo_balance(mut h: Box<Xo56RBNode<K, V>>) -> Box<Xo56RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo56Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo56RBNode<K, V>>) -> Box<Xo56RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo56Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo56RBNode<K, V>>) -> Box<Xo56RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo56Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo56RBNode<K, V>>) {
        h.color = Xo56Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo56Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo56Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo56Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo56RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo56RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo56RBNode<K, V>) -> (K, V, Option<Box<Xo56RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo56RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo56Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo56RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo56ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 56.
#[derive(Debug, Clone)]
pub struct Xo56ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo56ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo56#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo56#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 56).
#[derive(Debug)]
pub struct Xp56SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp56Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp56Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp56Node<K, V>>>,
    xp_right: Option<Box<Xp56Node<K, V>>>,
}

impl<K: Ord, V> Xp56Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp56SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp56SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp56Node<K, V>>>, key: &K) -> Option<Box<Xp56Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp56Node<K, V>>) -> Box<Xp56Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp56Node<K, V>>) -> Box<Xp56Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp56Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp56Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp56Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq56Treap ---------------

use std::cmp::Ordering as Xq56Ord;

struct Xq56TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq56TreapNode<K, V>>>,
    right: Option<Box<Xq56TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq56Treap<K, V> {
    root: Option<Box<Xq56TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq56TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_56_size<K, V>(node: &Option<Box<Xq56TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_56_update_size<K, V>(node: &mut Xq56TreapNode<K, V>) {
    node.size = 1 + xq_56_size(&node.left) + xq_56_size(&node.right);
}

fn xq_56_rotate_right<K, V>(mut node: Box<Xq56TreapNode<K, V>>) -> Box<Xq56TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_56_update_size(&mut node);
    left.right = Some(node);
    xq_56_update_size(&mut left);
    left
}

fn xq_56_rotate_left<K, V>(mut node: Box<Xq56TreapNode<K, V>>) -> Box<Xq56TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_56_update_size(&mut node);
    right.left = Some(node);
    xq_56_update_size(&mut right);
    right
}

fn xq_56_insert_node<K: Ord, V>(
    node: Option<Box<Xq56TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq56TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq56TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq56Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq56Ord::Less => {
                let (new_left, old) = xq_56_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_56_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_56_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq56Ord::Greater => {
                let (new_right, old) = xq_56_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_56_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_56_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_56_remove_node<K: Ord, V>(
    node: Option<Box<Xq56TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq56TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq56Ord::Less => {
                let (new_left, old) = xq_56_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_56_update_size(&mut n);
                (Some(n), old)
            }
            Xq56Ord::Greater => {
                let (new_right, old) = xq_56_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_56_update_size(&mut n);
                (Some(n), old)
            }
            Xq56Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_56_rotate_right(n);
                    let (new_right, old) = xq_56_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_56_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_56_rotate_left(n);
                    let (new_left, old) = xq_56_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_56_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_56_find_min<K, V>(node: &Option<Box<Xq56TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_56_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_56_find_max<K, V>(node: &Option<Box<Xq56TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_56_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_56_rank<K: Ord, V>(node: &Option<Box<Xq56TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq56Ord::Less => xq_56_rank(&n.left, key),
            Xq56Ord::Equal => xq_56_size(&n.left),
            Xq56Ord::Greater => 1 + xq_56_size(&n.left) + xq_56_rank(&n.right, key),
        },
    }
}

fn xq_56_kth<K, V>(node: &Option<Box<Xq56TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_56_size(&n.left);
        if k < left_size {
            xq_56_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_56_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_56_in_order<K: Clone, V>(node: &Option<Box<Xq56TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_56_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_56_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq56Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 56 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_56_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq56Ord::Equal => return Some(&n.value),
                Xq56Ord::Less => cur = &n.left,
                Xq56Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_56_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_56_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_56_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_56_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_56_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_56_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_56_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq56VEBTree ---------------

pub struct Xq56VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq56VEBTree>>,
    clusters: Vec<Option<Box<Xq56VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq56VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq56VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq56VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr56KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr56KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr56BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr56KDNode {
    xr_point: Xr56KDPoint,
    xr_left: Option<Box<Xr56KDNode>>,
    xr_right: Option<Box<Xr56KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr56KDTree {
    xr_root: Option<Box<Xr56KDNode>>,
    xr_size: usize,
}

impl Xr56KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr56KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr56KDNode>>,
        point: Xr56KDPoint,
        depth: usize,
    ) -> Box<Xr56KDNode> {
        match node {
            None => Box::new(Xr56KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr56KDPoint) -> Option<Xr56KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr56KDNode>,
        query: &Xr56KDPoint,
        depth: usize,
        best: &mut Xr56KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr56KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr56KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr56KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr56KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr56KDNode>>, pts: &mut Vec<Xr56KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr56KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr56BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr56BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs56PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs56PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs56PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs56PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs56ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs56ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs56ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs56RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs56RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs56RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs56CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs56CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs56CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
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
    fn extdocuments_validator_accepts_and_rejects() {
        let mut v = ExtDocumentsValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extdocuments_validator_warnings() {
        let mut v = ExtDocumentsValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extdocuments_validator_clear_and_merge() {
        let mut v = ExtDocumentsValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtDocumentsValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtDocumentsValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
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

    // --- New tests for version tracking, encoding, line endings ---

    #[test]
    fn version_tracker_push_and_diff() {
        let mut tracker = DocumentVersionTracker::new("file:///test.rs", "fn main() {}");
        assert_eq!(tracker.current_version(), 1);
        tracker.push_version("fn main() {\n    println!(\"hello\");\n}");
        assert_eq!(tracker.current_version(), 2);
        assert_eq!(tracker.version_count(), 2);
        let diff = tracker.diff(1, 2).unwrap();
        assert!(!diff.is_empty());
        assert!(tracker.has_changed(1, 2));
    }

    #[test]
    fn version_tracker_content_at() {
        let mut tracker = DocumentVersionTracker::new("file:///a.txt", "v1");
        tracker.push_version("v2");
        tracker.push_version("v3");
        assert_eq!(tracker.content_at(1), Some("v1"));
        assert_eq!(tracker.content_at(3), Some("v3"));
        assert!(tracker.content_at(99).is_none());
    }

    #[test]
    fn detect_encoding_utf8_bom() {
        let bytes = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(detect_encoding(&bytes), DocumentEncoding::Utf8Bom);
    }

    #[test]
    fn detect_encoding_ascii() {
        assert_eq!(detect_encoding(b"Hello world"), DocumentEncoding::Ascii);
    }

    #[test]
    fn detect_encoding_utf16le() {
        let bytes = [0xFF, 0xFE, 0x00, 0x41];
        assert_eq!(detect_encoding(&bytes), DocumentEncoding::Utf16Le);
    }

    #[test]
    fn detect_line_ending_lf() {
        assert_eq!(detect_line_ending("hello\nworld\n"), LineEnding::Lf);
    }

    #[test]
    fn detect_line_ending_crlf() {
        assert_eq!(detect_line_ending("hello\r\nworld\r\n"), LineEnding::CrLf);
    }

    #[test]
    fn detect_line_ending_mixed() {
        assert_eq!(detect_line_ending("hello\r\nworld\n"), LineEnding::Mixed);
    }

    #[test]
    fn normalize_line_endings_to_lf() {
        let input = "line1\r\nline2\rline3\n";
        let result = normalize_line_endings(input, LineEnding::Lf);
        assert_eq!(result, "line1\nline2\nline3\n");
        assert_eq!(detect_line_ending(&result), LineEnding::Lf);
    }

    #[test]
    fn normalize_line_endings_to_crlf() {
        let input = "a\nb\n";
        let result = normalize_line_endings(input, LineEnding::CrLf);
        assert_eq!(result, "a\r\nb\r\n");
    }

    #[test]
    fn count_lines_various() {
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("hello"), 1);
        assert_eq!(count_lines("a\nb\nc"), 3);
        assert_eq!(count_lines("a\r\nb\r\nc"), 3);
    }

    #[test]
    fn uri_extension_extracts_ext() {
        assert_eq!(uri_extension("file:///foo/bar.rs"), Some("rs"));
        assert_eq!(uri_extension("file:///foo/bar.tar.gz"), Some("gz"));
        assert_eq!(uri_extension("file:///noext"), None);
        assert_eq!(uri_extension(""), None);
    }

    #[test]
    fn uri_matches_language_rust() {
        assert!(uri_matches_language("file:///main.rs", "rust"));
        assert!(!uri_matches_language("file:///main.py", "rust"));
        assert!(!uri_matches_language("file:///main.rs", "python"));
    }

    #[test]
    fn uri_matches_language_various() {
        assert!(uri_matches_language("file:///a.js", "javascript"));
        assert!(uri_matches_language("file:///a.mjs", "javascript"));
        assert!(uri_matches_language("file:///a.ts", "typescript"));
        assert!(uri_matches_language("file:///a.py", "python"));
        assert!(uri_matches_language("file:///a.go", "go"));
        assert!(uri_matches_language("file:///a.md", "markdown"));
        assert!(!uri_matches_language("file:///a.rs", "unknown_lang"));
    }

    #[test]
    fn count_insertions_and_deletions() {
        let edits = vec![
            TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 0, text: "hi".into() },
            TextEdit { start_line: 1, start_col: 0, end_line: 2, end_col: 5, text: "".into() },
            TextEdit { start_line: 3, start_col: 0, end_line: 3, end_col: 3, text: "new".into() },
        ];
        assert_eq!(count_insertions(&edits), 1);
        assert_eq!(count_deletions(&edits), 1);
    }

    #[test]
    fn total_inserted_chars_sums() {
        let edits = vec![
            TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 0, text: "abc".into() },
            TextEdit { start_line: 1, start_col: 0, end_line: 1, end_col: 0, text: "de".into() },
        ];
        assert_eq!(total_inserted_chars(&edits), 5);
        assert_eq!(total_inserted_chars(&[]), 0);
    }

    #[test]
    fn max_edit_span_finds_max() {
        let edits = vec![
            TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 5, text: "x".into() },
            TextEdit { start_line: 1, start_col: 0, end_line: 4, end_col: 0, text: "y".into() },
        ];
        assert_eq!(max_edit_span(&edits), 4);
        assert_eq!(max_edit_span(&[]), 0);
    }

    #[test]
    fn edits_on_line_filters() {
        let edits = vec![
            TextEdit { start_line: 0, start_col: 0, end_line: 0, end_col: 5, text: "a".into() },
            TextEdit { start_line: 2, start_col: 0, end_line: 4, end_col: 0, text: "b".into() },
            TextEdit { start_line: 5, start_col: 0, end_line: 5, end_col: 3, text: "c".into() },
        ];
        assert_eq!(edits_on_line(&edits, 0).len(), 1);
        assert_eq!(edits_on_line(&edits, 3).len(), 1);
        assert_eq!(edits_on_line(&edits, 1).len(), 0);
    }

    #[test]
    fn validate_edit_batch_returns_first_error() {
        let good = vec![
            TextEdit { start_line: 0, start_col: 0, end_line: 1, end_col: 0, text: "ok".into() },
        ];
        assert!(validate_edit_batch(&good).is_ok());
        assert!(validate_edit_batch(&[]).is_ok());

        let bad = vec![
            TextEdit { start_line: 5, start_col: 0, end_line: 2, end_col: 0, text: "bad".into() },
        ];
        assert!(validate_edit_batch(&bad).is_err());
    }

    #[test]
    fn sort_edits_by_position_orders() {
        let mut edits = vec![
            TextEdit { start_line: 5, start_col: 3, end_line: 5, end_col: 10, text: "c".into() },
            TextEdit { start_line: 1, start_col: 0, end_line: 1, end_col: 5, text: "a".into() },
            TextEdit { start_line: 1, start_col: 5, end_line: 2, end_col: 0, text: "b".into() },
        ];
        sort_edits_by_position(&mut edits);
        assert_eq!(edits[0].start_line, 1);
        assert_eq!(edits[0].start_col, 0);
        assert_eq!(edits[1].start_line, 1);
        assert_eq!(edits[1].start_col, 5);
        assert_eq!(edits[2].start_line, 5);
    }

    // ── New tests: lifecycle, metadata, change log, filtering, URI utils ──

    #[test]
    fn lifecycle_tracker_open_and_transition() {
        let mut tracker = DocumentLifecycleTracker::new();
        tracker.open("file:///a.rs");
        assert_eq!(tracker.state("file:///a.rs"), Some(DocumentLifecycleState::Pristine));

        // Pristine -> Modified
        let s = tracker.transition("file:///a.rs", DocumentLifecycleState::Modified).unwrap();
        assert_eq!(s, DocumentLifecycleState::Modified);

        // Modified -> Saving
        tracker.transition("file:///a.rs", DocumentLifecycleState::Saving).unwrap();

        // Saving -> Saved
        tracker.transition("file:///a.rs", DocumentLifecycleState::Saved).unwrap();
        assert_eq!(tracker.state("file:///a.rs"), Some(DocumentLifecycleState::Saved));

        // Saved -> Closed (removes from tracker)
        tracker.transition("file:///a.rs", DocumentLifecycleState::Closed).unwrap();
        assert_eq!(tracker.state("file:///a.rs"), None);
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn lifecycle_invalid_transition_rejected() {
        let mut tracker = DocumentLifecycleTracker::new();
        tracker.open("file:///b.rs");
        // Pristine -> Saved is not valid
        let result = tracker.transition("file:///b.rs", DocumentLifecycleState::Saved);
        assert!(result.is_err());
        // Pristine -> Saving is not valid
        let result = tracker.transition("file:///b.rs", DocumentLifecycleState::Saving);
        assert!(result.is_err());
    }

    #[test]
    fn lifecycle_modified_uris_and_unsaved() {
        let mut tracker = DocumentLifecycleTracker::new();
        tracker.open("file:///x.rs");
        tracker.open("file:///y.rs");
        tracker.open("file:///z.rs");
        assert!(!tracker.has_unsaved_changes());

        tracker.transition("file:///x.rs", DocumentLifecycleState::Modified).unwrap();
        tracker.transition("file:///z.rs", DocumentLifecycleState::Modified).unwrap();
        assert!(tracker.has_unsaved_changes());

        let modified = tracker.modified_uris();
        assert_eq!(modified.len(), 2);
        assert!(modified.contains(&"file:///x.rs"));
        assert!(modified.contains(&"file:///z.rs"));

        let counts = tracker.state_counts();
        assert_eq!(counts[&DocumentLifecycleState::Modified], 2);
        assert_eq!(counts[&DocumentLifecycleState::Pristine], 1);
    }

    #[test]
    fn document_metadata_from_content() {
        let meta = DocumentMetadata::from_content(
            "file:///src/main.rs",
            "rust",
            3,
            "fn main() {\n    println!(\"hi\");\n}\n",
        );
        assert_eq!(meta.scheme, "file");
        assert!(!meta.is_untitled);
        assert_eq!(meta.version, 3);
        assert_eq!(meta.line_count, 3);
        assert!(meta.byte_size > 0);
        assert_eq!(meta.language_id, "rust");

        // Untitled document
        let untitled = DocumentMetadata::from_content("untitled://1", "plaintext", 1, "");
        assert!(untitled.is_untitled);
        assert_eq!(untitled.line_count, 0);
        assert_eq!(untitled.scheme, "untitled");

        // Display
        let s = format!("{meta}");
        assert!(s.contains("main.rs"));
        assert!(s.contains("rust"));
    }

    #[test]
    fn change_event_log_record_and_query() {
        let mut log = ChangeEventLog::new();
        assert!(log.is_empty());

        let edits = vec![
            TextEdit::new(0, 0, 0, 0, "hello"),
            TextEdit::new(1, 0, 1, 5, ""),
        ];
        log.record("file:///a.rs", 1, 2, &edits);
        log.record("file:///b.rs", 1, 2, &[TextEdit::new(0, 0, 0, 0, "world")]);
        log.record("file:///a.rs", 2, 3, &[TextEdit::new(0, 0, 0, 3, "hi")]);

        assert_eq!(log.len(), 3);
        assert!(!log.is_empty());
        assert_eq!(log.records_for("file:///a.rs").len(), 2);
        assert_eq!(log.records_for("file:///b.rs").len(), 1);
        assert_eq!(log.records_for("file:///c.rs").len(), 0);

        let last = log.last().unwrap();
        assert_eq!(last.uri, "file:///a.rs");
        assert_eq!(last.to_version, 3);

        assert!(log.total_chars_added() > 0);

        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn bridge_filter_by_language() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///a.rs".into(), language_id: "rust".into(), version: 1, content: "".into(),
        });
        bridge.handle(DocumentMessage::Open {
            uri: "file:///b.py".into(), language_id: "python".into(), version: 1, content: "".into(),
        });
        bridge.handle(DocumentMessage::Open {
            uri: "file:///c.rs".into(), language_id: "rust".into(), version: 1, content: "".into(),
        });

        let rust_docs = bridge.filter_by_language("rust");
        assert_eq!(rust_docs.len(), 2);
        let py_docs = bridge.filter_by_language("python");
        assert_eq!(py_docs.len(), 1);
        let go_docs = bridge.filter_by_language("go");
        assert!(go_docs.is_empty());
    }

    #[test]
    fn bridge_filter_by_scheme_and_close_matching() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///a.rs".into(), language_id: "rust".into(), version: 1, content: "a".into(),
        });
        bridge.handle(DocumentMessage::Open {
            uri: "untitled://1".into(), language_id: "plaintext".into(), version: 1, content: "b".into(),
        });
        bridge.handle(DocumentMessage::Open {
            uri: "file:///c.rs".into(), language_id: "rust".into(), version: 1, content: "c".into(),
        });

        let file_docs = bridge.filter_by_scheme("file");
        assert_eq!(file_docs.len(), 2);
        let untitled = bridge.filter_by_scheme("untitled");
        assert_eq!(untitled.len(), 1);

        let closed = bridge.close_matching(|uri| uri.starts_with("untitled://"));
        assert_eq!(closed, 1);
        assert_eq!(bridge.open_count(), 2);
    }

    #[test]
    fn bridge_search_content_and_metadata() {
        let mut bridge = DocumentBridge::new();
        bridge.handle(DocumentMessage::Open {
            uri: "file:///x.rs".into(), language_id: "rust".into(), version: 3,
            content: "fn main() { println!(\"hello\"); }".into(),
        });
        bridge.handle(DocumentMessage::Open {
            uri: "file:///y.py".into(), language_id: "python".into(), version: 1,
            content: "print('world')".into(),
        });

        let hits = bridge.search_content("hello");
        assert_eq!(hits.len(), 1);
        assert!(hits.contains(&"file:///x.rs"));

        let hits2 = bridge.search_content("nonexistent");
        assert!(hits2.is_empty());

        assert_eq!(bridge.get_content("file:///x.rs").unwrap(), "fn main() { println!(\"hello\"); }");
        assert_eq!(bridge.get_language_id("file:///y.py"), Some("python"));
        assert!(bridge.get_content("file:///missing").is_none());

        let newest = bridge.newest_document().unwrap();
        assert_eq!(newest, "file:///x.rs"); // version 3 > version 1

        let total = bridge.total_content_size();
        assert!(total > 40);

        let all_meta = bridge.all_metadata();
        assert_eq!(all_meta.len(), 2);
    }

    #[test]
    fn uri_utilities() {
        assert_eq!(uri_scheme("file:///foo.rs"), Some("file"));
        assert_eq!(uri_scheme("untitled://1"), Some("untitled"));
        assert_eq!(uri_scheme("noscheme"), None);

        assert_eq!(uri_path("file:///home/user/a.rs"), Some("/home/user/a.rs"));
        assert_eq!(uri_path("noscheme"), None);

        assert_eq!(uri_filename("file:///home/user/main.rs"), Some("main.rs"));
        assert_eq!(uri_filename("file:///"), None);

        assert_eq!(language_id_from_extension("rs"), "rust");
        assert_eq!(language_id_from_extension("ts"), "typescript");
        assert_eq!(language_id_from_extension("py"), "python");
        assert_eq!(language_id_from_extension("json"), "json");
        assert_eq!(language_id_from_extension("toml"), "toml");
        assert_eq!(language_id_from_extension("yaml"), "yaml");
        assert_eq!(language_id_from_extension("html"), "html");
        assert_eq!(language_id_from_extension("css"), "css");
        assert_eq!(language_id_from_extension("sh"), "shellscript");
        assert_eq!(language_id_from_extension("xyz"), "plaintext");

        assert_eq!(language_id_from_uri("file:///main.rs"), "rust");
        assert_eq!(language_id_from_uri("file:///app.js"), "javascript");
        assert_eq!(language_id_from_uri("file:///noext"), "plaintext");
    }

    #[test]
    fn lifecycle_state_display_and_serde() {
        assert_eq!(format!("{}", DocumentLifecycleState::Pristine), "pristine");
        assert_eq!(format!("{}", DocumentLifecycleState::Modified), "modified");
        assert_eq!(format!("{}", DocumentLifecycleState::Closed), "closed");

        let json = serde_json::to_string(&DocumentLifecycleState::Modified).unwrap();
        let parsed: DocumentLifecycleState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DocumentLifecycleState::Modified);
    }

#[test]
    fn docversionmanager_severity_ordering() {
        assert!(DocVersionManagerSeverity::Critical > DocVersionManagerSeverity::High);
        assert!(DocVersionManagerSeverity::High > DocVersionManagerSeverity::Medium);
        assert!(DocVersionManagerSeverity::Medium > DocVersionManagerSeverity::Low);
    }

    #[test]
    fn docversionmanager_severity_display() {
        assert_eq!(DocVersionManagerSeverity::Low.to_string(), "low");
        assert_eq!(DocVersionManagerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn docversionmanager_entry_creation() {
        let e = DocVersionManagerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, DocVersionManagerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn docversionmanager_entry_builder() {
        let e = DocVersionManagerEntry::new("e2", "Entry 2")
            .with_severity(DocVersionManagerSeverity::High)
            .with_detail("some detail")
            .with_version(42);
        assert_eq!(e.severity, DocVersionManagerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.version, 42);
    }

    #[test]
    fn docversionmanager_entry_enable_disable() {
        let mut e = DocVersionManagerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn docversionmanager_add_and_count() {
        let mut mgr = DocVersionManager::new("test");
        mgr.add(DocVersionManagerEntry::new("a", "A"));
        mgr.add(DocVersionManagerEntry::new("b", "B").with_severity(DocVersionManagerSeverity::High));
        assert_eq!(mgr.version(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn docversionmanager_remove() {
        let mut mgr = DocVersionManager::new("test");
        mgr.add(DocVersionManagerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn docversionmanager_capacity() {
        let mut mgr = DocVersionManager::new("test").with_capacity(1);
        assert!(mgr.add(DocVersionManagerEntry::new("a", "A")));
        assert!(!mgr.add(DocVersionManagerEntry::new("b", "B")));
    }

    #[test]
    fn docversionmanager_sorted_by_severity() {
        let mut mgr = DocVersionManager::new("test");
        mgr.add(DocVersionManagerEntry::new("lo", "Low"));
        mgr.add(DocVersionManagerEntry::new("hi", "High").with_severity(DocVersionManagerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, DocVersionManagerSeverity::Critical);
    }

    #[test]
    fn docversionmanager_summary() {
        let mgr = DocVersionManager::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn doclanguagemapper_config_defaults() {
        let cfg = DocLanguageMapperConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn doclanguagemapper_item_creation() {
        let item = DocLanguageMapperItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn doclanguagemapper_add_and_get() {
        let mut mgr = DocLanguageMapper::new(DocLanguageMapperConfig::new("test"));
        mgr.add(DocLanguageMapperItem::new("k1", "v1"));
        assert_eq!(mgr.language_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn doclanguagemapper_remove_item() {
        let mut mgr = DocLanguageMapper::new(DocLanguageMapperConfig::new("test"));
        mgr.add(DocLanguageMapperItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn doclanguagemapper_sorted_by_priority() {
        let mut mgr = DocLanguageMapper::new(DocLanguageMapperConfig::new("test"));
        mgr.add(DocLanguageMapperItem::new("lo", "low").with_priority(1));
        mgr.add(DocLanguageMapperItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn doclanguagemapper_items_with_tag() {
        let mut mgr = DocLanguageMapper::new(DocLanguageMapperConfig::new("test"));
        mgr.add(DocLanguageMapperItem::new("a", "1").with_tag("x"));
        mgr.add(DocLanguageMapperItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn doclanguagemapper_report() {
        let mgr = DocLanguageMapper::new(DocLanguageMapperConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn ext_documents_config_new() {
        let cfg = ExtDocumentsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_documents_config_set_get() {
        let mut cfg = ExtDocumentsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_documents_config_remove() {
        let mut cfg = ExtDocumentsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_documents_config_keys_sorted() {
        let mut cfg = ExtDocumentsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_documents_config_bump_version() {
        let mut cfg = ExtDocumentsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_documents_config_clear() {
        let mut cfg = ExtDocumentsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_documents_config_merge() {
        let mut cfg1 = ExtDocumentsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtDocumentsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_documents_config_disable() {
        let mut cfg = ExtDocumentsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_documents_rate_tracker_empty() {
        let rt = ExtDocumentsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_documents_rate_tracker_record() {
        let mut rt = ExtDocumentsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_documents_rate_tracker_prune() {
        let mut rt = ExtDocumentsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_documents_validator_valid() {
        let v = ExtDocumentsValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_documents_validator_errors() {
        let mut v = ExtDocumentsValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_documents_validator_clear() {
        let mut v = ExtDocumentsValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_documents_validator_merge() {
        let mut v1 = ExtDocumentsValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtDocumentsValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_documents_rate_tracker_clear() {
        let mut rt = ExtDocumentsRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zz_metrics_empty() {
        let m = ZzMetrics::new("ext_docs");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zz_metrics_record_and_mean() {
        let mut m = ZzMetrics::new("ext_docs");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zz_metrics_min_max() {
        let mut m = ZzMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zz_metrics_variance_and_std() {
        let mut m = ZzMetrics::new("v");
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
    fn zz_metrics_percentile() {
        let mut m = ZzMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn zz_metrics_merge() {
        let mut a = ZzMetrics::new("a");
        a.record(1.0);
        let mut b = ZzMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn zz_metrics_reset() {
        let mut m = ZzMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn zz_rate_window_empty() {
        let rw = ZzRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn zz_rate_window_tick_and_rate() {
        let mut rw = ZzRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn zz_lru_cache_basic() {
        let mut c = ZzLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn zz_lru_cache_contains_and_keys() {
        let mut c = ZzLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn zz_lru_cache_remove() {
        let mut c = ZzLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn zz_metrics_sum() {
        let mut m = ZzMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zz_metrics_label() {
        let m = ZzMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn zz_lru_cache_clear() {
        let mut c = ZzLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_documents
    #[test]
    fn xa_ext_documents_ring_new() {
        let rb = super::XaExtDocumentsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_documents_ring_push_len() {
        let mut rb = super::XaExtDocumentsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_documents_ring_wrap() {
        let mut rb = super::XaExtDocumentsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_documents_ring_mean_empty() {
        let rb = super::XaExtDocumentsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_documents_ring_mean_values() {
        let mut rb = super::XaExtDocumentsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_documents_ring_min_max() {
        let mut rb = super::XaExtDocumentsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_documents_ring_iter() {
        let mut rb = super::XaExtDocumentsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_documents_counter_new() {
        let c = super::XaExtDocumentsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_documents_counter_inc() {
        let mut c = super::XaExtDocumentsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_documents_counter_inc_by() {
        let mut c = super::XaExtDocumentsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_documents_counter_reset() {
        let mut c = super::XaExtDocumentsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_documents_counter_clear() {
        let mut c = super::XaExtDocumentsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_documents_counter_default() {
        let c = super::XaExtDocumentsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 57 ----

    #[test]
    fn xc_57_pool_new_empty() {
        let pool: super::Xc57Pool<i32> = super::Xc57Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_57_pool_release_acquire() {
        let mut pool = super::Xc57Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_57_pool_acquire_empty() {
        let mut pool: super::Xc57Pool<i32> = super::Xc57Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_57_pool_full() {
        let mut pool = super::Xc57Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_57_pool_drain() {
        let mut pool = super::Xc57Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_57_pool_stats() {
        let mut pool = super::Xc57Pool::new(8);
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
    fn xc_57_pool_clear() {
        let mut pool = super::Xc57Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_57_pool_shrink() {
        let mut pool = super::Xc57Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_57_pool_default() {
        let pool: super::Xc57Pool<String> = super::Xc57Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_57_pool_extend() {
        let mut pool = super::Xc57Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_57_pool_retain() {
        let mut pool = super::Xc57Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_57_scheduler_round_robin() {
        let mut sched = super::Xc57Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_57_scheduler_empty() {
        let mut sched = super::Xc57Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_57_scheduler_reset() {
        let mut sched = super::Xc57Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_57_scheduler_add_remove() {
        let mut sched = super::Xc57Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_57_scheduler_targets() {
        let sched = super::Xc57Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_57_hash_empty() {
        assert_eq!(super::xc_57_hash(b""), 5381);
    }

    #[test]
    fn xc_57_hash_data() {
        let h = super::xc_57_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_57_hash(b"hello"), h);
    }

    #[test]
    fn xc_57_reverse_str() {
        assert_eq!(super::xc_57_reverse("abc"), "cba");
        assert_eq!(super::xc_57_reverse(""), "");
    }


    // --- xd_46 deepening tests ---

    #[test]
    fn xd_46_sm_initial_state() {
        let sm = Xd46StateMachine::new();
        assert_eq!(sm.current_state(), Xd46State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_46_sm_valid_idle_to_running() {
        let mut sm = Xd46StateMachine::new();
        assert!(sm.transition(Xd46State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd46State::Running);
    }

    #[test]
    fn xd_46_sm_valid_running_to_paused() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        assert!(sm.transition(Xd46State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd46State::Paused);
    }

    #[test]
    fn xd_46_sm_valid_running_to_done() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        assert!(sm.transition(Xd46State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd46State::Done);
    }

    #[test]
    fn xd_46_sm_valid_paused_to_running() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        sm.transition(Xd46State::Paused).unwrap();
        assert!(sm.transition(Xd46State::Running).is_ok());
    }

    #[test]
    fn xd_46_sm_valid_done_to_idle() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        sm.transition(Xd46State::Done).unwrap();
        assert!(sm.transition(Xd46State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd46State::Idle);
    }

    #[test]
    fn xd_46_sm_invalid_idle_to_done() {
        let mut sm = Xd46StateMachine::new();
        assert!(sm.transition(Xd46State::Done).is_err());
    }

    #[test]
    fn xd_46_sm_invalid_idle_to_paused() {
        let mut sm = Xd46StateMachine::new();
        assert!(sm.transition(Xd46State::Paused).is_err());
    }

    #[test]
    fn xd_46_sm_history_tracking() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        sm.transition(Xd46State::Paused).unwrap();
        sm.transition(Xd46State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd46State::Idle);
        assert_eq!(sm.history()[0].to, Xd46State::Running);
        assert_eq!(sm.history()[1].from, Xd46State::Running);
        assert_eq!(sm.history()[2].to, Xd46State::Done);
    }

    #[test]
    fn xd_46_sm_serialize_deserialize() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd46StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd46State::Running));
    }

    #[test]
    fn xd_46_sm_deserialize_invalid() {
        assert_eq!(Xd46StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_46_sm_reset() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd46State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_46_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd46EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd46Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_46_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd46EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd46Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd46Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_46_bus_unsubscribe() {
        let mut bus = Xd46EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_46_event_kind_and_payload() {
        let e = Xd46Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd46Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_46_bus_clear_history() {
        let mut bus = Xd46EventBus::new();
        bus.publish(Xd46Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_46_sm_step_counter_increments() {
        let mut sm = Xd46StateMachine::new();
        sm.transition(Xd46State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd46State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #44 --

    #[test]
    fn xf44_trie_insert_search() {
        let mut t = Xf44Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf44_trie_starts_with() {
        let mut t = Xf44Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf44_trie_remove() {
        let mut t = Xf44Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf44_trie_word_count() {
        let mut t = Xf44Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf44_trie_longest_prefix() {
        let mut t = Xf44Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf44_trie_all_words() {
        let mut t = Xf44Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf44_trie_autocomplete() {
        let mut t = Xf44Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf44_trie_empty_search() {
        let t = Xf44Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf44_bloom_add_contains() {
        let mut bf = Xf44BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf44_bloom_probably_absent() {
        let bf = Xf44BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf44_bloom_false_positive_rate() {
        let mut bf = Xf44BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf44_bloom_clear() {
        let mut bf = Xf44BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf44_bloom_union() {
        let mut a = Xf44BloomFilter::xf_new(512, 2);
        let mut b = Xf44BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf44_bloom_intersection_estimate() {
        let mut a = Xf44BloomFilter::xf_new(512, 2);
        let mut b = Xf44BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf44_bloom_union_size_mismatch() {
        let a = Xf44BloomFilter::xf_new(256, 2);
        let b = Xf44BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh56_skip_insert_contains() {
        let mut sl = super::Xh56SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh56_skip_remove() {
        let mut sl = super::Xh56SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh56_skip_len() {
        let mut sl = super::Xh56SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh56_skip_range_query() {
        let mut sl = super::Xh56SkipList::xh_new(4);
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
    fn xh56_skip_floor_ceiling() {
        let mut sl = super::Xh56SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh56_skip_rank() {
        let mut sl = super::Xh56SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh56_skip_empty() {
        let sl = super::Xh56SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh56_skip_duplicates() {
        let mut sl = super::Xh56SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh56_bitset_set_test() {
        let mut bs = super::Xh56BitSet::xh_new(256);
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
    fn xh56_bitset_clear_count() {
        let mut bs = super::Xh56BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh56_bitset_and_or_xor() {
        let mut a = super::Xh56BitSet::xh_new(128);
        let mut b = super::Xh56BitSet::xh_new(128);
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
    fn xh56_bitset_iter_ones() {
        let mut bs = super::Xh56BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh56_bitset_first_last() {
        let mut bs = super::Xh56BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh56_bitset_empty() {
        let bs = super::Xh56BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi56_deque_push_pop_back() {
        let mut dq = super::Xi56Deque::xi_new(4);
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
    fn xi56_deque_push_pop_front() {
        let mut dq = super::Xi56Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi56_deque_mixed_ops() {
        let mut dq = super::Xi56Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi56_deque_get_and_split() {
        let mut dq = super::Xi56Deque::xi_new(8);
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
    fn xi56_deque_rotate_left() {
        let mut dq = super::Xi56Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi56_deque_rotate_right() {
        let mut dq = super::Xi56Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi56_deque_grow() {
        let mut dq = super::Xi56Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi56_deque_empty() {
        let dq = super::Xi56Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi56_interval_tree_insert_query() {
        let mut tree = super::Xi56IntervalTree::xi_new();
        tree.xi_insert(super::Xi56Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi56Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi56Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi56_interval_tree_overlap() {
        let mut tree = super::Xi56IntervalTree::xi_new();
        tree.xi_insert(super::Xi56Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi56Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi56Interval::xi_new(12, 20));
        let q = super::Xi56Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi56_interval_tree_remove() {
        let mut tree = super::Xi56IntervalTree::xi_new();
        tree.xi_insert(super::Xi56Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi56Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi56_interval_tree_gaps() {
        let mut tree = super::Xi56IntervalTree::xi_new();
        tree.xi_insert(super::Xi56Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi56Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi56Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi56Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi56Interval::xi_new(8, 10));
    }

    #[test]
    fn xi56_interval_tree_merge() {
        let mut tree = super::Xi56IntervalTree::xi_new();
        tree.xi_insert(super::Xi56Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi56Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi56Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi56Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi56Interval::xi_new(10, 15));
    }

    #[test]
    fn xi56_interval_tree_all() {
        let mut tree = super::Xi56IntervalTree::xi_new();
        tree.xi_insert(super::Xi56Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi56Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi56_interval_tree_empty() {
        let tree = super::Xi56IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi56_interval_tree_contains_point() {
        let iv = super::Xi56Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 56) ---

    #[test]
    fn xj_56_uf_make_and_find() {
        let mut uf = super::Xj56UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_56_uf_union_connected() {
        let mut uf = super::Xj56UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_56_uf_component_count() {
        let mut uf = super::Xj56UnionFind::xj_new();
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
    fn xj_56_uf_component_size() {
        let mut uf = super::Xj56UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_56_uf_largest_component() {
        let mut uf = super::Xj56UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_56_uf_many_elements() {
        let mut uf = super::Xj56UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_56_uf_separate_components() {
        let mut uf = super::Xj56UnionFind::xj_new();
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
    fn xj_56_uf_path_compression() {
        let mut uf = super::Xj56UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_56_bt_insert_get() {
        let mut bt = super::Xj56BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_56_bt_contains_len() {
        let mut bt = super::Xj56BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_56_bt_replace() {
        let mut bt = super::Xj56BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_56_bt_remove() {
        let mut bt = super::Xj56BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_56_bt_keys_values() {
        let mut bt = super::Xj56BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_56_bt_range() {
        let mut bt = super::Xj56BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_56_bt_min_max() {
        let mut bt = super::Xj56BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_56_bt_many_inserts() {
        let mut bt = super::Xj56BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_56 segment tree tests ---

    #[test]
    fn xk_56_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk56SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_56_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk56SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_56_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk56SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_56_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk56SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_56_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk56SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_56_st_single_element() {
        let data = vec![42];
        let st = super::Xk56SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_56_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk56SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_56_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk56SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_56 disjoint intervals tests ---

    #[test]
    fn xk_56_di_add_and_count() {
        let mut di = super::Xk56DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_56_di_merge_overlap() {
        let mut di = super::Xk56DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_56_di_contains() {
        let mut di = super::Xk56DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_56_di_remove() {
        let mut di = super::Xk56DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_56_di_covered_length() {
        let mut di = super::Xk56DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_56_di_gaps() {
        let mut di = super::Xk56DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_56_di_merge_adjacent() {
        let mut di = super::Xk56DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_56_di_empty() {
        let di = super::Xk56DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_56_rope_new_empty() {
        let rope = super::Xl56Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_56_rope_from_str() {
        let rope = super::Xl56Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_56_rope_insert_at() {
        let mut rope = super::Xl56Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_56_rope_delete_range() {
        let mut rope = super::Xl56Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_56_rope_char_at() {
        let rope = super::Xl56Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_56_rope_split_concat() {
        let rope = super::Xl56Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_56_rope_line_count() {
        let rope = super::Xl56Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_56_rope_line_at() {
        let rope = super::Xl56Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_56_sa_build_and_search() {
        let sa = super::Xl56SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_56_sa_count() {
        let sa = super::Xl56SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_56_sa_longest_repeated() {
        let sa = super::Xl56SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_56_sa_all_positions() {
        let sa = super::Xl56SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_56_sa_len() {
        let sa = super::Xl56SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_56_sa_empty() {
        let sa = super::Xl56SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_56_rope_slice() {
        let rope = super::Xl56Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_56_sa_search_start() {
        let sa = super::Xl56SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_56_sparse_set_get() {
        let mut m = super::Xm56MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_56_sparse_row_col() {
        let mut m = super::Xm56MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_56_sparse_transpose() {
        let mut m = super::Xm56MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_56_sparse_multiply_vec() {
        let mut m = super::Xm56MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_56_sparse_nnz_density() {
        let mut m = super::Xm56MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_56_sparse_clear() {
        let mut m = super::Xm56MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_56_sparse_overwrite_zero() {
        let mut m = super::Xm56MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_56_tokenizer_basic() {
        let t = super::Xm56Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_56_tokenizer_count() {
        let t = super::Xm56Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_56_tokenizer_unique() {
        let t = super::Xm56Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_56_tokenizer_frequency() {
        let t = super::Xm56Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_56_tokenizer_delimiter() {
        let t = super::Xm56Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_56_tokenizer_whitespace() {
        let t = super::Xm56Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_56_tokenizer_empty() {
        let t = super::Xm56Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 56 ----

    #[test]
    fn xn_56_fenwick_prefix_sum() {
        let mut ft = super::Xn56Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_56_fenwick_range_sum() {
        let mut ft = super::Xn56Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_56_fenwick_point_query() {
        let mut ft = super::Xn56Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_56_fenwick_len() {
        let ft = super::Xn56Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_56_fenwick_multiple_updates() {
        let mut ft = super::Xn56Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_56_fenwick_single_element() {
        let mut ft = super::Xn56Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_56_fenwick_find_kth() {
        let mut ft = super::Xn56Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_56_fenwick_negative_delta() {
        let mut ft = super::Xn56Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 56 ----

    #[test]
    fn xn_56_avl_insert_get() {
        let mut m = super::Xn56AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_56_avl_remove() {
        let mut m = super::Xn56AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_56_avl_in_order() {
        let mut m = super::Xn56AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_56_avl_min_max() {
        let mut m = super::Xn56AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_56_avl_floor_ceiling() {
        let mut m = super::Xn56AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_56_avl_height_balanced() {
        let mut m = super::Xn56AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_56_avl_overwrite() {
        let mut m = super::Xn56AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_56_avl_empty() {
        let m: super::Xn56AVL<i32, i32> = super::Xn56AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo56RedBlack tests ---

    #[test]
    fn xo_56_rb_insert_and_get() {
        let mut tree = super::Xo56RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_56_rb_len_and_empty() {
        let mut tree = super::Xo56RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_56_rb_min_max() {
        let mut tree = super::Xo56RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_56_rb_contains() {
        let mut tree = super::Xo56RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_56_rb_remove() {
        let mut tree = super::Xo56RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_56_rb_in_order() {
        let mut tree = super::Xo56RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_56_rb_black_height() {
        let mut tree = super::Xo56RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_56_rb_overwrite() {
        let mut tree = super::Xo56RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo56ConsistentHash tests ---

    #[test]
    fn xo_56_ch_add_and_count() {
        let mut ring = super::Xo56ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_56_ch_remove_node() {
        let mut ring = super::Xo56ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_56_ch_get_node() {
        let mut ring = super::Xo56ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_56_ch_empty_ring() {
        let ring = super::Xo56ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_56_ch_distribution() {
        let mut ring = super::Xo56ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_56_ch_rebalance() {
        let mut ring = super::Xo56ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_56_ch_virtual_nodes() {
        let mut ring = super::Xo56ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_56_ch_consistent_lookup() {
        let mut ring = super::Xo56ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_56_splay_insert_get() {
        let mut t = super::Xp56SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_56_splay_remove() {
        let mut t = super::Xp56SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_56_splay_count_increases() {
        let mut t = super::Xp56SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_56_splay_depth() {
        let mut t = super::Xp56SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_56_splay_len_empty() {
        let t = super::Xp56SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_56_splay_min_max() {
        let mut t = super::Xp56SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_56_splay_overwrite() {
        let mut t = super::Xp56SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_56_splay_remove_missing() {
        let mut t = super::Xp56SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_56 treap tests ----
    #[test]
    fn xq_56_treap_empty() {
        let t = super::Xq56Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_56_treap_insert_get() {
        let mut t = super::Xq56Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_56_treap_overwrite() {
        let mut t = super::Xq56Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_56_treap_remove() {
        let mut t = super::Xq56Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_56_treap_min_max() {
        let mut t = super::Xq56Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_56_treap_rank() {
        let mut t = super::Xq56Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_56_treap_kth() {
        let mut t = super::Xq56Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_56_treap_in_order() {
        let mut t = super::Xq56Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_56 VEB tree tests ----
    #[test]
    fn xq_56_veb_empty() {
        let v = super::Xq56VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_56_veb_insert_contains() {
        let mut v = super::Xq56VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_56_veb_min_max() {
        let mut v = super::Xq56VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_56_veb_delete() {
        let mut v = super::Xq56VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_56_veb_successor() {
        let mut v = super::Xq56VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_56_veb_predecessor() {
        let mut v = super::Xq56VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_56_veb_count() {
        let mut v = super::Xq56VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_56_veb_duplicate_insert() {
        let mut v = super::Xq56VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_56_kdtree_empty() {
        let tree = super::Xr56KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_56_kdtree_insert_one() {
        let mut tree = super::Xr56KDTree::xr_new();
        tree.xr_insert(super::Xr56KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_56_kdtree_insert_multiple() {
        let mut tree = super::Xr56KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr56KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_56_kdtree_nearest_neighbor() {
        let mut tree = super::Xr56KDTree::xr_new();
        tree.xr_insert(super::Xr56KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr56KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr56KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_56_kdtree_nn_empty() {
        let tree = super::Xr56KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr56KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_56_kdtree_range_search() {
        let mut tree = super::Xr56KDTree::xr_new();
        tree.xr_insert(super::Xr56KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr56KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr56KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_56_kdtree_range_empty() {
        let mut tree = super::Xr56KDTree::xr_new();
        tree.xr_insert(super::Xr56KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_56_kdtree_all_points() {
        let mut tree = super::Xr56KDTree::xr_new();
        tree.xr_insert(super::Xr56KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr56KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_56_kdtree_depth() {
        let mut tree = super::Xr56KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr56KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_56_kdtree_bounding_box() {
        let mut tree = super::Xr56KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr56KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr56KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_56_persistent_array_new() {
        let arr = super::Xs56PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_56_persistent_array_push() {
        let mut arr = super::Xs56PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_56_persistent_array_set() {
        let mut arr = super::Xs56PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_56_persistent_array_diff() {
        let mut arr = super::Xs56PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_56_persistent_array_rollback() {
        let mut arr = super::Xs56PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_56_persistent_array_history() {
        let mut arr = super::Xs56PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_56_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs56PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_56_persistent_array_from_vec() {
        let arr = super::Xs56PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_56_concurrent_queue_new() {
        let q = super::Xs56ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_56_concurrent_queue_push_pop() {
        let mut q = super::Xs56ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_56_concurrent_queue_full() {
        let mut q = super::Xs56ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_56_concurrent_queue_drain() {
        let mut q = super::Xs56ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_56_concurrent_queue_try_pop() {
        let mut q = super::Xs56ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_56_concurrent_queue_clear() {
        let mut q = super::Xs56ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_56_range_map_new() {
        let rm = super::Xs56RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_56_range_map_insert_get() {
        let mut rm = super::Xs56RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_56_range_map_overlap() {
        let mut rm = super::Xs56RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_56_range_map_remove() {
        let mut rm = super::Xs56RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_56_range_map_gaps() {
        let mut rm = super::Xs56RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_56_range_map_coverage() {
        let mut rm = super::Xs56RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_56_range_map_contains() {
        let mut rm = super::Xs56RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_56_range_map_clear() {
        let mut rm = super::Xs56RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_56_circular_buffer_new() {
        let buf = super::Xs56CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_56_circular_buffer_push_pop() {
        let mut buf = super::Xs56CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_56_circular_buffer_overwrite() {
        let mut buf = super::Xs56CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_56_circular_buffer_peek() {
        let mut buf = super::Xs56CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_56_circular_buffer_is_full() {
        let mut buf = super::Xs56CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_56_circular_buffer_iter() {
        let mut buf = super::Xs56CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_56_circular_buffer_clear() {
        let mut buf = super::Xs56CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_56_circular_buffer_to_vec() {
        let mut buf = super::Xs56CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

}
