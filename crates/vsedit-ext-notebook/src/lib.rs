//! Ext API: Notebook.
//!
//! RPC bridge between the extension host and the main thread for notebook support.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
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

// ── Cell Serialization Helpers ──

/// Serialize a notebook cell to a compact JSON string.
pub fn serialize_cell(cell: &NotebookCell) -> String {
    serde_json::to_string(cell).unwrap_or_default()
}

/// Deserialize a notebook cell from a JSON string.
pub fn deserialize_cell(json: &str) -> Result<NotebookCell, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Serialize all cells of a document to a JSON array string.
pub fn serialize_cells(doc: &NotebookDocument) -> String {
    serde_json::to_string(&doc.cells).unwrap_or_default()
}

// ── Cell Metadata Management ──

/// Metadata associated with a notebook cell for extension use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CellMetadata {
    pub cell_index: u32,
    pub execution_count: Option<u32>,
    pub tags: Vec<String>,
    pub custom: std::collections::HashMap<String, String>,
}

impl CellMetadata {
    /// Create empty metadata for a given cell index.
    pub fn new(cell_index: u32) -> Self {
        Self {
            cell_index,
            execution_count: None,
            tags: Vec::new(),
            custom: std::collections::HashMap::new(),
        }
    }

    /// Add a tag to this cell's metadata. Duplicates are ignored.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Check whether a specific tag is present.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Set a custom key-value pair.
    pub fn set_custom(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.custom.insert(key.into(), value.into());
    }
}

// ── Notebook Document Diff Computation ──

/// Represents a single change between two notebook documents.
#[derive(Debug, Clone, PartialEq)]
pub enum CellDiff {
    Added { index: u32 },
    Removed { index: u32 },
    ContentChanged { index: u32, old_len: usize, new_len: usize },
}

/// Compute the cell-level differences between two notebook documents.
pub fn diff_documents(old: &NotebookDocument, new: &NotebookDocument) -> Vec<CellDiff> {
    let mut diffs = Vec::new();
    let max_len = old.cells.len().max(new.cells.len());
    for i in 0..max_len {
        match (old.cells.get(i), new.cells.get(i)) {
            (None, Some(_)) => diffs.push(CellDiff::Added { index: i as u32 }),
            (Some(_), None) => diffs.push(CellDiff::Removed { index: i as u32 }),
            (Some(a), Some(b)) if a.content != b.content => {
                diffs.push(CellDiff::ContentChanged {
                    index: i as u32,
                    old_len: a.content.len(),
                    new_len: b.content.len(),
                });
            }
            _ => {}
        }
    }
    diffs
}

// ── Execution Order Tracking ──

/// Tracks the execution order of notebook cells.
#[derive(Debug, Clone)]
pub struct ExecutionTracker {
    order: Vec<u32>,
}

impl ExecutionTracker {
    pub fn new() -> Self {
        Self { order: Vec::new() }
    }

    /// Record that a cell at the given index was executed.
    pub fn record_execution(&mut self, cell_index: u32) {
        self.order.push(cell_index);
    }

    /// Return the execution order as a slice.
    pub fn execution_order(&self) -> &[u32] {
        &self.order
    }

    /// Return the number of executions recorded.
    pub fn execution_count(&self) -> usize {
        self.order.len()
    }

    /// Return the last executed cell index, if any.
    pub fn last_executed(&self) -> Option<u32> {
        self.order.last().copied()
    }

    /// Reset the execution history.
    pub fn reset(&mut self) {
        self.order.clear();
    }
}

impl Default for ExecutionTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// The MIME type of a cell output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutputMimeType {
    PlainText,
    Html,
    Markdown,
    Image,
    Error,
    Custom(String),
}

impl fmt::Display for OutputMimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputMimeType::PlainText => write!(f, "text/plain"),
            OutputMimeType::Html => write!(f, "text/html"),
            OutputMimeType::Markdown => write!(f, "text/markdown"),
            OutputMimeType::Image => write!(f, "image/png"),
            OutputMimeType::Error => write!(f, "application/vnd.code.notebook.error"),
            OutputMimeType::Custom(mime) => write!(f, "{}", mime),
        }
    }
}

/// Represents the output of a cell execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotebookCellOutput {
    pub cell_index: u32,
    pub mime_type: OutputMimeType,
    pub data: String,
    pub execution_order: Option<u32>,
    pub success: bool,
}

impl NotebookCellOutput {
    /// Create a successful plain-text output.
    pub fn text(cell_index: u32, data: impl Into<String>) -> Self {
        Self {
            cell_index,
            mime_type: OutputMimeType::PlainText,
            data: data.into(),
            execution_order: None,
            success: true,
        }
    }

    /// Create an error output.
    pub fn error(cell_index: u32, message: impl Into<String>) -> Self {
        Self {
            cell_index,
            mime_type: OutputMimeType::Error,
            data: message.into(),
            execution_order: None,
            success: false,
        }
    }

    /// Set the execution order.
    pub fn with_order(mut self, order: u32) -> Self {
        self.execution_order = Some(order);
        self
    }

    /// Returns `true` if this is an error output.
    pub fn is_error(&self) -> bool {
        !self.success
    }

    /// Returns the byte length of the output data.
    pub fn data_size(&self) -> usize {
        self.data.len()
    }
}

impl fmt::Display for NotebookCellOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "ok" } else { "err" };
        write!(
            f,
            "Output[{}] ({}, {}): {} bytes",
            self.cell_index,
            self.mime_type,
            status,
            self.data.len()
        )
    }
}

/// Picks the best kernel for a given language from a set of available kernels.
pub struct NotebookKernelPicker;

