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



// ---------------------------------------------------------------------------
// vsedit-ext-notebook: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtNotebookXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ExtNotebookXConfig {
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

impl std::fmt::Display for ExtNotebookXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ExtNotebookXRegistry {
    entries: Vec<ExtNotebookXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ExtNotebookXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ExtNotebookXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ExtNotebookXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ExtNotebookXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ExtNotebookXConfig> {
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

    pub fn active_entries(&self) -> Vec<&ExtNotebookXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ExtNotebookXConfig> {
        let mut sorted: Vec<&ExtNotebookXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ExtNotebookXConfig> {
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

    pub fn iter(&self) -> ExtNotebookXIterator<'_> {
        ExtNotebookXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ExtNotebookXIterator<'a> {
    inner: std::slice::Iter<'a, ExtNotebookXConfig>,
}

impl<'a> Iterator for ExtNotebookXIterator<'a> {
    type Item = &'a ExtNotebookXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ExtNotebookXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ExtNotebookXCache {
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
pub struct ExtNotebookXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ExtNotebookXFormatter {
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

    pub fn format_entry(&self, entry: &ExtNotebookXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ExtNotebookXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ExtNotebookXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ExtNotebookXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ExtNotebookXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ExtNotebookXValidator {
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

    pub fn validate(&self, entry: &ExtNotebookXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &ExtNotebookXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ExtNotebookXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 70
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer70 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer70 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_70(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_70<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_70<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_70(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_70(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 64
// ---------------------------------------------------------------------------

/// Generic object pool `Xc64Pool<T>`.
pub struct Xc64Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc64Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc64PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc64Pool<T> {
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
    pub fn stats(&self) -> Xc64PoolStats {
        Xc64PoolStats {
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

impl<T> Default for Xc64Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc64Scheduler`.
pub struct Xc64Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc64Scheduler {
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

impl Default for Xc64Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_64 hash for the given byte slice.
pub fn xc_64_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_64 convention.
pub fn xc_64_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe83 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe83Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe83PipelineError {
    pub stage: Xe83Stage,
    pub message: String,
}

impl std::fmt::Display for Xe83PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe83Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe83Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError>>>,
    stage_names: Vec<Xe83Stage>,
}

impl Xe83Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe83Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe83Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe83Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe83Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe83Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe83CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe83CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe83Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe83CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe83CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe83Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe83CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_83_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe83CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_83_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe83CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_83_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> {
    Ok(data)
}

pub fn xe_83_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_83_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_83_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_83_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe83PipelineError> {
    Err(Xe83PipelineError {
        stage: Xe83Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_81: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg81Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg81Graph {
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

impl Default for Xg81Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_81: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg81Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg81Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg81Heap<T>) {
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

impl<T: Ord> Default for Xg81Heap<T> {
    fn default() -> Self { Self::new() }
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

    #[test]
    fn extNotebook_x_config_new() {
        let c = ExtNotebookXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn extNotebook_x_config_builder() {
        let c = ExtNotebookXConfig::new("k")
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
    fn extNotebook_x_config_display() {
        let c = ExtNotebookXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn extNotebook_x_registry_insert_get() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extNotebook_x_registry_duplicate() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a")).unwrap();
        assert!(reg.insert(ExtNotebookXConfig::new("a")).is_err());
    }

    #[test]
    fn extNotebook_x_registry_remove() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a")).unwrap();
        reg.insert(ExtNotebookXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extNotebook_x_registry_active_entries() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a")).unwrap();
        reg.insert(ExtNotebookXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn extNotebook_x_registry_by_weight() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ExtNotebookXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn extNotebook_x_registry_tags() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ExtNotebookXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn extNotebook_x_registry_total_weight() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ExtNotebookXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn extNotebook_x_registry_iterator() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a")).unwrap();
        reg.insert(ExtNotebookXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn extNotebook_x_cache_put_get() {
        let mut cache = ExtNotebookXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn extNotebook_x_cache_eviction() {
        let mut cache = ExtNotebookXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn extNotebook_x_cache_lru_order() {
        let mut cache = ExtNotebookXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn extNotebook_x_cache_most_least_recent() {
        let mut cache = ExtNotebookXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn extNotebook_x_formatter_entry() {
        let e = ExtNotebookXConfig::new("k").with_value("v");
        let fmt = ExtNotebookXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn extNotebook_x_formatter_summary() {
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ExtNotebookXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn extNotebook_x_validator_valid() {
        let v = ExtNotebookXValidator::new();
        let c = ExtNotebookXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn extNotebook_x_validator_empty_key() {
        let v = ExtNotebookXValidator::new();
        let c = ExtNotebookXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extNotebook_x_validator_require_value() {
        let v = ExtNotebookXValidator::new().require_value(true);
        let c = ExtNotebookXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extNotebook_x_validator_allowed_tags() {
        let v = ExtNotebookXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ExtNotebookXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extNotebook_x_validator_validate_all() {
        let v = ExtNotebookXValidator::new();
        let mut reg = ExtNotebookXRegistry::new();
        reg.insert(ExtNotebookXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_70_push_and_len() {
        let mut rb = super::XbRingBuffer70::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_70_overwrite() {
        let mut rb = super::XbRingBuffer70::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_70_get_out_of_bounds() {
        let rb = super::XbRingBuffer70::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_70_drain_all() {
        let mut rb = super::XbRingBuffer70::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_70_peek_front_back() {
        let mut rb = super::XbRingBuffer70::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_70_clear() {
        let mut rb = super::XbRingBuffer70::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_70_capacity() {
        let rb = super::XbRingBuffer70::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_70_basic() {
        let h = super::xb_fnv1a_70(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_70(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_70_different_inputs() {
        let h1 = super::xb_fnv1a_70(b"abc");
        let h2 = super::xb_fnv1a_70(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_70_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_70(&data);
        let dec = super::xb_rle_decode_70(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_70_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_70(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_70(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_70_values() {
        assert!((super::xb_clamp_70(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_70(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_70(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_70_values() {
        assert!((super::xb_lerp_70(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_70(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_70(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_70_wrap_around_twice() {
        let mut rb = super::XbRingBuffer70::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 64 ----

    #[test]
    fn xc_64_pool_new_empty() {
        let pool: super::Xc64Pool<i32> = super::Xc64Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_64_pool_release_acquire() {
        let mut pool = super::Xc64Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_64_pool_acquire_empty() {
        let mut pool: super::Xc64Pool<i32> = super::Xc64Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_64_pool_full() {
        let mut pool = super::Xc64Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_64_pool_drain() {
        let mut pool = super::Xc64Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_64_pool_stats() {
        let mut pool = super::Xc64Pool::new(8);
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
    fn xc_64_pool_clear() {
        let mut pool = super::Xc64Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_64_pool_shrink() {
        let mut pool = super::Xc64Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_64_pool_default() {
        let pool: super::Xc64Pool<String> = super::Xc64Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_64_pool_extend() {
        let mut pool = super::Xc64Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_64_pool_retain() {
        let mut pool = super::Xc64Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_64_scheduler_round_robin() {
        let mut sched = super::Xc64Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_64_scheduler_empty() {
        let mut sched = super::Xc64Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_64_scheduler_reset() {
        let mut sched = super::Xc64Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_64_scheduler_add_remove() {
        let mut sched = super::Xc64Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_64_scheduler_targets() {
        let sched = super::Xc64Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_64_hash_empty() {
        assert_eq!(super::xc_64_hash(b""), 5381);
    }

    #[test]
    fn xc_64_hash_data() {
        let h = super::xc_64_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_64_hash(b"hello"), h);
    }

    #[test]
    fn xc_64_reverse_str() {
        assert_eq!(super::xc_64_reverse("abc"), "cba");
        assert_eq!(super::xc_64_reverse(""), "");
    }


    #[test]
    fn xe_83_pipeline_empty() {
        let p = super::Xe83Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_83_pipeline_parse_stage() {
        let p = super::Xe83Pipeline::new()
            .add_parse(super::xe_83_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_83_pipeline_transform_double() {
        let p = super::Xe83Pipeline::new()
            .add_transform(super::xe_83_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_83_pipeline_validate_reverse() {
        let p = super::Xe83Pipeline::new()
            .add_validate(super::xe_83_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_83_pipeline_emit_filter() {
        let p = super::Xe83Pipeline::new()
            .add_emit(super::xe_83_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_83_pipeline_multi_stage() {
        let p = super::Xe83Pipeline::new()
            .add_parse(super::xe_83_pipeline_identity)
            .add_transform(super::xe_83_pipeline_double)
            .add_validate(super::xe_83_pipeline_reverse)
            .add_emit(super::xe_83_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_83_pipeline_error_propagation() {
        let p = super::Xe83Pipeline::new()
            .add_parse(super::xe_83_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe83Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_83_pipeline_compose() {
        let p1 = super::Xe83Pipeline::new()
            .add_parse(super::xe_83_pipeline_identity);
        let p2 = super::Xe83Pipeline::new()
            .add_transform(super::xe_83_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_83_pipeline_error_display() {
        let e = super::Xe83PipelineError {
            stage: super::Xe83Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_83_cache_put_get() {
        let mut c = super::Xe83Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_83_cache_miss() {
        let mut c: super::Xe83Cache<&str, i32> = super::Xe83Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_83_cache_ttl_expiry() {
        let mut c = super::Xe83Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_83_cache_evict() {
        let mut c = super::Xe83Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_83_cache_capacity() {
        let mut c = super::Xe83Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_83_cache_stats() {
        let mut c = super::Xe83Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_83_cache_clear() {
        let mut c = super::Xe83Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_81 graph tests ------------------------------------------------

    #[test]
    fn xg_81_graph_empty() {
        let g = super::Xg81Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_81_graph_add_node() {
        let mut g = super::Xg81Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_81_graph_add_edge() {
        let mut g = super::Xg81Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_81_graph_neighbors() {
        let mut g = super::Xg81Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_81_graph_has_path() {
        let mut g = super::Xg81Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_81_graph_self_path() {
        let g = super::Xg81Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_81_graph_topo_sort() {
        let mut g = super::Xg81Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_81_graph_cycle_detect_false() {
        let mut g = super::Xg81Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_81_graph_cycle_detect_true() {
        let mut g = super::Xg81Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_81 heap tests -------------------------------------------------

    #[test]
    fn xg_81_heap_empty() {
        let h: super::Xg81Heap<i32> = super::Xg81Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_81_heap_push_pop() {
        let mut h = super::Xg81Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_81_heap_peek() {
        let mut h = super::Xg81Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_81_heap_drain_sorted() {
        let mut h = super::Xg81Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_81_heap_merge() {
        let mut a = super::Xg81Heap::new();
        let mut b = super::Xg81Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_81_heap_default() {
        let h: super::Xg81Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_81_graph_default() {
        let g: super::Xg81Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }

}
