//! Interactive editor (notebook-like cells).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Code,
    Markup,
}

impl fmt::Display for CellKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellKind::Code => write!(f, "Code"),
            CellKind::Markup => write!(f, "Markup"),
        }
    }
}

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
            CellStatus::Idle => write!(f, "Idle"),
            CellStatus::Running => write!(f, "Running"),
            CellStatus::Success => write!(f, "Success"),
            CellStatus::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CellOutput {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct Cell {
    pub id: String,
    pub kind: CellKind,
    pub source: String,
    pub language: Option<String>,
    pub outputs: Vec<CellOutput>,
    pub status: CellStatus,
}

impl Cell {
    pub fn has_outputs(&self) -> bool {
        !self.outputs.is_empty()
    }

    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }
}

pub struct InteractiveSession {
    cells: Vec<Cell>,
    next_cell_id: u64,
}

impl InteractiveSession {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next_cell_id: 0,
        }
    }

    pub fn add_cell(
        &mut self,
        kind: CellKind,
        source: impl Into<String>,
        language: Option<String>,
    ) -> String {
        let id = format!("cell-{}", self.next_cell_id);
        self.next_cell_id += 1;
        self.cells.push(Cell {
            id: id.clone(),
            kind,
            source: source.into(),
            language,
            outputs: Vec::new(),
            status: CellStatus::Idle,
        });
        id
    }

    pub fn remove_cell(&mut self, id: &str) -> bool {
        let len = self.cells.len();
        self.cells.retain(|c| c.id != id);
        self.cells.len() < len
    }

    pub fn move_cell(&mut self, id: &str, new_index: usize) {
        if let Some(pos) = self.cells.iter().position(|c| c.id == id) {
            let cell = self.cells.remove(pos);
            let idx = new_index.min(self.cells.len());
            self.cells.insert(idx, cell);
        }
    }

    /// Placeholder execution — marks the cell with a simple output.
    pub fn execute_cell(&mut self, id: &str) {
        if let Some(cell) = self.cells.iter_mut().find(|c| c.id == id) {
            cell.outputs.push(CellOutput {
                mime_type: "text/plain".into(),
                data: format!("Executed: {}", cell.source),
            });
        }
    }

    pub fn get_cell(&self, id: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.id == id)
    }

    pub fn get_cell_mut(&mut self, id: &str) -> Option<&mut Cell> {
        self.cells.iter_mut().find(|c| c.id == id)
    }

    pub fn update_source(&mut self, id: &str, source: &str) -> bool {
        if let Some(cell) = self.get_cell_mut(id) {
            cell.source = source.to_string();
            true
        } else {
            false
        }
    }

    pub fn clear_outputs(&mut self, id: &str) {
        if let Some(cell) = self.get_cell_mut(id) {
            cell.outputs.clear();
        }
    }

    pub fn add_output(&mut self, id: &str, output: CellOutput) {
        if let Some(cell) = self.get_cell_mut(id) {
            cell.outputs.push(output);
        }
    }

    pub fn get_code_cells(&self) -> Vec<&Cell> {
        self.cells.iter().filter(|c| c.kind == CellKind::Code).collect()
    }

    pub fn get_markup_cells(&self) -> Vec<&Cell> {
        self.cells.iter().filter(|c| c.kind == CellKind::Markup).collect()
    }

    pub fn clear_all_outputs(&mut self) {
        for cell in &mut self.cells {
            cell.outputs.clear();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn set_cell_status(&mut self, id: &str, status: CellStatus) {
        if let Some(cell) = self.get_cell_mut(id) {
            cell.status = status;
        }
    }

    pub fn get_cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }
}

impl Default for InteractiveSession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional interactive session utilities
// ---------------------------------------------------------------------------

impl CellOutput {
    /// Create a new plain text output.
    pub fn plain(data: impl Into<String>) -> Self {
        Self { mime_type: "text/plain".into(), data: data.into() }
    }

    /// Create a new HTML output.
    pub fn html(data: impl Into<String>) -> Self {
        Self { mime_type: "text/html".into(), data: data.into() }
    }

    /// Whether this output is plain text.
    pub fn is_plain_text(&self) -> bool {
        self.mime_type == "text/plain"
    }

    /// Whether this output is HTML.
    pub fn is_html(&self) -> bool {
        self.mime_type == "text/html"
    }

    /// Byte size of the output data.
    pub fn data_size(&self) -> usize {
        self.data.len()
    }
}

impl PartialEq for CellOutput {
    fn eq(&self, other: &Self) -> bool {
        self.mime_type == other.mime_type && self.data == other.data
    }
}

impl Eq for CellOutput {}

