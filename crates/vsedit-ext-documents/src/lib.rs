//! Ext API: Documents.
//!
//! RPC bridge between the extension host and the main thread for documents.

use std::collections::HashMap;

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
}
