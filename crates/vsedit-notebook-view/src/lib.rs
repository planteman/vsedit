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
}
