//! Ext API: Notebook.
//!
//! RPC bridge between the extension host and the main thread for notebook support.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_notebook";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NotebookMessage {
    OpenDocument {
        uri: String,
    },
    CloseDocument {
        uri: String,
    },
    ExecuteCell {
        uri: String,
        cell_index: u32,
    },
    RegisterKernel {
        id: String,
        label: String,
    },
    UnregisterKernel {
        id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum NotebookCellKind {
    Markup,
    Code,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookCell {
    pub index: u32,
    pub kind: NotebookCellKind,
    pub language_id: String,
    pub content: String,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookDocument {
    pub uri: String,
    pub notebook_type: String,
    pub cells: Vec<NotebookCell>,
    pub is_dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookKernel {
    pub id: String,
    pub label: String,
    pub supported_languages: Vec<String>,
}

// ── Error Types ──

#[derive(Debug, Clone, PartialEq)]
pub enum NotebookError {
    DocumentNotOpen(String),
    CellOutOfBounds { uri: String, index: u32, count: u32 },
    KernelNotFound(String),
    DuplicateKernel(String),
    InvalidUri(String),
    UnsupportedLanguage { kernel_id: String, language: String },
}

impl fmt::Display for NotebookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentNotOpen(uri) => write!(f, "document not open: {uri}"),
            Self::CellOutOfBounds { uri, index, count } => {
                write!(f, "cell index {index} out of bounds (document {uri} has {count} cells)")
            }
            Self::KernelNotFound(id) => write!(f, "kernel not found: {id}"),
            Self::DuplicateKernel(id) => write!(f, "kernel already registered: {id}"),
            Self::InvalidUri(uri) => write!(f, "invalid notebook URI: {uri}"),
            Self::UnsupportedLanguage { kernel_id, language } => {
                write!(f, "kernel {kernel_id} does not support language: {language}")
            }
        }
    }
}

impl std::error::Error for NotebookError {}

// ── Display Implementations ──

impl fmt::Display for NotebookCellKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markup => write!(f, "markup"),
            Self::Code => write!(f, "code"),
        }
    }
}

impl fmt::Display for NotebookCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Cell[{}] ({}, {}): {} chars",
            self.index,
            self.kind,
            self.language_id,
            self.content.len()
        )
    }
}

impl fmt::Display for NotebookDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Notebook({}, {} cells{})",
            self.uri,
            self.cells.len(),
            if self.is_dirty { ", dirty" } else { "" }
        )
    }
}

impl fmt::Display for NotebookKernel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Kernel({}: {})", self.id, self.label)
    }
}

// ── Builder ──

/// Builder for constructing `NotebookDocument` instances with validation.
pub struct NotebookDocumentBuilder {
    uri: Option<String>,
    notebook_type: String,
    cells: Vec<NotebookCell>,
}

impl NotebookDocumentBuilder {
    pub fn new() -> Self {
        Self {
            uri: None,
            notebook_type: "jupyter-notebook".to_string(),
            cells: Vec::new(),
        }
    }

    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    pub fn notebook_type(mut self, ty: impl Into<String>) -> Self {
        self.notebook_type = ty.into();
        self
    }

    pub fn add_code_cell(mut self, language: impl Into<String>, content: impl Into<String>) -> Self {
        let index = self.cells.len() as u32;
        self.cells.push(NotebookCell {
            index,
            kind: NotebookCellKind::Code,
            language_id: language.into(),
            content: content.into(),
            outputs: Vec::new(),
        });
        self
    }

    pub fn add_markup_cell(mut self, content: impl Into<String>) -> Self {
        let index = self.cells.len() as u32;
        self.cells.push(NotebookCell {
            index,
            kind: NotebookCellKind::Markup,
            language_id: "markdown".to_string(),
            content: content.into(),
            outputs: Vec::new(),
        });
        self
    }

    pub fn build(self) -> Result<NotebookDocument, NotebookError> {
        let uri = self.uri.ok_or_else(|| NotebookError::InvalidUri(String::new()))?;
        if uri.is_empty() || !uri.contains("://") {
            return Err(NotebookError::InvalidUri(uri));
        }
        Ok(NotebookDocument {
            uri,
            notebook_type: self.notebook_type,
            cells: self.cells,
            is_dirty: false,
        })
    }
}

impl Default for NotebookDocumentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Domain Logic on Core Types ──

impl NotebookCell {
    /// Returns `true` if this cell contains no meaningful content.
    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    /// Returns the number of lines in the cell content.
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            self.content.lines().count()
        }
    }
}

