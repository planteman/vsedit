//! Notebook editor.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Errors that can occur when manipulating a notebook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotebookError {
    CellNotFound(usize),
    InvalidIndex(usize),
    EmptyNotebook,
}

impl fmt::Display for NotebookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotebookError::CellNotFound(idx) => write!(f, "cell not found at index {idx}"),
            NotebookError::InvalidIndex(idx) => write!(f, "invalid cell index {idx}"),
            NotebookError::EmptyNotebook => write!(f, "notebook is empty"),
        }
    }
}

/// The kind of a notebook cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotebookCellKind {
    Code,
    Markup,
}

impl fmt::Display for NotebookCellKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotebookCellKind::Code => write!(f, "Code"),
            NotebookCellKind::Markup => write!(f, "Markup"),
        }
    }
}

/// Output produced by executing a cell.
#[derive(Debug, Clone)]
pub struct NotebookCellOutput {
    pub mime_type: String,
    pub data: String,
}

/// A single cell in a notebook.
#[derive(Debug, Clone)]
pub struct NotebookCell {
    pub source: String,
    pub kind: NotebookCellKind,
    pub language: String,
    pub outputs: Vec<NotebookCellOutput>,
    pub execution_order: Option<u32>,
}

impl fmt::Display for NotebookCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let preview: String = self.source.chars().take(40).collect();
        write!(f, "{}: {}...", self.kind, preview)
    }
}

impl NotebookCell {
    /// Append an output to this cell.
    pub fn add_output(&mut self, output: NotebookCellOutput) {
        self.outputs.push(output);
    }

    /// Returns `true` if this cell has at least one output.
    pub fn has_output(&self) -> bool {
        !self.outputs.is_empty()
    }
}

/// A notebook document containing cells and metadata.
pub struct NotebookDocument {
    pub uri: String,
    pub cells: Vec<NotebookCell>,
    pub metadata: HashMap<String, String>,
    pub dirty: bool,
}

impl NotebookDocument {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            cells: Vec::new(),
            metadata: HashMap::new(),
            dirty: false,
        }
    }

    pub fn add_cell(&mut self, cell: NotebookCell) {
        self.cells.push(cell);
        self.dirty = true;
    }

    pub fn remove_cell(&mut self, index: usize) -> Option<NotebookCell> {
        if index < self.cells.len() {
            self.dirty = true;
            Some(self.cells.remove(index))
        } else {
            None
        }
    }

    pub fn move_cell(&mut self, from: usize, to: usize) {
        if from < self.cells.len() && to < self.cells.len() && from != to {
            let cell = self.cells.remove(from);
            self.cells.insert(to, cell);
            self.dirty = true;
        }
    }

    pub fn get_cell(&self, index: usize) -> Option<&NotebookCell> {
        self.cells.get(index)
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Return only cells with kind `Code`.
    pub fn code_cells(&self) -> Vec<&NotebookCell> {
        self.cells.iter().filter(|c| c.kind == NotebookCellKind::Code).collect()
    }

    /// Return only cells with kind `Markup`.
    pub fn markup_cells(&self) -> Vec<&NotebookCell> {
        self.cells.iter().filter(|c| c.kind == NotebookCellKind::Markup).collect()
    }

    /// Find the first cell whose source contains the given substring.
    pub fn find_cell_by_source(&self, substring: &str) -> Option<(usize, &NotebookCell)> {
        self.cells.iter().enumerate().find(|(_, c)| c.source.contains(substring))
    }

    /// Insert a cell at a specific index, returning an error if the index is
    /// beyond one past the end.
    pub fn insert_cell(&mut self, index: usize, cell: NotebookCell) -> Result<(), NotebookError> {
        if index > self.cells.len() {
            return Err(NotebookError::InvalidIndex(index));
        }
        self.cells.insert(index, cell);
        self.dirty = true;
        Ok(())
    }

    /// Swap two cells by index.
    pub fn swap_cells(&mut self, a: usize, b: usize) -> Result<(), NotebookError> {
        let len = self.cells.len();
        if a >= len {
            return Err(NotebookError::InvalidIndex(a));
        }
        if b >= len {
            return Err(NotebookError::InvalidIndex(b));
        }
        self.cells.swap(a, b);
        self.dirty = true;
        Ok(())
    }

    /// Clear outputs of every cell in the notebook.
    pub fn clear_outputs(&mut self) {
        for cell in &mut self.cells {
            cell.outputs.clear();
        }
    }

    /// Sum of source lines across all cells.
    pub fn total_lines(&self) -> usize {
        self.cells.iter().map(|c| c.source.lines().count()).sum()
    }
}

impl PartialEq for NotebookCellOutput {
    fn eq(&self, other: &Self) -> bool {
        self.mime_type == other.mime_type && self.data == other.data
    }
}

/// Metadata associated with a notebook cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookCellMetadata {
    pub editable: bool,
    pub deletable: bool,
    pub tags: Vec<String>,
}

impl Default for NotebookCellMetadata {
    fn default() -> Self {
        Self {
            editable: true,
            deletable: true,
            tags: Vec::new(),
        }
    }
}

impl NotebookCellMetadata {
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn remove_tag(&mut self, tag: &str) -> bool {
        let before = self.tags.len();
        self.tags.retain(|t| t != tag);
        self.tags.len() < before
    }
}

/// A simple notebook serializer that converts notebooks to/from a text format.
#[derive(Debug, Clone)]
pub struct NotebookSerializer;

impl NotebookSerializer {
    /// Serialize a notebook document to a simple text representation.
    pub fn serialize(doc: &NotebookDocument) -> String {
        let mut output = String::new();
        output.push_str(&format!("# Notebook: {}\n", doc.uri));
        output.push_str(&format!("# Cells: {}\n", doc.cell_count()));
        output.push_str(&format!("# Dirty: {}\n", doc.is_dirty()));
        for (key, value) in &doc.metadata {
            output.push_str(&format!("# Meta: {}={}\n", key, value));
        }
        output.push_str("---\n");
        for (i, cell) in doc.cells.iter().enumerate() {
            output.push_str(&format!("## Cell {} [{}] ({})\n", i, cell.kind, cell.language));
            if let Some(order) = cell.execution_order {
                output.push_str(&format!("## Execution: {}\n", order));
            }
            output.push_str(&cell.source);
            output.push('\n');
            for out in &cell.outputs {
                output.push_str(&format!(">> [{}] {}\n", out.mime_type, out.data));
            }
            output.push_str("---\n");
        }
        output
    }

    /// Calculate the byte size of a serialized notebook.
    pub fn estimated_size(doc: &NotebookDocument) -> usize {
        let mut size = doc.uri.len() + 50; // header overhead
        for cell in &doc.cells {
            size += cell.source.len() + cell.language.len() + 30;
            for out in &cell.outputs {
                size += out.mime_type.len() + out.data.len() + 10;
            }
        }
        for (k, v) in &doc.metadata {
            size += k.len() + v.len() + 10;
        }
        size
    }
}

impl NotebookDocument {
    /// Return cells filtered by language.
    pub fn cells_by_language(&self, language: &str) -> Vec<&NotebookCell> {
        self.cells.iter().filter(|c| c.language == language).collect()
    }

    /// Duplicate a cell at the given index, inserting the copy after it.
    pub fn duplicate_cell(&mut self, index: usize) -> Result<(), NotebookError> {
        let cell = self.cells.get(index)
            .ok_or(NotebookError::CellNotFound(index))?
            .clone();
        self.cells.insert(index + 1, cell);
        self.dirty = true;
        Ok(())
    }

    /// Return the index of the first cell with the given execution order.
    pub fn find_by_execution_order(&self, order: u32) -> Option<usize> {
        self.cells.iter().position(|c| c.execution_order == Some(order))
    }

    /// Split a code cell at the given line number within its source.
    /// Returns an error if the cell is not found or the line number is out of range.
    pub fn split_cell(&mut self, index: usize, at_line: usize) -> Result<(), NotebookError> {
        let cell = self.cells.get(index)
            .ok_or(NotebookError::CellNotFound(index))?;
        let lines: Vec<&str> = cell.source.lines().collect();
        if at_line == 0 || at_line >= lines.len() {
            return Err(NotebookError::InvalidIndex(at_line));
        }
        let first_part: String = lines[..at_line].join("\n");
        let second_part: String = lines[at_line..].join("\n");
        let kind = cell.kind;
        let language = cell.language.clone();
        self.cells[index].source = first_part;
        self.cells[index].outputs.clear();
        let new_cell = NotebookCell {
            source: second_part,
            kind,
            language,
            outputs: Vec::new(),
            execution_order: None,
        };
        self.cells.insert(index + 1, new_cell);
        self.dirty = true;
        Ok(())
    }

    /// Merge two adjacent cells. The source of the second cell is appended to the first.
    pub fn merge_cells(&mut self, first: usize, second: usize) -> Result<(), NotebookError> {
        if first >= self.cells.len() {
            return Err(NotebookError::CellNotFound(first));
        }
        if second >= self.cells.len() {
            return Err(NotebookError::CellNotFound(second));
        }
        if second != first + 1 {
            return Err(NotebookError::InvalidIndex(second));
        }
        let second_source = self.cells[second].source.clone();
        self.cells[first].source.push('\n');
        self.cells[first].source.push_str(&second_source);
        self.cells.remove(second);
        self.dirty = true;
        Ok(())
    }

    /// Return summary statistics about the notebook.
    pub fn summary(&self) -> NotebookSummary {
        let code_count = self.code_cells().len();
        let markup_count = self.markup_cells().len();
        let total_outputs: usize = self.cells.iter().map(|c| c.outputs.len()).sum();
        let total_chars: usize = self.cells.iter().map(|c| c.source.len()).sum();
        NotebookSummary {
            total_cells: self.cells.len(),
            code_cells: code_count,
            markup_cells: markup_count,
            total_outputs,
            total_source_chars: total_chars,
            total_source_lines: self.total_lines(),
        }
    }
}

/// Summary statistics for a notebook document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookSummary {
    pub total_cells: usize,
    pub code_cells: usize,
    pub markup_cells: usize,
    pub total_outputs: usize,
    pub total_source_chars: usize,
    pub total_source_lines: usize,
}

impl fmt::Display for NotebookSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} cells ({} code, {} markup), {} lines, {} outputs",
            self.total_cells, self.code_cells, self.markup_cells,
            self.total_source_lines, self.total_outputs
        )
    }
}

// ---------------------------------------------------------------------------
// Cell execution tracking
// ---------------------------------------------------------------------------

/// Tracks the execution state and timing of notebook cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellExecutionState {
    Idle,
    Running,
    Succeeded { duration_ms: u64 },
    Failed { duration_ms: u64, error: String },
}

impl fmt::Display for CellExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellExecutionState::Idle => write!(f, "Idle"),
            CellExecutionState::Running => write!(f, "Running"),
            CellExecutionState::Succeeded { duration_ms } => {
                write!(f, "Succeeded ({duration_ms}ms)")
            }
            CellExecutionState::Failed { duration_ms, error } => {
                write!(f, "Failed ({duration_ms}ms): {error}")
            }
        }
    }
}

/// Tracks execution history and state for all cells in a notebook.
pub struct CellExecutionTracker {
    states: HashMap<usize, CellExecutionState>,
    execution_count: u32,
}

impl CellExecutionTracker {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            execution_count: 0,
        }
    }

    pub fn mark_running(&mut self, cell_index: usize) -> u32 {
        self.execution_count += 1;
        self.states.insert(cell_index, CellExecutionState::Running);
        self.execution_count
    }

    pub fn mark_succeeded(&mut self, cell_index: usize, duration_ms: u64) {
        self.states.insert(cell_index, CellExecutionState::Succeeded { duration_ms });
    }

    pub fn mark_failed(&mut self, cell_index: usize, duration_ms: u64, error: String) {
        self.states.insert(cell_index, CellExecutionState::Failed { duration_ms, error });
    }

    pub fn get_state(&self, cell_index: usize) -> &CellExecutionState {
        self.states.get(&cell_index).unwrap_or(&CellExecutionState::Idle)
    }

    pub fn running_cells(&self) -> Vec<usize> {
        self.states.iter()
            .filter(|(_, s)| matches!(s, CellExecutionState::Running))
            .map(|(i, _)| *i)
            .collect()
    }

    pub fn total_executions(&self) -> u32 {
        self.execution_count
    }
}

// ---------------------------------------------------------------------------
// Cell output management
// ---------------------------------------------------------------------------

/// Manages output buffers for notebook cells, supporting append and replace.
pub struct CellOutputManager {
    max_outputs_per_cell: usize,
}

impl CellOutputManager {
    pub fn new(max_outputs_per_cell: usize) -> Self {
        Self { max_outputs_per_cell }
    }

    /// Append an output to a cell, respecting the maximum output limit.
    /// Returns `true` if the output was added, `false` if the limit was reached.
    pub fn append_output(&self, cell: &mut NotebookCell, output: NotebookCellOutput) -> bool {
        if cell.outputs.len() >= self.max_outputs_per_cell {
            return false;
        }
        cell.outputs.push(output);
        true
    }

    /// Replace all outputs of a cell with a single output.
    pub fn replace_outputs(&self, cell: &mut NotebookCell, output: NotebookCellOutput) {
        cell.outputs.clear();
        cell.outputs.push(output);
    }

    /// Return the total byte size of all outputs in a cell.
    pub fn output_byte_size(cell: &NotebookCell) -> usize {
        cell.outputs.iter().map(|o| o.mime_type.len() + o.data.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// Notebook outline generation
// ---------------------------------------------------------------------------

/// An entry in a notebook outline (table of contents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookOutlineEntry {
    pub cell_index: usize,
    pub heading_level: u8,
    pub text: String,
}

/// Generate an outline from markup cells that contain markdown headings.
pub fn generate_notebook_outline(doc: &NotebookDocument) -> Vec<NotebookOutlineEntry> {
    let mut entries = Vec::new();
    for (idx, cell) in doc.cells.iter().enumerate() {
        if cell.kind != NotebookCellKind::Markup {
            continue;
        }
        for line in cell.source.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix('#') {
                let mut level: u8 = 1;
                let mut remaining = rest;
                while let Some(r) = remaining.strip_prefix('#') {
                    level += 1;
                    remaining = r;
                    if level >= 6 {
                        break;
                    }
                }
                let text = remaining.trim().to_string();
                if !text.is_empty() {
                    entries.push(NotebookOutlineEntry {
                        cell_index: idx,
                        heading_level: level,
                        text,
                    });
                }
            }
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Cell dependency analysis
// ---------------------------------------------------------------------------

/// Represents a dependency edge: the cell at `cell_index` depends on `depends_on`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellDependency {
    pub cell_index: usize,
    pub depends_on: usize,
    pub symbol: String,
}

/// Analyze simple variable dependencies between code cells.
/// A cell that uses a name defined (assigned with `=`) in an earlier cell
/// is considered to depend on that cell.
pub fn analyze_cell_dependencies(doc: &NotebookDocument) -> Vec<CellDependency> {
    let mut definitions: Vec<(usize, String)> = Vec::new();
    let mut deps = Vec::new();

    for (idx, cell) in doc.cells.iter().enumerate() {
        if cell.kind != NotebookCellKind::Code {
            continue;
        }
        // Check if this cell uses symbols defined in earlier cells
        for (def_idx, symbol) in &definitions {
            if cell.source.contains(symbol.as_str()) {
                deps.push(CellDependency {
                    cell_index: idx,
                    depends_on: *def_idx,
                    symbol: symbol.clone(),
                });
            }
        }
        // Extract simple definitions (lines like `name = ...`)
        for line in cell.source.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed.split('=').next() {
                let name = name.trim();
                if !name.is_empty()
                    && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_')
                {
                    definitions.push((idx, name.to_string()));
                }
            }
        }
    }
    deps
}

/// Represents an action available on the notebook toolbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolbarAction {
    RunCell(usize),
    AddCellAbove(usize),
    AddCellBelow(usize),
    DeleteCell(usize),
    MoveCellUp(usize),
    MoveCellDown(usize),
    ClearOutputs,
    RunAll,
}

/// Toolbar that manages available cell actions based on notebook state.
pub struct NotebookToolbar {
    actions: Vec<ToolbarAction>,
}

impl NotebookToolbar {
    pub fn new() -> Self {
        Self { actions: Vec::new() }
    }

    /// Compute available actions for the cell at `index` in a notebook with `cell_count` cells.
    pub fn compute_actions(&mut self, index: usize, cell_count: usize) {
        self.actions.clear();
        if cell_count == 0 {
            return;
        }
        if index < cell_count {
            self.actions.push(ToolbarAction::RunCell(index));
            self.actions.push(ToolbarAction::AddCellAbove(index));
            self.actions.push(ToolbarAction::AddCellBelow(index));
            self.actions.push(ToolbarAction::DeleteCell(index));
            if index > 0 {
                self.actions.push(ToolbarAction::MoveCellUp(index));
            }
            if index + 1 < cell_count {
                self.actions.push(ToolbarAction::MoveCellDown(index));
            }
        }
        self.actions.push(ToolbarAction::ClearOutputs);
        self.actions.push(ToolbarAction::RunAll);
    }

    pub fn available_actions(&self) -> &[ToolbarAction] {
        &self.actions
    }

    pub fn has_action(&self, action: &ToolbarAction) -> bool {
        self.actions.contains(action)
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }
}

/// Status information displayed next to a cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellStatusLine {
    pub cell_index: usize,
    pub execution_order: Option<u32>,
    pub status_text: String,
    pub is_running: bool,
}

