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

}
