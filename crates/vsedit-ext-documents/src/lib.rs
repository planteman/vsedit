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
}
