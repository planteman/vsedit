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

/// Accumulated statistics for interactive operations.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractiveStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl InteractiveStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &InteractiveStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for InteractiveStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InteractiveStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InteractiveStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for interactive.
#[derive(Debug, Clone)]
pub struct InteractiveValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl InteractiveValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for InteractiveValidator {
    fn default() -> Self {
        Self::new()
    }
}
// ---------------------------------------------------------------------------
// Interactive zones (clickable regions)
// ---------------------------------------------------------------------------

/// A clickable/interactive region within a cell's rendered output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveZone {
    pub id: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub tooltip: Option<String>,
    pub action: ZoneAction,
}

/// What happens when a zone is clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneAction {
    /// Navigate to a URL.
    OpenUrl(String),
    /// Execute a command by ID.
    RunCommand(String),
    /// Copy text to clipboard.
    CopyText(String),
    /// No action (purely decorative zone).
    None,
}

impl fmt::Display for ZoneAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZoneAction::OpenUrl(u) => write!(f, "open:{u}"),
            ZoneAction::RunCommand(c) => write!(f, "cmd:{c}"),
            ZoneAction::CopyText(_) => write!(f, "copy"),
            ZoneAction::None => write!(f, "none"),
        }
    }
}

impl InteractiveZone {
    pub fn new(id: impl Into<String>, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            id: id.into(),
            start_line,
            start_col,
            end_line,
            end_col,
            tooltip: None,
            action: ZoneAction::None,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_action(mut self, action: ZoneAction) -> Self {
        self.action = action;
        self
    }

    /// Check if a position (line, col) falls within this zone.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if self.start_line == self.end_line {
            return col >= self.start_col && col <= self.end_col;
        }
        if line == self.start_line {
            return col >= self.start_col;
        }
        if line == self.end_line {
            return col <= self.end_col;
        }
        true
    }

    /// Check if this zone overlaps with another.
    pub fn overlaps(&self, other: &InteractiveZone) -> bool {
        !(self.end_line < other.start_line
            || other.end_line < self.start_line
            || (self.end_line == other.start_line && self.end_col < other.start_col)
            || (other.end_line == self.start_line && other.end_col < self.start_col))
    }
}

// ---------------------------------------------------------------------------
// Cell dependency graph
// ---------------------------------------------------------------------------

/// Tracks which cells depend on other cells (e.g. a code cell referencing
/// a variable defined in an earlier cell).
#[derive(Debug, Clone)]
pub struct CellDependencyGraph {
    /// Map from cell ID to the set of cell IDs it depends on.
    edges: Vec<(String, String)>,
}

impl CellDependencyGraph {
    pub fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Record that `cell_id` depends on `depends_on_id`.
    pub fn add_dependency(&mut self, cell_id: impl Into<String>, depends_on_id: impl Into<String>) {
        let edge = (cell_id.into(), depends_on_id.into());
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }
    }

    /// Remove a specific dependency.
    pub fn remove_dependency(&mut self, cell_id: &str, depends_on_id: &str) {
        self.edges.retain(|(c, d)| !(c == cell_id && d == depends_on_id));
    }

    /// Remove all edges involving a cell (as source or target).
    pub fn remove_cell(&mut self, cell_id: &str) {
        self.edges.retain(|(c, d)| c != cell_id && d != cell_id);
    }

    /// Get the direct dependencies of a cell.
    pub fn dependencies_of(&self, cell_id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|(c, _)| c == cell_id)
            .map(|(_, d)| d.as_str())
            .collect()
    }

    /// Get the cells that directly depend on the given cell.
    pub fn dependents_of(&self, cell_id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|(_, d)| d == cell_id)
            .map(|(c, _)| c.as_str())
            .collect()
    }

    /// Whether the graph has any edges.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Total number of dependency edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Return all unique cell IDs referenced in the graph.
    pub fn all_cell_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.edges.iter()
            .flat_map(|(c, d)| [c.as_str(), d.as_str()])
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

impl Default for CellDependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cell execution plan (topological ordering)
// ---------------------------------------------------------------------------

/// An ordered plan for executing cells respecting dependencies.
#[derive(Debug, Clone)]
pub struct CellExecutionPlan {
    /// Cell IDs in the order they should be executed.
    pub order: Vec<String>,
    /// Cell IDs that could not be scheduled (part of a cycle).
    pub unresolved: Vec<String>,
}

impl CellExecutionPlan {
    /// Build an execution plan from a dependency graph and the list of cell IDs
    /// present in the session. Uses Kahn's algorithm for topological sort.
    pub fn build(graph: &CellDependencyGraph, cell_ids: &[String]) -> Self {
        use std::collections::{HashMap, VecDeque};

        // In-degree map (only for cells in the session).
        let id_set: std::collections::HashSet<&str> = cell_ids.iter().map(|s| s.as_str()).collect();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for id in &id_set {
            in_degree.insert(id, 0);
        }
        for (c, d) in &graph.edges {
            if id_set.contains(c.as_str()) && id_set.contains(d.as_str()) {
                *in_degree.entry(c.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> = VecDeque::new();
        // Seed with zero in-degree nodes, preserving original order for stability.
        for id in cell_ids {
            if in_degree.get(id.as_str()).copied().unwrap_or(0) == 0 {
                queue.push_back(id.as_str());
            }
        }

        let mut order: Vec<String> = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.to_string());
            for (c, d) in &graph.edges {
                if d.as_str() == node && id_set.contains(c.as_str()) {
                    if let Some(deg) = in_degree.get_mut(c.as_str()) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(c.as_str());
                        }
                    }
                }
            }
        }

        let ordered_set: std::collections::HashSet<&str> = order.iter().map(|s| s.as_str()).collect();
        let unresolved: Vec<String> = cell_ids
            .iter()
            .filter(|id| !ordered_set.contains(id.as_str()))
            .cloned()
            .collect();

        Self { order, unresolved }
    }

    /// Whether all cells were successfully scheduled.
    pub fn is_complete(&self) -> bool {
        self.unresolved.is_empty()
    }

    /// Total number of cells in the plan (scheduled + unresolved).
    pub fn total_cells(&self) -> usize {
        self.order.len() + self.unresolved.len()
    }
}

// ---------------------------------------------------------------------------
// Session exporter (Markdown)
// ---------------------------------------------------------------------------

/// Export an `InteractiveSession` to various text formats.
pub struct SessionExporter;

impl SessionExporter {
    /// Export the session to a Markdown string.
    pub fn to_markdown(session: &InteractiveSession) -> String {
        let mut out = String::new();
        for cell in session.get_cells() {
            match cell.kind {
                CellKind::Markup => {
                    out.push_str(&cell.source);
                    out.push_str("\n\n");
                }
                CellKind::Code => {
                    let lang = cell.language.as_deref().unwrap_or("");
                    out.push_str(&format!("```{lang}\n"));
                    out.push_str(&cell.source);
                    out.push_str("\n```\n\n");
                    for o in &cell.outputs {
                        out.push_str("**Output:**\n\n");
                        out.push_str("```\n");
                        out.push_str(&o.data);
                        out.push_str("\n```\n\n");
                    }
                }
            }
        }
        out.truncate(out.trim_end().len());
        out.push('\n');
        out
    }