impl Cell {
    /// The number of lines in the source code.
    pub fn source_line_count(&self) -> usize {
        if self.source.is_empty() { 0 } else { self.source.lines().count().max(1) }
    }

    /// Word count of the source.
    pub fn source_word_count(&self) -> usize {
        self.source.split_whitespace().count()
    }

    /// Total byte size of all outputs.
    pub fn total_output_size(&self) -> usize {
        self.outputs.iter().map(|o| o.data_size()).sum()
    }

    /// Whether the cell has completed execution (success or error).
    pub fn is_finished(&self) -> bool {
        matches!(self.status, CellStatus::Success | CellStatus::Error)
    }

    /// Whether the cell is currently running.
    pub fn is_running(&self) -> bool {
        self.status == CellStatus::Running
    }

    /// Get all plain text outputs.
    pub fn plain_text_outputs(&self) -> Vec<&CellOutput> {
        self.outputs.iter().filter(|o| o.is_plain_text()).collect()
    }
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Cell {}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} ({}) - {} outputs",
            self.id, self.kind, self.status, self.outputs.len()
        )
    }
}

/// A snapshot of the session for undo/redo support.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub cells: Vec<Cell>,
    pub description: String,
}

impl InteractiveSession {
    /// Take a snapshot of the current session state.
    pub fn snapshot(&self, description: impl Into<String>) -> SessionSnapshot {
        SessionSnapshot {
            cells: self.cells.clone(),
            description: description.into(),
        }
    }

    /// Restore from a snapshot.
    pub fn restore(&mut self, snapshot: SessionSnapshot) {
        self.cells = snapshot.cells;
    }

    /// Swap the position of two cells by their IDs.
    pub fn swap_cells(&mut self, id_a: &str, id_b: &str) -> bool {
        let pos_a = self.cells.iter().position(|c| c.id == id_a);
        let pos_b = self.cells.iter().position(|c| c.id == id_b);
        if let (Some(a), Some(b)) = (pos_a, pos_b) {
            self.cells.swap(a, b);
            true
        } else {
            false
        }
    }

    /// Insert a cell at a specific position.
    pub fn insert_cell_at(
        &mut self,
        index: usize,
        kind: CellKind,
        source: impl Into<String>,
        language: Option<String>,
    ) -> String {
        let id = format!("cell-{}", self.next_cell_id);
        self.next_cell_id += 1;
        let idx = index.min(self.cells.len());
        self.cells.insert(idx, Cell {
            id: id.clone(),
            kind,
            source: source.into(),
            language,
            outputs: Vec::new(),
            status: CellStatus::Idle,
        });
        id
    }

    /// Duplicate a cell, inserting the copy right after the original.
    pub fn duplicate_cell(&mut self, id: &str) -> Option<String> {
        let pos = self.cells.iter().position(|c| c.id == id)?;
        let mut clone = self.cells[pos].clone();
        let new_id = format!("cell-{}", self.next_cell_id);
        self.next_cell_id += 1;
        clone.id = new_id.clone();
        clone.outputs.clear();
        clone.status = CellStatus::Idle;
        self.cells.insert(pos + 1, clone);
        Some(new_id)
    }

    /// Get the index of a cell by ID.
    pub fn cell_index(&self, id: &str) -> Option<usize> {
        self.cells.iter().position(|c| c.id == id)
    }

    /// Count cells that have completed with errors.
    pub fn error_count(&self) -> usize {
        self.cells.iter().filter(|c| c.status == CellStatus::Error).count()
    }

    /// Count cells currently running.
    pub fn running_count(&self) -> usize {
        self.cells.iter().filter(|c| c.status == CellStatus::Running).count()
    }

    /// Total number of outputs across all cells.
    pub fn total_output_count(&self) -> usize {
        self.cells.iter().map(|c| c.output_count()).sum()
    }

    /// Execute all code cells in order.
    pub fn execute_all(&mut self) {
        let code_ids: Vec<String> = self.cells
            .iter()
            .filter(|c| c.kind == CellKind::Code)
            .map(|c| c.id.clone())
            .collect();
        for id in code_ids {
            self.execute_cell(&id);
        }
    }