impl CellStatusLine {
    /// Build a status line from the execution tracker for a given cell.
    pub fn from_tracker(cell_index: usize, tracker: &CellExecutionTracker) -> Self {
        let state = tracker.get_state(cell_index);
        let (status_text, is_running) = match state {
            CellExecutionState::Idle => ("Idle".to_string(), false),
            CellExecutionState::Running => ("Running...".to_string(), true),
            CellExecutionState::Succeeded { duration_ms } => {
                (format!("Done ({}ms)", duration_ms), false)
            }
            CellExecutionState::Failed { duration_ms, error } => {
                (format!("Failed ({}ms): {}", duration_ms, error), false)
            }
        };
        Self {
            cell_index,
            execution_order: None,
            status_text,
            is_running,
        }
    }

    pub fn with_execution_order(mut self, order: u32) -> Self {
        self.execution_order = Some(order);
        self
    }
}

impl fmt::Display for CellStatusLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(order) = self.execution_order {
            write!(f, "[{}] {}", order, self.status_text)
        } else {
            write!(f, "[ ] {}", self.status_text)
        }
    }
}

/// Export format for a notebook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Python,
    Markdown,
}

/// Export a notebook document to the specified format.
pub fn notebook_export_format(doc: &NotebookDocument, format: ExportFormat) -> String {
    let mut output = String::new();
    match format {
        ExportFormat::Python => {
            for (i, cell) in doc.cells.iter().enumerate() {
                match cell.kind {
                    NotebookCellKind::Code => {
                        if i > 0 { output.push('\n'); }
                        output.push_str(&cell.source);
                        output.push('\n');
                    }
                    NotebookCellKind::Markup => {
                        if i > 0 { output.push('\n'); }
                        for line in cell.source.lines() {
                            output.push_str("# ");
                            output.push_str(line);
                            output.push('\n');
                        }
                    }
                }
            }
        }
        ExportFormat::Markdown => {
            for (i, cell) in doc.cells.iter().enumerate() {
                if i > 0 {
                    output.push_str("\n\n");
                }
                match cell.kind {
                    NotebookCellKind::Code => {
                        output.push_str(&format!("```{}\n", cell.language));
                        output.push_str(&cell.source);
                        output.push_str("\n```");
                    }
                    NotebookCellKind::Markup => {
                        output.push_str(&cell.source);
                    }
                }
            }
            output.push('\n');
        }
    }
    output
}

// ---------------------------------------------------------------------------
// NotebookCellKind helpers
// ---------------------------------------------------------------------------

impl NotebookCellKind {
    /// Returns all cell kind variants.
    pub fn all() -> &'static [NotebookCellKind] {
        &[NotebookCellKind::Code, NotebookCellKind::Markup]
    }

    /// Returns true if this is a code cell.
    pub fn is_code(&self) -> bool {
        matches!(self, NotebookCellKind::Code)
    }

    /// Returns true if this is a markup/markdown cell.
    pub fn is_markup(&self) -> bool {
        matches!(self, NotebookCellKind::Markup)
    }
}

impl Default for NotebookCellKind {
    fn default() -> Self {
        NotebookCellKind::Code
    }
}

// ---------------------------------------------------------------------------
// NotebookCell helpers
// ---------------------------------------------------------------------------

impl NotebookCell {
    /// Create a new code cell.
    pub fn code(source: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            kind: NotebookCellKind::Code,
            source: source.into(),
            language: language.into(),
            outputs: Vec::new(),
            execution_order: None,
        }
    }

    /// Create a new markup cell.
    pub fn markup(source: impl Into<String>) -> Self {
        Self {
            kind: NotebookCellKind::Markup,
            source: source.into(),
            language: "markdown".to_string(),
            outputs: Vec::new(),
            execution_order: None,
        }
    }

    /// Returns the number of lines in the source.
    pub fn line_count(&self) -> usize {
        self.source.lines().count().max(1)
    }

    /// Returns true if the cell has outputs.
    pub fn has_outputs(&self) -> bool {
        !self.outputs.is_empty()
    }

    /// Returns the character count of the source.
    pub fn char_count(&self) -> usize {
        self.source.len()
    }

    /// Returns true if the source is empty or whitespace-only.
    pub fn is_empty(&self) -> bool {
        self.source.trim().is_empty()
    }
}

// ---------------------------------------------------------------------------
// NotebookDocument helpers
// ---------------------------------------------------------------------------

impl NotebookDocument {
    /// Returns the number of code cells.
    pub fn code_cell_count(&self) -> usize {
        self.cells.iter().filter(|c| c.kind.is_code()).count()
    }

    /// Returns the number of markup cells.
    pub fn markup_cell_count(&self) -> usize {
        self.cells.iter().filter(|c| c.kind.is_markup()).count()
    }

    /// Returns all unique languages used in code cells.
    pub fn languages(&self) -> Vec<String> {
        let mut langs: Vec<String> = self.cells.iter()
            .filter(|c| c.kind.is_code())
            .map(|c| c.language.clone())
            .collect();
        langs.sort();
        langs.dedup();
        langs
    }

    /// Returns the total line count across all cells.
    pub fn total_line_count(&self) -> usize {
        self.cells.iter().map(|c| c.line_count()).sum()
    }

    /// Find cells containing a text substring.
    pub fn search_cells(&self, query: &str) -> Vec<usize> {
        let q = query.to_lowercase();
        self.cells.iter()
            .enumerate()
            .filter(|(_, c)| c.source.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// CellExecutionState helpers
// ---------------------------------------------------------------------------

impl CellExecutionState {
    /// Returns true if the cell has finished execution.
    pub fn is_finished(&self) -> bool {
        matches!(self, CellExecutionState::Succeeded { .. } | CellExecutionState::Failed { .. })
    }

    /// Returns true if the cell is currently running.
    pub fn is_running(&self) -> bool {
        matches!(self, CellExecutionState::Running)
    }

    /// Returns an icon character.
    pub fn icon(&self) -> char {
        match self {
            CellExecutionState::Idle => '○',
            CellExecutionState::Running => '●',
            CellExecutionState::Succeeded { .. } => '✓',
            CellExecutionState::Failed { .. } => '✗',
        }
    }
}

impl Default for CellExecutionState {
    fn default() -> Self {
        CellExecutionState::Idle
    }
}

// ---------------------------------------------------------------------------
// ExportFormat helpers
// ---------------------------------------------------------------------------

impl ExportFormat {
    /// Returns all export format variants.
    pub fn all() -> &'static [ExportFormat] {
        &[ExportFormat::Python, ExportFormat::Markdown]
    }

    /// Returns the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Python => "py",
            ExportFormat::Markdown => "md",
        }
    }

    /// Parse from a string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "python" | "py" => Some(Self::Python),
            "markdown" | "md" => Some(Self::Markdown),
            _ => None,
        }
    }
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportFormat::Python => write!(f, "Python"),
            ExportFormat::Markdown => write!(f, "Markdown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Execution order tracking
// ---------------------------------------------------------------------------

/// Records the order in which cells were executed, supporting re-execution
/// and providing the ability to replay or inspect execution history.
#[derive(Debug, Clone)]
pub struct ExecutionOrderLog {
    entries: Vec<ExecutionLogEntry>,
}

/// A single entry in the execution order log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionLogEntry {
    pub cell_index: usize,
    pub execution_number: u32,
    pub source_snapshot: String,
}

impl ExecutionOrderLog {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Record that a cell was executed with the given source at that moment.
    pub fn record(&mut self, cell_index: usize, source: &str) -> u32 {
        let execution_number = self.entries.len() as u32 + 1;
        self.entries.push(ExecutionLogEntry {
            cell_index,
            execution_number,
            source_snapshot: source.to_string(),
        });
        execution_number
    }

    /// Return entries in execution order.
    pub fn entries(&self) -> &[ExecutionLogEntry] {
        &self.entries
    }

    /// Return only the entries for a specific cell, in execution order.
    pub fn entries_for_cell(&self, cell_index: usize) -> Vec<&ExecutionLogEntry> {
        self.entries.iter().filter(|e| e.cell_index == cell_index).collect()
    }

    /// Return the last execution number for a cell, if any.
    pub fn last_execution(&self, cell_index: usize) -> Option<u32> {
        self.entries.iter().rev()
            .find(|e| e.cell_index == cell_index)
            .map(|e| e.execution_number)
    }

    /// Detect cells that were re-executed with different source (stale runs).
    /// Returns cell indices whose most recent execution source differs from
    /// the current source in the document.
    pub fn stale_cells(&self, doc: &NotebookDocument) -> Vec<usize> {
        let mut seen: HashMap<usize, &str> = HashMap::new();
        for entry in &self.entries {
            seen.insert(entry.cell_index, &entry.source_snapshot);
        }
        let mut stale = Vec::new();
        for (idx, last_source) in &seen {
            if let Some(cell) = doc.cells.get(*idx) {
                if cell.source != **last_source {
                    stale.push(*idx);
                }
            }
        }
        stale.sort();
        stale
    }

    /// Return the total number of executions recorded.
    pub fn total_executions(&self) -> usize {
        self.entries.len()
    }

    /// Return unique cell indices in the order they were first executed.
    pub fn execution_order(&self) -> Vec<usize> {
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        for entry in &self.entries {
            if seen.insert(entry.cell_index) {
                order.push(entry.cell_index);
            }
        }
        order
    }
}

// ---------------------------------------------------------------------------
// Notebook metadata management
// ---------------------------------------------------------------------------

/// Structured kernel and language metadata for a notebook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookKernelInfo {
    pub name: String,
    pub language: String,
    pub version: Option<String>,
}

impl NotebookKernelInfo {
    pub fn new(name: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            language: language.into(),
            version: None,
        }
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Format as a display string suitable for status bars.
    pub fn display_name(&self) -> String {
        match &self.version {
            Some(v) => format!("{} ({} {})", self.name, self.language, v),
            None => format!("{} ({})", self.name, self.language),
        }
    }
}

impl fmt::Display for NotebookKernelInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Manages notebook-level metadata beyond the simple key-value HashMap,
/// including kernel info, trust state, and format version.
#[derive(Debug, Clone)]
pub struct NotebookMetadataManager {
    pub kernel: Option<NotebookKernelInfo>,
    pub trusted: bool,
    pub format_version: (u8, u8),
    custom: HashMap<String, String>,
}

impl NotebookMetadataManager {
    pub fn new() -> Self {
        Self {
            kernel: None,
            trusted: false,
            format_version: (4, 5),
            custom: HashMap::new(),
        }
    }

    pub fn set_kernel(&mut self, kernel: NotebookKernelInfo) {
        self.kernel = Some(kernel);
    }

    pub fn set_trusted(&mut self, trusted: bool) {
        self.trusted = trusted;
    }

    pub fn set_custom(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.custom.insert(key.into(), value.into());
    }

    pub fn get_custom(&self, key: &str) -> Option<&str> {
        self.custom.get(key).map(|s| s.as_str())
    }

    /// Merge metadata from the document's HashMap into this manager,
    /// extracting known keys into structured fields.
    pub fn import_from_document(&mut self, doc: &NotebookDocument) {
        if let Some(kernel_name) = doc.metadata.get("kernel") {
            let language = doc.metadata.get("language")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let mut info = NotebookKernelInfo::new(kernel_name.clone(), language);
            if let Some(ver) = doc.metadata.get("kernel_version") {
                info = info.with_version(ver.clone());
            }
            self.kernel = Some(info);
        }
        for (k, v) in &doc.metadata {
            if !matches!(k.as_str(), "kernel" | "language" | "kernel_version") {
                self.custom.insert(k.clone(), v.clone());
            }
        }
    }

    /// Export structured metadata back into a flat HashMap.
    pub fn export_to_map(&self) -> HashMap<String, String> {
        let mut map = self.custom.clone();
        if let Some(ref kernel) = self.kernel {
            map.insert("kernel".into(), kernel.name.clone());
            map.insert("language".into(), kernel.language.clone());
            if let Some(ref v) = kernel.version {
                map.insert("kernel_version".into(), v.clone());
            }
        }
        map.insert("trusted".into(), self.trusted.to_string());
        map.insert("format_version".into(),
                    format!("{}.{}", self.format_version.0, self.format_version.1));
        map
    }
}

// ---------------------------------------------------------------------------
// Cell output diffing
// ---------------------------------------------------------------------------

/// Describes a difference between two sets of cell outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputDiff {
    /// An output was added at the given index.
    Added { index: usize, output: NotebookCellOutput },
    /// An output was removed from the given index.
    Removed { index: usize, output: NotebookCellOutput },
    /// An output changed at the given index.
    Changed {
        index: usize,
        old: NotebookCellOutput,
        new: NotebookCellOutput,
    },
}

impl Eq for NotebookCellOutput {}

/// Compute the differences between two output lists.
/// Uses a simple positional comparison (not LCS) which is appropriate for
/// cell outputs that are typically short lists.
pub fn diff_cell_outputs(
    old: &[NotebookCellOutput],
    new: &[NotebookCellOutput],
) -> Vec<OutputDiff> {
    let mut diffs = Vec::new();
    let max_len = old.len().max(new.len());
    for i in 0..max_len {
        match (old.get(i), new.get(i)) {
            (Some(o), Some(n)) if o != n => {
                diffs.push(OutputDiff::Changed {
                    index: i,
                    old: o.clone(),
                    new: n.clone(),
                });
            }
            (Some(_), Some(_)) => {} // identical
            (None, Some(n)) => {
                diffs.push(OutputDiff::Added {
                    index: i,
                    output: n.clone(),
                });
            }
            (Some(o), None) => {
                diffs.push(OutputDiff::Removed {
                    index: i,
                    output: o.clone(),
                });
            }
            (None, None) => unreachable!(),
        }
    }
    diffs
}

// ---------------------------------------------------------------------------
// Cell dependency graph with topological sort
// ---------------------------------------------------------------------------

/// A directed acyclic graph of cell dependencies with topological ordering.
pub struct CellDependencyGraph {
    /// Adjacency list: cell_index -> set of cells it depends on.
    edges: HashMap<usize, HashSet<usize>>,
    cell_count: usize,
}

impl CellDependencyGraph {
    /// Build a dependency graph from analyzed dependencies.
    pub fn from_dependencies(deps: &[CellDependency], cell_count: usize) -> Self {
        let mut edges: HashMap<usize, HashSet<usize>> = HashMap::new();
        for dep in deps {
            edges.entry(dep.cell_index).or_default().insert(dep.depends_on);
        }
        Self { edges, cell_count }
    }

    /// Return the direct dependencies of a cell.
    pub fn dependencies_of(&self, cell_index: usize) -> Vec<usize> {
        let mut deps: Vec<usize> = self.edges.get(&cell_index)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();
        deps.sort();
        deps
    }

    /// Return cells that depend on the given cell (reverse lookup).
    pub fn dependents_of(&self, cell_index: usize) -> Vec<usize> {
        let mut result: Vec<usize> = self.edges.iter()
            .filter(|(_, deps)| deps.contains(&cell_index))
            .map(|(idx, _)| *idx)
            .collect();
        result.sort();
        result
    }

    /// Compute a topological ordering of cells. Returns `None` if a cycle is
    /// detected (which shouldn't happen in a well-formed notebook).
    pub fn topological_order(&self) -> Option<Vec<usize>> {
        let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut in_deg: HashMap<usize, usize> = HashMap::new();
        for i in 0..self.cell_count {
            in_deg.insert(i, 0);
        }
        // Execution edge: depends_on -> cell_index
        for (&cell_idx, deps) in &self.edges {
            for &dep in deps {
                adj.entry(dep).or_default().push(cell_idx);
                *in_deg.entry(cell_idx).or_insert(0) += 1;
            }
        }
        let mut queue: VecDeque<usize> = in_deg.iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(idx, _)| *idx)
            .collect();
        // Sort the initial queue for deterministic output
        let mut sorted_start: Vec<usize> = queue.drain(..).collect();
        sorted_start.sort();
        queue.extend(sorted_start);

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node);
            if let Some(neighbors) = adj.get(&node) {
                let mut sorted_neighbors = neighbors.clone();
                sorted_neighbors.sort();
                for &next in &sorted_neighbors {
                    let deg = in_deg.get_mut(&next).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
        if order.len() == self.cell_count {
            Some(order)
        } else {
            None // cycle detected
        }
    }

    /// Return all cells transitively required to run the given cell.
    pub fn transitive_dependencies(&self, cell_index: usize) -> Vec<usize> {
        let mut visited = HashSet::new();
        let mut stack = vec![cell_index];
        while let Some(idx) = stack.pop() {
            if let Some(deps) = self.edges.get(&idx) {
                for &dep in deps {
                    if visited.insert(dep) {
                        stack.push(dep);
                    }
                }
            }
        }
        let mut result: Vec<usize> = visited.into_iter().collect();
        result.sort();
        result
    }
}

// ---------------------------------------------------------------------------
// NotebookCellToolbar – action buttons per cell
// ---------------------------------------------------------------------------

/// An action available in a cell's toolbar.
#[derive(Debug, Clone, PartialEq)]
pub struct CellToolbarAction {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub tooltip: String,
    pub enabled: bool,
}

impl CellToolbarAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: icon.into(),
            tooltip: String::new(),
            enabled: true,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = tooltip.into();
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A toolbar for a notebook cell.
#[derive(Debug, Clone)]
pub struct NotebookCellToolbar {
    pub cell_index: usize,
    pub actions: Vec<CellToolbarAction>,
}

