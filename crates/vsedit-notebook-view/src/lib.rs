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


}