    /// Export only the source of code cells, separated by blank lines.
    pub fn to_script(session: &InteractiveSession) -> String {
        session
            .get_cells()
            .iter()
            .filter(|c| c.kind == CellKind::Code)
            .map(|c| c.source.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// Cell diff (comparing cell versions)
// ---------------------------------------------------------------------------

/// Represents a single change between two versions of a cell's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    /// Line present in both versions.
    Equal(String),
    /// Line added in the new version.
    Added(String),
    /// Line removed from the old version.
    Removed(String),
}

/// A diff between two cell source strings.
#[derive(Debug, Clone)]
pub struct CellDiff {
    pub ops: Vec<DiffOp>,
}

impl CellDiff {
    /// Compute a simple line-level diff between `old` and `new` source text.
    /// Uses a basic LCS-based approach.
    pub fn compute(old: &str, new: &str) -> Self {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let n = old_lines.len();
        let m = new_lines.len();

        // Build LCS table.
        let mut table = vec![vec![0u32; m + 1]; n + 1];
        for i in 1..=n {
            for j in 1..=m {
                if old_lines[i - 1] == new_lines[j - 1] {
                    table[i][j] = table[i - 1][j - 1] + 1;
                } else {
                    table[i][j] = table[i - 1][j].max(table[i][j - 1]);
                }
            }
        }

        // Backtrack to produce diff ops.
        let mut ops = Vec::new();
        let (mut i, mut j) = (n, m);
        while i > 0 || j > 0 {
            if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
                ops.push(DiffOp::Equal(old_lines[i - 1].to_string()));
                i -= 1;
                j -= 1;
            } else if j > 0 && (i == 0 || table[i][j - 1] >= table[i - 1][j]) {
                ops.push(DiffOp::Added(new_lines[j - 1].to_string()));
                j -= 1;
            } else {
                ops.push(DiffOp::Removed(old_lines[i - 1].to_string()));
                i -= 1;
            }
        }
        ops.reverse();
        Self { ops }
    }

    /// Number of lines added.
    pub fn additions(&self) -> usize {
        self.ops.iter().filter(|o| matches!(o, DiffOp::Added(_))).count()
    }

    /// Number of lines removed.
    pub fn deletions(&self) -> usize {
        self.ops.iter().filter(|o| matches!(o, DiffOp::Removed(_))).count()
    }

    /// Whether the two sources are identical.
    pub fn is_unchanged(&self) -> bool {
        self.ops.iter().all(|o| matches!(o, DiffOp::Equal(_)))
    }

    /// Render the diff in unified-diff-like format.
    pub fn to_string_unified(&self) -> String {
        let mut out = String::new();
        for op in &self.ops {
            match op {
                DiffOp::Equal(l) => {
                    out.push_str(&format!(" {l}\n"));
                }
                DiffOp::Added(l) => {
                    out.push_str(&format!("+{l}\n"));
                }
                DiffOp::Removed(l) => {
                    out.push_str(&format!("-{l}\n"));
                }
            }
        }
        out
    }
}

/// Manages a collection of interactive zones.
pub struct ZoneTracker {
    zones: Vec<InteractiveZone>,
}

impl ZoneTracker {
    pub fn new() -> Self {
        Self { zones: Vec::new() }
    }

    pub fn add(&mut self, zone: InteractiveZone) {
        self.zones.push(zone);
    }

    pub fn hit_test(&self, line: u32, col: u32) -> Option<&InteractiveZone> {
        self.zones.iter().find(|z| z.contains(line, col))
    }

    pub fn zones_on_line(&self, line: u32) -> Vec<&InteractiveZone> {
        self.zones.iter().filter(|z| line >= z.start_line && line <= z.end_line).collect()
    }

    pub fn clear(&mut self) {
        self.zones.clear();
    }

    pub fn count(&self) -> usize {
        self.zones.len()
    }

    pub fn remove_by_id(&mut self, id: &str) {
        self.zones.retain(|z| z.id != id);
    }
}

// ---------------------------------------------------------------------------
// CellSearchResult – structured search results
// ---------------------------------------------------------------------------

/// A single match found within a cell's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSearchMatch {
    /// The cell ID where the match was found.
    pub cell_id: String,
    /// Zero-based line number within the cell source.
    pub line: usize,
    /// Zero-based column where the match starts.
    pub col: usize,
    /// The matched text fragment.
    pub matched_text: String,
}

/// Performs a structured search across all cells in a session.
pub fn search_cells_detailed(session: &InteractiveSession, query: &str) -> Vec<CellSearchMatch> {
    let q = query.to_lowercase();
    let mut results = Vec::new();
    for cell in session.get_cells() {
        for (line_idx, line) in cell.source.lines().enumerate() {
            let lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = lower[start..].find(&q) {
                let col = start + pos;
                let end = col + query.len();
                let matched_text = line[col..end.min(line.len())].to_string();
                results.push(CellSearchMatch {
                    cell_id: cell.id.clone(),
                    line: line_idx,
                    col,
                    matched_text,
                });
                start = col + 1;
            }
        }
    }
    results
}

// ---------------------------------------------------------------------------
// SessionStatistics – aggregate session metrics
// ---------------------------------------------------------------------------

/// Aggregate statistics about an interactive session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatistics {
    pub total_cells: usize,
    pub code_cells: usize,
    pub markup_cells: usize,
    pub total_lines: usize,
    pub total_outputs: usize,
    pub error_cells: usize,
    pub idle_cells: usize,
}

impl SessionStatistics {
    /// Compute statistics from a session.
    pub fn from_session(session: &InteractiveSession) -> Self {
        let cells = session.get_cells();
        let total_cells = cells.len();
        let code_cells = cells.iter().filter(|c| c.kind == CellKind::Code).count();
        let markup_cells = cells.iter().filter(|c| c.kind == CellKind::Markup).count();
        let total_lines: usize = cells.iter().map(|c| c.source_line_count()).sum();
        let total_outputs: usize = cells.iter().map(|c| c.output_count()).sum();
        let error_cells = cells.iter().filter(|c| c.status == CellStatus::Error).count();
        let idle_cells = cells.iter().filter(|c| c.status == CellStatus::Idle).count();
        Self {
            total_cells,
            code_cells,
            markup_cells,
            total_lines,
            total_outputs,
            error_cells,
            idle_cells,
        }
    }

    /// Fraction of cells that are code cells.
    pub fn code_ratio(&self) -> f64 {
        if self.total_cells == 0 {
            return 0.0;
        }
        self.code_cells as f64 / self.total_cells as f64
    }

    /// Average lines per cell.
    pub fn avg_lines_per_cell(&self) -> f64 {
        if self.total_cells == 0 {
            return 0.0;
        }
        self.total_lines as f64 / self.total_cells as f64
    }
}

impl fmt::Display for SessionStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionStats(cells={}, code={}, markup={}, lines={}, outputs={})",
            self.total_cells, self.code_cells, self.markup_cells,
            self.total_lines, self.total_outputs,
        )
    }
}

// ---------------------------------------------------------------------------
// CellLanguageMap – tracks language distribution
// ---------------------------------------------------------------------------

/// Counts how many cells use each programming language.
#[derive(Debug, Clone, Default)]
pub struct CellLanguageMap {
    counts: HashMap<String, usize>,
}