impl NotebookDocument {
    /// Validate that a cell index is within bounds.
    pub fn validate_cell_index(&self, index: u32) -> Result<(), NotebookError> {
        let count = self.cells.len() as u32;
        if index >= count {
            Err(NotebookError::CellOutOfBounds {
                uri: self.uri.clone(),
                index,
                count,
            })
        } else {
            Ok(())
        }
    }

    /// Returns only the code cells.
    pub fn code_cells(&self) -> Vec<&NotebookCell> {
        self.cells
            .iter()
            .filter(|c| c.kind == NotebookCellKind::Code)
            .collect()
    }

    /// Returns only the markup cells.
    pub fn markup_cells(&self) -> Vec<&NotebookCell> {
        self.cells
            .iter()
            .filter(|c| c.kind == NotebookCellKind::Markup)
            .collect()
    }

    /// Total character count across all cells.
    pub fn total_content_length(&self) -> usize {
        self.cells.iter().map(|c| c.content.len()).sum()
    }

    /// Set of distinct languages used in code cells.
    pub fn languages_used(&self) -> Vec<String> {
        let mut langs: Vec<String> = self
            .code_cells()
            .iter()
            .map(|c| c.language_id.clone())
            .collect();
        langs.sort();
        langs.dedup();
        langs
    }

    /// Mark the document as dirty.
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// Mark the document as clean (saved).
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }
}

impl NotebookKernel {
    /// Check whether this kernel supports a given language.
    pub fn supports_language(&self, language: &str) -> bool {
        self.supported_languages.iter().any(|l| l == language)
    }

    /// Validate that this kernel can execute a cell, returning an error if not.
    pub fn validate_cell(&self, cell: &NotebookCell) -> Result<(), NotebookError> {
        if cell.kind != NotebookCellKind::Code {
            return Ok(());
        }
        if !self.supported_languages.is_empty() && !self.supports_language(&cell.language_id) {
            return Err(NotebookError::UnsupportedLanguage {
                kernel_id: self.id.clone(),
                language: cell.language_id.clone(),
            });
        }
        Ok(())
    }
}

// ── Bridge ──

#[derive(Clone)]
pub struct NotebookBridge {
    kernels: Vec<NotebookKernel>,
    open_documents: Vec<String>,
}

impl NotebookBridge {
    pub fn new() -> Self {
        Self {
            kernels: Vec::new(),
            open_documents: Vec::new(),
        }
    }

    pub fn register_kernel(&mut self, kernel: NotebookKernel) {
        if !self.kernels.iter().any(|k| k.id == kernel.id) {
            self.kernels.push(kernel);
        }
    }

    pub fn unregister_kernel(&mut self, id: &str) {
        self.kernels.retain(|k| k.id != id);
    }

    pub fn get_kernel(&self, id: &str) -> Option<&NotebookKernel> {
        self.kernels.iter().find(|k| k.id == id)
    }

    pub fn open_document(&mut self, uri: &str) {
        if !self.open_documents.contains(&uri.to_string()) {
            self.open_documents.push(uri.to_string());
        }
    }

    pub fn close_document(&mut self, uri: &str) {
        self.open_documents.retain(|u| u != uri);
    }

    pub fn is_document_open(&self, uri: &str) -> bool {
        self.open_documents.iter().any(|u| u == uri)
    }

    pub fn open_document_count(&self) -> usize {
        self.open_documents.len()
    }

    pub fn kernel_count(&self) -> usize {
        self.kernels.len()
    }

    /// Find kernels that support a given language.
    pub fn kernels_for_language(&self, language: &str) -> Vec<&NotebookKernel> {
        self.kernels
            .iter()
            .filter(|k| k.supports_language(language))
            .collect()
    }

    pub fn handle_message(&mut self, msg: &NotebookMessage) -> serde_json::Value {
        match msg {
            NotebookMessage::OpenDocument { uri } => {
                self.open_document(uri);
                serde_json::json!({"opened": true})
            }
            NotebookMessage::CloseDocument { uri } => {
                self.close_document(uri);
                serde_json::json!({"closed": true})
            }
            NotebookMessage::ExecuteCell { uri, cell_index } => {
                let is_open = self.open_documents.contains(uri);
                serde_json::json!({"executed": is_open, "cell": cell_index})
            }
            NotebookMessage::RegisterKernel { id, label } => {
                self.register_kernel(NotebookKernel {
                    id: id.clone(),
                    label: label.clone(),
                    supported_languages: Vec::new(),
                });
                serde_json::json!({"registered": true})
            }
            NotebookMessage::UnregisterKernel { id } => {
                self.unregister_kernel(id);
                serde_json::json!({"unregistered": true})
            }
        }
    }
}

