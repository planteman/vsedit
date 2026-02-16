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
}