impl CellLanguageMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a language map from a session's cells.
    pub fn from_session(session: &InteractiveSession) -> Self {
        let mut counts = HashMap::new();
        for cell in session.get_cells() {
            if cell.kind == CellKind::Code {
                let lang = cell.language.as_deref().unwrap_or("unknown");
                *counts.entry(lang.to_string()).or_insert(0) += 1;
            }
        }
        Self { counts }
    }

    /// Get the count for a specific language.
    pub fn count(&self, language: &str) -> usize {
        self.counts.get(language).copied().unwrap_or(0)
    }

    /// The most common language (ties broken alphabetically).
    pub fn dominant_language(&self) -> Option<&str> {
        self.counts
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(k, _)| k.as_str())
    }

    /// Number of distinct languages.
    pub fn language_count(&self) -> usize {
        self.counts.len()
    }

    /// All languages sorted alphabetically.
    pub fn languages(&self) -> Vec<&str> {
        let mut langs: Vec<&str> = self.counts.keys().map(|s| s.as_str()).collect();
        langs.sort();
        langs
    }
}

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// InteractiveWidgetGrid – multi-widget layout
// ---------------------------------------------------------------------------

/// A grid layout for interactive widgets.
///
/// Each cell in the grid can hold an optional widget ID.
#[derive(Debug, Clone)]
pub struct InteractiveWidgetGrid {
    /// Number of columns.
    pub cols: usize,
    /// Number of rows.
    pub rows: usize,
    /// Grid cells (row-major order).
    cells: Vec<Option<String>>,
}

impl InteractiveWidgetGrid {
    /// Create a grid with the given dimensions.
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![None; cols * rows],
        }
    }

    /// Place a widget at a specific cell.
    pub fn place(&mut self, col: usize, row: usize, widget_id: impl Into<String>) -> bool {
        if col >= self.cols || row >= self.rows {
            return false;
        }
        self.cells[row * self.cols + col] = Some(widget_id.into());
        true
    }

    /// Remove a widget from a cell.
    pub fn remove(&mut self, col: usize, row: usize) -> Option<String> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        self.cells[row * self.cols + col].take()
    }

    /// Get the widget at a cell.
    pub fn get(&self, col: usize, row: usize) -> Option<&str> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        self.cells[row * self.cols + col].as_deref()
    }

    /// Number of occupied cells.
    pub fn occupied_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// Whether the grid is fully occupied.
    pub fn is_full(&self) -> bool {
        self.occupied_count() == self.cols * self.rows
    }

    /// Total number of cells.
    pub fn total_cells(&self) -> usize {
        self.cols * self.rows
    }

    /// Clear all cells.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = None;
        }
    }

    /// Find the first empty cell (column, row).
    pub fn first_empty(&self) -> Option<(usize, usize)> {
        for row in 0..self.rows {
            for col in 0..self.cols {
                if self.cells[row * self.cols + col].is_none() {
                    return Some((col, row));
                }
            }
        }
        None
    }
}

impl fmt::Display for InteractiveWidgetGrid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WidgetGrid({}x{}, {}/{})",
            self.cols,
            self.rows,
            self.occupied_count(),
            self.total_cells()
        )
    }
}

// ---------------------------------------------------------------------------
// InteractiveFormValidator – field validation rules
// ---------------------------------------------------------------------------

/// Validation rule for a form field.
#[derive(Debug, Clone)]
pub struct FieldRule {
    /// Field name.
    pub field: String,
    /// Validation kind.
    pub kind: ValidationKind,
    /// Error message when validation fails.
    pub message: String,
}

/// Kinds of field validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationKind {
    /// Field must not be empty.
    Required,
    /// Field must have at least N characters.
    MinLength(usize),
    /// Field must have at most N characters.
    MaxLength(usize),
    /// Field must match a pattern (simple prefix/suffix check).
    Pattern(String),
}

/// Result of validating a form field.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Field name.
    pub field: String,
    /// Whether validation passed.
    pub valid: bool,
    /// Error message if invalid.
    pub message: Option<String>,
}

/// Validates form fields against a set of rules.
#[derive(Debug, Clone)]
pub struct InteractiveFormValidator {
    rules: Vec<FieldRule>,
}

impl Default for InteractiveFormValidator {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
}

impl InteractiveFormValidator {
    /// Create a new validator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a validation rule.
    pub fn add_rule(&mut self, field: impl Into<String>, kind: ValidationKind, message: impl Into<String>) {
        self.rules.push(FieldRule {
            field: field.into(),
            kind,
            message: message.into(),
        });
    }

    /// Validate a single field value.
    pub fn validate_field(&self, field: &str, value: &str) -> Vec<ValidationResult> {
        self.rules
            .iter()
            .filter(|r| r.field == field)
            .map(|rule| {
                let valid = match &rule.kind {
                    ValidationKind::Required => !value.is_empty(),
                    ValidationKind::MinLength(n) => value.len() >= *n,
                    ValidationKind::MaxLength(n) => value.len() <= *n,
                    ValidationKind::Pattern(pat) => value.contains(pat.as_str()),
                };
                ValidationResult {
                    field: field.to_string(),
                    valid,
                    message: if valid { None } else { Some(rule.message.clone()) },
                }
            })
            .collect()
    }

    /// Validate all fields in a form (field → value map).
    pub fn validate_all(&self, form: &HashMap<String, String>) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        for rule in &self.rules {
            let value = form.get(&rule.field).map(|s| s.as_str()).unwrap_or("");
            let res = self.validate_field(&rule.field, value);
            results.extend(res);
        }
        results
    }

    /// Whether all fields in the form are valid.
    pub fn is_valid(&self, form: &HashMap<String, String>) -> bool {
        self.validate_all(form).iter().all(|r| r.valid)
    }

    /// Number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

// ---------------------------------------------------------------------------
// Interactive drag-resize handler
// ---------------------------------------------------------------------------

/// Handles drag-to-resize for interactive panels.
#[derive(Debug, Clone)]
pub struct DragResizeHandler {
    /// Minimum width.
    pub min_width: u32,
    /// Maximum width.
    pub max_width: u32,
    /// Minimum height.
    pub min_height: u32,
    /// Maximum height.
    pub max_height: u32,
    /// Current width.
    pub width: u32,
    /// Current height.
    pub height: u32,
}

impl Default for DragResizeHandler {
    fn default() -> Self {
        Self {
            min_width: 100,
            max_width: 2000,
            min_height: 50,
            max_height: 1000,
            width: 400,
            height: 300,
        }
    }
}

impl DragResizeHandler {
    /// Create with default constraints.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Resize by a delta, clamping to constraints.
    pub fn resize(&mut self, delta_width: i32, delta_height: i32) {
        let new_w = (self.width as i32 + delta_width).max(0) as u32;
        let new_h = (self.height as i32 + delta_height).max(0) as u32;
        self.width = new_w.clamp(self.min_width, self.max_width);
        self.height = new_h.clamp(self.min_height, self.max_height);
    }

    /// Set width, clamped.
    pub fn set_width(&mut self, width: u32) {
        self.width = width.clamp(self.min_width, self.max_width);
    }

    /// Set height, clamped.
    pub fn set_height(&mut self, height: u32) {
        self.height = height.clamp(self.min_height, self.max_height);
    }

    /// Current dimensions as (width, height).
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl fmt::Display for DragResizeHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

// ---------------------------------------------------------------------------
// Keyboard-navigable interactive list
// ---------------------------------------------------------------------------

/// A list that supports keyboard navigation (up/down/select).
#[derive(Debug, Clone)]
pub struct NavigableList<T> {
    items: Vec<T>,
    selected_index: Option<usize>,
}

impl<T> Default for NavigableList<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_index: None,
        }
    }
}