impl NotebookKernelPicker {
    /// Find all kernels that support the given language.
    pub fn kernels_for_language<'a>(
        kernels: &'a [NotebookKernel],
        language: &str,
    ) -> Vec<&'a NotebookKernel> {
        kernels
            .iter()
            .filter(|k| k.supports_language(language))
            .collect()
    }

    /// Pick the best kernel for a language. Prefers kernels with fewer supported
    /// languages (more specialized).
    pub fn pick_best<'a>(
        kernels: &'a [NotebookKernel],
        language: &str,
    ) -> Option<&'a NotebookKernel> {
        let mut candidates: Vec<&NotebookKernel> = Self::kernels_for_language(kernels, language);
        candidates.sort_by_key(|k| k.supported_languages.len());
        candidates.first().copied()
    }

    /// Return kernels sorted by relevance for a given language.
    /// Kernels that support the language come first, sorted by specialization.
    pub fn ranked_kernels<'a>(
        kernels: &'a [NotebookKernel],
        language: &str,
    ) -> Vec<&'a NotebookKernel> {
        let mut all: Vec<(&NotebookKernel, bool)> = kernels
            .iter()
            .map(|k| (k, k.supports_language(language)))
            .collect();
        all.sort_by(|a, b| {
            b.1.cmp(&a.1) // supporting kernels first
                .then_with(|| a.0.supported_languages.len().cmp(&b.0.supported_languages.len()))
        });
        all.into_iter().map(|(k, _)| k).collect()
    }
}

/// Build a mapping from cell index to its execution order number
/// based on recorded outputs.
pub fn cell_execution_order(outputs: &[NotebookCellOutput]) -> std::collections::HashMap<u32, u32> {
    let mut map = std::collections::HashMap::new();
    let mut counter = 1u32;
    for output in outputs {
        let order = output.execution_order.unwrap_or_else(|| {
            let o = counter;
            counter += 1;
            o
        });
        map.entry(output.cell_index).or_insert(order);
    }
    map
}

/// Return the total number of successful cell executions.
pub fn count_successful_executions(outputs: &[NotebookCellOutput]) -> usize {
    outputs.iter().filter(|o| o.success).count()
}

/// Return the total number of failed cell executions.
pub fn count_failed_executions(outputs: &[NotebookCellOutput]) -> usize {
    outputs.iter().filter(|o| !o.success).count()
}

// ---------------------------------------------------------------------------
// NotebookDocument – additional query helpers
// ---------------------------------------------------------------------------

impl NotebookDocument {
    /// Return the number of cells in this document.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Search all cells for one whose content contains the given text.
    /// Returns `(cell_index_in_vec, &cell)`.
    pub fn find_cell_by_content(&self, text: &str) -> Option<(usize, &NotebookCell)> {
        self.cells
            .iter()
            .enumerate()
            .find(|(_, c)| c.content.contains(text))
    }
}

// ---------------------------------------------------------------------------
// NotebookCell – additional helpers
// ---------------------------------------------------------------------------

impl NotebookCell {
    /// Approximate word count of the cell content.
    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    /// Returns `true` if this is a code cell.
    pub fn is_code(&self) -> bool {
        self.kind == NotebookCellKind::Code
    }

    /// Returns `true` if this is a markup cell.
    pub fn is_markup(&self) -> bool {
        self.kind == NotebookCellKind::Markup
    }
}

// ---------------------------------------------------------------------------
// NotebookBridge – additional query helpers
// ---------------------------------------------------------------------------

impl NotebookBridge {
    /// Return the IDs of all registered kernels.
    pub fn all_kernel_ids(&self) -> Vec<&str> {
        self.kernels.iter().map(|k| k.id.as_str()).collect()
    }

    /// Return the URIs of all open documents.
    pub fn open_document_uris(&self) -> Vec<&str> {
        self.open_documents.iter().map(|u| u.as_str()).collect()
    }
}

impl fmt::Display for NotebookBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NotebookBridge(kernels={}, open_docs={})",
            self.kernels.len(),
            self.open_documents.len()
        )
    }
}

// ---------------------------------------------------------------------------
// CellMetadata – helpers
// ---------------------------------------------------------------------------

impl CellMetadata {
    /// Return the number of tags on this cell.
    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

// ── Cell Execution State Tracking ──

/// The execution state of a single notebook cell.
#[derive(Debug, Clone, PartialEq)]
pub enum CellExecutionState {
    Idle,
    Running,
    Success,
    Error(String),
}

impl fmt::Display for CellExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Running => write!(f, "running"),
            Self::Success => write!(f, "success"),
            Self::Error(msg) => write!(f, "error: {msg}"),
        }
    }
}

impl From<bool> for CellExecutionState {
    /// `true` maps to `Success`, `false` maps to `Error`.
    fn from(ok: bool) -> Self {
        if ok {
            Self::Success
        } else {
            Self::Error("execution failed".into())
        }
    }
}

/// Tracks per-cell execution state across a notebook.
#[derive(Debug, Clone)]
pub struct CellExecutionTracker {
    states: std::collections::HashMap<u32, CellExecutionState>,
}

impl CellExecutionTracker {
    pub fn new() -> Self {
        Self {
            states: std::collections::HashMap::new(),
        }
    }

    /// Get the execution state of a cell (defaults to `Idle`).
    pub fn state(&self, cell_index: u32) -> &CellExecutionState {
        static IDLE: CellExecutionState = CellExecutionState::Idle;
        self.states.get(&cell_index).unwrap_or(&IDLE)
    }

    pub fn mark_running(&mut self, cell_index: u32) {
        self.states.insert(cell_index, CellExecutionState::Running);
    }

    pub fn mark_success(&mut self, cell_index: u32) {
        self.states.insert(cell_index, CellExecutionState::Success);
    }

    pub fn mark_error(&mut self, cell_index: u32, msg: impl Into<String>) {
        self.states
            .insert(cell_index, CellExecutionState::Error(msg.into()));
    }

    /// Return indices of all cells currently running.
    pub fn running_cells(&self) -> Vec<u32> {
        let mut cells: Vec<u32> = self
            .states
            .iter()
            .filter(|(_, s)| matches!(s, CellExecutionState::Running))
            .map(|(&idx, _)| idx)
            .collect();
        cells.sort();
        cells
    }

    /// Reset all tracked states.
    pub fn reset_all(&mut self) {
        self.states.clear();
    }
}

impl Default for CellExecutionTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Notebook Outline (Table of Contents) ──

/// A single heading entry extracted from a markup cell.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    /// Heading level (1 for `#`, 2 for `##`, etc.).
    pub level: u8,
    /// The heading text.
    pub title: String,
    /// Index of the cell containing this heading.
    pub cell_index: u32,
}

/// Extracts a table-of-contents outline from the markup cells of a notebook.
#[derive(Debug, Clone)]
pub struct NotebookOutline {
    entries: Vec<OutlineEntry>,
}