impl NotebookCellToolbar {
    pub fn new(cell_index: usize) -> Self {
        Self { cell_index, actions: Vec::new() }
    }

    /// Build a default toolbar for a cell.
    pub fn default_for(cell_index: usize, kind: NotebookCellKind) -> Self {
        let mut tb = Self::new(cell_index);
        match kind {
            NotebookCellKind::Code => {
                tb.actions.push(CellToolbarAction::new("run", "Run", "▶").with_tooltip("Run Cell"));
                tb.actions.push(CellToolbarAction::new("clear", "Clear", "✕").with_tooltip("Clear Output"));
            }
            NotebookCellKind::Markup => {
                tb.actions.push(CellToolbarAction::new("edit", "Edit", "✎").with_tooltip("Edit Cell"));
            }
        }
        tb.actions.push(CellToolbarAction::new("delete", "Delete", "🗑").with_tooltip("Delete Cell"));
        tb
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Find an action by ID.
    pub fn find_action(&self, id: &str) -> Option<&CellToolbarAction> {
        self.actions.iter().find(|a| a.id == id)
    }
}

// ---------------------------------------------------------------------------
// NotebookOutputCollapse – toggle collapse state
// ---------------------------------------------------------------------------

/// Tracks collapsed state for cell outputs.
#[derive(Debug, Clone)]
pub struct NotebookOutputCollapse {
    collapsed: HashSet<usize>,
}

impl NotebookOutputCollapse {
    pub fn new() -> Self {
        Self { collapsed: HashSet::new() }
    }

    /// Toggle collapse for a cell's output.
    pub fn toggle(&mut self, cell_index: usize) {
        if !self.collapsed.remove(&cell_index) {
            self.collapsed.insert(cell_index);
        }
    }

    pub fn is_collapsed(&self, cell_index: usize) -> bool {
        self.collapsed.contains(&cell_index)
    }

    /// Collapse all cells.
    pub fn collapse_all(&mut self, cell_count: usize) {
        for i in 0..cell_count {
            self.collapsed.insert(i);
        }
    }

    /// Expand all cells.
    pub fn expand_all(&mut self) {
        self.collapsed.clear();
    }

    pub fn collapsed_count(&self) -> usize {
        self.collapsed.len()
    }
}

impl Default for NotebookOutputCollapse {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// NotebookCellStatusBar – status information per cell
// ---------------------------------------------------------------------------

/// Status bar information for a notebook cell.
#[derive(Debug, Clone)]
pub struct NotebookCellStatusBar {
    pub cell_index: usize,
    pub language: String,
    pub execution_time_ms: Option<u64>,
    pub status: CellStatus,
}

/// Execution status of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellStatus {
    Idle,
    Running,
    Success,
    Error,
}

impl fmt::Display for CellStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Success => write!(f, "✓"),
            Self::Error => write!(f, "✗"),
        }
    }
}

impl NotebookCellStatusBar {
    pub fn new(cell_index: usize, language: impl Into<String>) -> Self {
        Self {
            cell_index,
            language: language.into(),
            execution_time_ms: None,
            status: CellStatus::Idle,
        }
    }

    /// Format execution time as a human-readable string.
    pub fn execution_time_label(&self) -> String {
        match self.execution_time_ms {
            Some(ms) if ms < 1000 => format!("{}ms", ms),
            Some(ms) => format!("{:.1}s", ms as f64 / 1000.0),
            None => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// NotebookScrollSync – keeps cells in view during execution
// ---------------------------------------------------------------------------

/// Manages scroll synchronization during cell execution.
#[derive(Debug, Clone)]
pub struct NotebookScrollSync {
    pub enabled: bool,
    pub follow_executing: bool,
    viewport_start: usize,
    viewport_end: usize,
}

impl NotebookScrollSync {
    pub fn new() -> Self {
        Self {
            enabled: true,
            follow_executing: true,
            viewport_start: 0,
            viewport_end: 0,
        }
    }

    /// Update the visible viewport range.
    pub fn set_viewport(&mut self, start: usize, end: usize) {
        self.viewport_start = start;
        self.viewport_end = end;
    }

    /// Check if a cell index is currently visible.
    pub fn is_visible(&self, cell_index: usize) -> bool {
        cell_index >= self.viewport_start && cell_index <= self.viewport_end
    }

    /// Return the cell index to scroll to when execution reaches a cell.
    pub fn scroll_target(&self, executing_cell: usize) -> Option<usize> {
        if !self.enabled || !self.follow_executing {
            return None;
        }
        if self.is_visible(executing_cell) {
            None
        } else {
            Some(executing_cell)
        }
    }
}

impl Default for NotebookScrollSync {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// NotebookCellSearchEngine
// ---------------------------------------------------------------------------

/// A match found by the search engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSearchMatch {
    pub cell_index: usize,
    pub line_number: usize,
    pub column_start: usize,
    pub column_end: usize,
    pub matched_text: String,
}

/// Searches across notebook cells for text patterns.
#[derive(Debug)]
pub struct NotebookCellSearchEngine {
    case_sensitive: bool,
    whole_word: bool,
    matches: Vec<CellSearchMatch>,
}

impl NotebookCellSearchEngine {
    pub fn new(case_sensitive: bool, whole_word: bool) -> Self {
        Self {
            case_sensitive,
            whole_word,
            matches: Vec::new(),
        }
    }

    /// Clear previous results.
    pub fn clear(&mut self) {
        self.matches.clear();
    }

    /// Search a set of cell sources for a pattern.
    pub fn search(&mut self, cells: &[(usize, &str)], pattern: &str) {
        self.matches.clear();
        if pattern.is_empty() {
            return;
        }
        let pat = if self.case_sensitive { pattern.to_string() } else { pattern.to_lowercase() };
        for &(cell_idx, source) in cells {
            for (line_no, line) in source.lines().enumerate() {
                let haystack = if self.case_sensitive { line.to_string() } else { line.to_lowercase() };
                let mut start = 0;
                while let Some(pos) = haystack[start..].find(&pat) {
                    let abs_pos = start + pos;
                    if self.whole_word {
                        let before_ok = abs_pos == 0 || !haystack.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
                        let after_pos = abs_pos + pat.len();
                        let after_ok = after_pos >= haystack.len() || !haystack.as_bytes()[after_pos].is_ascii_alphanumeric();
                        if before_ok && after_ok {
                            self.matches.push(CellSearchMatch {
                                cell_index: cell_idx,
                                line_number: line_no,
                                column_start: abs_pos,
                                column_end: abs_pos + pat.len(),
                                matched_text: line[abs_pos..abs_pos + pat.len()].to_string(),
                            });
                        }
                    } else {
                        self.matches.push(CellSearchMatch {
                            cell_index: cell_idx,
                            line_number: line_no,
                            column_start: abs_pos,
                            column_end: abs_pos + pat.len(),
                            matched_text: line[abs_pos..abs_pos + pat.len()].to_string(),
                        });
                    }
                    start = abs_pos + 1;
                }
            }
        }
    }

    /// Get all matches.
    pub fn matches(&self) -> &[CellSearchMatch] {
        &self.matches
    }

    /// Count of matches.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Matches in a specific cell.
    pub fn matches_in_cell(&self, cell_index: usize) -> Vec<&CellSearchMatch> {
        self.matches.iter().filter(|m| m.cell_index == cell_index).collect()
    }
}

// ---------------------------------------------------------------------------
// NotebookCellDependencyTracker
// ---------------------------------------------------------------------------

/// Tracks dependencies between notebook cells based on variable usage.
#[derive(Debug)]
pub struct NotebookCellDependencyTracker {
    /// cell_index -> set of variable names defined in that cell
    definitions: HashMap<usize, HashSet<String>>,
    /// cell_index -> set of variable names used in that cell
    usages: HashMap<usize, HashSet<String>>,
}

impl NotebookCellDependencyTracker {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            usages: HashMap::new(),
        }
    }

    /// Register that a cell defines a variable.
    pub fn add_definition(&mut self, cell_index: usize, var_name: &str) {
        self.definitions.entry(cell_index).or_default().insert(var_name.to_string());
    }

    /// Register that a cell uses a variable.
    pub fn add_usage(&mut self, cell_index: usize, var_name: &str) {
        self.usages.entry(cell_index).or_default().insert(var_name.to_string());
    }

    /// Find which cells a given cell depends on (cells that define variables this cell uses).
    pub fn dependencies_of(&self, cell_index: usize) -> Vec<usize> {
        let used = match self.usages.get(&cell_index) {
            Some(u) => u,
            None => return Vec::new(),
        };
        let mut deps = Vec::new();
        for (&def_cell, def_vars) in &self.definitions {
            if def_cell == cell_index {
                continue;
            }
            if used.iter().any(|u| def_vars.contains(u)) {
                deps.push(def_cell);
            }
        }
        deps.sort();
        deps
    }

    /// Find which cells depend on a given cell (cells that use variables this cell defines).
    pub fn dependents_of(&self, cell_index: usize) -> Vec<usize> {
        let defined = match self.definitions.get(&cell_index) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let mut deps = Vec::new();
        for (&use_cell, use_vars) in &self.usages {
            if use_cell == cell_index {
                continue;
            }
            if defined.iter().any(|d| use_vars.contains(d)) {
                deps.push(use_cell);
            }
        }
        deps.sort();
        deps
    }

    /// Build a topological execution order for all registered cells.
    /// Returns `None` if there is a cycle.
    pub fn execution_order(&self) -> Option<Vec<usize>> {
        let all_cells: HashSet<usize> = self.definitions.keys().chain(self.usages.keys()).copied().collect();
        let mut in_degree: HashMap<usize, usize> = all_cells.iter().map(|&c| (c, 0)).collect();
        let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();

        for &cell in &all_cells {
            for dep in self.dependencies_of(cell) {
                adj.entry(dep).or_default().push(cell);
                *in_degree.entry(cell).or_default() += 1;
            }
        }

        let queue: VecDeque<usize> = in_degree.iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(c, _)| *c)
            .collect();
        let mut sorted: Vec<usize> = queue.iter().copied().collect();
        sorted.sort();
        let mut result_queue: VecDeque<usize> = sorted.into_iter().collect();
        let mut result = Vec::new();

        while let Some(cell) = result_queue.pop_front() {
            result.push(cell);
            if let Some(neighbors) = adj.get(&cell) {
                for &n in neighbors {
                    let deg = in_degree.get_mut(&n).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        result_queue.push_back(n);
                    }
                }
            }
        }

        if result.len() == all_cells.len() { Some(result) } else { None }
    }

    /// Total number of tracked cells.
    pub fn cell_count(&self) -> usize {
        let all: HashSet<usize> = self.definitions.keys().chain(self.usages.keys()).copied().collect();
        all.len()
    }
}



// ---------------------------------------------------------------------------
// NotebookCellToolbarActions
// ---------------------------------------------------------------------------

/// Represents a registered toolbar action for notebook cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookToolbarActionEntry {
    /// Unique action identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Tooltip text.
    pub tooltip: String,
    /// Icon name (e.g. "play", "delete", "move-up").
    pub icon: String,
    /// Whether the action is currently enabled.
    pub enabled: bool,
    /// Optional keyboard shortcut representation.
    pub shortcut: Option<String>,
}

impl fmt::Display for NotebookToolbarActionEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.enabled { "enabled" } else { "disabled" };
        write!(f, "[{}] {} ({})", self.id, self.label, state)
    }
}

/// Manages registration and execution of cell toolbar actions.
#[derive(Debug, Clone)]
pub struct NotebookCellToolbarActions {
    actions: Vec<NotebookToolbarActionEntry>,
    /// Log of executed action IDs for undo/replay.
    execution_log: VecDeque<String>,
    max_log_size: usize,
}

impl NotebookCellToolbarActions {
    /// Create a new empty toolbar actions manager.
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            execution_log: VecDeque::new(),
            max_log_size: 100,
        }
    }

    /// Register a new toolbar action.
    pub fn register(&mut self, entry: NotebookToolbarActionEntry) -> Result<(), NotebookError> {
        if self.actions.iter().any(|a| a.id == entry.id) {
            return Err(NotebookError::InvalidIndex(0));
        }
        self.actions.push(entry);
        Ok(())
    }

    /// Unregister an action by ID.
    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.actions.len();
        self.actions.retain(|a| a.id != id);
        self.actions.len() < before
    }

    /// Find an action by ID.
    pub fn get_action(&self, id: &str) -> Option<&NotebookToolbarActionEntry> {
        self.actions.iter().find(|a| a.id == id)
    }

    /// Execute an action by ID. Returns the action label if found and enabled.
    pub fn execute(&mut self, id: &str) -> Result<String, NotebookError> {
        let action = self.actions.iter().find(|a| a.id == id);
        match action {
            None => Err(NotebookError::CellNotFound(0)),
            Some(a) if !a.enabled => Err(NotebookError::InvalidIndex(0)),
            Some(a) => {
                let label = a.label.clone();
                if self.execution_log.len() >= self.max_log_size {
                    self.execution_log.pop_front();
                }
                self.execution_log.push_back(id.to_string());
                Ok(label)
            }
        }
    }

    /// Return the number of registered actions.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Return the number of enabled actions.
    pub fn enabled_count(&self) -> usize {
        self.actions.iter().filter(|a| a.enabled).count()
    }

    /// Enable or disable an action by ID.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(a) = self.actions.iter_mut().find(|a| a.id == id) {
            a.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Return the execution log as a slice.
    pub fn execution_log(&self) -> Vec<&str> {
        self.execution_log.iter().map(|s| s.as_str()).collect()
    }

    /// Clear the execution log.
    pub fn clear_log(&mut self) {
        self.execution_log.clear();
    }

    /// Return all action IDs.
    pub fn action_ids(&self) -> Vec<&str> {
        self.actions.iter().map(|a| a.id.as_str()).collect()
    }

    /// Return all actions that match a label substring.
    pub fn search_actions(&self, query: &str) -> Vec<&NotebookToolbarActionEntry> {
        let q = query.to_lowercase();
        self.actions.iter().filter(|a| a.label.to_lowercase().contains(&q)).collect()
    }
}

impl fmt::Display for NotebookCellToolbarActions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ToolbarActions({} registered, {} enabled)",
            self.action_count(),
            self.enabled_count()
        )
    }
}

// ---------------------------------------------------------------------------
// NotebookStatusIndicator
// ---------------------------------------------------------------------------

/// Represents the overall execution status of a notebook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotebookExecutionStatus {
    Idle,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl fmt::Display for NotebookExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Idle => "Idle",
            Self::Running => "Running",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        };
        write!(f, "{label}")
    }
}

/// Tracks and displays notebook execution status across cells.
#[derive(Debug, Clone)]
pub struct NotebookStatusIndicator {
    /// Current overall status.
    status: NotebookExecutionStatus,
    /// Number of cells that have succeeded.
    cells_succeeded: usize,
    /// Number of cells that have failed.
    cells_failed: usize,
    /// Number of cells currently running.
    cells_running: usize,
    /// Total cells in the notebook.
    total_cells: usize,
    /// Status history log.
    history: Vec<(NotebookExecutionStatus, u64)>,
    /// Elapsed time in ms for the current/last run.
    elapsed_ms: u64,
}

impl NotebookStatusIndicator {
    /// Create a new indicator for a notebook.
    pub fn new(total_cells: usize) -> Self {
        Self {
            status: NotebookExecutionStatus::Idle,
            cells_succeeded: 0,
            cells_failed: 0,
            cells_running: 0,
            total_cells,
            history: Vec::new(),
            elapsed_ms: 0,
        }
    }

    /// Mark the notebook as running.
    pub fn start(&mut self, timestamp_ms: u64) {
        self.status = NotebookExecutionStatus::Running;
        self.cells_succeeded = 0;
        self.cells_failed = 0;
        self.cells_running = 0;
        self.elapsed_ms = 0;
        self.history.push((NotebookExecutionStatus::Running, timestamp_ms));
    }

    /// Record a cell success.
    pub fn cell_succeeded(&mut self) {
        self.cells_succeeded += 1;
        if self.cells_running > 0 {
            self.cells_running -= 1;
        }
        self.check_completion();
    }

    /// Record a cell failure.
    pub fn cell_failed(&mut self) {
        self.cells_failed += 1;
        if self.cells_running > 0 {
            self.cells_running -= 1;
        }
        self.check_completion();
    }

    /// Record a cell starting execution.
    pub fn cell_started(&mut self) {
        self.cells_running += 1;
    }

    /// Cancel the execution.
    pub fn cancel(&mut self, timestamp_ms: u64) {
        self.status = NotebookExecutionStatus::Cancelled;
        self.cells_running = 0;
        self.history.push((NotebookExecutionStatus::Cancelled, timestamp_ms));
    }

    /// Set total elapsed time.
    pub fn set_elapsed(&mut self, ms: u64) {
        self.elapsed_ms = ms;
    }

    fn check_completion(&mut self) {
        let completed = self.cells_succeeded + self.cells_failed;
        if completed >= self.total_cells && self.cells_running == 0 {
            if self.cells_failed > 0 {
                self.status = NotebookExecutionStatus::Failed;
            } else {
                self.status = NotebookExecutionStatus::Succeeded;
            }
        }
    }

    /// Current status.
    pub fn status(&self) -> NotebookExecutionStatus {
        self.status
    }

    /// Progress as a fraction 0.0 .. 1.0.
    pub fn progress(&self) -> f64 {
        if self.total_cells == 0 {
            return 1.0;
        }
        let done = self.cells_succeeded + self.cells_failed;
        done as f64 / self.total_cells as f64
    }