    /// Concatenate all cell sources (code cells only) with newlines.
    pub fn concatenated_source(&self) -> String {
        self.cells
            .iter()
            .filter(|c| c.kind == CellKind::Code)
            .map(|c| c.source.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Find cells whose source contains the given substring.
    pub fn search_cells(&self, query: &str) -> Vec<&Cell> {
        let q = query.to_lowercase();
        self.cells.iter().filter(|c| c.source.to_lowercase().contains(&q)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_cells() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "print(1)", Some("python".into()));
        assert_eq!(s.cell_count(), 1);
        assert!(s.remove_cell(&id));
        assert_eq!(s.cell_count(), 0);
        assert!(!s.remove_cell("nonexistent"));
    }

    #[test]
    fn move_cell_reorders() {
        let mut s = InteractiveSession::new();
        let a = s.add_cell(CellKind::Code, "a", None);
        let _b = s.add_cell(CellKind::Code, "b", None);
        let _c = s.add_cell(CellKind::Code, "c", None);
        s.move_cell(&a, 2);
        assert_eq!(s.get_cells()[0].source, "b");
        assert_eq!(s.get_cells()[1].source, "c");
        assert_eq!(s.get_cells()[2].source, "a");
    }

    #[test]
    fn execute_cell_adds_output() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Markup, "# Hello", None);
        s.execute_cell(&id);
        let cell = &s.get_cells()[0];
        assert_eq!(cell.outputs.len(), 1);
        assert_eq!(cell.outputs[0].mime_type, "text/plain");
        assert!(cell.outputs[0].data.contains("# Hello"));
    }