impl NotebookOutline {
    /// Build an outline from a notebook document by scanning markup cells
    /// for lines that start with one or more `#` characters.
    pub fn from_document(doc: &NotebookDocument) -> Self {
        let mut entries = Vec::new();
        for cell in &doc.cells {
            if cell.kind != NotebookCellKind::Markup {
                continue;
            }
            for line in cell.content.lines() {
                let trimmed = line.trim_start();
                if !trimmed.starts_with('#') {
                    continue;
                }
                let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
                let title = trimmed[hashes..].trim().to_string();
                if !title.is_empty() && hashes <= 6 {
                    entries.push(OutlineEntry {
                        level: hashes as u8,
                        title,
                        cell_index: cell.index,
                    });
                }
            }
        }
        Self { entries }
    }

    /// Return all outline entries.
    pub fn entries(&self) -> &[OutlineEntry] {
        &self.entries
    }

    /// Return entries filtered to a maximum heading level.
    pub fn entries_up_to_level(&self, max_level: u8) -> Vec<&OutlineEntry> {
        self.entries.iter().filter(|e| e.level <= max_level).collect()
    }
}

impl fmt::Display for NotebookOutline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Outline({} entries)", self.entries.len())
    }
}

// ── Cell Dependency Analyzer ──

/// Variable definition and reference information for a single cell.
#[derive(Debug, Clone)]
pub struct CellVarInfo {
    pub cell_index: u32,
    /// Variable names defined (assigned) in this cell.
    pub definitions: Vec<String>,
    /// Variable names referenced (used) in this cell.
    pub references: Vec<String>,
}

/// Analyzes code cells for variable definitions and references to suggest
/// execution order based on data dependencies.
#[derive(Debug, Clone)]
pub struct CellDependencyAnalyzer {
    cells: Vec<CellVarInfo>,
}

impl CellDependencyAnalyzer {
    /// Analyze a notebook document. Uses simple heuristic pattern matching
    /// for `name = ...` assignments and bare identifiers as references.
    pub fn analyze(doc: &NotebookDocument) -> Self {
        let mut cells = Vec::new();
        for cell in &doc.cells {
            if cell.kind != NotebookCellKind::Code {
                continue;
            }
            let mut definitions = Vec::new();
            let mut references = Vec::new();

            for line in cell.content.lines() {
                let trimmed = line.trim();
                // Detect simple assignments: `name = ...`
                if let Some(eq_pos) = trimmed.find('=') {
                    let lhs = trimmed[..eq_pos].trim();
                    // Only treat it as a definition if the LHS is a simple identifier
                    if !lhs.is_empty()
                        && lhs.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        && !lhs.starts_with(|c: char| c.is_ascii_digit())
                    {
                        if !definitions.contains(&lhs.to_string()) {
                            definitions.push(lhs.to_string());
                        }
                    }
                    // Scan the RHS for identifier references
                    let rhs = trimmed[eq_pos + 1..].trim();
                    Self::extract_idents(rhs, &mut references);
                } else {
                    Self::extract_idents(trimmed, &mut references);
                }
            }

            // Remove self-definitions from references
            references.retain(|r| !definitions.contains(r));

            cells.push(CellVarInfo {
                cell_index: cell.index,
                definitions,
                references,
            });
        }
        Self { cells }
    }

    /// Extract simple identifiers from a string fragment.
    fn extract_idents(s: &str, out: &mut Vec<String>) {
        for token in s.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            let t = token.trim();
            if !t.is_empty()
                && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && !t.starts_with(|c: char| c.is_ascii_digit())
                && !out.contains(&t.to_string())
            {
                out.push(t.to_string());
            }
        }
    }

    /// Get the variable info for a cell by its index.
    pub fn cell_info(&self, cell_index: u32) -> Option<&CellVarInfo> {
        self.cells.iter().find(|c| c.cell_index == cell_index)
    }

    /// Return the indices of cells that define variables referenced by the
    /// given cell (i.e. its dependencies).
    pub fn dependencies_of(&self, cell_index: u32) -> Vec<u32> {
        let info = match self.cell_info(cell_index) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let mut deps = Vec::new();
        for r in &info.references {
            for other in &self.cells {
                if other.cell_index != cell_index && other.definitions.contains(r) {
                    if !deps.contains(&other.cell_index) {
                        deps.push(other.cell_index);
                    }
                }
            }
        }
        deps.sort();
        deps
    }

    /// Suggest an execution order that respects data dependencies
    /// (topological sort; falls back to document order on cycles).
    pub fn suggested_order(&self) -> Vec<u32> {
        let indices: Vec<u32> = self.cells.iter().map(|c| c.cell_index).collect();
        let mut visited = std::collections::HashSet::new();
        let mut order = Vec::new();

        for &idx in &indices {
            self.topo_visit(idx, &mut visited, &mut order, &indices);
        }
        order
    }

    fn topo_visit(
        &self,
        idx: u32,
        visited: &mut std::collections::HashSet<u32>,
        order: &mut Vec<u32>,
        all: &[u32],
    ) {
        if visited.contains(&idx) {
            return;
        }
        visited.insert(idx);
        for dep in self.dependencies_of(idx) {
            if all.contains(&dep) {
                self.topo_visit(dep, visited, order, all);
            }
        }
        order.push(idx);
    }
}

// ── Notebook Exporter ──

/// Serializes a `NotebookDocument` to a simple markdown representation.
pub struct NotebookExporter;

impl NotebookExporter {
    /// Export a notebook document as markdown. Markup cells are emitted
    /// verbatim; code cells are wrapped in fenced code blocks.
    pub fn to_markdown(doc: &NotebookDocument) -> String {
        let mut out = String::new();
        for (i, cell) in doc.cells.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match cell.kind {
                NotebookCellKind::Markup => {
                    out.push_str(&cell.content);
                    out.push('\n');
                }
                NotebookCellKind::Code => {
                    out.push_str("```");
                    out.push_str(&cell.language_id);
                    out.push('\n');
                    out.push_str(&cell.content);
                    out.push_str("\n```\n");
                }
            }
        }
        out
    }

    /// Export only code cells, each wrapped in a fenced code block.
    pub fn code_cells_to_markdown(doc: &NotebookDocument) -> String {
        let mut out = String::new();
        for cell in doc.code_cells() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("```");
            out.push_str(&cell.language_id);
            out.push('\n');
            out.push_str(&cell.content);
            out.push_str("\n```\n");
        }
        out
    }
}

