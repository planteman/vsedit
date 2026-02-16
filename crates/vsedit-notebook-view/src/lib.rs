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
}