    #[test]
    fn get_cell_by_id() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "x = 1", None);
        assert!(s.get_cell(&id).is_some());
        assert_eq!(s.get_cell(&id).unwrap().source, "x = 1");
        assert!(s.get_cell("nonexistent").is_none());
    }

    #[test]
    fn update_source_changes_content() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "old", None);
        assert!(s.update_source(&id, "new"));
        assert_eq!(s.get_cell(&id).unwrap().source, "new");
        assert!(!s.update_source("bad-id", "nope"));
    }

    #[test]
    fn clear_and_add_outputs() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "1+1", None);
        s.add_output(&id, CellOutput { mime_type: "text/plain".into(), data: "2".into() });
        assert!(s.get_cell(&id).unwrap().has_outputs());
        assert_eq!(s.get_cell(&id).unwrap().output_count(), 1);
        s.clear_outputs(&id);
        assert!(!s.get_cell(&id).unwrap().has_outputs());
    }

    #[test]
    fn filter_cells_by_kind() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "a", None);
        s.add_cell(CellKind::Markup, "b", None);
        s.add_cell(CellKind::Code, "c", None);
        assert_eq!(s.get_code_cells().len(), 2);
        assert_eq!(s.get_markup_cells().len(), 1);
    }

    #[test]
    fn cell_status_lifecycle() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "run", None);
        assert_eq!(s.get_cell(&id).unwrap().status, CellStatus::Idle);
        s.set_cell_status(&id, CellStatus::Running);
        assert_eq!(s.get_cell(&id).unwrap().status, CellStatus::Running);
        s.set_cell_status(&id, CellStatus::Success);
        assert_eq!(s.get_cell(&id).unwrap().status, CellStatus::Success);
    }

    #[test]
    fn is_empty_and_clear_all_outputs() {
        let mut s = InteractiveSession::new();
        assert!(s.is_empty());
        let a = s.add_cell(CellKind::Code, "a", None);
        let b = s.add_cell(CellKind::Markup, "b", None);
        assert!(!s.is_empty());
        s.execute_cell(&a);
        s.execute_cell(&b);
        assert!(s.get_cell(&a).unwrap().has_outputs());
        s.clear_all_outputs();
        assert!(!s.get_cell(&a).unwrap().has_outputs());
        assert!(!s.get_cell(&b).unwrap().has_outputs());
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", CellKind::Code), "Code");
        assert_eq!(format!("{}", CellKind::Markup), "Markup");
        assert_eq!(format!("{}", CellStatus::Idle), "Idle");
        assert_eq!(format!("{}", CellStatus::Running), "Running");
        assert_eq!(format!("{}", CellStatus::Success), "Success");
        assert_eq!(format!("{}", CellStatus::Error), "Error");
    }

    #[test]
    fn cell_output_plain_and_html() {
        let p = CellOutput::plain("hello");
        assert!(p.is_plain_text());
        assert!(!p.is_html());
        let h = CellOutput::html("<b>hi</b>");
        assert!(h.is_html());
        assert!(!h.is_plain_text());
    }

    #[test]
    fn cell_output_data_size() {
        let o = CellOutput::plain("hello");
        assert_eq!(o.data_size(), 5);
    }

    #[test]
    fn cell_output_equality() {
        let a = CellOutput::plain("x");
        let b = CellOutput::plain("x");
        let c = CellOutput::plain("y");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn cell_source_line_count() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "a\nb\nc", None);
        assert_eq!(s.get_cell(&id).unwrap().source_line_count(), 3);
    }

    #[test]
    fn cell_source_word_count() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "let x = 42", None);
        assert_eq!(s.get_cell(&id).unwrap().source_word_count(), 4);
    }

    #[test]
    fn cell_is_finished_and_running() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "x", None);
        assert!(!s.get_cell(&id).unwrap().is_finished());
        assert!(!s.get_cell(&id).unwrap().is_running());
        s.set_cell_status(&id, CellStatus::Running);
        assert!(s.get_cell(&id).unwrap().is_running());
        s.set_cell_status(&id, CellStatus::Success);
        assert!(s.get_cell(&id).unwrap().is_finished());
    }

    #[test]
    fn cell_display() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "x", None);
        let cell = s.get_cell(&id).unwrap();
        let display = format!("{cell}");
        assert!(display.contains("cell-0"));
        assert!(display.contains("Code"));
    }

    #[test]
    fn swap_cells() {
        let mut s = InteractiveSession::new();
        let a = s.add_cell(CellKind::Code, "a", None);
        let b = s.add_cell(CellKind::Code, "b", None);
        assert!(s.swap_cells(&a, &b));
        assert_eq!(s.get_cells()[0].source, "b");
        assert_eq!(s.get_cells()[1].source, "a");
        assert!(!s.swap_cells("bad", &b));
    }

    #[test]
    fn insert_cell_at_position() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "a", None);
        s.add_cell(CellKind::Code, "c", None);
        s.insert_cell_at(1, CellKind::Code, "b", None);
        assert_eq!(s.get_cells()[1].source, "b");
    }

    #[test]
    fn duplicate_cell() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "original", Some("python".into()));
        s.execute_cell(&id);
        let dup_id = s.duplicate_cell(&id).unwrap();
        assert_eq!(s.cell_count(), 2);
        let dup = s.get_cell(&dup_id).unwrap();
        assert_eq!(dup.source, "original");
        assert!(dup.outputs.is_empty());
        assert_eq!(dup.status, CellStatus::Idle);
    }

    #[test]
    fn duplicate_cell_not_found() {
        let mut s = InteractiveSession::new();
        assert!(s.duplicate_cell("nope").is_none());
    }

    #[test]
    fn cell_index() {
        let mut s = InteractiveSession::new();
        let a = s.add_cell(CellKind::Code, "a", None);
        let b = s.add_cell(CellKind::Code, "b", None);
        assert_eq!(s.cell_index(&a), Some(0));
        assert_eq!(s.cell_index(&b), Some(1));
        assert_eq!(s.cell_index("nope"), None);
    }

    #[test]
    fn error_and_running_counts() {
        let mut s = InteractiveSession::new();
        let a = s.add_cell(CellKind::Code, "a", None);
        let b = s.add_cell(CellKind::Code, "b", None);
        let c = s.add_cell(CellKind::Code, "c", None);
        s.set_cell_status(&a, CellStatus::Error);
        s.set_cell_status(&b, CellStatus::Running);
        assert_eq!(s.error_count(), 1);
        assert_eq!(s.running_count(), 1);
        let _ = c;
    }

    #[test]
    fn total_output_count() {
        let mut s = InteractiveSession::new();
        let a = s.add_cell(CellKind::Code, "a", None);
        let b = s.add_cell(CellKind::Code, "b", None);
        s.execute_cell(&a);
        s.execute_cell(&b);
        assert_eq!(s.total_output_count(), 2);
    }

    #[test]
    fn execute_all_code_cells() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "x", None);
        s.add_cell(CellKind::Markup, "# doc", None);
        s.add_cell(CellKind::Code, "y", None);
        s.execute_all();
        assert_eq!(s.total_output_count(), 2);
    }

    #[test]
    fn concatenated_source() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "a = 1", None);
        s.add_cell(CellKind::Markup, "# note", None);
        s.add_cell(CellKind::Code, "b = 2", None);
        assert_eq!(s.concatenated_source(), "a = 1\nb = 2");
    }

    #[test]
    fn search_cells_by_content() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "let x = 42", None);
        s.add_cell(CellKind::Code, "let y = 99", None);
        s.add_cell(CellKind::Markup, "# X marks the spot", None);
        assert_eq!(s.search_cells("x").len(), 2);
        assert_eq!(s.search_cells("99").len(), 1);
    }

    #[test]
    fn snapshot_and_restore() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "original", None);
        let snap = s.snapshot("before edit");
        s.update_source("cell-0", "modified");
        assert_eq!(s.get_cell("cell-0").unwrap().source, "modified");
        s.restore(snap);
        assert_eq!(s.get_cell("cell-0").unwrap().source, "original");
    }

    #[test]
    fn cell_plain_text_outputs() {
        let mut s = InteractiveSession::new();
        let id = s.add_cell(CellKind::Code, "x", None);
        s.add_output(&id, CellOutput::plain("text"));
        s.add_output(&id, CellOutput::html("<b>bold</b>"));
        let cell = s.get_cell(&id).unwrap();
        assert_eq!(cell.plain_text_outputs().len(), 1);
    }
}