// ── Kernel Matcher ──────────────────────────────────────────────────────────

/// Matches kernels to notebooks based on language overlap.
pub struct NotebookKernelMatcher;

impl NotebookKernelMatcher {
    /// Returns the number of cells whose language is supported by `kernel`.
    pub fn match_score(kernel: &NotebookKernel, notebook: &NotebookDocument) -> usize {
        notebook
            .cells
            .iter()
            .filter(|c| kernel.supported_languages.contains(&c.language_id))
            .count()
    }

    /// Returns `true` if any cell in `notebook` has a language supported by `kernel`.
    pub fn matches(kernel: &NotebookKernel, notebook: &NotebookDocument) -> bool {
        Self::match_score(kernel, notebook) > 0
    }

    /// Returns the index of the kernel with the highest match score, or `None`
    /// if no kernel matches any cell.
    pub fn best_match(
        kernels: &[NotebookKernel],
        notebook: &NotebookDocument,
    ) -> Option<usize> {
        kernels
            .iter()
            .enumerate()
            .map(|(i, k)| (i, Self::match_score(k, notebook)))
            .filter(|(_, score)| *score > 0)
            .max_by_key(|(_, score)| *score)
            .map(|(i, _)| i)
    }
}

// ── Output Renderer ─────────────────────────────────────────────────────────

/// Selects and renders the most appropriate cell output based on a MIME-type
/// priority list.
pub struct NotebookOutputRenderer {
    pub priority_order: Vec<OutputMimeType>,
}

impl NotebookOutputRenderer {
    /// Creates a renderer with the default priority order:
    /// Html > Markdown > PlainText > Image > Error.
    pub fn new() -> Self {
        Self {
            priority_order: vec![
                OutputMimeType::Html,
                OutputMimeType::Markdown,
                OutputMimeType::PlainText,
                OutputMimeType::Image,
                OutputMimeType::Error,
            ],
        }
    }

    /// Returns the output whose MIME type appears earliest in the priority list.
    pub fn preferred_output<'a>(
        &self,
        outputs: &'a [NotebookCellOutput],
    ) -> Option<&'a NotebookCellOutput> {
        for mime in &self.priority_order {
            if let Some(out) = outputs.iter().find(|o| &o.mime_type == mime) {
                return Some(out);
            }
        }
        outputs.first()
    }

    /// Renders an output as a plain-text string suitable for terminal display.
    pub fn render_text(output: &NotebookCellOutput) -> String {
        match &output.mime_type {
            OutputMimeType::PlainText => output.data.clone(),
            OutputMimeType::Html => format!("[HTML] {}", output.data),
            OutputMimeType::Markdown => format!("[Markdown] {}", output.data),
            OutputMimeType::Image => "[binary image data]".to_string(),
            OutputMimeType::Error => format!("[Error] {}", output.data),
            OutputMimeType::Custom(mime) => format!("[{}] {}", mime, output.data),
        }
    }
}

impl Default for NotebookOutputRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Cell Execution Queue ────────────────────────────────────────────────────

/// A FIFO queue of cell indices awaiting execution, with at most one cell
/// running at a time.
pub struct NotebookCellExecutionQueue {
    queue: VecDeque<usize>,
    running: Option<usize>,
}

impl NotebookCellExecutionQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            running: None,
        }
    }

    /// Appends a cell index to the back of the queue.
    pub fn enqueue(&mut self, idx: usize) {
        self.queue.push_back(idx);
    }

    /// Pops the next cell from the front of the queue and marks it as running.
    /// Returns `None` if the queue is empty or a cell is already running.
    pub fn dequeue(&mut self) -> Option<usize> {
        if self.running.is_some() {
            return None;
        }
        if let Some(idx) = self.queue.pop_front() {
            self.running = Some(idx);
            Some(idx)
        } else {
            None
        }
    }

    /// Marks the currently running cell as complete.
    pub fn complete(&mut self) {
        self.running = None;
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Clears the queue and the running cell.
    pub fn cancel_all(&mut self) {
        self.queue.clear();
        self.running = None;
    }

    /// Returns `true` if `idx` is in the pending queue or currently running.
    pub fn contains(&self, idx: usize) -> bool {
        self.running == Some(idx) || self.queue.contains(&idx)
    }
}

impl Default for NotebookCellExecutionQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ── Variable Inspector ──────────────────────────────────────────────────────

/// Tracks named variables produced during notebook execution, keyed by name.
pub struct NotebookVariableInspector {
    variables: HashMap<String, String>,
}

impl NotebookVariableInspector {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(name.into(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.variables.remove(name)
    }

    /// Returns all variables sorted by name.
    pub fn list(&self) -> Vec<(&String, &String)> {
        let mut entries: Vec<_> = self.variables.iter().collect();
        entries.sort_by_key(|(k, _)| *k);
        entries
    }

    pub fn count(&self) -> usize {
        self.variables.len()
    }

    pub fn clear(&mut self) {
        self.variables.clear();
    }
}

impl Default for NotebookVariableInspector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NotebookKernelSelector - notebook kernel selector
// ---------------------------------------------------------------------------

/// Severity level for notebook kernel selector issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NotebookKernelSelectorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for NotebookKernelSelectorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [NotebookKernelSelector].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookKernelSelectorEntry {
    pub id: String,
    pub label: String,
    pub severity: NotebookKernelSelectorSeverity,
    pub detail: Option<String>,
    pub kernel_count: usize,
    enabled: bool,
}

impl NotebookKernelSelectorEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: NotebookKernelSelectorSeverity::Low,
            detail: None,
            kernel_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: NotebookKernelSelectorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_kernel_count(mut self, val: usize) -> Self {
        self.kernel_count = val;
        self
    }

    pub fn has_selected_kernel(&self) -> bool {
        self.enabled && self.severity >= NotebookKernelSelectorSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.kernel_count, det)
    }
}

impl fmt::Display for NotebookKernelSelectorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [NotebookKernelSelectorEntry] items.
#[derive(Debug, Clone)]
pub struct NotebookKernelSelector {
    entries: Vec<NotebookKernelSelectorEntry>,
    name: String,
    capacity: usize,
}

