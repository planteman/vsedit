//! Interactive editor (notebook-like cells).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Code,
    Markup,
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
}