    /// Number of cells completed.
    pub fn completed_cells(&self) -> usize {
        self.cells_succeeded + self.cells_failed
    }

    /// Summary string for the status bar.
    pub fn summary(&self) -> String {
        format!(
            "{}: {}/{} cells ({} ok, {} fail, {} running) [{}ms]",
            self.status,
            self.completed_cells(),
            self.total_cells,
            self.cells_succeeded,
            self.cells_failed,
            self.cells_running,
            self.elapsed_ms
        )
    }

    /// History length.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Reset to idle.
    pub fn reset(&mut self) {
        self.status = NotebookExecutionStatus::Idle;
        self.cells_succeeded = 0;
        self.cells_failed = 0;
        self.cells_running = 0;
        self.elapsed_ms = 0;
    }
}

impl fmt::Display for NotebookStatusIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}




// ---------------------------------------------------------------------------
// notebook_view – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for notebook view rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YNotebookViewCellKind {
    Code,
    Markdown,
    Raw,
    Output,
}

impl YNotebookViewCellKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Code => 0,
            Self::Markdown => 1,
            Self::Raw => 2,
            Self::Output => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Code => "Code",
            Self::Markdown => "Markdown",
            Self::Raw => "Raw",
            Self::Output => "Output",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YNotebookViewCellKind] {
        &[
            YNotebookViewCellKind::Code,
            YNotebookViewCellKind::Markdown,
            YNotebookViewCellKind::Raw,
            YNotebookViewCellKind::Output,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YNotebookViewCellKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks cell state data.
#[derive(Debug, Clone)]
pub struct YNotebookViewNotebookCellState {
    pub cell_id: String,
    pub executing: bool,
    pub output_count: usize,
}

impl YNotebookViewNotebookCellState {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            cell_id: String::new(),
            executing: false,
            output_count: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YNotebookViewNotebookCellState({}: {:?})", "cell_id", self.cell_id)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_notebook_view_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_notebook_view_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_notebook_view_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_notebook_view_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_notebook_view_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_notebook_view_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_notebook_view_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_notebook_view_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// notebook_view – Extended notebook kernel state helpers
// ---------------------------------------------------------------------------

/// Priority levels for notebook kernel state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZNotebookViewPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZNotebookViewPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZNotebookViewPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZNotebookViewPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks notebook kernel state data.
#[derive(Debug, Clone)]
pub struct ZNotebookViewNotebookKernelState {
    pub running_cells: Vec<String>,
    pub kernel_id: String,
    pub busy: bool,
}

impl ZNotebookViewNotebookKernelState {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            running_cells: Vec::new(),
            kernel_id: String::new(),
            busy: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.running_cells.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.running_cells.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.running_cells.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZNotebookViewNotebookKernelState[kernel_id={:?}, busy={:?}]", self.kernel_id, self.busy)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.busy = !c.busy;
        c
    }
}

/// Compute a simple rolling hash for notebook kernel state.
pub fn z_notebook_view_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_notebook_view_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_notebook_view_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_notebook_view_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_notebook_view_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_notebook_view_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_notebook_view_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 128
// ---------------------------------------------------------------------------

/// Generic object pool `Xc128Pool<T>`.
pub struct Xc128Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc128Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc128PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc128Pool<T> {
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
    pub fn stats(&self) -> Xc128PoolStats {
        Xc128PoolStats {
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

impl<T> Default for Xc128Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc128Scheduler`.
pub struct Xc128Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc128Scheduler {
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

impl Default for Xc128Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_128 hash for the given byte slice.
pub fn xc_128_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_128 convention.
pub fn xc_128_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_26 deepening: state machine + event bus ---

/// States for the Xd26 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd26State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd26State {
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
pub struct Xd26Transition {
    pub from: Xd26State,
    pub to: Xd26State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd26StateMachine {
    current: Xd26State,
    history: Vec<Xd26Transition>,
    step_counter: usize,
}

impl Xd26StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd26State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd26State {
        self.current
    }

    pub fn history(&self) -> &[Xd26Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd26State) -> Result<Xd26State, String> {
        let allowed = match (self.current, target) {
            (Xd26State::Idle, Xd26State::Running) => true,
            (Xd26State::Running, Xd26State::Paused) => true,
            (Xd26State::Running, Xd26State::Done) => true,
            (Xd26State::Paused, Xd26State::Running) => true,
            (Xd26State::Paused, Xd26State::Done) => true,
            (Xd26State::Done, Xd26State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_26: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd26Transition {
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
            "Xd26SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd26State> {
        let prefix = "Xd26SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd26State::Idle),
            "Running" => Some(Xd26State::Running),
            "Paused" => Some(Xd26State::Paused),
            "Done" => Some(Xd26State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd26State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd26 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd26Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd26Event {
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

type Xd26HandlerFn = Box<dyn Fn(&Xd26Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd26EventBus {
    handlers: Vec<(usize, Option<String>, Xd26HandlerFn)>,
    next_id: usize,
    published: Vec<Xd26Event>,
}

impl Xd26EventBus {
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
        F: Fn(&Xd26Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd26Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd26Event) {
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

    pub fn published_events(&self) -> &[Xd26Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #24
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf24Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf24TrieNode {
    children: std::collections::HashMap<char, Xf24TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf24Trie {
    root: Xf24TrieNode,
    count: usize,
}

impl Xf24Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf24TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf24TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf24TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf24BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf24BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 127).
pub struct Xh127SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh127SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 169 as u64,
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

/// A compact bit set supporting boolean operations (variant 127).
pub struct Xh127BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh127BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 127).
pub struct Xi127Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi127Deque<T> {
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
pub struct Xi127Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi127Interval {
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

/// A simple interval tree (variant 127).
pub struct Xi127IntervalTree {
    xi_intervals: Vec<Xi127Interval>,
}

impl Xi127IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi127Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi127Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi127Interval) -> Vec<&Xi127Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi127Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi127Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi127Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi127Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi127Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi127Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 127) ---

/// Disjoint set / union-find for crate 127.
pub struct Xj127UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj127UnionFind {
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

const XJ127_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 127.
pub struct Xj127BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj127BTreeNode<K, V>>>,
    len: usize,
}

struct Xj127BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj127BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj127BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ127_BTREE_ORDER - 1
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
        let mid = XJ127_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj127BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj127BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj127BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj127BTreeNode::xj_new_leaf();
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


// --- xk_127 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk127SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk127SegmentTree {
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
pub struct Xk127DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk127DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_127).
#[derive(Debug, Clone)]
pub struct Xl127Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl127Rope {
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

/// Suffix array for efficient string searching (xl_127).
#[derive(Debug, Clone)]
pub struct Xl127SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl127SuffixArray {
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
pub struct Xm127MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm127MatrixSparse {
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
pub struct Xm127Tokenizer {
    text: String,
}

impl Xm127Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 127.
pub struct Xn127Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn127Fenwick {
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

// ----- AVL tree map — crate 127 -----

#[derive(Debug, Clone)]
struct Xn127AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn127AvlNode<K, V>>>,
    right: Option<Box<Xn127AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 127.
#[derive(Debug, Clone)]
pub struct Xn127AVL<K, V> {
    root: Option<Box<Xn127AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn127AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn127AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn127AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn127AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn127AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn127AvlNode<K, V>>) -> Box<Xn127AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn127AvlNode<K, V>>) -> Box<Xn127AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn127AvlNode<K, V>>) -> Box<Xn127AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn127AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn127AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn127AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn127AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn127AvlNode<K, V>>) -> &Xn127AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn127AvlNode<K, V>>) -> (Box<Xn127AvlNode<K, V>>, Option<Box<Xn127AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn127AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn127AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn127AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn127AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn127AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn127AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn127AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo127RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo127Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo127RBNode<K, V> {
    key: K,
    value: V,
    color: Xo127Color,
    left: Option<Box<Xo127RBNode<K, V>>>,
    right: Option<Box<Xo127RBNode<K, V>>>,
}

/// A red-black tree map for crate 127.
#[derive(Debug, Clone)]
pub struct Xo127RedBlack<K, V> {
    root: Option<Box<Xo127RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo127RedBlack<K, V> {
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
            r.color = Xo127Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo127RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo127RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo127RBNode {
                    key, value, color: Xo127Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo127RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo127Color::Red)
    }

    fn xo_balance(mut h: Box<Xo127RBNode<K, V>>) -> Box<Xo127RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo127Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo127RBNode<K, V>>) -> Box<Xo127RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo127Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo127RBNode<K, V>>) -> Box<Xo127RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo127Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo127RBNode<K, V>>) {
        h.color = Xo127Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo127Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo127Color::Black; }
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
            r.color = Xo127Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo127RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo127RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo127RBNode<K, V>) -> (K, V, Option<Box<Xo127RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo127RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo127Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo127RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo127ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 127.
#[derive(Debug, Clone)]
pub struct Xo127ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo127ConsistentHash {
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
            let vkey = format!("{}#xo127#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo127#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 127).
#[derive(Debug)]
pub struct Xp127SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp127Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp127Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp127Node<K, V>>>,
    xp_right: Option<Box<Xp127Node<K, V>>>,
}

impl<K: Ord, V> Xp127Node<K, V> {
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

impl<K: Ord, V> Default for Xp127SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp127SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp127Node<K, V>>>, key: &K) -> Option<Box<Xp127Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp127Node<K, V>>) -> Box<Xp127Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp127Node<K, V>>) -> Box<Xp127Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp127Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp127Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp127Node::xp_new(key, val));
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


// --------------- Xq127Treap ---------------

use std::cmp::Ordering as Xq127Ord;

struct Xq127TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq127TreapNode<K, V>>>,
    right: Option<Box<Xq127TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq127Treap<K, V> {
    root: Option<Box<Xq127TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq127TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_127_size<K, V>(node: &Option<Box<Xq127TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_127_update_size<K, V>(node: &mut Xq127TreapNode<K, V>) {
    node.size = 1 + xq_127_size(&node.left) + xq_127_size(&node.right);
}

fn xq_127_rotate_right<K, V>(mut node: Box<Xq127TreapNode<K, V>>) -> Box<Xq127TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_127_update_size(&mut node);
    left.right = Some(node);
    xq_127_update_size(&mut left);
    left
}

fn xq_127_rotate_left<K, V>(mut node: Box<Xq127TreapNode<K, V>>) -> Box<Xq127TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_127_update_size(&mut node);
    right.left = Some(node);
    xq_127_update_size(&mut right);
    right
}

fn xq_127_insert_node<K: Ord, V>(
    node: Option<Box<Xq127TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq127TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq127TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq127Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq127Ord::Less => {
                let (new_left, old) = xq_127_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_127_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_127_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq127Ord::Greater => {
                let (new_right, old) = xq_127_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_127_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_127_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_127_remove_node<K: Ord, V>(
    node: Option<Box<Xq127TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq127TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq127Ord::Less => {
                let (new_left, old) = xq_127_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_127_update_size(&mut n);
                (Some(n), old)
            }
            Xq127Ord::Greater => {
                let (new_right, old) = xq_127_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_127_update_size(&mut n);
                (Some(n), old)
            }
            Xq127Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_127_rotate_right(n);
                    let (new_right, old) = xq_127_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_127_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_127_rotate_left(n);
                    let (new_left, old) = xq_127_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_127_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_127_find_min<K, V>(node: &Option<Box<Xq127TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_127_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_127_find_max<K, V>(node: &Option<Box<Xq127TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_127_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_127_rank<K: Ord, V>(node: &Option<Box<Xq127TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq127Ord::Less => xq_127_rank(&n.left, key),
            Xq127Ord::Equal => xq_127_size(&n.left),
            Xq127Ord::Greater => 1 + xq_127_size(&n.left) + xq_127_rank(&n.right, key),
        },
    }
}

fn xq_127_kth<K, V>(node: &Option<Box<Xq127TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_127_size(&n.left);
        if k < left_size {
            xq_127_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_127_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_127_in_order<K: Clone, V>(node: &Option<Box<Xq127TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_127_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_127_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq127Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 127 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_127_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq127Ord::Equal => return Some(&n.value),
                Xq127Ord::Less => cur = &n.left,
                Xq127Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_127_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_127_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_127_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_127_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_127_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_127_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_127_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq127VEBTree ---------------

pub struct Xq127VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq127VEBTree>>,
    clusters: Vec<Option<Box<Xq127VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq127VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq127VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq127VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr127KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr127KDPoint {
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
pub struct Xr127BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr127KDNode {
    xr_point: Xr127KDPoint,
    xr_left: Option<Box<Xr127KDNode>>,
    xr_right: Option<Box<Xr127KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr127KDTree {
    xr_root: Option<Box<Xr127KDNode>>,
    xr_size: usize,
}

impl Xr127KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr127KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr127KDNode>>,
        point: Xr127KDPoint,
        depth: usize,
    ) -> Box<Xr127KDNode> {
        match node {
            None => Box::new(Xr127KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr127KDPoint) -> Option<Xr127KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr127KDNode>,
        query: &Xr127KDPoint,
        depth: usize,
        best: &mut Xr127KDPoint,
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
    ) -> Vec<Xr127KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr127KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr127KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr127KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr127KDNode>>, pts: &mut Vec<Xr127KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr127KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr127BoundingBox> {
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
        Some(Xr127BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs127PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs127PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs127PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs127PersistentArray {
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
pub struct Xs127ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs127ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs127ConcurrentQueue {
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
pub struct Xs127RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs127RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs127RangeMap {
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
pub struct Xs127CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs127CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs127CircularBuffer {
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

/// Auxiliary statistics tracker for xs_127 data structures.
#[derive(Debug, Clone)]
pub struct Xs127StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs127StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs127StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}


// --- xy_ Cuckoo Hash Map ---

/// Cuckoo hash map with two hash functions and O(1) amortized lookup.
#[derive(Debug, Clone)]
pub struct XyCuckooMap<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xy_table1: Vec<Option<(K, V)>>,
    xy_table2: Vec<Option<(K, V)>>,
    xy_capacity: usize,
    xy_size: usize,
    xy_seed1: u64,
    xy_seed2: u64,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XyCuckooMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CuckooMap(size={}, cap={})", self.xy_size, self.xy_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> Default for XyCuckooMap<K, V> {
    fn default() -> Self { Self::xy_new(16) }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XyCuckooMap<K, V> {
    /// Create a new cuckoo hash map with given capacity.
    pub fn xy_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xy_table1: (0..cap).map(|_| None).collect(),
            xy_table2: (0..cap).map(|_| None).collect(),
            xy_capacity: cap,
            xy_size: 0,
            xy_seed1: 0x517cc1b727220a95,
            xy_seed2: 0x6c62272e07bb0142,
        }
    }

    fn xy_hash1(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed1.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    fn xy_hash2(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed2.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    /// Number of elements.
    pub fn xy_len(&self) -> usize { self.xy_size }

    /// Is empty.
    pub fn xy_is_empty(&self) -> bool { self.xy_size == 0 }

    /// Insert a key-value pair.
    pub fn xy_insert(&mut self, key: K, value: V) -> bool {
        if self.xy_get(&key).is_some() {
            let h1 = self.xy_hash1(&key);
            if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == key) {
                self.xy_table1[h1] = Some((key, value));
            } else {
                let h2 = self.xy_hash2(&key);
                self.xy_table2[h2] = Some((key, value));
            }
            return true;
        }
        let mut k = key;
        let mut v = value;
        for _ in 0..self.xy_capacity {
            let h1 = self.xy_hash1(&k);
            if self.xy_table1[h1].is_none() {
                self.xy_table1[h1] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old = self.xy_table1[h1].take().unwrap();
            self.xy_table1[h1] = Some((k, v));
            k = old.0;
            v = old.1;
            let h2 = self.xy_hash2(&k);
            if self.xy_table2[h2].is_none() {
                self.xy_table2[h2] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old2 = self.xy_table2[h2].take().unwrap();
            self.xy_table2[h2] = Some((k, v));
            k = old2.0;
            v = old2.1;
        }
        // Rehash needed — just put in table1 with linear probing fallback
        for i in 0..self.xy_capacity {
            if self.xy_table1[i].is_none() {
                self.xy_table1[i] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
        }
        false
    }

    /// Look up a key.
    pub fn xy_get(&self, key: &K) -> Option<&V> {
        let h1 = self.xy_hash1(key);
        if let Some((k, v)) = &self.xy_table1[h1] {
            if *k == *key { return Some(v); }
        }
        let h2 = self.xy_hash2(key);
        if let Some((k, v)) = &self.xy_table2[h2] {
            if *k == *key { return Some(v); }
        }
        None
    }

    /// Check if key exists.
    pub fn xy_contains(&self, key: &K) -> bool { self.xy_get(key).is_some() }

    /// Remove a key.
    pub fn xy_remove(&mut self, key: &K) -> Option<V> {
        let h1 = self.xy_hash1(key);
        if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table1[h1].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        let h2 = self.xy_hash2(key);
        if self.xy_table2[h2].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table2[h2].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        None
    }

    /// Clear the map.
    pub fn xy_clear(&mut self) {
        for slot in &mut self.xy_table1 { *slot = None; }
        for slot in &mut self.xy_table2 { *slot = None; }
        self.xy_size = 0;
    }

    /// Collect all keys.
    pub fn xy_keys(&self) -> Vec<K> {
        let mut keys = Vec::new();
        for slot in &self.xy_table1 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        for slot in &self.xy_table2 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        keys
    }
}

// --- xy_ Count-Min Sketch ---

/// Count-min sketch for approximate frequency counting with bounded error.
#[derive(Debug, Clone)]
pub struct XyCountMinSketch {
    xy_table: Vec<Vec<u64>>,
    xy_width: usize,
    xy_depth: usize,
    xy_seeds: Vec<u64>,
}

impl std::fmt::Display for XyCountMinSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CMS(w={}, d={})", self.xy_width, self.xy_depth)
    }
}

impl Default for XyCountMinSketch {
    fn default() -> Self { Self::xy_new(1000, 5) }
}

impl XyCountMinSketch {
    /// Create a new count-min sketch with given width and depth.
    pub fn xy_new(width: usize, depth: usize) -> Self {
        let seeds: Vec<u64> = (0..depth).map(|i| 0x9e3779b97f4a7c15u64.wrapping_add((i as u64).wrapping_mul(0x517cc1b727220a95))).collect();
        Self {
            xy_table: vec![vec![0u64; width]; depth],
            xy_width: width,
            xy_depth: depth,
            xy_seeds: seeds,
        }
    }

    fn xy_hash(&self, item: u64, seed: u64) -> usize {
        let h = item.wrapping_mul(seed).wrapping_add(seed >> 16);
        (h ^ (h >> 32)) as usize % self.xy_width
    }

    /// Increment the count for an item.
    pub fn xy_add(&mut self, item: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += 1;
        }
    }

    /// Add with a specific count.
    pub fn xy_add_count(&mut self, item: u64, count: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += count;
        }
    }

    /// Estimate the count for an item (guaranteed to be >= actual count).
    pub fn xy_estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            min_count = min_count.min(self.xy_table[i][idx]);
        }
        min_count
    }

    /// Width of the sketch.
    pub fn xy_width(&self) -> usize { self.xy_width }

    /// Depth of the sketch.
    pub fn xy_depth(&self) -> usize { self.xy_depth }

    /// Clear the sketch.
    pub fn xy_clear(&mut self) {
        for row in &mut self.xy_table {
            for cell in row { *cell = 0; }
        }
    }

    /// Merge another sketch into this one.
    pub fn xy_merge(&mut self, other: &XyCountMinSketch) {
        if self.xy_width != other.xy_width || self.xy_depth != other.xy_depth { return; }
        for i in 0..self.xy_depth {
            for j in 0..self.xy_width {
                self.xy_table[i][j] += other.xy_table[i][j];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_cell(src: &str) -> NotebookCell {
        NotebookCell {
            source: src.to_string(),
            kind: NotebookCellKind::Code,
            language: "python".to_string(),
            outputs: Vec::new(),
            execution_order: None,
        }
    }

    fn markup_cell(src: &str) -> NotebookCell {
        NotebookCell {
            source: src.to_string(),
            kind: NotebookCellKind::Markup,
            language: "markdown".to_string(),
            outputs: Vec::new(),
            execution_order: None,
        }
    }

    #[test]
    fn add_and_get_cells() {
        let mut doc = NotebookDocument::new("notebook.ipynb");
        doc.add_cell(code_cell("print('a')"));
        doc.add_cell(code_cell("print('b')"));
        assert_eq!(doc.cell_count(), 2);
        assert_eq!(doc.get_cell(0).unwrap().source, "print('a')");
        assert!(doc.is_dirty());
    }

    #[test]
    fn remove_cell() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("a"));
        doc.add_cell(code_cell("b"));
        let removed = doc.remove_cell(0);
        assert_eq!(removed.unwrap().source, "a");
        assert_eq!(doc.cell_count(), 1);
        assert!(doc.remove_cell(99).is_none());
    }

    #[test]
    fn move_cell() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("a"));
        doc.add_cell(code_cell("b"));
        doc.add_cell(code_cell("c"));
        doc.mark_clean();
        doc.move_cell(0, 2);
        assert_eq!(doc.get_cell(0).unwrap().source, "b");
        assert_eq!(doc.get_cell(2).unwrap().source, "a");
        assert!(doc.is_dirty());
    }

    #[test]
    fn dirty_tracking() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        assert!(!doc.is_dirty());
        doc.mark_dirty();
        assert!(doc.is_dirty());
        doc.mark_clean();
        assert!(!doc.is_dirty());
    }

    #[test]
    fn notebook_error_display() {
        assert_eq!(NotebookError::CellNotFound(3).to_string(), "cell not found at index 3");
        assert_eq!(NotebookError::InvalidIndex(5).to_string(), "invalid cell index 5");
        assert_eq!(NotebookError::EmptyNotebook.to_string(), "notebook is empty");
    }

    #[test]
    fn cell_kind_display() {
        assert_eq!(NotebookCellKind::Code.to_string(), "Code");
        assert_eq!(NotebookCellKind::Markup.to_string(), "Markup");
    }

    #[test]
    fn cell_display_truncates() {
        let cell = code_cell("abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJ");
        let display = format!("{cell}");
        assert!(display.starts_with("Code: "));
        // 40-char preview
        assert!(display.contains("abcdefghijklmnopqrstuvwxyz0123456789ABCD"));
    }

    #[test]
    fn code_and_markup_cells_filter() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("x = 1"));
        doc.add_cell(markup_cell("# Title"));
        doc.add_cell(code_cell("y = 2"));
        assert_eq!(doc.code_cells().len(), 2);
        assert_eq!(doc.markup_cells().len(), 1);
        assert_eq!(doc.markup_cells()[0].source, "# Title");
    }

    #[test]
    fn find_cell_by_source() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("import os"));
        doc.add_cell(code_cell("print('hello')"));
        let found = doc.find_cell_by_source("hello");
        assert!(found.is_some());
        let (idx, cell) = found.unwrap();
        assert_eq!(idx, 1);
        assert_eq!(cell.source, "print('hello')");
        assert!(doc.find_cell_by_source("missing").is_none());
    }

    #[test]
    fn insert_cell_at_index() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("a"));
        doc.add_cell(code_cell("c"));
        doc.mark_clean();
        assert!(doc.insert_cell(1, code_cell("b")).is_ok());
        assert_eq!(doc.cell_count(), 3);
        assert_eq!(doc.get_cell(1).unwrap().source, "b");
        assert!(doc.is_dirty());
        // out of bounds
        assert_eq!(doc.insert_cell(99, code_cell("z")), Err(NotebookError::InvalidIndex(99)));
    }

    #[test]
    fn swap_cells() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("first"));
        doc.add_cell(code_cell("second"));
        doc.mark_clean();
        assert!(doc.swap_cells(0, 1).is_ok());
        assert_eq!(doc.get_cell(0).unwrap().source, "second");
        assert_eq!(doc.get_cell(1).unwrap().source, "first");
        assert!(doc.is_dirty());
        assert_eq!(doc.swap_cells(0, 99), Err(NotebookError::InvalidIndex(99)));
    }

    #[test]
    fn clear_outputs() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        let mut cell = code_cell("x = 1");
        cell.add_output(NotebookCellOutput {
            mime_type: "text/plain".to_string(),
            data: "1".to_string(),
        });
        doc.add_cell(cell);
        assert!(doc.get_cell(0).unwrap().has_output());
        doc.clear_outputs();
        assert!(!doc.get_cell(0).unwrap().has_output());
    }

    #[test]
    fn add_output_and_has_output() {
        let mut cell = code_cell("print(1)");
        assert!(!cell.has_output());
        cell.add_output(NotebookCellOutput {
            mime_type: "text/plain".to_string(),
            data: "1".to_string(),
        });
        assert!(cell.has_output());
        assert_eq!(cell.outputs.len(), 1);
    }

    #[test]
    fn total_lines() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("line1\nline2\nline3"));
        doc.add_cell(code_cell("single"));
        doc.add_cell(markup_cell("a\nb"));
        assert_eq!(doc.total_lines(), 6);
    }

    #[test]
    fn insert_cell_at_end() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("a"));
        // inserting at index == len is valid (append)
        assert!(doc.insert_cell(1, code_cell("b")).is_ok());
        assert_eq!(doc.cell_count(), 2);
        assert_eq!(doc.get_cell(1).unwrap().source, "b");
    }

    #[test]
    fn cell_metadata_defaults() {
        let meta = NotebookCellMetadata::default();
        assert!(meta.editable);
        assert!(meta.deletable);
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn cell_metadata_tags() {
        let mut meta = NotebookCellMetadata::default().with_tag("frozen");
        assert!(meta.has_tag("frozen"));
        assert!(!meta.has_tag("other"));
        assert!(meta.remove_tag("frozen"));
        assert!(!meta.has_tag("frozen"));
        assert!(!meta.remove_tag("frozen"));
    }

    #[test]
    fn cells_by_language() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(NotebookCell {
            source: "x = 1".into(),
            kind: NotebookCellKind::Code,
            language: "python".into(),
            outputs: Vec::new(),
            execution_order: None,
        });
        doc.add_cell(NotebookCell {
            source: "let x = 1;".into(),
            kind: NotebookCellKind::Code,
            language: "rust".into(),
            outputs: Vec::new(),
            execution_order: None,
        });
        assert_eq!(doc.cells_by_language("python").len(), 1);
        assert_eq!(doc.cells_by_language("rust").len(), 1);
        assert_eq!(doc.cells_by_language("java").len(), 0);
    }

    #[test]
    fn duplicate_cell() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("original"));
        doc.mark_clean();
        assert!(doc.duplicate_cell(0).is_ok());
        assert_eq!(doc.cell_count(), 2);
        assert_eq!(doc.get_cell(0).unwrap().source, "original");
        assert_eq!(doc.get_cell(1).unwrap().source, "original");
        assert!(doc.is_dirty());
        assert_eq!(doc.duplicate_cell(99), Err(NotebookError::CellNotFound(99)));
    }

    #[test]
    fn find_by_execution_order() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        let mut c = code_cell("a");
        c.execution_order = Some(5);
        doc.add_cell(c);
        doc.add_cell(code_cell("b"));
        assert_eq!(doc.find_by_execution_order(5), Some(0));
        assert_eq!(doc.find_by_execution_order(99), None);
    }

    #[test]
    fn split_cell() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("line1\nline2\nline3\nline4"));
        doc.mark_clean();
        assert!(doc.split_cell(0, 2).is_ok());
        assert_eq!(doc.cell_count(), 2);
        assert_eq!(doc.get_cell(0).unwrap().source, "line1\nline2");
        assert_eq!(doc.get_cell(1).unwrap().source, "line3\nline4");
        assert!(doc.is_dirty());
    }

    #[test]
    fn split_cell_errors() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("line1\nline2"));
        assert_eq!(doc.split_cell(99, 1), Err(NotebookError::CellNotFound(99)));
        assert_eq!(doc.split_cell(0, 0), Err(NotebookError::InvalidIndex(0)));
        assert_eq!(doc.split_cell(0, 5), Err(NotebookError::InvalidIndex(5)));
    }

    #[test]
    fn merge_cells() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("first"));
        doc.add_cell(code_cell("second"));
        doc.mark_clean();
        assert!(doc.merge_cells(0, 1).is_ok());
        assert_eq!(doc.cell_count(), 1);
        assert_eq!(doc.get_cell(0).unwrap().source, "first\nsecond");
        assert!(doc.is_dirty());
    }

    #[test]
    fn merge_cells_errors() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("a"));
        doc.add_cell(code_cell("b"));
        doc.add_cell(code_cell("c"));
        assert_eq!(doc.merge_cells(99, 1), Err(NotebookError::CellNotFound(99)));
        assert_eq!(doc.merge_cells(0, 99), Err(NotebookError::CellNotFound(99)));
        assert_eq!(doc.merge_cells(0, 2), Err(NotebookError::InvalidIndex(2)));
    }

    #[test]
    fn notebook_summary() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("a\nb\nc"));
        doc.add_cell(markup_cell("# Title"));
        let mut c = code_cell("x = 1");
        c.add_output(NotebookCellOutput { mime_type: "text/plain".into(), data: "1".into() });
        doc.add_cell(c);
        let summary = doc.summary();
        assert_eq!(summary.total_cells, 3);
        assert_eq!(summary.code_cells, 2);
        assert_eq!(summary.markup_cells, 1);
        assert_eq!(summary.total_outputs, 1);
        assert_eq!(summary.total_source_lines, 5);
    }

    #[test]
    fn notebook_summary_display() {
        let summary = NotebookSummary {
            total_cells: 5,
            code_cells: 3,
            markup_cells: 2,
            total_outputs: 1,
            total_source_chars: 100,
            total_source_lines: 20,
        };
        let display = summary.to_string();
        assert!(display.contains("5 cells"));
        assert!(display.contains("3 code"));
        assert!(display.contains("2 markup"));
    }

    #[test]
    fn serializer_output() {
        let mut doc = NotebookDocument::new("test.ipynb");
        doc.add_cell(code_cell("print('hello')"));
        doc.metadata.insert("kernel".into(), "python3".into());
        let serialized = NotebookSerializer::serialize(&doc);
        assert!(serialized.contains("# Notebook: test.ipynb"));
        assert!(serialized.contains("# Cells: 1"));
        assert!(serialized.contains("print('hello')"));
        assert!(serialized.contains("kernel=python3"));
    }

    #[test]
    fn serializer_estimated_size() {
        let mut doc = NotebookDocument::new("test.ipynb");
        doc.add_cell(code_cell("x = 1"));
        let size = NotebookSerializer::estimated_size(&doc);
        assert!(size > 0);
    }

    #[test]
    fn cell_output_partial_eq() {
        let a = NotebookCellOutput { mime_type: "text/plain".into(), data: "hello".into() };
        let b = NotebookCellOutput { mime_type: "text/plain".into(), data: "hello".into() };
        let c = NotebookCellOutput { mime_type: "text/html".into(), data: "hello".into() };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn execution_tracker_lifecycle() {
        let mut tracker = CellExecutionTracker::new();
        assert_eq!(*tracker.get_state(0), CellExecutionState::Idle);
        let order = tracker.mark_running(0);
        assert_eq!(order, 1);
        assert_eq!(*tracker.get_state(0), CellExecutionState::Running);
        assert_eq!(tracker.running_cells(), vec![0]);
        tracker.mark_succeeded(0, 150);
        assert_eq!(*tracker.get_state(0), CellExecutionState::Succeeded { duration_ms: 150 });
        assert!(tracker.running_cells().is_empty());
        assert_eq!(tracker.total_executions(), 1);
    }

    #[test]
    fn execution_tracker_failure() {
        let mut tracker = CellExecutionTracker::new();
        tracker.mark_running(2);
        tracker.mark_failed(2, 300, "RuntimeError".into());
        assert_eq!(
            *tracker.get_state(2),
            CellExecutionState::Failed { duration_ms: 300, error: "RuntimeError".into() }
        );
    }

    #[test]
    fn execution_state_display() {
        assert_eq!(CellExecutionState::Idle.to_string(), "Idle");
        assert_eq!(CellExecutionState::Running.to_string(), "Running");
        assert_eq!(
            CellExecutionState::Succeeded { duration_ms: 42 }.to_string(),
            "Succeeded (42ms)"
        );
        assert!(CellExecutionState::Failed { duration_ms: 10, error: "err".into() }
            .to_string()
            .contains("err"));
    }

    #[test]
    fn output_manager_append_and_limit() {
        let mgr = CellOutputManager::new(2);
        let mut cell = code_cell("x = 1");
        let out = || NotebookCellOutput { mime_type: "text/plain".into(), data: "v".into() };
        assert!(mgr.append_output(&mut cell, out()));
        assert!(mgr.append_output(&mut cell, out()));
        assert!(!mgr.append_output(&mut cell, out()));
        assert_eq!(cell.outputs.len(), 2);
        mgr.replace_outputs(&mut cell, out());
        assert_eq!(cell.outputs.len(), 1);
        assert!(CellOutputManager::output_byte_size(&cell) > 0);
    }

    #[test]
    fn generate_outline_from_markup() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(markup_cell("# Introduction\nSome text\n## Background"));
        doc.add_cell(code_cell("x = 1"));
        doc.add_cell(markup_cell("### Details"));
        let outline = generate_notebook_outline(&doc);
        assert_eq!(outline.len(), 3);
        assert_eq!(outline[0].heading_level, 1);
        assert_eq!(outline[0].text, "Introduction");
        assert_eq!(outline[0].cell_index, 0);
        assert_eq!(outline[1].heading_level, 2);
        assert_eq!(outline[1].text, "Background");
        assert_eq!(outline[2].cell_index, 2);
    }

    #[test]
    fn cell_dependency_analysis() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(code_cell("data = load()"));
        doc.add_cell(code_cell("result = process(data)"));
        doc.add_cell(markup_cell("# Notes"));
        doc.add_cell(code_cell("print(result)"));
        let deps = analyze_cell_dependencies(&doc);
        assert!(deps.iter().any(|d| d.cell_index == 1 && d.depends_on == 0 && d.symbol == "data"));
        assert!(deps.iter().any(|d| d.cell_index == 3 && d.depends_on == 1 && d.symbol == "result"));
    }

    #[test]
    fn toolbar_compute_middle_cell() {
        let mut toolbar = NotebookToolbar::new();
        toolbar.compute_actions(1, 3);
        assert!(toolbar.has_action(&ToolbarAction::RunCell(1)));
        assert!(toolbar.has_action(&ToolbarAction::MoveCellUp(1)));
        assert!(toolbar.has_action(&ToolbarAction::MoveCellDown(1)));
        assert!(toolbar.has_action(&ToolbarAction::RunAll));
    }

    #[test]
    fn toolbar_first_cell_no_move_up() {
        let mut toolbar = NotebookToolbar::new();
        toolbar.compute_actions(0, 3);
        assert!(!toolbar.has_action(&ToolbarAction::MoveCellUp(0)));
        assert!(toolbar.has_action(&ToolbarAction::MoveCellDown(0)));
    }

    #[test]
    fn toolbar_last_cell_no_move_down() {
        let mut toolbar = NotebookToolbar::new();
        toolbar.compute_actions(2, 3);
        assert!(toolbar.has_action(&ToolbarAction::MoveCellUp(2)));
        assert!(!toolbar.has_action(&ToolbarAction::MoveCellDown(2)));
    }

    #[test]
    fn toolbar_empty_notebook() {
        let mut toolbar = NotebookToolbar::new();
        toolbar.compute_actions(0, 0);
        assert_eq!(toolbar.action_count(), 0);
    }

    #[test]
    fn cell_status_line_idle() {
        let tracker = CellExecutionTracker::new();
        let status = CellStatusLine::from_tracker(0, &tracker);
        assert_eq!(status.status_text, "Idle");
        assert!(!status.is_running);
    }

    #[test]
    fn cell_status_line_running() {
        let mut tracker = CellExecutionTracker::new();
        tracker.mark_running(0);
        let status = CellStatusLine::from_tracker(0, &tracker);
        assert!(status.is_running);
        assert_eq!(status.status_text, "Running...");
    }

    #[test]
    fn export_python_format() {
        let mut doc = NotebookDocument::new("test.ipynb");
        doc.add_cell(markup_cell("# Title"));
        doc.add_cell(code_cell("x = 1"));
        let result = notebook_export_format(&doc, ExportFormat::Python);
        assert!(result.contains("# # Title"));
        assert!(result.contains("x = 1"));
    }

    #[test]
    fn export_markdown_format() {
        let mut doc = NotebookDocument::new("test.ipynb");
        doc.add_cell(code_cell("x = 1"));
        doc.add_cell(markup_cell("Some text"));
        let result = notebook_export_format(&doc, ExportFormat::Markdown);
        assert!(result.contains("```python"));
        assert!(result.contains("x = 1"));
        assert!(result.contains("Some text"));
    }

    #[test]
    fn test_cell_kind_helpers() {
        assert!(NotebookCellKind::Code.is_code());
        assert!(!NotebookCellKind::Code.is_markup());
        assert!(NotebookCellKind::Markup.is_markup());
        assert_eq!(NotebookCellKind::all().len(), 2);
        assert_eq!(NotebookCellKind::default(), NotebookCellKind::Code);
    }

    #[test]
    fn test_notebook_cell_code() {
        let cell = NotebookCell::code("let x = 1;", "rust");
        assert_eq!(cell.line_count(), 1);
        assert!(!cell.is_empty());
        assert!(!cell.has_outputs());
        assert!(format!("{cell}").contains("Code"));
    }

    #[test]
    fn test_notebook_cell_markup() {
        let cell = NotebookCell::markup("# Title\n\nSome text");
        assert!(cell.kind.is_markup());
        assert_eq!(cell.line_count(), 3);
    }

    #[test]
    fn test_notebook_cell_empty() {
        let cell = NotebookCell::code("   ", "python");
        assert!(cell.is_empty());
    }

    #[test]
    fn test_notebook_document_helpers() {
        let mut doc = NotebookDocument::new("test.ipynb");
        doc.add_cell(NotebookCell::code("x = 1", "python"));
        doc.add_cell(NotebookCell::markup("# Header"));
        doc.add_cell(NotebookCell::code("y = 2", "python"));
        doc.add_cell(NotebookCell::code("console.log(1)", "javascript"));
        assert_eq!(doc.code_cell_count(), 3);
        assert_eq!(doc.markup_cell_count(), 1);
        assert_eq!(doc.languages(), vec!["javascript", "python"]);
        assert_eq!(doc.total_line_count(), 4);
    }

    #[test]
    fn test_notebook_search_cells() {
        let mut doc = NotebookDocument::new("test.ipynb");
        doc.add_cell(NotebookCell::code("let x = 42;", "rust"));
        doc.add_cell(NotebookCell::markup("Some text"));
        doc.add_cell(NotebookCell::code("let y = 42;", "rust"));
        let results = doc.search_cells("42");
        assert_eq!(results, vec![0, 2]);
    }

    #[test]
    fn test_cell_execution_state_helpers() {
        assert!(CellExecutionState::Succeeded { duration_ms: 10 }.is_finished());
        assert!(CellExecutionState::Running.is_running());
        assert!(!CellExecutionState::Idle.is_finished());
        assert_eq!(CellExecutionState::Succeeded { duration_ms: 0 }.icon(), '✓');
        assert_eq!(CellExecutionState::default(), CellExecutionState::Idle);
    }

    #[test]
    fn test_export_format_helpers() {
        assert_eq!(ExportFormat::all().len(), 2);
        assert_eq!(ExportFormat::Markdown.extension(), "md");
        assert_eq!(ExportFormat::from_name("python"), Some(ExportFormat::Python));
        assert_eq!(ExportFormat::from_name("bogus"), None);
        assert_eq!(format!("{}", ExportFormat::Markdown), "Markdown");
    }

    // --- New tests ---

    #[test]
    fn execution_order_log_records_and_queries() {
        let mut log = ExecutionOrderLog::new();
        let n1 = log.record(0, "x = 1");
        let n2 = log.record(1, "y = 2");
        let n3 = log.record(0, "x = 10");
        assert_eq!(n1, 1);
        assert_eq!(n2, 2);
        assert_eq!(n3, 3);
        assert_eq!(log.total_executions(), 3);
        assert_eq!(log.entries_for_cell(0).len(), 2);
        assert_eq!(log.last_execution(0), Some(3));
        assert_eq!(log.last_execution(1), Some(2));
        assert_eq!(log.last_execution(99), None);
        assert_eq!(log.execution_order(), vec![0, 1]);
    }

    #[test]
    fn execution_order_log_stale_cells() {
        let mut log = ExecutionOrderLog::new();
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.add_cell(NotebookCell::code("x = 1", "python"));
        doc.add_cell(NotebookCell::code("y = 2", "python"));
        log.record(0, "x = 1");
        log.record(1, "y = 2");
        // Modify cell 0 source after execution
        doc.cells[0].source = "x = 100".to_string();
        let stale = log.stale_cells(&doc);
        assert_eq!(stale, vec![0]);
    }

    #[test]
    fn notebook_kernel_info_display() {
        let k = NotebookKernelInfo::new("python3", "Python")
            .with_version("3.11");
        assert_eq!(k.display_name(), "python3 (Python 3.11)");
        assert_eq!(k.to_string(), "python3 (Python 3.11)");

        let k2 = NotebookKernelInfo::new("irkernel", "R");
        assert_eq!(k2.display_name(), "irkernel (R)");
    }

    #[test]
    fn metadata_manager_import_export_roundtrip() {
        let mut doc = NotebookDocument::new("nb.ipynb");
        doc.metadata.insert("kernel".into(), "python3".into());
        doc.metadata.insert("language".into(), "python".into());
        doc.metadata.insert("kernel_version".into(), "3.11".into());
        doc.metadata.insert("author".into(), "test".into());

        let mut mgr = NotebookMetadataManager::new();
        mgr.import_from_document(&doc);
        assert_eq!(mgr.kernel.as_ref().unwrap().name, "python3");
        assert_eq!(mgr.kernel.as_ref().unwrap().language, "python");
        assert_eq!(mgr.kernel.as_ref().unwrap().version.as_deref(), Some("3.11"));
        assert_eq!(mgr.get_custom("author"), Some("test"));

        mgr.set_trusted(true);
        let map = mgr.export_to_map();
        assert_eq!(map.get("kernel").unwrap(), "python3");
        assert_eq!(map.get("trusted").unwrap(), "true");
        assert_eq!(map.get("format_version").unwrap(), "4.5");
        assert_eq!(map.get("author").unwrap(), "test");
    }

    #[test]
    fn diff_cell_outputs_detects_changes() {
        let old = vec![
            NotebookCellOutput { mime_type: "text/plain".into(), data: "1".into() },
            NotebookCellOutput { mime_type: "text/plain".into(), data: "2".into() },
        ];
        let new = vec![
            NotebookCellOutput { mime_type: "text/plain".into(), data: "1".into() },
            NotebookCellOutput { mime_type: "text/plain".into(), data: "changed".into() },
            NotebookCellOutput { mime_type: "text/html".into(), data: "<b>new</b>".into() },
        ];
        let diffs = diff_cell_outputs(&old, &new);
        assert_eq!(diffs.len(), 2);
        assert!(matches!(&diffs[0], OutputDiff::Changed { index: 1, .. }));
        assert!(matches!(&diffs[1], OutputDiff::Added { index: 2, .. }));

        // Test removal
        let diffs2 = diff_cell_outputs(&new, &old);
        assert_eq!(diffs2.len(), 2);
        assert!(matches!(&diffs2[1], OutputDiff::Removed { index: 2, .. }));

        // Identical outputs produce no diffs
        assert!(diff_cell_outputs(&old, &old).is_empty());
    }

    #[test]
    fn dependency_graph_topological_order() {
        // cell 0: data = load()
        // cell 1: result = process(data)
        // cell 2: # markdown (no deps)
        // cell 3: print(result)
        let deps = vec![
            CellDependency { cell_index: 1, depends_on: 0, symbol: "data".into() },
            CellDependency { cell_index: 3, depends_on: 1, symbol: "result".into() },
        ];
        let graph = CellDependencyGraph::from_dependencies(&deps, 4);
        assert_eq!(graph.dependencies_of(1), vec![0]);
        assert_eq!(graph.dependents_of(0), vec![1]);
        assert_eq!(graph.dependents_of(1), vec![3]);

        let order = graph.topological_order().unwrap();
        // cell 0 must come before 1, cell 1 before 3
        let pos = |idx: usize| order.iter().position(|&x| x == idx).unwrap();
        assert!(pos(0) < pos(1));
        assert!(pos(1) < pos(3));

        // Transitive dependencies of cell 3 = {0, 1}
        assert_eq!(graph.transitive_dependencies(3), vec![0, 1]);
        assert_eq!(graph.transitive_dependencies(1), vec![0]);
        assert!(graph.transitive_dependencies(0).is_empty());
    }

    // -- NotebookCellToolbar tests --

    #[test]
    fn cell_toolbar_default_code() {
        let tb = NotebookCellToolbar::default_for(0, NotebookCellKind::Code);
        assert!(tb.find_action("run").is_some());
        assert!(tb.find_action("clear").is_some());
        assert!(tb.find_action("delete").is_some());
        assert!(!tb.is_empty());
    }

    #[test]
    fn cell_toolbar_default_markup() {
        let tb = NotebookCellToolbar::default_for(1, NotebookCellKind::Markup);
        assert!(tb.find_action("edit").is_some());
        assert!(tb.find_action("run").is_none());
    }

    #[test]
    fn cell_toolbar_action_with_tooltip() {
        let action = CellToolbarAction::new("run", "Run", "▶").with_tooltip("Run Cell").with_enabled(false);
        assert_eq!(action.tooltip, "Run Cell");
        assert!(!action.enabled);
    }

    // -- NotebookOutputCollapse tests --

    #[test]
    fn output_collapse_toggle() {
        let mut oc = NotebookOutputCollapse::new();
        assert!(!oc.is_collapsed(0));
        oc.toggle(0);
        assert!(oc.is_collapsed(0));
        oc.toggle(0);
        assert!(!oc.is_collapsed(0));
    }

    #[test]
    fn output_collapse_all() {
        let mut oc = NotebookOutputCollapse::default();
        oc.collapse_all(5);
        assert_eq!(oc.collapsed_count(), 5);
        assert!(oc.is_collapsed(3));
        oc.expand_all();
        assert_eq!(oc.collapsed_count(), 0);
    }

    // -- NotebookCellStatusBar tests --

    #[test]
    fn cell_status_bar_time_ms() {
        let mut bar = NotebookCellStatusBar::new(0, "python");
        bar.execution_time_ms = Some(500);
        assert_eq!(bar.execution_time_label(), "500ms");
    }

    #[test]
    fn cell_status_bar_time_seconds() {
        let mut bar = NotebookCellStatusBar::new(0, "python");
        bar.execution_time_ms = Some(2500);
        assert_eq!(bar.execution_time_label(), "2.5s");
    }

    #[test]
    fn cell_status_display() {
        assert_eq!(format!("{}", CellStatus::Running), "Running");
        assert_eq!(format!("{}", CellStatus::Success), "✓");
    }

    #[test]
    fn cell_status_bar_no_time() {
        let bar = NotebookCellStatusBar::new(0, "rust");
        assert!(bar.execution_time_label().is_empty());
        assert_eq!(bar.status, CellStatus::Idle);
    }

    // -- NotebookScrollSync tests --

    #[test]
    fn scroll_sync_visible() {
        let mut ss = NotebookScrollSync::new();
        ss.set_viewport(2, 5);
        assert!(ss.is_visible(3));
        assert!(!ss.is_visible(1));
        assert!(!ss.is_visible(6));
    }

    #[test]
    fn scroll_sync_target_when_not_visible() {
        let mut ss = NotebookScrollSync::new();
        ss.set_viewport(0, 3);
        assert_eq!(ss.scroll_target(5), Some(5));
        assert_eq!(ss.scroll_target(2), None); // already visible
    }

    #[test]
    fn scroll_sync_disabled() {
        let mut ss = NotebookScrollSync::default();
        ss.enabled = false;
        ss.set_viewport(0, 3);
        assert_eq!(ss.scroll_target(5), None);
    }

    // -- NotebookCellSearchEngine tests ----------------------------------------

    #[test]
    fn search_basic() {
        let mut e = NotebookCellSearchEngine::new(true, false);
        e.search(&[(0, "hello world")], "world");
        assert_eq!(e.match_count(), 1);
        assert_eq!(e.matches()[0].column_start, 6);
    }

    #[test]
    fn search_case_insensitive() {
        let mut e = NotebookCellSearchEngine::new(false, false);
        e.search(&[(0, "Hello World")], "hello");
        assert_eq!(e.match_count(), 1);
    }

    #[test]
    fn search_whole_word() {
        let mut e = NotebookCellSearchEngine::new(true, true);
        e.search(&[(0, "foobar foo barfoo")], "foo");
        assert_eq!(e.match_count(), 1);
        assert_eq!(e.matches()[0].column_start, 7);
    }

    #[test]
    fn search_multiple_cells() {
        let mut e = NotebookCellSearchEngine::new(true, false);
        e.search(&[(0, "aaa"), (1, "bbb aaa"), (2, "ccc")], "aaa");
        assert_eq!(e.match_count(), 2);
        assert_eq!(e.matches_in_cell(1).len(), 1);
    }

    #[test]
    fn search_empty_pattern() {
        let mut e = NotebookCellSearchEngine::new(true, false);
        e.search(&[(0, "hello")], "");
        assert_eq!(e.match_count(), 0);
    }

    #[test]
    fn search_multiline_cell() {
        let mut e = NotebookCellSearchEngine::new(true, false);
        e.search(&[(0, "line1\nline2 target\nline3")], "target");
        assert_eq!(e.match_count(), 1);
        assert_eq!(e.matches()[0].line_number, 1);
    }

    #[test]
    fn search_clear() {
        let mut e = NotebookCellSearchEngine::new(true, false);
        e.search(&[(0, "hello")], "hello");
        assert_eq!(e.match_count(), 1);
        e.clear();
        assert_eq!(e.match_count(), 0);
    }

    // -- NotebookCellDependencyTracker tests ----------------------------------

    #[test]
    fn dep_tracker_basic_dependency() {
        let mut t = NotebookCellDependencyTracker::new();
        t.add_definition(0, "x");
        t.add_usage(1, "x");
        assert_eq!(t.dependencies_of(1), vec![0]);
        assert_eq!(t.dependents_of(0), vec![1]);
    }

    #[test]
    fn dep_tracker_no_self_dependency() {
        let mut t = NotebookCellDependencyTracker::new();
        t.add_definition(0, "x");
        t.add_usage(0, "x");
        assert!(t.dependencies_of(0).is_empty());
    }

    #[test]
    fn dep_tracker_execution_order() {
        let mut t = NotebookCellDependencyTracker::new();
        t.add_definition(0, "a");
        t.add_definition(1, "b");
        t.add_usage(1, "a");
        t.add_usage(2, "b");
        t.add_definition(2, "c");
        let order = t.execution_order().unwrap();
        let pos_0 = order.iter().position(|&c| c == 0).unwrap();
        let pos_1 = order.iter().position(|&c| c == 1).unwrap();
        let pos_2 = order.iter().position(|&c| c == 2).unwrap();
        assert!(pos_0 < pos_1);
        assert!(pos_1 < pos_2);
    }

    #[test]
    fn dep_tracker_cell_count() {
        let mut t = NotebookCellDependencyTracker::new();
        t.add_definition(0, "x");
        t.add_usage(1, "y");
        assert_eq!(t.cell_count(), 2);
    }

    #[test]
    fn dep_tracker_no_deps() {
        let t = NotebookCellDependencyTracker::new();
        assert!(t.dependencies_of(0).is_empty());
        assert!(t.dependents_of(0).is_empty());
    }



    #[test]
    fn toolbar_actions_register() {
        let mut mgr = NotebookCellToolbarActions::new();
        let action = NotebookToolbarActionEntry {
            id: "run".into(), label: "Run Cell".into(), tooltip: "Run".into(),
            icon: "play".into(), enabled: true, shortcut: None,
        };
        assert!(mgr.register(action).is_ok());
        assert_eq!(mgr.action_count(), 1);
    }

    #[test]
    fn toolbar_actions_duplicate_register_fails() {
        let mut mgr = NotebookCellToolbarActions::new();
        let action = NotebookToolbarActionEntry {
            id: "run".into(), label: "Run".into(), tooltip: "".into(),
            icon: "play".into(), enabled: true, shortcut: None,
        };
        mgr.register(action.clone()).unwrap();
        assert!(mgr.register(action).is_err());
    }

    #[test]
    fn toolbar_actions_execute() {
        let mut mgr = NotebookCellToolbarActions::new();
        let action = NotebookToolbarActionEntry {
            id: "run".into(), label: "Run Cell".into(), tooltip: "".into(),
            icon: "play".into(), enabled: true, shortcut: None,
        };
        mgr.register(action).unwrap();
        let label = mgr.execute("run").unwrap();
        assert_eq!(label, "Run Cell");
        assert_eq!(mgr.execution_log().len(), 1);
    }

    #[test]
    fn toolbar_actions_execute_disabled() {
        let mut mgr = NotebookCellToolbarActions::new();
        let action = NotebookToolbarActionEntry {
            id: "del".into(), label: "Delete".into(), tooltip: "".into(),
            icon: "trash".into(), enabled: false, shortcut: None,
        };
        mgr.register(action).unwrap();
        assert!(mgr.execute("del").is_err());
    }

    #[test]
    fn toolbar_actions_unregister() {
        let mut mgr = NotebookCellToolbarActions::new();
        let action = NotebookToolbarActionEntry {
            id: "run".into(), label: "Run".into(), tooltip: "".into(),
            icon: "play".into(), enabled: true, shortcut: None,
        };
        mgr.register(action).unwrap();
        assert!(mgr.unregister("run"));
        assert_eq!(mgr.action_count(), 0);
    }

    #[test]
    fn toolbar_actions_set_enabled() {
        let mut mgr = NotebookCellToolbarActions::new();
        let action = NotebookToolbarActionEntry {
            id: "run".into(), label: "Run".into(), tooltip: "".into(),
            icon: "play".into(), enabled: true, shortcut: None,
        };
        mgr.register(action).unwrap();
        assert_eq!(mgr.enabled_count(), 1);
        mgr.set_enabled("run", false);
        assert_eq!(mgr.enabled_count(), 0);
    }

    #[test]
    fn toolbar_actions_search() {
        let mut mgr = NotebookCellToolbarActions::new();
        for (id, label) in &[("run", "Run Cell"), ("del", "Delete Cell"), ("mv", "Move Up")] {
            let action = NotebookToolbarActionEntry {
                id: id.to_string(), label: label.to_string(), tooltip: "".into(),
                icon: "x".into(), enabled: true, shortcut: None,
            };
            mgr.register(action).unwrap();
        }
        assert_eq!(mgr.search_actions("cell").len(), 2);
    }

    #[test]
    fn toolbar_actions_display() {
        let mgr = NotebookCellToolbarActions::new();
        let s = format!("{mgr}");
        assert!(s.contains("0 registered"));
    }

    #[test]
    fn status_indicator_basic_flow() {
        let mut ind = NotebookStatusIndicator::new(3);
        assert_eq!(ind.status(), NotebookExecutionStatus::Idle);
        ind.start(1000);
        assert_eq!(ind.status(), NotebookExecutionStatus::Running);
        ind.cell_started();
        ind.cell_succeeded();
        ind.cell_started();
        ind.cell_succeeded();
        ind.cell_started();
        ind.cell_succeeded();
        assert_eq!(ind.status(), NotebookExecutionStatus::Succeeded);
        assert_eq!(ind.completed_cells(), 3);
    }

    #[test]
    fn status_indicator_failure() {
        let mut ind = NotebookStatusIndicator::new(2);
        ind.start(0);
        ind.cell_started();
        ind.cell_succeeded();
        ind.cell_started();
        ind.cell_failed();
        assert_eq!(ind.status(), NotebookExecutionStatus::Failed);
    }

    #[test]
    fn status_indicator_cancel() {
        let mut ind = NotebookStatusIndicator::new(5);
        ind.start(0);
        ind.cell_started();
        ind.cancel(100);
        assert_eq!(ind.status(), NotebookExecutionStatus::Cancelled);
    }

    #[test]
    fn status_indicator_progress() {
        let mut ind = NotebookStatusIndicator::new(4);
        ind.start(0);
        assert!((ind.progress() - 0.0).abs() < f64::EPSILON);
        ind.cell_started();
        ind.cell_succeeded();
        ind.cell_started();
        ind.cell_succeeded();
        assert!((ind.progress() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn status_indicator_summary() {
        let mut ind = NotebookStatusIndicator::new(2);
        ind.start(0);
        ind.set_elapsed(500);
        let s = ind.summary();
        assert!(s.contains("500ms"));
        assert!(s.contains("Running"));
    }

    #[test]
    fn status_indicator_reset() {
        let mut ind = NotebookStatusIndicator::new(2);
        ind.start(0);
        ind.cell_started();
        ind.cell_succeeded();
        ind.reset();
        assert_eq!(ind.status(), NotebookExecutionStatus::Idle);
        assert_eq!(ind.completed_cells(), 0);
    }

    #[test]
    fn status_indicator_display() {
        let ind = NotebookStatusIndicator::new(3);
        let s = format!("{ind}");
        assert!(s.contains("Idle"));
    }



    // -- notebook_view extended domain tests ----------------------------------------

    #[test]
    fn y_notebook_view_enum_index() {
        assert_eq!(YNotebookViewCellKind::Code.index(), 0);
        assert_eq!(YNotebookViewCellKind::Markdown.index(), 1);
        assert_eq!(YNotebookViewCellKind::Raw.index(), 2);
        assert_eq!(YNotebookViewCellKind::Output.index(), 3);
    }

    #[test]
    fn y_notebook_view_enum_label() {
        assert_eq!(YNotebookViewCellKind::Code.label(), "Code");
        assert_eq!(YNotebookViewCellKind::Markdown.label(), "Markdown");
        assert_eq!(YNotebookViewCellKind::Raw.label(), "Raw");
        assert_eq!(YNotebookViewCellKind::Output.label(), "Output");
    }

    #[test]
    fn y_notebook_view_enum_all() {
        let all = YNotebookViewCellKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_notebook_view_enum_is_default() {
        assert!(YNotebookViewCellKind::Code.is_default());
        assert!(!YNotebookViewCellKind::Output.is_default());
    }

    #[test]
    fn y_notebook_view_enum_display() {
        assert_eq!(format!("{}", YNotebookViewCellKind::Code), "Code");
    }

    #[test]
    fn y_notebook_view_struct_new() {
        let s = YNotebookViewNotebookCellState::new();
        let _ = s.summary();
    }

    #[test]
    fn y_notebook_view_fingerprint_deterministic() {
        let h1 = y_notebook_view_fingerprint("hello");
        let h2 = y_notebook_view_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_notebook_view_fingerprint("a"), y_notebook_view_fingerprint("b"));
    }

    #[test]
    fn y_notebook_view_truncate_short() {
        assert_eq!(y_notebook_view_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_notebook_view_truncate_long() {
        let r = y_notebook_view_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_notebook_view_normalize_key_basic() {
        assert_eq!(y_notebook_view_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_notebook_view_split_path_basic() {
        let parts = y_notebook_view_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_notebook_view_count_occurrences_basic() {
        assert_eq!(y_notebook_view_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_notebook_view_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_notebook_view_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_notebook_view_in_range_basic() {
        assert!(y_notebook_view_in_range(5, 1, 10));
        assert!(y_notebook_view_in_range(1, 1, 10));
        assert!(y_notebook_view_in_range(10, 1, 10));
        assert!(!y_notebook_view_in_range(0, 1, 10));
        assert!(!y_notebook_view_in_range(11, 1, 10));
    }

    #[test]
    fn y_notebook_view_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_notebook_view_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_notebook_view_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_notebook_view_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- notebook_view Z-extended tests -----------------------------------------------

    #[test]
    fn z_notebook_view_priority_weight() {
        assert_eq!(ZNotebookViewPriority::Idle.weight(), 0);
        assert_eq!(ZNotebookViewPriority::Normal.weight(), 2);
        assert_eq!(ZNotebookViewPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_notebook_view_priority_label() {
        assert_eq!(ZNotebookViewPriority::Low.label(), "low");
        assert_eq!(ZNotebookViewPriority::High.label(), "high");
    }

    #[test]
    fn z_notebook_view_priority_is_elevated() {
        assert!(!ZNotebookViewPriority::Normal.is_elevated());
        assert!(ZNotebookViewPriority::High.is_elevated());
        assert!(ZNotebookViewPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_notebook_view_priority_display() {
        assert_eq!(format!("{}", ZNotebookViewPriority::Idle), "idle");
    }

    #[test]
    fn z_notebook_view_priority_all_asc() {
        let all = ZNotebookViewPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZNotebookViewPriority::Idle);
        assert_eq!(all[4], ZNotebookViewPriority::Realtime);
    }

    #[test]
    fn z_notebook_view_struct_new() {
        let s = ZNotebookViewNotebookKernelState::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_notebook_view_struct_toggled_clone() {
        let s = ZNotebookViewNotebookKernelState::new();
        let t = s.toggled_clone();
        assert_ne!(s.busy, t.busy);
    }

    #[test]
    fn z_notebook_view_rolling_hash_deterministic() {
        let h1 = z_notebook_view_rolling_hash(b"test");
        let h2 = z_notebook_view_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_notebook_view_rolling_hash(b"a"), z_notebook_view_rolling_hash(b"b"));
    }

    #[test]
    fn z_notebook_view_pad_to_basic() {
        assert_eq!(z_notebook_view_pad_to("hi", 5), "hi   ");
        assert_eq!(z_notebook_view_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_notebook_view_is_identifier_basic() {
        assert!(z_notebook_view_is_identifier("foo_bar"));
        assert!(z_notebook_view_is_identifier("abc123"));
        assert!(!z_notebook_view_is_identifier(""));
        assert!(!z_notebook_view_is_identifier("has space"));
    }

    #[test]
    fn z_notebook_view_levenshtein_basic() {
        assert_eq!(z_notebook_view_levenshtein("", ""), 0);
        assert_eq!(z_notebook_view_levenshtein("abc", "abc"), 0);
        assert_eq!(z_notebook_view_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_notebook_view_unique_words_basic() {
        let w = z_notebook_view_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_notebook_view_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_notebook_view_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_notebook_view_common_prefix_basic() {
        assert_eq!(z_notebook_view_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_notebook_view_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_notebook_view_struct_clear() {
        let mut s = ZNotebookViewNotebookKernelState::new();
        s.running_cells.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_notebook_view_rolling_hash_empty() {
        let h = z_notebook_view_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 128 ----

    #[test]
    fn xc_128_pool_new_empty() {
        let pool: super::Xc128Pool<i32> = super::Xc128Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_128_pool_release_acquire() {
        let mut pool = super::Xc128Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_128_pool_acquire_empty() {
        let mut pool: super::Xc128Pool<i32> = super::Xc128Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_128_pool_full() {
        let mut pool = super::Xc128Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_128_pool_drain() {
        let mut pool = super::Xc128Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_128_pool_stats() {
        let mut pool = super::Xc128Pool::new(8);
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
    fn xc_128_pool_clear() {
        let mut pool = super::Xc128Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_128_pool_shrink() {
        let mut pool = super::Xc128Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_128_pool_default() {
        let pool: super::Xc128Pool<String> = super::Xc128Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_128_pool_extend() {
        let mut pool = super::Xc128Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_128_pool_retain() {
        let mut pool = super::Xc128Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_128_scheduler_round_robin() {
        let mut sched = super::Xc128Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_128_scheduler_empty() {
        let mut sched = super::Xc128Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_128_scheduler_reset() {
        let mut sched = super::Xc128Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_128_scheduler_add_remove() {
        let mut sched = super::Xc128Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_128_scheduler_targets() {
        let sched = super::Xc128Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_128_hash_empty() {
        assert_eq!(super::xc_128_hash(b""), 5381);
    }

    #[test]
    fn xc_128_hash_data() {
        let h = super::xc_128_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_128_hash(b"hello"), h);
    }

    #[test]
    fn xc_128_reverse_str() {
        assert_eq!(super::xc_128_reverse("abc"), "cba");
        assert_eq!(super::xc_128_reverse(""), "");
    }


    // --- xd_26 deepening tests ---

    #[test]
    fn xd_26_sm_initial_state() {
        let sm = Xd26StateMachine::new();
        assert_eq!(sm.current_state(), Xd26State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_26_sm_valid_idle_to_running() {
        let mut sm = Xd26StateMachine::new();
        assert!(sm.transition(Xd26State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd26State::Running);
    }

    #[test]
    fn xd_26_sm_valid_running_to_paused() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        assert!(sm.transition(Xd26State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd26State::Paused);
    }

    #[test]
    fn xd_26_sm_valid_running_to_done() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        assert!(sm.transition(Xd26State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd26State::Done);
    }

    #[test]
    fn xd_26_sm_valid_paused_to_running() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        sm.transition(Xd26State::Paused).unwrap();
        assert!(sm.transition(Xd26State::Running).is_ok());
    }

    #[test]
    fn xd_26_sm_valid_done_to_idle() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        sm.transition(Xd26State::Done).unwrap();
        assert!(sm.transition(Xd26State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd26State::Idle);
    }

    #[test]
    fn xd_26_sm_invalid_idle_to_done() {
        let mut sm = Xd26StateMachine::new();
        assert!(sm.transition(Xd26State::Done).is_err());
    }

    #[test]
    fn xd_26_sm_invalid_idle_to_paused() {
        let mut sm = Xd26StateMachine::new();
        assert!(sm.transition(Xd26State::Paused).is_err());
    }

    #[test]
    fn xd_26_sm_history_tracking() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        sm.transition(Xd26State::Paused).unwrap();
        sm.transition(Xd26State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd26State::Idle);
        assert_eq!(sm.history()[0].to, Xd26State::Running);
        assert_eq!(sm.history()[1].from, Xd26State::Running);
        assert_eq!(sm.history()[2].to, Xd26State::Done);
    }

    #[test]
    fn xd_26_sm_serialize_deserialize() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd26StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd26State::Running));
    }

    #[test]
    fn xd_26_sm_deserialize_invalid() {
        assert_eq!(Xd26StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_26_sm_reset() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd26State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_26_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd26EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd26Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_26_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd26EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd26Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd26Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_26_bus_unsubscribe() {
        let mut bus = Xd26EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_26_event_kind_and_payload() {
        let e = Xd26Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd26Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_26_bus_clear_history() {
        let mut bus = Xd26EventBus::new();
        bus.publish(Xd26Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_26_sm_step_counter_increments() {
        let mut sm = Xd26StateMachine::new();
        sm.transition(Xd26State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd26State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #24 --

    #[test]
    fn xf24_trie_insert_search() {
        let mut t = Xf24Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf24_trie_starts_with() {
        let mut t = Xf24Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf24_trie_remove() {
        let mut t = Xf24Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf24_trie_word_count() {
        let mut t = Xf24Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf24_trie_longest_prefix() {
        let mut t = Xf24Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf24_trie_all_words() {
        let mut t = Xf24Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf24_trie_autocomplete() {
        let mut t = Xf24Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf24_trie_empty_search() {
        let t = Xf24Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf24_bloom_add_contains() {
        let mut bf = Xf24BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf24_bloom_probably_absent() {
        let bf = Xf24BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf24_bloom_false_positive_rate() {
        let mut bf = Xf24BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf24_bloom_clear() {
        let mut bf = Xf24BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf24_bloom_union() {
        let mut a = Xf24BloomFilter::xf_new(512, 2);
        let mut b = Xf24BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf24_bloom_intersection_estimate() {
        let mut a = Xf24BloomFilter::xf_new(512, 2);
        let mut b = Xf24BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf24_bloom_union_size_mismatch() {
        let a = Xf24BloomFilter::xf_new(256, 2);
        let b = Xf24BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh127_skip_insert_contains() {
        let mut sl = super::Xh127SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh127_skip_remove() {
        let mut sl = super::Xh127SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh127_skip_len() {
        let mut sl = super::Xh127SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh127_skip_range_query() {
        let mut sl = super::Xh127SkipList::xh_new(4);
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
    fn xh127_skip_floor_ceiling() {
        let mut sl = super::Xh127SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh127_skip_rank() {
        let mut sl = super::Xh127SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh127_skip_empty() {
        let sl = super::Xh127SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh127_skip_duplicates() {
        let mut sl = super::Xh127SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh127_bitset_set_test() {
        let mut bs = super::Xh127BitSet::xh_new(256);
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
    fn xh127_bitset_clear_count() {
        let mut bs = super::Xh127BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh127_bitset_and_or_xor() {
        let mut a = super::Xh127BitSet::xh_new(128);
        let mut b = super::Xh127BitSet::xh_new(128);
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
    fn xh127_bitset_iter_ones() {
        let mut bs = super::Xh127BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh127_bitset_first_last() {
        let mut bs = super::Xh127BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh127_bitset_empty() {
        let bs = super::Xh127BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi127_deque_push_pop_back() {
        let mut dq = super::Xi127Deque::xi_new(4);
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
    fn xi127_deque_push_pop_front() {
        let mut dq = super::Xi127Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi127_deque_mixed_ops() {
        let mut dq = super::Xi127Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi127_deque_get_and_split() {
        let mut dq = super::Xi127Deque::xi_new(8);
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
    fn xi127_deque_rotate_left() {
        let mut dq = super::Xi127Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi127_deque_rotate_right() {
        let mut dq = super::Xi127Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi127_deque_grow() {
        let mut dq = super::Xi127Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi127_deque_empty() {
        let dq = super::Xi127Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi127_interval_tree_insert_query() {
        let mut tree = super::Xi127IntervalTree::xi_new();
        tree.xi_insert(super::Xi127Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi127Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi127Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi127_interval_tree_overlap() {
        let mut tree = super::Xi127IntervalTree::xi_new();
        tree.xi_insert(super::Xi127Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi127Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi127Interval::xi_new(12, 20));
        let q = super::Xi127Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi127_interval_tree_remove() {
        let mut tree = super::Xi127IntervalTree::xi_new();
        tree.xi_insert(super::Xi127Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi127Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi127_interval_tree_gaps() {
        let mut tree = super::Xi127IntervalTree::xi_new();
        tree.xi_insert(super::Xi127Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi127Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi127Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi127Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi127Interval::xi_new(8, 10));
    }

    #[test]
    fn xi127_interval_tree_merge() {
        let mut tree = super::Xi127IntervalTree::xi_new();
        tree.xi_insert(super::Xi127Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi127Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi127Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi127Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi127Interval::xi_new(10, 15));
    }

    #[test]
    fn xi127_interval_tree_all() {
        let mut tree = super::Xi127IntervalTree::xi_new();
        tree.xi_insert(super::Xi127Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi127Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi127_interval_tree_empty() {
        let tree = super::Xi127IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi127_interval_tree_contains_point() {
        let iv = super::Xi127Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 127) ---

    #[test]
    fn xj_127_uf_make_and_find() {
        let mut uf = super::Xj127UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_127_uf_union_connected() {
        let mut uf = super::Xj127UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_127_uf_component_count() {
        let mut uf = super::Xj127UnionFind::xj_new();
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
    fn xj_127_uf_component_size() {
        let mut uf = super::Xj127UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_127_uf_largest_component() {
        let mut uf = super::Xj127UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_127_uf_many_elements() {
        let mut uf = super::Xj127UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_127_uf_separate_components() {
        let mut uf = super::Xj127UnionFind::xj_new();
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
    fn xj_127_uf_path_compression() {
        let mut uf = super::Xj127UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_127_bt_insert_get() {
        let mut bt = super::Xj127BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_127_bt_contains_len() {
        let mut bt = super::Xj127BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_127_bt_replace() {
        let mut bt = super::Xj127BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_127_bt_remove() {
        let mut bt = super::Xj127BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_127_bt_keys_values() {
        let mut bt = super::Xj127BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_127_bt_range() {
        let mut bt = super::Xj127BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_127_bt_min_max() {
        let mut bt = super::Xj127BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_127_bt_many_inserts() {
        let mut bt = super::Xj127BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_127 segment tree tests ---

    #[test]
    fn xk_127_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk127SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_127_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk127SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_127_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk127SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_127_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk127SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_127_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk127SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_127_st_single_element() {
        let data = vec![42];
        let st = super::Xk127SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_127_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk127SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_127_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk127SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_127 disjoint intervals tests ---

    #[test]
    fn xk_127_di_add_and_count() {
        let mut di = super::Xk127DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_127_di_merge_overlap() {
        let mut di = super::Xk127DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_127_di_contains() {
        let mut di = super::Xk127DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_127_di_remove() {
        let mut di = super::Xk127DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_127_di_covered_length() {
        let mut di = super::Xk127DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_127_di_gaps() {
        let mut di = super::Xk127DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_127_di_merge_adjacent() {
        let mut di = super::Xk127DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_127_di_empty() {
        let di = super::Xk127DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_127_rope_new_empty() {
        let rope = super::Xl127Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_127_rope_from_str() {
        let rope = super::Xl127Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_127_rope_insert_at() {
        let mut rope = super::Xl127Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_127_rope_delete_range() {
        let mut rope = super::Xl127Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_127_rope_char_at() {
        let rope = super::Xl127Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_127_rope_split_concat() {
        let rope = super::Xl127Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_127_rope_line_count() {
        let rope = super::Xl127Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_127_rope_line_at() {
        let rope = super::Xl127Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_127_sa_build_and_search() {
        let sa = super::Xl127SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_127_sa_count() {
        let sa = super::Xl127SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_127_sa_longest_repeated() {
        let sa = super::Xl127SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_127_sa_all_positions() {
        let sa = super::Xl127SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_127_sa_len() {
        let sa = super::Xl127SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_127_sa_empty() {
        let sa = super::Xl127SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_127_rope_slice() {
        let rope = super::Xl127Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_127_sa_search_start() {
        let sa = super::Xl127SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_127_sparse_set_get() {
        let mut m = super::Xm127MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_127_sparse_row_col() {
        let mut m = super::Xm127MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_127_sparse_transpose() {
        let mut m = super::Xm127MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_127_sparse_multiply_vec() {
        let mut m = super::Xm127MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_127_sparse_nnz_density() {
        let mut m = super::Xm127MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_127_sparse_clear() {
        let mut m = super::Xm127MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_127_sparse_overwrite_zero() {
        let mut m = super::Xm127MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_127_tokenizer_basic() {
        let t = super::Xm127Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_127_tokenizer_count() {
        let t = super::Xm127Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_127_tokenizer_unique() {
        let t = super::Xm127Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_127_tokenizer_frequency() {
        let t = super::Xm127Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_127_tokenizer_delimiter() {
        let t = super::Xm127Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_127_tokenizer_whitespace() {
        let t = super::Xm127Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_127_tokenizer_empty() {
        let t = super::Xm127Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 127 ----

    #[test]
    fn xn_127_fenwick_prefix_sum() {
        let mut ft = super::Xn127Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_127_fenwick_range_sum() {
        let mut ft = super::Xn127Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_127_fenwick_point_query() {
        let mut ft = super::Xn127Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_127_fenwick_len() {
        let ft = super::Xn127Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_127_fenwick_multiple_updates() {
        let mut ft = super::Xn127Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_127_fenwick_single_element() {
        let mut ft = super::Xn127Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_127_fenwick_find_kth() {
        let mut ft = super::Xn127Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_127_fenwick_negative_delta() {
        let mut ft = super::Xn127Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 127 ----

    #[test]
    fn xn_127_avl_insert_get() {
        let mut m = super::Xn127AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_127_avl_remove() {
        let mut m = super::Xn127AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_127_avl_in_order() {
        let mut m = super::Xn127AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_127_avl_min_max() {
        let mut m = super::Xn127AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_127_avl_floor_ceiling() {
        let mut m = super::Xn127AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_127_avl_height_balanced() {
        let mut m = super::Xn127AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_127_avl_overwrite() {
        let mut m = super::Xn127AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_127_avl_empty() {
        let m: super::Xn127AVL<i32, i32> = super::Xn127AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo127RedBlack tests ---

    #[test]
    fn xo_127_rb_insert_and_get() {
        let mut tree = super::Xo127RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_127_rb_len_and_empty() {
        let mut tree = super::Xo127RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_127_rb_min_max() {
        let mut tree = super::Xo127RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_127_rb_contains() {
        let mut tree = super::Xo127RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_127_rb_remove() {
        let mut tree = super::Xo127RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_127_rb_in_order() {
        let mut tree = super::Xo127RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_127_rb_black_height() {
        let mut tree = super::Xo127RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_127_rb_overwrite() {
        let mut tree = super::Xo127RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo127ConsistentHash tests ---

    #[test]
    fn xo_127_ch_add_and_count() {
        let mut ring = super::Xo127ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_127_ch_remove_node() {
        let mut ring = super::Xo127ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_127_ch_get_node() {
        let mut ring = super::Xo127ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_127_ch_empty_ring() {
        let ring = super::Xo127ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_127_ch_distribution() {
        let mut ring = super::Xo127ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_127_ch_rebalance() {
        let mut ring = super::Xo127ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_127_ch_virtual_nodes() {
        let mut ring = super::Xo127ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_127_ch_consistent_lookup() {
        let mut ring = super::Xo127ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_127_splay_insert_get() {
        let mut t = super::Xp127SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_127_splay_remove() {
        let mut t = super::Xp127SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_127_splay_count_increases() {
        let mut t = super::Xp127SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_127_splay_depth() {
        let mut t = super::Xp127SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_127_splay_len_empty() {
        let t = super::Xp127SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_127_splay_min_max() {
        let mut t = super::Xp127SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_127_splay_overwrite() {
        let mut t = super::Xp127SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_127_splay_remove_missing() {
        let mut t = super::Xp127SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_127 treap tests ----
    #[test]
    fn xq_127_treap_empty() {
        let t = super::Xq127Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_127_treap_insert_get() {
        let mut t = super::Xq127Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_127_treap_overwrite() {
        let mut t = super::Xq127Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_127_treap_remove() {
        let mut t = super::Xq127Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_127_treap_min_max() {
        let mut t = super::Xq127Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_127_treap_rank() {
        let mut t = super::Xq127Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_127_treap_kth() {
        let mut t = super::Xq127Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_127_treap_in_order() {
        let mut t = super::Xq127Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_127 VEB tree tests ----
    #[test]
    fn xq_127_veb_empty() {
        let v = super::Xq127VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_127_veb_insert_contains() {
        let mut v = super::Xq127VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_127_veb_min_max() {
        let mut v = super::Xq127VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_127_veb_delete() {
        let mut v = super::Xq127VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_127_veb_successor() {
        let mut v = super::Xq127VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_127_veb_predecessor() {
        let mut v = super::Xq127VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_127_veb_count() {
        let mut v = super::Xq127VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_127_veb_duplicate_insert() {
        let mut v = super::Xq127VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_127_kdtree_empty() {
        let tree = super::Xr127KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_127_kdtree_insert_one() {
        let mut tree = super::Xr127KDTree::xr_new();
        tree.xr_insert(super::Xr127KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_127_kdtree_insert_multiple() {
        let mut tree = super::Xr127KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr127KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_127_kdtree_nearest_neighbor() {
        let mut tree = super::Xr127KDTree::xr_new();
        tree.xr_insert(super::Xr127KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr127KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr127KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_127_kdtree_nn_empty() {
        let tree = super::Xr127KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr127KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_127_kdtree_range_search() {
        let mut tree = super::Xr127KDTree::xr_new();
        tree.xr_insert(super::Xr127KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr127KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr127KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_127_kdtree_range_empty() {
        let mut tree = super::Xr127KDTree::xr_new();
        tree.xr_insert(super::Xr127KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_127_kdtree_all_points() {
        let mut tree = super::Xr127KDTree::xr_new();
        tree.xr_insert(super::Xr127KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr127KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_127_kdtree_depth() {
        let mut tree = super::Xr127KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr127KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_127_kdtree_bounding_box() {
        let mut tree = super::Xr127KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr127KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr127KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_127_persistent_array_new() {
        let arr = super::Xs127PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_127_persistent_array_push() {
        let mut arr = super::Xs127PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_127_persistent_array_set() {
        let mut arr = super::Xs127PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_127_persistent_array_diff() {
        let mut arr = super::Xs127PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_127_persistent_array_rollback() {
        let mut arr = super::Xs127PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_127_persistent_array_history() {
        let mut arr = super::Xs127PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_127_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs127PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_127_persistent_array_from_vec() {
        let arr = super::Xs127PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_127_concurrent_queue_new() {
        let q = super::Xs127ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_127_concurrent_queue_push_pop() {
        let mut q = super::Xs127ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_127_concurrent_queue_full() {
        let mut q = super::Xs127ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_127_concurrent_queue_drain() {
        let mut q = super::Xs127ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_127_concurrent_queue_try_pop() {
        let mut q = super::Xs127ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_127_concurrent_queue_clear() {
        let mut q = super::Xs127ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_127_range_map_new() {
        let rm = super::Xs127RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_127_range_map_insert_get() {
        let mut rm = super::Xs127RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_127_range_map_overlap() {
        let mut rm = super::Xs127RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_127_range_map_remove() {
        let mut rm = super::Xs127RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_127_range_map_gaps() {
        let mut rm = super::Xs127RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_127_range_map_coverage() {
        let mut rm = super::Xs127RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_127_range_map_contains() {
        let mut rm = super::Xs127RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_127_range_map_clear() {
        let mut rm = super::Xs127RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_127_circular_buffer_new() {
        let buf = super::Xs127CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_127_circular_buffer_push_pop() {
        let mut buf = super::Xs127CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_127_circular_buffer_overwrite() {
        let mut buf = super::Xs127CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_127_circular_buffer_peek() {
        let mut buf = super::Xs127CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_127_circular_buffer_is_full() {
        let mut buf = super::Xs127CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_127_circular_buffer_iter() {
        let mut buf = super::Xs127CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_127_circular_buffer_clear() {
        let mut buf = super::Xs127CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_127_circular_buffer_to_vec() {
        let mut buf = super::Xs127CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_127_stats_tracker_new() {
        let tracker = super::Xs127StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_127_stats_tracker_mean() {
        let mut tracker = super::Xs127StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_127_stats_tracker_min_max() {
        let mut tracker = super::Xs127StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_127_stats_tracker_median() {
        let mut tracker = super::Xs127StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_127_stats_tracker_variance() {
        let mut tracker = super::Xs127StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_127_stats_tracker_range() {
        let mut tracker = super::Xs127StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_127_stats_tracker_clear() {
        let mut tracker = super::Xs127StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_127_stats_tracker_sum() {
        let mut tracker = super::Xs127StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }


    // --- xy_ Cuckoo Hash Map tests ---

    #[test]
    fn xy_cuckoo_new() {
        let m = super::XyCuckooMap::<String, i32>::xy_new(16);
        assert!(m.xy_is_empty());
        assert_eq!(m.xy_len(), 0);
    }

    #[test]
    fn xy_cuckoo_insert_get() {
        let mut m = super::XyCuckooMap::xy_new(32);
        m.xy_insert("hello".to_string(), 1);
        m.xy_insert("world".to_string(), 2);
        assert_eq!(m.xy_get(&"hello".to_string()), Some(&1));
        assert_eq!(m.xy_get(&"world".to_string()), Some(&2));
        assert_eq!(m.xy_get(&"missing".to_string()), None);
    }

    #[test]
    fn xy_cuckoo_contains() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(42, "a");
        assert!(m.xy_contains(&42));
        assert!(!m.xy_contains(&99));
    }

    #[test]
    fn xy_cuckoo_replace() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(5, "old");
        m.xy_insert(5, "new");
        assert_eq!(m.xy_get(&5), Some(&"new"));
    }

    #[test]
    fn xy_cuckoo_remove() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(10, "val");
        assert_eq!(m.xy_remove(&10), Some("val"));
        assert!(!m.xy_contains(&10));
    }

    #[test]
    fn xy_cuckoo_many() {
        let mut m = super::XyCuckooMap::xy_new(64);
        for i in 0..30 {
            m.xy_insert(i, i * 10);
        }
        assert_eq!(m.xy_len(), 30);
        for i in 0..30 {
            assert!(m.xy_contains(&i));
        }
    }

    #[test]
    fn xy_cuckoo_keys() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_insert(2, "b");
        let keys = m.xy_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn xy_cuckoo_clear() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_clear();
        assert!(m.xy_is_empty());
    }

    #[test]
    fn xy_cuckoo_display() {
        let m = super::XyCuckooMap::<i32, i32>::xy_new(16);
        assert!(format!("{}", m).contains("CuckooMap"));
    }

    #[test]
    fn xy_cuckoo_default() {
        let m = super::XyCuckooMap::<i32, i32>::default();
        assert!(m.xy_is_empty());
    }

    // --- xy_ Count-Min Sketch tests ---

    #[test]
    fn xy_cms_new() {
        let cms = super::XyCountMinSketch::xy_new(100, 5);
        assert_eq!(cms.xy_width(), 100);
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_add_estimate() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..10 { cms.xy_add(42); }
        assert!(cms.xy_estimate(42) >= 10);
    }

    #[test]
    fn xy_cms_add_count() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        cms.xy_add_count(7, 100);
        assert!(cms.xy_estimate(7) >= 100);
    }

    #[test]
    fn xy_cms_unseen() {
        let cms = super::XyCountMinSketch::xy_new(1000, 5);
        assert_eq!(cms.xy_estimate(999), 0);
    }

    #[test]
    fn xy_cms_merge() {
        let mut a = super::XyCountMinSketch::xy_new(100, 3);
        let mut b = super::XyCountMinSketch::xy_new(100, 3);
        a.xy_add(1);
        b.xy_add(1);
        a.xy_merge(&b);
        assert!(a.xy_estimate(1) >= 2);
    }

    #[test]
    fn xy_cms_clear() {
        let mut cms = super::XyCountMinSketch::xy_new(100, 3);
        cms.xy_add(1);
        cms.xy_clear();
        assert_eq!(cms.xy_estimate(1), 0);
    }

    #[test]
    fn xy_cms_display() {
        let cms = super::XyCountMinSketch::xy_new(100, 3);
        assert!(format!("{}", cms).contains("CMS"));
    }

    #[test]
    fn xy_cms_default() {
        let cms = super::XyCountMinSketch::default();
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_multiple_items() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for i in 0..100 { cms.xy_add(i); }
        for i in 0..100 { assert!(cms.xy_estimate(i) >= 1); }
    }

    #[test]
    fn xy_cms_heavy_hitter() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..1000 { cms.xy_add(42); }
        for i in 0..10 { cms.xy_add(i); }
        assert!(cms.xy_estimate(42) > cms.xy_estimate(0));
    }

}