impl NotebookKernelSelector {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: NotebookKernelSelectorEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<NotebookKernelSelectorEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&NotebookKernelSelectorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn kernel_count(&self) -> usize { self.entries.len() }

    pub fn has_selected_kernel(&self) -> bool {
        self.entries.iter().any(|e| e.has_selected_kernel())
    }

    pub fn entries_by_severity(&self, severity: NotebookKernelSelectorSeverity) -> Vec<&NotebookKernelSelectorEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= NotebookKernelSelectorSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&NotebookKernelSelectorEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&NotebookKernelSelectorEntry> {
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
// NotebookCellOutputFormatter - notebook cell output formatter
// ---------------------------------------------------------------------------

/// Configuration for [NotebookCellOutputFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookCellOutputFormatterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub output_count: usize,
}

impl NotebookCellOutputFormatterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, output_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_output_count(mut self, val: usize) -> Self { self.output_count = val; self }
}

impl Default for NotebookCellOutputFormatterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [NotebookCellOutputFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookCellOutputFormatterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl NotebookCellOutputFormatterItem {
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

    pub fn has_output(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for NotebookCellOutputFormatterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [NotebookCellOutputFormatterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct NotebookCellOutputFormatter {
    config: NotebookCellOutputFormatterConfig,
    items: Vec<NotebookCellOutputFormatterItem>,
}

impl NotebookCellOutputFormatter {
    pub fn new(config: NotebookCellOutputFormatterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: NotebookCellOutputFormatterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<NotebookCellOutputFormatterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&NotebookCellOutputFormatterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn output_count(&self) -> usize { self.items.len() }

    pub fn has_output(&self) -> bool {
        self.items.iter().any(|i| i.has_output())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&NotebookCellOutputFormatterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&NotebookCellOutputFormatterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &NotebookCellOutputFormatterConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
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

    #[test]
    fn serialize_and_deserialize_cell() {
        let cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Code,
            language_id: "python".into(),
            content: "x = 1".into(),
            outputs: vec![],
        };
        let json = serialize_cell(&cell);
        let back = deserialize_cell(&json).unwrap();
        assert_eq!(cell, back);
    }

    #[test]
    fn deserialize_cell_invalid_json() {
        let result = deserialize_cell("not json");
        assert!(result.is_err());
    }

    #[test]
    fn cell_metadata_tags_and_custom() {
        let mut meta = CellMetadata::new(0);
        meta.add_tag("important");
        meta.add_tag("important"); // duplicate ignored
        meta.set_custom("author", "alice");
        assert!(meta.has_tag("important"));
        assert!(!meta.has_tag("other"));
        assert_eq!(meta.tags.len(), 1);
        assert_eq!(meta.custom.get("author").unwrap(), "alice");
    }

    #[test]
    fn diff_documents_detects_changes() {
        let old = NotebookDocumentBuilder::new()
            .uri("file:///a.ipynb")
            .add_code_cell("python", "x = 1")
            .add_code_cell("python", "y = 2")
            .build()
            .unwrap();
        let new = NotebookDocumentBuilder::new()
            .uri("file:///a.ipynb")
            .add_code_cell("python", "x = 1")
            .add_code_cell("python", "y = 999")
            .add_markup_cell("# Added")
            .build()
            .unwrap();
        let diffs = diff_documents(&old, &new);
        assert_eq!(diffs.len(), 2);
        assert!(matches!(diffs[0], CellDiff::ContentChanged { index: 1, .. }));
        assert!(matches!(diffs[1], CellDiff::Added { index: 2 }));
    }

    #[test]
    fn execution_tracker_lifecycle() {
        let mut tracker = ExecutionTracker::new();
        assert_eq!(tracker.execution_count(), 0);
        assert!(tracker.last_executed().is_none());
        tracker.record_execution(0);
        tracker.record_execution(1);
        tracker.record_execution(0);
        assert_eq!(tracker.execution_count(), 3);
        assert_eq!(tracker.last_executed(), Some(0));
        assert_eq!(tracker.execution_order(), &[0, 1, 0]);
        tracker.reset();
        assert_eq!(tracker.execution_count(), 0);
    }

    #[test]
    fn serialize_cells_roundtrip() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "a = 1")
            .add_markup_cell("# Title")
            .build()
            .unwrap();
        let json = serialize_cells(&doc);
        let cells: Vec<NotebookCell> = serde_json::from_str(&json).unwrap();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].kind, NotebookCellKind::Code);
        assert_eq!(cells[1].kind, NotebookCellKind::Markup);
    }

    #[test]
    fn cell_output_text() {
        let output = NotebookCellOutput::text(0, "hello world");
        assert!(output.success);
        assert_eq!(output.data, "hello world");
        assert_eq!(output.data_size(), 11);
        assert!(!output.is_error());
    }

    #[test]
    fn cell_output_error() {
        let output = NotebookCellOutput::error(1, "NameError: x is not defined");
        assert!(!output.success);
        assert!(output.is_error());
        assert_eq!(output.mime_type, OutputMimeType::Error);
    }

    #[test]
    fn cell_output_with_order() {
        let output = NotebookCellOutput::text(0, "result").with_order(5);
        assert_eq!(output.execution_order, Some(5));
    }

    #[test]
    fn cell_output_display() {
        let output = NotebookCellOutput::text(0, "hello");
        let s = format!("{}", output);
        assert!(s.contains("Output[0]"));
        assert!(s.contains("ok"));
    }

    #[test]
    fn kernel_picker_best_most_specialized() {
        let kernels = vec![
            NotebookKernel {
                id: "general".into(),
                label: "General".into(),
                supported_languages: vec!["python".into(), "r".into(), "julia".into()],
            },
            NotebookKernel {
                id: "py".into(),
                label: "Python".into(),
                supported_languages: vec!["python".into()],
            },
        ];
        let best = NotebookKernelPicker::pick_best(&kernels, "python").unwrap();
        assert_eq!(best.id, "py");
    }

    #[test]
    fn kernel_picker_no_match() {
        let kernels = vec![NotebookKernel {
            id: "py".into(),
            label: "Python".into(),
            supported_languages: vec!["python".into()],
        }];
        assert!(NotebookKernelPicker::pick_best(&kernels, "rust").is_none());
    }

    #[test]
    fn kernel_picker_ranked() {
        let kernels = vec![
            NotebookKernel {
                id: "rs".into(),
                label: "Rust".into(),
                supported_languages: vec!["rust".into()],
            },
            NotebookKernel {
                id: "py".into(),
                label: "Python".into(),
                supported_languages: vec!["python".into()],
            },
        ];
        let ranked = NotebookKernelPicker::ranked_kernels(&kernels, "python");
        assert_eq!(ranked[0].id, "py");
    }

    #[test]
    fn cell_execution_order_tracking() {
        let outputs = vec![
            NotebookCellOutput::text(0, "a").with_order(1),
            NotebookCellOutput::text(2, "b").with_order(2),
            NotebookCellOutput::error(1, "fail").with_order(3),
        ];
        let order = cell_execution_order(&outputs);
        assert_eq!(order.get(&0), Some(&1));
        assert_eq!(order.get(&2), Some(&2));
        assert_eq!(order.get(&1), Some(&3));
        assert_eq!(count_successful_executions(&outputs), 2);
        assert_eq!(count_failed_executions(&outputs), 1);
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn document_cell_count() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "x = 1")
            .add_markup_cell("# Title")
            .add_code_cell("python", "y = 2")
            .build()
            .unwrap();
        assert_eq!(doc.cell_count(), 3);
    }

    #[test]
    fn document_find_cell_by_content() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "import os")
            .add_code_cell("python", "print('hello')")
            .add_markup_cell("# Notes")
            .build()
            .unwrap();
        let (idx, cell) = doc.find_cell_by_content("hello").unwrap();
        assert_eq!(idx, 1);
        assert!(cell.content.contains("hello"));
        assert!(doc.find_cell_by_content("nonexistent").is_none());
    }

    #[test]
    fn cell_word_count() {
        let cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Code,
            language_id: "python".into(),
            content: "x = 1\ny = 2".into(),
            outputs: vec![],
        };
        assert_eq!(cell.word_count(), 6);
    }

    #[test]
    fn cell_is_code_and_is_markup() {
        let code_cell = NotebookCell {
            index: 0,
            kind: NotebookCellKind::Code,
            language_id: "python".into(),
            content: "x = 1".into(),
            outputs: vec![],
        };
        assert!(code_cell.is_code());
        assert!(!code_cell.is_markup());

        let markup_cell = NotebookCell {
            index: 1,
            kind: NotebookCellKind::Markup,
            language_id: "markdown".into(),
            content: "# Title".into(),
            outputs: vec![],
        };
        assert!(markup_cell.is_markup());
        assert!(!markup_cell.is_code());
    }

    #[test]
    fn bridge_all_kernel_ids() {
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
        let ids = bridge.all_kernel_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"py"));
        assert!(ids.contains(&"rs"));
    }

    #[test]
    fn bridge_open_document_uris() {
        let mut bridge = NotebookBridge::new();
        bridge.open_document("file:///a.ipynb");
        bridge.open_document("file:///b.ipynb");
        let uris = bridge.open_document_uris();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&"file:///a.ipynb"));
        assert!(uris.contains(&"file:///b.ipynb"));
    }

    #[test]
    fn bridge_display() {
        let mut bridge = NotebookBridge::new();
        bridge.register_kernel(NotebookKernel {
            id: "py".into(),
            label: "Python".into(),
            supported_languages: vec![],
        });
        bridge.open_document("file:///nb.ipynb");
        let s = format!("{bridge}");
        assert!(s.contains("NotebookBridge"));
        assert!(s.contains("kernels=1"));
        assert!(s.contains("open_docs=1"));
    }

    #[test]
    fn cell_metadata_tag_count() {
        let mut meta = CellMetadata::new(0);
        assert_eq!(meta.tag_count(), 0);
        meta.add_tag("important");
        meta.add_tag("review");
        assert_eq!(meta.tag_count(), 2);
        // Duplicate ignored
        meta.add_tag("important");
        assert_eq!(meta.tag_count(), 2);
    }

    // -- CellExecutionState & CellExecutionTracker tests --------------------

    #[test]
    fn cell_execution_state_display() {
        assert_eq!(format!("{}", CellExecutionState::Idle), "idle");
        assert_eq!(format!("{}", CellExecutionState::Running), "running");
        assert_eq!(format!("{}", CellExecutionState::Success), "success");
        assert_eq!(format!("{}", CellExecutionState::Error("oops".into())), "error: oops");
    }

    #[test]
    fn cell_execution_tracker_lifecycle() {
        let mut tracker = CellExecutionTracker::new();
        assert_eq!(tracker.state(0), &CellExecutionState::Idle);

        tracker.mark_running(0);
        assert_eq!(tracker.state(0), &CellExecutionState::Running);
        assert_eq!(tracker.running_cells(), vec![0]);

        tracker.mark_success(0);
        assert_eq!(tracker.state(0), &CellExecutionState::Success);
        assert!(tracker.running_cells().is_empty());

        tracker.mark_running(1);
        tracker.mark_error(1, "division by zero");
        assert!(matches!(tracker.state(1), CellExecutionState::Error(_)));

        tracker.reset_all();
        assert_eq!(tracker.state(0), &CellExecutionState::Idle);
        assert_eq!(tracker.state(1), &CellExecutionState::Idle);
    }

    // -- NotebookOutline tests ----------------------------------------------

    #[test]
    fn notebook_outline_extracts_headings() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_markup_cell("# Introduction\nSome text\n## Background")
            .add_code_cell("python", "x = 1")
            .add_markup_cell("## Methods\n### Sub-method")
            .build()
            .unwrap();
        let outline = NotebookOutline::from_document(&doc);
        let entries = outline.entries();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].title, "Introduction");
        assert_eq!(entries[0].level, 1);
        assert_eq!(entries[0].cell_index, 0);
        assert_eq!(entries[1].title, "Background");
        assert_eq!(entries[1].level, 2);
        assert_eq!(entries[2].title, "Methods");
        assert_eq!(entries[2].level, 2);
        assert_eq!(entries[2].cell_index, 2);
        assert_eq!(entries[3].title, "Sub-method");
        assert_eq!(entries[3].level, 3);
    }

    #[test]
    fn notebook_outline_empty_document() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "x = 1")
            .build()
            .unwrap();
        let outline = NotebookOutline::from_document(&doc);
        assert!(outline.entries().is_empty());
        assert_eq!(format!("{outline}"), "Outline(0 entries)");
    }

    // -- CellDependencyAnalyzer tests ---------------------------------------

    #[test]
    fn cell_dependency_analyzer_basic() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_code_cell("python", "x = 1\ny = 2")
            .add_code_cell("python", "z = x + y")
            .add_code_cell("python", "w = 42")
            .build()
            .unwrap();
        let analyzer = CellDependencyAnalyzer::analyze(&doc);

        let info0 = analyzer.cell_info(0).unwrap();
        assert!(info0.definitions.contains(&"x".to_string()));
        assert!(info0.definitions.contains(&"y".to_string()));

        let info1 = analyzer.cell_info(1).unwrap();
        assert!(info1.definitions.contains(&"z".to_string()));
        assert!(info1.references.contains(&"x".to_string()));
        assert!(info1.references.contains(&"y".to_string()));

        let deps = analyzer.dependencies_of(1);
        assert!(deps.contains(&0));

        let order = analyzer.suggested_order();
        let pos0 = order.iter().position(|&i| i == 0).unwrap();
        let pos1 = order.iter().position(|&i| i == 1).unwrap();
        assert!(pos0 < pos1);
    }

    // -- NotebookExporter tests ---------------------------------------------

    #[test]
    fn notebook_exporter_markdown() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .add_markup_cell("# Title\nSome description.")
            .add_code_cell("python", "x = 42\nprint(x)")
            .add_markup_cell("## Results")
            .build()
            .unwrap();
        let md = NotebookExporter::to_markdown(&doc);
        assert!(md.contains("# Title"));
        assert!(md.contains("```python"));
        assert!(md.contains("x = 42"));
        assert!(md.contains("```"));
        assert!(md.contains("## Results"));
    }

    #[test]
    fn notebook_exporter_empty_document() {
        let doc = NotebookDocumentBuilder::new()
            .uri("file:///nb.ipynb")
            .build()
            .unwrap();
        let md = NotebookExporter::to_markdown(&doc);
        assert!(md.trim().is_empty() || md.contains("nb.ipynb"));
    }

    // -- From impls tests ---------------------------------------------------

    #[test]
    fn cell_execution_state_from_bool() {
        let s: CellExecutionState = true.into();
        assert_eq!(s, CellExecutionState::Success);
        let s: CellExecutionState = false.into();
        assert!(matches!(s, CellExecutionState::Error(_)));
    }

    // -- Kernel Matcher tests ------------------------------------------------

    fn make_notebook(languages: &[&str]) -> NotebookDocument {
        let cells: Vec<NotebookCell> = languages
            .iter()
            .enumerate()
            .map(|(i, lang)| NotebookCell {
                index: i as u32,
                kind: NotebookCellKind::Code,
                language_id: lang.to_string(),
                content: String::new(),
                outputs: vec![],
            })
            .collect();
        NotebookDocument {
            uri: "file:///test.ipynb".into(),
            notebook_type: "jupyter-notebook".into(),
            cells,
            is_dirty: false,
        }
    }

    fn make_kernel(id: &str, languages: &[&str]) -> NotebookKernel {
        NotebookKernel {
            id: id.into(),
            label: id.into(),
            supported_languages: languages.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn kernel_matcher_matches_true() {
        let kernel = make_kernel("py", &["python"]);
        let nb = make_notebook(&["python", "markdown"]);
        assert!(NotebookKernelMatcher::matches(&kernel, &nb));
    }

    #[test]
    fn kernel_matcher_matches_false() {
        let kernel = make_kernel("rs", &["rust"]);
        let nb = make_notebook(&["python", "javascript"]);
        assert!(!NotebookKernelMatcher::matches(&kernel, &nb));
    }

    #[test]
    fn kernel_matcher_score() {
        let kernel = make_kernel("multi", &["python", "javascript"]);
        let nb = make_notebook(&["python", "javascript", "python"]);
        assert_eq!(NotebookKernelMatcher::match_score(&kernel, &nb), 3);
    }

    #[test]
    fn kernel_matcher_best_match() {
        let kernels = vec![
            make_kernel("rs", &["rust"]),
            make_kernel("py", &["python"]),
            make_kernel("multi", &["python", "javascript"]),
        ];
        let nb = make_notebook(&["python", "javascript", "python"]);
        assert_eq!(NotebookKernelMatcher::best_match(&kernels, &nb), Some(2));
    }

    #[test]
    fn kernel_matcher_best_match_none() {
        let kernels = vec![make_kernel("rs", &["rust"])];
        let nb = make_notebook(&["python"]);
        assert_eq!(NotebookKernelMatcher::best_match(&kernels, &nb), None);
    }

    // -- Output Renderer tests -----------------------------------------------

    #[test]
    fn output_renderer_preferred() {
        let renderer = NotebookOutputRenderer::new();
        let outputs = vec![
            NotebookCellOutput {
                cell_index: 0,
                mime_type: OutputMimeType::PlainText,
                data: "hello".into(),
                execution_order: None,
                success: true,
            },
            NotebookCellOutput {
                cell_index: 0,
                mime_type: OutputMimeType::Html,
                data: "<b>hello</b>".into(),
                execution_order: None,
                success: true,
            },
        ];
        let pref = renderer.preferred_output(&outputs).unwrap();
        assert_eq!(pref.mime_type, OutputMimeType::Html);
    }

    #[test]
    fn output_renderer_render_text() {
        let output = NotebookCellOutput {
            cell_index: 0,
            mime_type: OutputMimeType::PlainText,
            data: "42".into(),
            execution_order: Some(1),
            success: true,
        };
        assert_eq!(NotebookOutputRenderer::render_text(&output), "42");
    }

    #[test]
    fn output_renderer_render_html() {
        let output = NotebookCellOutput {
            cell_index: 0,
            mime_type: OutputMimeType::Html,
            data: "<p>hi</p>".into(),
            execution_order: None,
            success: true,
        };
        assert!(NotebookOutputRenderer::render_text(&output).starts_with("[HTML]"));
    }

    // -- Cell Execution Queue tests ------------------------------------------

    #[test]
    fn execution_queue_basic_flow() {
        let mut q = NotebookCellExecutionQueue::new();
        q.enqueue(0);
        q.enqueue(1);
        assert_eq!(q.pending_count(), 2);
        assert!(!q.is_running());

        let idx = q.dequeue();
        assert_eq!(idx, Some(0));
        assert!(q.is_running());
        assert_eq!(q.pending_count(), 1);

        // Cannot dequeue while running
        assert_eq!(q.dequeue(), None);

        q.complete();
        assert!(!q.is_running());

        let idx = q.dequeue();
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn execution_queue_cancel_all() {
        let mut q = NotebookCellExecutionQueue::new();
        q.enqueue(0);
        q.enqueue(1);
        let _ = q.dequeue();
        q.cancel_all();
        assert!(!q.is_running());
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn execution_queue_contains() {
        let mut q = NotebookCellExecutionQueue::new();
        q.enqueue(5);
        q.enqueue(10);
        assert!(q.contains(5));
        assert!(q.contains(10));
        assert!(!q.contains(99));

        let _ = q.dequeue(); // 5 is now running
        assert!(q.contains(5));
    }

    // -- Variable Inspector tests --------------------------------------------

    #[test]
    fn variable_inspector_crud() {
        let mut vi = NotebookVariableInspector::new();
        vi.set("x", "42");
        vi.set("y", "hello");
        assert_eq!(vi.count(), 2);
        assert_eq!(vi.get("x"), Some(&"42".to_string()));
        vi.remove("x");
        assert_eq!(vi.get("x"), None);
        assert_eq!(vi.count(), 1);
    }

    #[test]
    fn variable_inspector_list_sorted() {
        let mut vi = NotebookVariableInspector::new();
        vi.set("z", "3");
        vi.set("a", "1");
        vi.set("m", "2");
        let list = vi.list();
        let names: Vec<&str> = list.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, vec!["a", "m", "z"]);
    }

    #[test]
    fn variable_inspector_clear() {
        let mut vi = NotebookVariableInspector::new();
        vi.set("x", "1");
        vi.set("y", "2");
        vi.clear();
        assert_eq!(vi.count(), 0);
        assert!(vi.list().is_empty());
    }

#[test]
    fn notebookkernelselector_severity_ordering() {
        assert!(NotebookKernelSelectorSeverity::Critical > NotebookKernelSelectorSeverity::High);
        assert!(NotebookKernelSelectorSeverity::High > NotebookKernelSelectorSeverity::Medium);
        assert!(NotebookKernelSelectorSeverity::Medium > NotebookKernelSelectorSeverity::Low);
    }

    #[test]
    fn notebookkernelselector_severity_display() {
        assert_eq!(NotebookKernelSelectorSeverity::Low.to_string(), "low");
        assert_eq!(NotebookKernelSelectorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn notebookkernelselector_entry_creation() {
        let e = NotebookKernelSelectorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, NotebookKernelSelectorSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn notebookkernelselector_entry_builder() {
        let e = NotebookKernelSelectorEntry::new("e2", "Entry 2")
            .with_severity(NotebookKernelSelectorSeverity::High)
            .with_detail("some detail")
            .with_kernel_count(42);
        assert_eq!(e.severity, NotebookKernelSelectorSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.kernel_count, 42);
    }

    #[test]
    fn notebookkernelselector_entry_enable_disable() {
        let mut e = NotebookKernelSelectorEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn notebookkernelselector_add_and_count() {
        let mut mgr = NotebookKernelSelector::new("test");
        mgr.add(NotebookKernelSelectorEntry::new("a", "A"));
        mgr.add(NotebookKernelSelectorEntry::new("b", "B").with_severity(NotebookKernelSelectorSeverity::High));
        assert_eq!(mgr.kernel_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn notebookkernelselector_remove() {
        let mut mgr = NotebookKernelSelector::new("test");
        mgr.add(NotebookKernelSelectorEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn notebookkernelselector_capacity() {
        let mut mgr = NotebookKernelSelector::new("test").with_capacity(1);
        assert!(mgr.add(NotebookKernelSelectorEntry::new("a", "A")));
        assert!(!mgr.add(NotebookKernelSelectorEntry::new("b", "B")));
    }

    #[test]
    fn notebookkernelselector_sorted_by_severity() {
        let mut mgr = NotebookKernelSelector::new("test");
        mgr.add(NotebookKernelSelectorEntry::new("lo", "Low"));
        mgr.add(NotebookKernelSelectorEntry::new("hi", "High").with_severity(NotebookKernelSelectorSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, NotebookKernelSelectorSeverity::Critical);
    }

    #[test]
    fn notebookkernelselector_summary() {
        let mgr = NotebookKernelSelector::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn notebookcelloutputformatter_config_defaults() {
        let cfg = NotebookCellOutputFormatterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn notebookcelloutputformatter_item_creation() {
        let item = NotebookCellOutputFormatterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn notebookcelloutputformatter_add_and_get() {
        let mut mgr = NotebookCellOutputFormatter::new(NotebookCellOutputFormatterConfig::new("test"));
        mgr.add(NotebookCellOutputFormatterItem::new("k1", "v1"));
        assert_eq!(mgr.output_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn notebookcelloutputformatter_remove_item() {
        let mut mgr = NotebookCellOutputFormatter::new(NotebookCellOutputFormatterConfig::new("test"));
        mgr.add(NotebookCellOutputFormatterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn notebookcelloutputformatter_sorted_by_priority() {
        let mut mgr = NotebookCellOutputFormatter::new(NotebookCellOutputFormatterConfig::new("test"));
        mgr.add(NotebookCellOutputFormatterItem::new("lo", "low").with_priority(1));
        mgr.add(NotebookCellOutputFormatterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn notebookcelloutputformatter_items_with_tag() {
        let mut mgr = NotebookCellOutputFormatter::new(NotebookCellOutputFormatterConfig::new("test"));
        mgr.add(NotebookCellOutputFormatterItem::new("a", "1").with_tag("x"));
        mgr.add(NotebookCellOutputFormatterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn notebookcelloutputformatter_report() {
        let mgr = NotebookCellOutputFormatter::new(NotebookCellOutputFormatterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }
}
