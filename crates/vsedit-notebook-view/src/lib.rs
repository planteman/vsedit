//! Notebook editor.

use std::collections::HashMap;
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
}