impl Default for NotebookBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NotebookBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotebookBridge")
            .field("kernel_count", &self.kernels.len())
            .field("open_documents", &self.open_documents)
            .finish()
    }
}

/// Initialize the notebook extension API bridge.
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
        let msg = NotebookMessage::ExecuteCell {
            uri: "file:///nb.ipynb".into(),
            cell_index: 3,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: NotebookMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn cell_serialization() {
        let cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Code,
            language_id: "python".into(),
            content: "print('hi')".into(),
            outputs: vec!["hi".into()],
        };
        let json = serde_json::to_string(&cell).unwrap();
        let back: NotebookCell = serde_json::from_str(&json).unwrap();
        assert_eq!(cell, back);
    }

    #[test]
    fn bridge_kernel_lifecycle() {
        let mut bridge = NotebookBridge::new();
        bridge.register_kernel(NotebookKernel {
            id: "py".into(),
            label: "Python".into(),
            supported_languages: vec!["python".into()],
        });
        assert!(bridge.get_kernel("py").is_some());
        bridge.unregister_kernel("py");
        assert!(bridge.get_kernel("py").is_none());
    }

    #[test]
    fn bridge_open_close_document() {
        let mut bridge = NotebookBridge::new();
        bridge.open_document("file:///nb.ipynb");
        assert!(bridge.open_documents.contains(&"file:///nb.ipynb".to_string()));
        bridge.close_document("file:///nb.ipynb");
        assert!(!bridge.open_documents.contains(&"file:///nb.ipynb".to_string()));
    }

    #[test]
    fn bridge_execute_closed_doc() {
        let mut bridge = NotebookBridge::new();
        let result = bridge.handle_message(&NotebookMessage::ExecuteCell {
            uri: "file:///nb.ipynb".into(),
            cell_index: 0,
        });
        assert_eq!(result["executed"], false);
    }

    #[test]
    fn error_display_document_not_open() {
        let err = NotebookError::DocumentNotOpen("file:///test.ipynb".into());
        assert_eq!(err.to_string(), "document not open: file:///test.ipynb");
    }

    #[test]
    fn error_display_cell_out_of_bounds() {
        let err = NotebookError::CellOutOfBounds {
            uri: "file:///nb.ipynb".into(),
            index: 5,
            count: 3,
        };
        let msg = err.to_string();
        assert!(msg.contains("5"));
        assert!(msg.contains("3"));
    }

    #[test]
    fn error_display_unsupported_language() {
        let err = NotebookError::UnsupportedLanguage {
            kernel_id: "py".into(),
            language: "rust".into(),
        };
        assert!(err.to_string().contains("rust"));
    }

    #[test]
    fn cell_display_and_helpers() {
        let cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Code,
            language_id: "python".into(),
            content: "x = 1\ny = 2\nz = x + y".into(),
            outputs: vec![],
        };
        assert_eq!(cell.line_count(), 3);
        assert!(!cell.is_empty());
        let display = format!("{cell}");
        assert!(display.contains("code"));
        assert!(display.contains("python"));
    }

    #[test]
    fn cell_empty_detection() {
        let cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Code,
            language_id: "python".into(),
            content: "   \n  ".into(),
            outputs: vec![],
        };
        assert!(cell.is_empty());
    }

    #[test]
    fn document_builder_success() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///my_notebook.ipynb")
            .add_markup_cell("# Hello")
            .add_code_cell("python", "print('hi')")
            .add_code_cell("python", "x = 42")
            .build()
            .unwrap();
        assert_eq!(doc.cells.len(), 3);
        assert_eq!(doc.code_cells().len(), 2);
        assert_eq!(doc.markup_cells().len(), 1);
        assert!(!doc.is_dirty);
    }

    #[test]
    fn document_builder_invalid_uri() {
        let result = NotebookDocumentBuilder::new()
            .uri("bad-uri")
            .build();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NotebookError::InvalidUri(_)));
    }

    #[test]
    fn document_builder_missing_uri() {
        let result = NotebookDocumentBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn document_validate_cell_index() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "x = 1")
            .build()
            .unwrap();
        assert!(doc.validate_cell_index(0).is_ok());
        assert!(doc.validate_cell_index(1).is_err());
    }

    #[test]
    fn document_languages_used() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "x = 1")
            .add_code_cell("rust", "let x = 1;")
            .add_code_cell("python", "y = 2")
            .add_markup_cell("# Title")
            .build()
            .unwrap();
        let langs = doc.languages_used();
        assert_eq!(langs, vec!["python", "rust"]);
    }

    #[test]
    fn document_total_content_length() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "abc")
            .add_code_cell("python", "de")
            .build()
            .unwrap();
        assert_eq!(doc.total_content_length(), 5);
    }

    #[test]
    fn document_dirty_flag() {
        let mut doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .build()
            .unwrap();
        assert!(!doc.is_dirty);
        doc.mark_dirty();
        assert!(doc.is_dirty);
        doc.mark_clean();
        assert!(!doc.is_dirty);
    }

    #[test]
    fn kernel_supports_language() {
        let kernel = NotebookKernel {
            id: "py".into(),
            label: "Python 3".into(),
            supported_languages: vec!["python".into(), "python3".into()],
        };
        assert!(kernel.supports_language("python"));
        assert!(!kernel.supports_language("rust"));
    }

    #[test]
    fn kernel_validate_cell_unsupported() {
        let kernel = NotebookKernel {
            id: "py".into(),
            label: "Python".into(),
            supported_languages: vec!["python".into()],
        };
        let cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Code,
            language_id: "rust".into(),
            content: "let x = 1;".into(),
            outputs: vec![],
        };
        let result = kernel.validate_cell(&cell);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NotebookError::UnsupportedLanguage { .. }
        ));
    }

    #[test]
    fn kernel_validate_markup_cell_always_ok() {
        let kernel = NotebookKernel {
            id: "py".into(),
            label: "Python".into(),
            supported_languages: vec!["python".into()],
        };
        let cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Markup,
            language_id: "markdown".into(),
            content: "# Hello".into(),
            outputs: vec![],
        };
        assert!(kernel.validate_cell(&cell).is_ok());
    }

    #[test]
    fn bridge_kernels_for_language() {
        let mut bridge = NotebookBridge::new();
        bridge.register_kernel(NotebookKernel {
            id: "py".into(),
            label: "Python".into(),
            supported_languages: vec!["python".into()],
        });
        bridge.register_kernel(NotebookKernel {
            id: "rs".into(),
            label: "Rust".into(),
            supported_languages: vec!["rust".into()],
        });
        assert_eq!(bridge.kernels_for_language("python").len(), 1);
        assert_eq!(bridge.kernels_for_language("java").len(), 0);
    }

    #[test]
    fn bridge_counts_and_state() {
        let mut bridge = NotebookBridge::new();
        assert_eq!(bridge.kernel_count(), 0);
        assert_eq!(bridge.open_document_count(), 0);
        bridge.open_document("file:///a.ipynb");
        bridge.open_document("file:///b.ipynb");
        assert_eq!(bridge.open_document_count(), 2);
        assert!(bridge.is_document_open("file:///a.ipynb"));
        assert!(!bridge.is_document_open("file:///c.ipynb"));
    }

    #[test]
    fn bridge_debug_impl() {
        let bridge = NotebookBridge::new();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("NotebookBridge"));
        assert!(debug.contains("kernel_count"));
    }

    #[test]
    fn bridge_duplicate_kernel_ignored() {
        let mut bridge = NotebookBridge::new();
        let kernel = NotebookKernel {
            id: "py".into(),
            label: "Python".into(),
            supported_languages: vec!["python".into()],
        };
        bridge.register_kernel(kernel.clone());
        bridge.register_kernel(kernel);
        assert_eq!(bridge.kernel_count(), 1);
    }

    #[test]
    fn bridge_duplicate_open_document_ignored() {
        let mut bridge = NotebookBridge::new();
        bridge.open_document("file:///a.ipynb");
        bridge.open_document("file:///a.ipynb");
        assert_eq!(bridge.open_document_count(), 1);
    }

    #[test]
    fn document_display() {
        let mut doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "x = 1")
            .build()
            .unwrap();
        let clean = format!("{doc}");
        assert!(clean.contains("1 cells"));
        assert!(!clean.contains("dirty"));
        doc.mark_dirty();
        let dirty = format!("{doc}");
        assert!(dirty.contains("dirty"));
    }

    #[test]
    fn notebook_cell_kind_display() {
        assert_eq!(format!("{}", NotebookCellKind::Code), "code");
        assert_eq!(format!("{}", NotebookCellKind::Markup), "markup");
    }

    #[test]
    fn kernel_display() {
        let k = NotebookKernel {
            id: "py".into(),
            label: "Python 3".into(),
            supported_languages: vec![],
        };
        let s = format!("{k}");
        assert!(s.contains("py"));
        assert!(s.contains("Python 3"));
    }
}