impl<T> NavigableList<T> {
    /// Create an empty navigable list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from a vector of items.
    pub fn from_items(items: Vec<T>) -> Self {
        let selected = if items.is_empty() { None } else { Some(0) };
        Self {
            items,
            selected_index: selected,
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx > 0 {
                self.selected_index = Some(idx - 1);
            }
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx + 1 < self.items.len() {
                self.selected_index = Some(idx + 1);
            }
        }
    }

    /// Select first item.
    pub fn select_first(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = Some(0);
        }
    }

    /// Select last item.
    pub fn select_last(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = Some(self.items.len() - 1);
        }
    }

    /// Get the currently selected item.
    pub fn selected(&self) -> Option<&T> {
        self.selected_index.and_then(|idx| self.items.get(idx))
    }

    /// Get the selected index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add an item to the end.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
        if self.selected_index.is_none() {
            self.selected_index = Some(0);
        }
    }

    /// Get all items.
    pub fn items(&self) -> &[T] {
        &self.items
    }
}


// ── Interactive Cell Renderer ──

/// Rendering options for cell display.
#[derive(Debug, Clone)]
pub struct CellRenderOptions {
    pub show_line_numbers: bool,
    pub max_output_lines: usize,
    pub indent_size: usize,
    pub wrap_width: Option<usize>,
    pub show_status_indicator: bool,
}

impl Default for CellRenderOptions {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            max_output_lines: 50,
            indent_size: 4,
            wrap_width: None,
            show_status_indicator: true,
        }
    }
}

/// A rendered line with optional decorations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLine {
    pub line_number: Option<u32>,
    pub content: String,
    pub is_continuation: bool,
}

impl RenderedLine {
    pub fn new(number: Option<u32>, content: impl Into<String>) -> Self {
        Self {
            line_number: number,
            content: content.into(),
            is_continuation: false,
        }
    }

    pub fn continuation(content: impl Into<String>) -> Self {
        Self {
            line_number: None,
            content: content.into(),
            is_continuation: true,
        }
    }
}

impl fmt::Display for RenderedLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(n) = self.line_number {
            write!(f, "{:>4} | {}", n, self.content)
        } else if self.is_continuation {
            write!(f, "     | {}", self.content)
        } else {
            write!(f, "       {}", self.content)
        }
    }
}

/// Formats cells for display in the interactive editor.
pub struct InteractiveCellRenderer {
    options: CellRenderOptions,
}

impl InteractiveCellRenderer {
    pub fn new() -> Self {
        Self {
            options: CellRenderOptions::default(),
        }
    }

    pub fn with_options(options: CellRenderOptions) -> Self {
        Self { options }
    }

    /// Render the source code lines of a cell.
    pub fn render_source(&self, source: &str) -> Vec<RenderedLine> {
        let mut lines = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let num = if self.options.show_line_numbers {
                Some((i + 1) as u32)
            } else {
                None
            };
            if let Some(wrap_width) = self.options.wrap_width {
                if line.len() > wrap_width {
                    let mut start = 0;
                    let mut first = true;
                    while start < line.len() {
                        let end = (start + wrap_width).min(line.len());
                        let chunk = &line[start..end];
                        if first {
                            lines.push(RenderedLine::new(num, chunk));
                            first = false;
                        } else {
                            lines.push(RenderedLine::continuation(chunk));
                        }
                        start = end;
                    }
                    continue;
                }
            }
            lines.push(RenderedLine::new(num, line));
        }
        lines
    }

    /// Render the output of a cell, truncating if needed.
    pub fn render_output(&self, output: &str) -> Vec<RenderedLine> {
        let all_lines: Vec<&str> = output.lines().collect();
        let truncated = all_lines.len() > self.options.max_output_lines;
        let visible = if truncated {
            &all_lines[..self.options.max_output_lines]
        } else {
            &all_lines
        };
        let mut rendered: Vec<RenderedLine> = visible
            .iter()
            .map(|l| RenderedLine::new(None, *l))
            .collect();
        if truncated {
            let hidden = all_lines.len() - self.options.max_output_lines;
            rendered.push(RenderedLine::new(
                None,
                format!("... ({} more lines)", hidden),
            ));
        }
        rendered
    }

    /// Render a status indicator string for a cell.
    pub fn render_status_indicator(&self, status: CellStatus) -> &str {
        if !self.options.show_status_indicator {
            return "";
        }
        match status {
            CellStatus::Idle => "[ ]",
            CellStatus::Running => "[*]",
            CellStatus::Success => "[✓]",
            CellStatus::Error => "[✗]",
        }
    }

    /// Render a full cell (source + output) as a string.
    pub fn render_cell(&self, kind: CellKind, source: &str, output: Option<&str>, status: CellStatus) -> String {
        let mut result = String::new();
        result.push_str(&format!("--- {} {} ---\n", kind, self.render_status_indicator(status)));
        for line in self.render_source(source) {
            result.push_str(&format!("{}\n", line));
        }
        if let Some(out) = output {
            result.push_str("--- Output ---\n");
            for line in self.render_output(out) {
                result.push_str(&format!("{}\n", line));
            }
        }
        result
    }
}

// ── Interactive Output Formatter ──

/// Formats output text with ANSI stripping, truncation, and normalization.
pub struct InteractiveOutputFormatter {
    max_length: usize,
    strip_ansi: bool,
    normalize_newlines: bool,
    trim_trailing: bool,
}

impl InteractiveOutputFormatter {
    pub fn new(max_length: usize) -> Self {
        Self {
            max_length,
            strip_ansi: true,
            normalize_newlines: true,
            trim_trailing: true,
        }
    }

    pub fn with_strip_ansi(mut self, strip: bool) -> Self {
        self.strip_ansi = strip;
        self
    }

    pub fn with_normalize_newlines(mut self, normalize: bool) -> Self {
        self.normalize_newlines = normalize;
        self
    }

    pub fn with_trim_trailing(mut self, trim: bool) -> Self {
        self.trim_trailing = trim;
        self
    }

    /// Format the given output text according to configured rules.
    pub fn format(&self, text: &str) -> String {
        let mut result = text.to_string();
        if self.strip_ansi {
            result = Self::strip_ansi_codes(&result);
        }
        if self.normalize_newlines {
            result = result.replace("\r\n", "\n").replace('\r', "\n");
        }
        if self.trim_trailing {
            result = result.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
        }
        if result.len() > self.max_length {
            result.truncate(self.max_length);
            result.push_str("...");
        }
        result
    }

    /// Strip ANSI escape codes from text.
    pub fn strip_ansi_codes(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Skip until we find a letter (end of ANSI sequence)
                if chars.peek() == Some(&'[') {
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            } else {
                result.push(ch);
            }
        }
        result
    }

    /// Count the number of visible characters (excluding ANSI codes).
    pub fn visible_length(text: &str) -> usize {
        Self::strip_ansi_codes(text).len()
    }

    /// Split text into chunks of at most `chunk_size` visible characters.
    pub fn chunk_output(text: &str, chunk_size: usize) -> Vec<String> {
        if chunk_size == 0 {
            return vec![text.to_string()];
        }
        let clean = Self::strip_ansi_codes(text);
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < clean.len() {
            let end = (start + chunk_size).min(clean.len());
            chunks.push(clean[start..end].to_string());
            start = end;
        }
        if chunks.is_empty() {
            chunks.push(String::new());
        }
        chunks
    }
}



// ---------------------------------------------------------------------------
// vsedit-interactive: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl InteractiveXConfig {
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

