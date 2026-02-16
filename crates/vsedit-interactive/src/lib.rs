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
}