impl std::fmt::Display for InteractiveXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct InteractiveXRegistry {
    entries: Vec<InteractiveXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl InteractiveXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: InteractiveXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&InteractiveXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut InteractiveXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<InteractiveXConfig> {
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

    pub fn active_entries(&self) -> Vec<&InteractiveXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&InteractiveXConfig> {
        let mut sorted: Vec<&InteractiveXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&InteractiveXConfig> {
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

    pub fn iter(&self) -> InteractiveXIterator<'_> {
        InteractiveXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct InteractiveXIterator<'a> {
    inner: std::slice::Iter<'a, InteractiveXConfig>,
}

impl<'a> Iterator for InteractiveXIterator<'a> {
    type Item = &'a InteractiveXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct InteractiveXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl InteractiveXCache {
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
pub struct InteractiveXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl InteractiveXFormatter {
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

    pub fn format_entry(&self, entry: &InteractiveXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &InteractiveXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &InteractiveXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for InteractiveXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct InteractiveXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl InteractiveXValidator {
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

    pub fn validate(&self, entry: &InteractiveXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &InteractiveXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for InteractiveXValidator {
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
// xb_ utilities – batch 99
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer99 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer99 {
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
pub fn xb_fnv1a_99(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_99<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_99<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_99(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_99(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 98
// ---------------------------------------------------------------------------

/// Generic object pool `Xc98Pool<T>`.
pub struct Xc98Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc98Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc98PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc98Pool<T> {
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
    pub fn stats(&self) -> Xc98PoolStats {
        Xc98PoolStats {
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

impl<T> Default for Xc98Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc98Scheduler`.
pub struct Xc98Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc98Scheduler {
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

impl Default for Xc98Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_98 hash for the given byte slice.
pub fn xc_98_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_98 convention.
pub fn xc_98_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe112 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe112Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe112PipelineError {
    pub stage: Xe112Stage,
    pub message: String,
}

impl std::fmt::Display for Xe112PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe112Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe112Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError>>>,
    stage_names: Vec<Xe112Stage>,
}

impl Xe112Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe112Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe112Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe112Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe112Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> {
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

    pub fn compose(mut self, other: Xe112Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe112CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe112CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe112Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe112CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe112CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe112Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe112CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_112_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe112CacheEntry {
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

    fn xe_112_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe112CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_112_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> {
    Ok(data)
}

pub fn xe_112_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_112_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_112_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_112_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe112PipelineError> {
    Err(Xe112PipelineError {
        stage: Xe112Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_110: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg110Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg110Graph {
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

impl Default for Xg110Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_110: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg110Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg110Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg110Heap<T>) {
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

impl<T: Ord> Default for Xg110Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 97).
pub struct Xh97SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh97SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 139 as u64,
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

/// A compact bit set supporting boolean operations (variant 97).
pub struct Xh97BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh97BitSet {
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

    #[test]
    fn interactive_stats_new_defaults() {
        let stats = InteractiveStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn interactive_stats_record_success() {
        let mut stats = InteractiveStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn interactive_stats_record_failure() {
        let mut stats = InteractiveStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn interactive_stats_reset() {
        let mut stats = InteractiveStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn interactive_stats_merge() {
        let mut a = InteractiveStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = InteractiveStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn interactive_stats_display() {
        let mut stats = InteractiveStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn interactive_stats_default() {
        let stats = InteractiveStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn interactive_validator_accepts_valid_name() {
        let v = InteractiveValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn interactive_validator_rejects_empty() {
        let v = InteractiveValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn interactive_validator_rejects_too_long() {
        let v = InteractiveValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn interactive_validator_forbidden_prefix() {
        let v = InteractiveValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn interactive_validator_allowed_chars() {
        let v = InteractiveValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn interactive_validator_range() {
        let v = InteractiveValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn interactive_sanitize_removes_control() {
        let result = InteractiveValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn interactive_truncate_short_string() {
        assert_eq!(InteractiveValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn interactive_truncate_long_string() {
        let result = InteractiveValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn interactive_is_ascii_printable() {
        assert!(InteractiveValidator::is_ascii_printable("Hello World 123"));
        assert!(!InteractiveValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- InteractiveZone --

    #[test]
    fn zone_contains_single_line() {
        let z = InteractiveZone::new("z1", 5, 10, 5, 20);
        assert!(z.contains(5, 10));
        assert!(z.contains(5, 15));
        assert!(z.contains(5, 20));
        assert!(!z.contains(5, 9));
        assert!(!z.contains(5, 21));
        assert!(!z.contains(4, 15));
    }

    #[test]
    fn zone_contains_multi_line() {
        let z = InteractiveZone::new("z1", 3, 5, 6, 10);
        assert!(z.contains(3, 5));
        assert!(z.contains(4, 0));
        assert!(z.contains(6, 10));
        assert!(!z.contains(6, 11));
        assert!(!z.contains(2, 5));
    }

    #[test]
    fn zone_tracker_hit_test() {
        let mut tracker = ZoneTracker::new();
        tracker.add(InteractiveZone::new("a", 1, 0, 1, 10));
        tracker.add(InteractiveZone::new("b", 3, 0, 3, 5));
        assert_eq!(tracker.hit_test(1, 5).unwrap().id, "a");
        assert_eq!(tracker.hit_test(3, 3).unwrap().id, "b");
        assert!(tracker.hit_test(2, 0).is_none());
    }

    #[test]
    fn zone_tracker_zones_on_line() {
        let mut tracker = ZoneTracker::new();
        tracker.add(InteractiveZone::new("a", 1, 0, 2, 10));
        tracker.add(InteractiveZone::new("b", 2, 5, 2, 15));
        let on_2 = tracker.zones_on_line(2);
        assert_eq!(on_2.len(), 2);
    }

    #[test]
    fn zone_tracker_remove_by_id() {
        let mut tracker = ZoneTracker::new();
        tracker.add(InteractiveZone::new("a", 1, 0, 1, 10));
        tracker.add(InteractiveZone::new("b", 2, 0, 2, 10));
        tracker.remove_by_id("a");
        assert_eq!(tracker.count(), 1);
        assert!(tracker.hit_test(1, 5).is_none());
    }

    #[test]
    fn zone_overlaps_detection() {
        let z1 = InteractiveZone::new("a", 1, 0, 1, 10);
        let z2 = InteractiveZone::new("b", 1, 5, 1, 15);
        let z3 = InteractiveZone::new("c", 2, 0, 2, 5);
        assert!(z1.overlaps(&z2));
        assert!(!z1.overlaps(&z3));
    }

    #[test]
    fn zone_action_display() {
        assert_eq!(format!("{}", ZoneAction::OpenUrl("http://x".into())), "open:http://x");
        assert_eq!(format!("{}", ZoneAction::RunCommand("copy".into())), "cmd:copy");
        assert_eq!(format!("{}", ZoneAction::None), "none");
    }

    // -- CellDependencyGraph --

    #[test]
    fn dependency_graph_basic_operations() {
        let mut g = CellDependencyGraph::new();
        assert!(g.is_empty());
        g.add_dependency("c2", "c1");
        g.add_dependency("c3", "c1");
        g.add_dependency("c3", "c2");
        assert_eq!(g.edge_count(), 3);
        assert_eq!(g.dependencies_of("c3"), vec!["c1", "c2"]);
        assert_eq!(g.dependents_of("c1"), vec!["c2", "c3"]);

        // duplicate edge is ignored
        g.add_dependency("c2", "c1");
        assert_eq!(g.edge_count(), 3);

        g.remove_dependency("c3", "c2");
        assert_eq!(g.edge_count(), 2);
        assert_eq!(g.dependencies_of("c3"), vec!["c1"]);

        g.remove_cell("c1");
        assert!(g.is_empty());
    }

    // -- CellExecutionPlan --

    #[test]
    fn execution_plan_topological_order() {
        let mut g = CellDependencyGraph::new();
        // c2 depends on c1, c3 depends on c2
        g.add_dependency("c2", "c1");
        g.add_dependency("c3", "c2");
        let ids: Vec<String> = vec!["c1".into(), "c2".into(), "c3".into()];
        let plan = CellExecutionPlan::build(&g, &ids);
        assert!(plan.is_complete());
        assert_eq!(plan.order, vec!["c1", "c2", "c3"]);
    }

    #[test]
    fn execution_plan_detects_cycle() {
        let mut g = CellDependencyGraph::new();
        g.add_dependency("a", "b");
        g.add_dependency("b", "a");
        let ids: Vec<String> = vec!["a".into(), "b".into()];
        let plan = CellExecutionPlan::build(&g, &ids);
        assert!(!plan.is_complete());
        assert_eq!(plan.unresolved.len(), 2);
        assert_eq!(plan.total_cells(), 2);
    }

    // -- SessionExporter --

    #[test]
    fn session_exporter_markdown() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Markup, "# Title", None);
        let code_id = s.add_cell(CellKind::Code, "print(1)", Some("python".into()));
        s.add_output(&code_id, CellOutput::plain("1"));

        let md = SessionExporter::to_markdown(&s);
        assert!(md.contains("# Title"));
        assert!(md.contains("```python"));
        assert!(md.contains("print(1)"));
        assert!(md.contains("**Output:**"));
        assert!(md.contains("1"));
    }

    #[test]
    fn session_exporter_script() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "x = 1", None);
        s.add_cell(CellKind::Markup, "# ignored", None);
        s.add_cell(CellKind::Code, "y = 2", None);
        let script = SessionExporter::to_script(&s);
        assert_eq!(script, "x = 1\n\ny = 2");
    }

    // -- CellDiff --

    #[test]
    fn cell_diff_identical_sources() {
        let diff = CellDiff::compute("a\nb\nc", "a\nb\nc");
        assert!(diff.is_unchanged());
        assert_eq!(diff.additions(), 0);
        assert_eq!(diff.deletions(), 0);
    }

    #[test]
    fn cell_diff_detects_changes() {
        let diff = CellDiff::compute("line1\nline2\nline3", "line1\nchanged\nline3");
        assert!(!diff.is_unchanged());
        assert_eq!(diff.additions(), 1);
        assert_eq!(diff.deletions(), 1);
        let unified = diff.to_string_unified();
        assert!(unified.contains("+changed"));
        assert!(unified.contains("-line2"));
        assert!(unified.contains(" line1"));
    }

    #[test]
    fn search_cells_detailed_finds_matches() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "let x = 42;\nlet y = x + 1;", Some("rust".into()));
        s.add_cell(CellKind::Code, "println!(\"x is {}\", x);", Some("rust".into()));
        let results = search_cells_detailed(&s, "x");
        assert!(results.len() >= 3);
        assert!(results.iter().all(|r| r.matched_text == "x"));
    }

    #[test]
    fn search_cells_detailed_case_insensitive() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "Hello WORLD", None);
        let results = search_cells_detailed(&s, "hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "Hello");
    }

    #[test]
    fn session_statistics_basic() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "line1\nline2", Some("python".into()));
        s.add_cell(CellKind::Markup, "# Title", None);
        s.add_cell(CellKind::Code, "x = 1", Some("python".into()));
        let stats = SessionStatistics::from_session(&s);
        assert_eq!(stats.total_cells, 3);
        assert_eq!(stats.code_cells, 2);
        assert_eq!(stats.markup_cells, 1);
        assert_eq!(stats.total_lines, 4); // 2 + 1 + 1
        assert!((stats.code_ratio() - 2.0 / 3.0).abs() < 0.01);
        let display = format!("{stats}");
        assert!(display.contains("SessionStats"));
    }

    #[test]
    fn session_statistics_empty() {
        let s = InteractiveSession::new();
        let stats = SessionStatistics::from_session(&s);
        assert_eq!(stats.total_cells, 0);
        assert_eq!(stats.code_ratio(), 0.0);
        assert_eq!(stats.avg_lines_per_cell(), 0.0);
    }

    #[test]
    fn cell_language_map_from_session() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "x = 1", Some("python".into()));
        s.add_cell(CellKind::Code, "let x = 1;", Some("rust".into()));
        s.add_cell(CellKind::Code, "y = 2", Some("python".into()));
        s.add_cell(CellKind::Markup, "# Header", None);
        let map = CellLanguageMap::from_session(&s);
        assert_eq!(map.count("python"), 2);
        assert_eq!(map.count("rust"), 1);
        assert_eq!(map.count("java"), 0);
        assert_eq!(map.dominant_language(), Some("python"));
        assert_eq!(map.language_count(), 2);
        let langs = map.languages();
        assert_eq!(langs, vec!["python", "rust"]);
    }

    #[test]
    fn cell_language_map_unknown_language() {
        let mut s = InteractiveSession::new();
        s.add_cell(CellKind::Code, "code", None);
        let map = CellLanguageMap::from_session(&s);
        assert_eq!(map.count("unknown"), 1);
        assert_eq!(map.dominant_language(), Some("unknown"));
    }

    // -- InteractiveWidgetGrid tests --

    #[test]
    fn grid_place_and_get() {
        let mut g = InteractiveWidgetGrid::new(3, 3);
        assert!(g.place(0, 0, "widget_a"));
        assert_eq!(g.get(0, 0), Some("widget_a"));
        assert_eq!(g.get(1, 1), None);
    }

    #[test]
    fn grid_remove() {
        let mut g = InteractiveWidgetGrid::new(2, 2);
        g.place(0, 0, "w");
        assert_eq!(g.remove(0, 0), Some("w".into()));
        assert_eq!(g.occupied_count(), 0);
    }

    #[test]
    fn grid_out_of_bounds() {
        let mut g = InteractiveWidgetGrid::new(2, 2);
        assert!(!g.place(5, 5, "oob"));
        assert_eq!(g.get(5, 5), None);
    }

    #[test]
    fn grid_first_empty() {
        let mut g = InteractiveWidgetGrid::new(2, 2);
        g.place(0, 0, "a");
        assert_eq!(g.first_empty(), Some((1, 0)));
    }

    #[test]
    fn grid_full() {
        let mut g = InteractiveWidgetGrid::new(1, 1);
        g.place(0, 0, "a");
        assert!(g.is_full());
    }

    // -- InteractiveFormValidator tests --

    #[test]
    fn validator_required() {
        let mut v = InteractiveFormValidator::new();
        v.add_rule("name", ValidationKind::Required, "Name is required");
        let results = v.validate_field("name", "");
        assert!(!results[0].valid);
        let results = v.validate_field("name", "Alice");
        assert!(results[0].valid);
    }

    #[test]
    fn validator_min_length() {
        let mut v = InteractiveFormValidator::new();
        v.add_rule("password", ValidationKind::MinLength(8), "Too short");
        let results = v.validate_field("password", "abc");
        assert!(!results[0].valid);
        let results = v.validate_field("password", "abcdefgh");
        assert!(results[0].valid);
    }

    #[test]
    fn validator_max_length() {
        let mut v = InteractiveFormValidator::new();
        v.add_rule("name", ValidationKind::MaxLength(5), "Too long");
        let results = v.validate_field("name", "abcdef");
        assert!(!results[0].valid);
    }

    #[test]
    fn validator_is_valid() {
        let mut v = InteractiveFormValidator::new();
        v.add_rule("name", ValidationKind::Required, "Required");
        let mut form = HashMap::new();
        form.insert("name".into(), "Alice".into());
        assert!(v.is_valid(&form));
        form.insert("name".into(), "".into());
        assert!(!v.is_valid(&form));
    }

    // -- DragResizeHandler tests --

    #[test]
    fn drag_resize() {
        let mut h = DragResizeHandler::new(400, 300);
        h.resize(100, -50);
        assert_eq!(h.dimensions(), (500, 250));
    }

    #[test]
    fn drag_resize_clamp() {
        let mut h = DragResizeHandler::new(400, 300);
        h.resize(-500, -500);
        assert_eq!(h.width, h.min_width);
        assert_eq!(h.height, h.min_height);
    }

    // -- NavigableList tests --

    #[test]
    fn navigable_list_navigation() {
        let mut list = NavigableList::from_items(vec!["a", "b", "c"]);
        assert_eq!(list.selected(), Some(&"a"));
        list.move_down();
        assert_eq!(list.selected(), Some(&"b"));
        list.move_down();
        list.move_down(); // should stay at end
        assert_eq!(list.selected(), Some(&"c"));
        list.move_up();
        assert_eq!(list.selected(), Some(&"b"));
    }

    #[test]
    fn navigable_list_select_first_last() {
        let mut list = NavigableList::from_items(vec![1, 2, 3]);
        list.select_last();
        assert_eq!(list.selected(), Some(&3));
        list.select_first();
        assert_eq!(list.selected(), Some(&1));
    }

    #[test]
    fn navigable_list_empty() {
        let list: NavigableList<i32> = NavigableList::new();
        assert!(list.is_empty());
        assert!(list.selected().is_none());
    }

    #[test]
    fn navigable_list_push() {
        let mut list = NavigableList::new();
        list.push("a");
        assert_eq!(list.selected(), Some(&"a"));
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn rendered_line_display_with_number() {
        let line = RenderedLine::new(Some(1), "hello");
        assert_eq!(format!("{}", line), "   1 | hello");
    }

    #[test]
    fn rendered_line_display_continuation() {
        let line = RenderedLine::continuation("continued");
        assert_eq!(format!("{}", line), "     | continued");
    }

    #[test]
    fn cell_renderer_source_basic() {
        let renderer = InteractiveCellRenderer::new();
        let lines = renderer.render_source("line1\nline2");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_number, Some(1));
        assert_eq!(lines[1].line_number, Some(2));
    }

    #[test]
    fn cell_renderer_source_no_line_numbers() {
        let opts = CellRenderOptions {
            show_line_numbers: false,
            ..Default::default()
        };
        let renderer = InteractiveCellRenderer::with_options(opts);
        let lines = renderer.render_source("hello");
        assert_eq!(lines[0].line_number, None);
    }

    #[test]
    fn cell_renderer_source_wrapping() {
        let opts = CellRenderOptions {
            wrap_width: Some(5),
            ..Default::default()
        };
        let renderer = InteractiveCellRenderer::with_options(opts);
        let lines = renderer.render_source("1234567890");
        assert_eq!(lines.len(), 2);
        assert!(lines[1].is_continuation);
    }

    #[test]
    fn cell_renderer_output_truncation() {
        let opts = CellRenderOptions {
            max_output_lines: 2,
            ..Default::default()
        };
        let renderer = InteractiveCellRenderer::with_options(opts);
        let lines = renderer.render_output("a\nb\nc\nd");
        assert_eq!(lines.len(), 3); // 2 visible + 1 truncation msg
        assert!(lines[2].content.contains("2 more lines"));
    }

    #[test]
    fn cell_renderer_status_indicator() {
        let renderer = InteractiveCellRenderer::new();
        assert_eq!(renderer.render_status_indicator(CellStatus::Idle), "[ ]");
        assert_eq!(renderer.render_status_indicator(CellStatus::Running), "[*]");
        assert_eq!(renderer.render_status_indicator(CellStatus::Success), "[✓]");
        assert_eq!(renderer.render_status_indicator(CellStatus::Error), "[✗]");
    }

    #[test]
    fn cell_renderer_full_cell() {
        let renderer = InteractiveCellRenderer::new();
        let result = renderer.render_cell(CellKind::Code, "print(1)", Some("1"), CellStatus::Success);
        assert!(result.contains("Code"));
        assert!(result.contains("print(1)"));
        assert!(result.contains("Output"));
    }

    #[test]
    fn output_formatter_strip_ansi() {
        let text = "\x1b[31mred\x1b[0m normal";
        let clean = InteractiveOutputFormatter::strip_ansi_codes(text);
        assert_eq!(clean, "red normal");
    }

    #[test]
    fn output_formatter_format() {
        let fmt = InteractiveOutputFormatter::new(100);
        let result = fmt.format("hello  \r\nworld  ");
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn output_formatter_truncate() {
        let fmt = InteractiveOutputFormatter::new(5);
        let result = fmt.format("hello world");
        assert_eq!(result, "hello...");
    }

    #[test]
    fn output_formatter_chunk() {
        let chunks = InteractiveOutputFormatter::chunk_output("abcdefghij", 3);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0], "abc");
        assert_eq!(chunks[3], "j");
    }

    #[test]
    fn output_formatter_visible_length() {
        let len = InteractiveOutputFormatter::visible_length("\x1b[32mhi\x1b[0m");
        assert_eq!(len, 2);
    }



    #[test]
    fn interactive_x_config_new() {
        let c = InteractiveXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn interactive_x_config_builder() {
        let c = InteractiveXConfig::new("k")
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
    fn interactive_x_config_display() {
        let c = InteractiveXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn interactive_x_registry_insert_get() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn interactive_x_registry_duplicate() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a")).unwrap();
        assert!(reg.insert(InteractiveXConfig::new("a")).is_err());
    }

    #[test]
    fn interactive_x_registry_remove() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a")).unwrap();
        reg.insert(InteractiveXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn interactive_x_registry_active_entries() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a")).unwrap();
        reg.insert(InteractiveXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn interactive_x_registry_by_weight() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(InteractiveXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn interactive_x_registry_tags() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(InteractiveXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn interactive_x_registry_total_weight() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(InteractiveXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn interactive_x_registry_iterator() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a")).unwrap();
        reg.insert(InteractiveXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn interactive_x_cache_put_get() {
        let mut cache = InteractiveXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn interactive_x_cache_eviction() {
        let mut cache = InteractiveXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn interactive_x_cache_lru_order() {
        let mut cache = InteractiveXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn interactive_x_cache_most_least_recent() {
        let mut cache = InteractiveXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn interactive_x_formatter_entry() {
        let e = InteractiveXConfig::new("k").with_value("v");
        let fmt = InteractiveXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn interactive_x_formatter_summary() {
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("a").with_weight(5)).unwrap();
        let fmt = InteractiveXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn interactive_x_validator_valid() {
        let v = InteractiveXValidator::new();
        let c = InteractiveXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn interactive_x_validator_empty_key() {
        let v = InteractiveXValidator::new();
        let c = InteractiveXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn interactive_x_validator_require_value() {
        let v = InteractiveXValidator::new().require_value(true);
        let c = InteractiveXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn interactive_x_validator_allowed_tags() {
        let v = InteractiveXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = InteractiveXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn interactive_x_validator_validate_all() {
        let v = InteractiveXValidator::new();
        let mut reg = InteractiveXRegistry::new();
        reg.insert(InteractiveXConfig::new("ok")).unwrap();
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
    fn xb_ring_buffer_99_push_and_len() {
        let mut rb = super::XbRingBuffer99::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_99_overwrite() {
        let mut rb = super::XbRingBuffer99::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_99_get_out_of_bounds() {
        let rb = super::XbRingBuffer99::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_99_drain_all() {
        let mut rb = super::XbRingBuffer99::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_99_peek_front_back() {
        let mut rb = super::XbRingBuffer99::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_99_clear() {
        let mut rb = super::XbRingBuffer99::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_99_capacity() {
        let rb = super::XbRingBuffer99::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_99_basic() {
        let h = super::xb_fnv1a_99(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_99(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_99_different_inputs() {
        let h1 = super::xb_fnv1a_99(b"abc");
        let h2 = super::xb_fnv1a_99(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_99_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_99(&data);
        let dec = super::xb_rle_decode_99(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_99_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_99(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_99(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_99_values() {
        assert!((super::xb_clamp_99(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_99(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_99(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_99_values() {
        assert!((super::xb_lerp_99(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_99(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_99(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_99_wrap_around_twice() {
        let mut rb = super::XbRingBuffer99::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 98 ----

    #[test]
    fn xc_98_pool_new_empty() {
        let pool: super::Xc98Pool<i32> = super::Xc98Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_98_pool_release_acquire() {
        let mut pool = super::Xc98Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_98_pool_acquire_empty() {
        let mut pool: super::Xc98Pool<i32> = super::Xc98Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_98_pool_full() {
        let mut pool = super::Xc98Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_98_pool_drain() {
        let mut pool = super::Xc98Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_98_pool_stats() {
        let mut pool = super::Xc98Pool::new(8);
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
    fn xc_98_pool_clear() {
        let mut pool = super::Xc98Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_98_pool_shrink() {
        let mut pool = super::Xc98Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_98_pool_default() {
        let pool: super::Xc98Pool<String> = super::Xc98Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_98_pool_extend() {
        let mut pool = super::Xc98Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_98_pool_retain() {
        let mut pool = super::Xc98Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_98_scheduler_round_robin() {
        let mut sched = super::Xc98Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_98_scheduler_empty() {
        let mut sched = super::Xc98Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_98_scheduler_reset() {
        let mut sched = super::Xc98Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_98_scheduler_add_remove() {
        let mut sched = super::Xc98Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_98_scheduler_targets() {
        let sched = super::Xc98Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_98_hash_empty() {
        assert_eq!(super::xc_98_hash(b""), 5381);
    }

    #[test]
    fn xc_98_hash_data() {
        let h = super::xc_98_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_98_hash(b"hello"), h);
    }

    #[test]
    fn xc_98_reverse_str() {
        assert_eq!(super::xc_98_reverse("abc"), "cba");
        assert_eq!(super::xc_98_reverse(""), "");
    }


    #[test]
    fn xe_112_pipeline_empty() {
        let p = super::Xe112Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_112_pipeline_parse_stage() {
        let p = super::Xe112Pipeline::new()
            .add_parse(super::xe_112_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_112_pipeline_transform_double() {
        let p = super::Xe112Pipeline::new()
            .add_transform(super::xe_112_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_112_pipeline_validate_reverse() {
        let p = super::Xe112Pipeline::new()
            .add_validate(super::xe_112_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_112_pipeline_emit_filter() {
        let p = super::Xe112Pipeline::new()
            .add_emit(super::xe_112_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_112_pipeline_multi_stage() {
        let p = super::Xe112Pipeline::new()
            .add_parse(super::xe_112_pipeline_identity)
            .add_transform(super::xe_112_pipeline_double)
            .add_validate(super::xe_112_pipeline_reverse)
            .add_emit(super::xe_112_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_112_pipeline_error_propagation() {
        let p = super::Xe112Pipeline::new()
            .add_parse(super::xe_112_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe112Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_112_pipeline_compose() {
        let p1 = super::Xe112Pipeline::new()
            .add_parse(super::xe_112_pipeline_identity);
        let p2 = super::Xe112Pipeline::new()
            .add_transform(super::xe_112_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_112_pipeline_error_display() {
        let e = super::Xe112PipelineError {
            stage: super::Xe112Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_112_cache_put_get() {
        let mut c = super::Xe112Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_112_cache_miss() {
        let mut c: super::Xe112Cache<&str, i32> = super::Xe112Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_112_cache_ttl_expiry() {
        let mut c = super::Xe112Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_112_cache_evict() {
        let mut c = super::Xe112Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_112_cache_capacity() {
        let mut c = super::Xe112Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_112_cache_stats() {
        let mut c = super::Xe112Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_112_cache_clear() {
        let mut c = super::Xe112Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_110 graph tests ------------------------------------------------

    #[test]
    fn xg_110_graph_empty() {
        let g = super::Xg110Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_110_graph_add_node() {
        let mut g = super::Xg110Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_110_graph_add_edge() {
        let mut g = super::Xg110Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_110_graph_neighbors() {
        let mut g = super::Xg110Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_110_graph_has_path() {
        let mut g = super::Xg110Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_110_graph_self_path() {
        let g = super::Xg110Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_110_graph_topo_sort() {
        let mut g = super::Xg110Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_110_graph_cycle_detect_false() {
        let mut g = super::Xg110Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_110_graph_cycle_detect_true() {
        let mut g = super::Xg110Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_110 heap tests -------------------------------------------------

    #[test]
    fn xg_110_heap_empty() {
        let h: super::Xg110Heap<i32> = super::Xg110Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_110_heap_push_pop() {
        let mut h = super::Xg110Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_110_heap_peek() {
        let mut h = super::Xg110Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_110_heap_drain_sorted() {
        let mut h = super::Xg110Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_110_heap_merge() {
        let mut a = super::Xg110Heap::new();
        let mut b = super::Xg110Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_110_heap_default() {
        let h: super::Xg110Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_110_graph_default() {
        let g: super::Xg110Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh97_skip_insert_contains() {
        let mut sl = super::Xh97SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh97_skip_remove() {
        let mut sl = super::Xh97SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh97_skip_len() {
        let mut sl = super::Xh97SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh97_skip_range_query() {
        let mut sl = super::Xh97SkipList::xh_new(4);
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
    fn xh97_skip_floor_ceiling() {
        let mut sl = super::Xh97SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh97_skip_rank() {
        let mut sl = super::Xh97SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh97_skip_empty() {
        let sl = super::Xh97SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh97_skip_duplicates() {
        let mut sl = super::Xh97SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh97_bitset_set_test() {
        let mut bs = super::Xh97BitSet::xh_new(256);
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
    fn xh97_bitset_clear_count() {
        let mut bs = super::Xh97BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh97_bitset_and_or_xor() {
        let mut a = super::Xh97BitSet::xh_new(128);
        let mut b = super::Xh97BitSet::xh_new(128);
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
    fn xh97_bitset_iter_ones() {
        let mut bs = super::Xh97BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh97_bitset_first_last() {
        let mut bs = super::Xh97BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh97_bitset_empty() {
        let bs = super::Xh97BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
