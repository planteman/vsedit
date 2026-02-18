//! Editor services – coordination layer for managing open editors.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    EditorNotFound(usize),
    NoActiveEditor,
    IndexOutOfBounds { index: usize, len: usize },
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorError::EditorNotFound(idx) => write!(f, "editor not found at index {idx}"),
            EditorError::NoActiveEditor => write!(f, "no active editor"),
            EditorError::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds (len {len})")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Visual,
    Command,
}

impl fmt::Display for EditorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorMode::Normal => write!(f, "NORMAL"),
            EditorMode::Insert => write!(f, "INSERT"),
            EditorMode::Visual => write!(f, "VISUAL"),
            EditorMode::Command => write!(f, "COMMAND"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub uri: Option<String>,
    pub line: u32,
    pub column: u32,
    pub mode: EditorMode,
    pub dirty: bool,
    pub language_id: Option<String>,
}

impl EditorState {
    /// Returns the filename portion of the uri, or `"untitled"` if no uri is set.
    pub fn display_name(&self) -> &str {
        match &self.uri {
            Some(uri) => uri.rsplit('/').next().unwrap_or("untitled"),
            None => "untitled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorEvent {
    Opened(String),
    Closed(String),
    Changed(String),
    Saved(String),
    SelectionChanged,
    CursorMoved,
}

impl fmt::Display for EditorEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorEvent::Opened(uri) => write!(f, "opened: {uri}"),
            EditorEvent::Closed(uri) => write!(f, "closed: {uri}"),
            EditorEvent::Changed(uri) => write!(f, "changed: {uri}"),
            EditorEvent::Saved(uri) => write!(f, "saved: {uri}"),
            EditorEvent::SelectionChanged => write!(f, "selection changed"),
            EditorEvent::CursorMoved => write!(f, "cursor moved"),
        }
    }
}

pub struct EditorService {
    active_editors: Vec<EditorState>,
    active_index: Option<usize>,
}

impl EditorService {
    pub fn new() -> Self {
        Self {
            active_editors: Vec::new(),
            active_index: None,
        }
    }

    pub fn open_editor(&mut self, uri: &str, language_id: Option<&str>) -> usize {
        let state = EditorState {
            uri: Some(uri.to_string()),
            line: 0,
            column: 0,
            mode: EditorMode::Normal,
            dirty: false,
            language_id: language_id.map(|s| s.to_string()),
        };
        self.active_editors.push(state);
        let index = self.active_editors.len() - 1;
        self.active_index = Some(index);
        index
    }

    pub fn close_editor(&mut self, index: usize) {
        if index < self.active_editors.len() {
            self.active_editors.remove(index);
            // Adjust active_index after removal.
            if self.active_editors.is_empty() {
                self.active_index = None;
            } else if let Some(active) = self.active_index {
                if active == index {
                    self.active_index = if self.active_editors.is_empty() {
                        None
                    } else {
                        Some(active.min(self.active_editors.len() - 1))
                    };
                } else if active > index {
                    self.active_index = Some(active - 1);
                }
            }
        }
    }

    pub fn get_active(&self) -> Option<&EditorState> {
        self.active_index.and_then(|i| self.active_editors.get(i))
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.active_editors.len() {
            self.active_index = Some(index);
        }
    }

    pub fn mark_dirty(&mut self, index: usize) {
        if let Some(editor) = self.active_editors.get_mut(index) {
            editor.dirty = true;
        }
    }

    pub fn mark_clean(&mut self, index: usize) {
        if let Some(editor) = self.active_editors.get_mut(index) {
            editor.dirty = false;
        }
    }

    pub fn editor_count(&self) -> usize {
        self.active_editors.len()
    }

    pub fn get_editor(&self, index: usize) -> Option<&EditorState> {
        self.active_editors.get(index)
    }

    pub fn find_by_uri(&self, uri: &str) -> Option<usize> {
        self.active_editors
            .iter()
            .position(|e| e.uri.as_deref() == Some(uri))
    }

    pub fn dirty_editors(&self) -> Vec<usize> {
        self.active_editors
            .iter()
            .enumerate()
            .filter(|(_, e)| e.dirty)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn close_all(&mut self) {
        self.active_editors.clear();
        self.active_index = None;
    }

    pub fn set_mode(&mut self, mode: EditorMode) -> Result<(), EditorError> {
        let idx = self.active_index.ok_or(EditorError::NoActiveEditor)?;
        self.active_editors[idx].mode = mode;
        Ok(())
    }

    pub fn move_cursor(&mut self, line: u32, column: u32) -> Result<(), EditorError> {
        let idx = self.active_index.ok_or(EditorError::NoActiveEditor)?;
        self.active_editors[idx].line = line;
        self.active_editors[idx].column = column;
        Ok(())
    }

    pub fn next_editor(&mut self) -> Option<usize> {
        if self.active_editors.is_empty() {
            return None;
        }
        let next = match self.active_index {
            Some(i) => (i + 1) % self.active_editors.len(),
            None => 0,
        };
        self.active_index = Some(next);
        Some(next)
    }

    pub fn prev_editor(&mut self) -> Option<usize> {
        if self.active_editors.is_empty() {
            return None;
        }
        let prev = match self.active_index {
            Some(0) => self.active_editors.len() - 1,
            Some(i) => i - 1,
            None => self.active_editors.len() - 1,
        };
        self.active_index = Some(prev);
        Some(prev)
    }

    /// Returns true if active_editors is empty.
    pub fn is_active_editors_empty(&self) -> bool {
        self.active_editors.is_empty()
    }

    /// Get the first active_editor, if any.
    pub fn first_active_editor(&self) -> Option<&EditorState> {
        self.active_editors.first()
    }

    /// Get the last active_editor, if any.
    pub fn last_active_editor(&self) -> Option<&EditorState> {
        self.active_editors.last()
    }

    /// Retain only active_editors matching the predicate.
    pub fn retain_active_editors(&mut self, f: impl Fn(&EditorState) -> bool) {
        self.active_editors.retain(|item| f(item));
    }
}

impl Default for EditorService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for editor-services operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorServicesStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl EditorServicesStats {
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
    pub fn merge(&mut self, other: &EditorServicesStats) {
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

impl Default for EditorServicesStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EditorServicesStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EditorServicesStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for editor-services.
#[derive(Debug, Clone)]
pub struct EditorServicesValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl EditorServicesValidator {
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

impl Default for EditorServicesValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorDecorationService
// ---------------------------------------------------------------------------

/// A text decoration type (e.g., underline, highlight, error squiggly).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecorationType {
    Highlight,
    Underline,
    ErrorSquiggly,
    WarningSquiggly,
    InfoSquiggly,
    Custom(String),
}

/// A single decoration applied to a range in an editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDecoration {
    pub id: u64,
    pub decoration_type: DecorationType,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub tooltip: Option<String>,
}

/// Service for managing decorations across editors.
pub struct EditorDecorationService {
    decorations: Vec<EditorDecoration>,
    next_id: u64,
}

impl EditorDecorationService {
    pub fn new() -> Self {
        Self {
            decorations: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a decoration and return its assigned ID.
    pub fn add_decoration(
        &mut self,
        decoration_type: DecorationType,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.decorations.push(EditorDecoration {
            id,
            decoration_type,
            start_line,
            start_col,
            end_line,
            end_col,
            tooltip: None,
        });
        id
    }

    /// Add a decoration with a tooltip.
    pub fn add_decoration_with_tooltip(
        &mut self,
        decoration_type: DecorationType,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
        tooltip: impl Into<String>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.decorations.push(EditorDecoration {
            id,
            decoration_type,
            start_line,
            start_col,
            end_line,
            end_col,
            tooltip: Some(tooltip.into()),
        });
        id
    }

    /// Remove a decoration by ID.
    pub fn remove_decoration(&mut self, id: u64) -> bool {
        let len = self.decorations.len();
        self.decorations.retain(|d| d.id != id);
        self.decorations.len() != len
    }

    /// Get all decorations on a specific line.
    pub fn decorations_on_line(&self, line: u32) -> Vec<&EditorDecoration> {
        self.decorations
            .iter()
            .filter(|d| d.start_line <= line && d.end_line >= line)
            .collect()
    }

    /// Get all decorations of a specific type.
    pub fn decorations_of_type(&self, decoration_type: &DecorationType) -> Vec<&EditorDecoration> {
        self.decorations
            .iter()
            .filter(|d| &d.decoration_type == decoration_type)
            .collect()
    }

    /// Remove all decorations of a specific type.
    pub fn clear_type(&mut self, decoration_type: &DecorationType) {
        self.decorations.retain(|d| &d.decoration_type != decoration_type);
    }

    /// Total number of active decorations.
    pub fn count(&self) -> usize {
        self.decorations.len()
    }

    /// Remove all decorations.
    pub fn clear_all(&mut self) {
        self.decorations.clear();
    }
}

impl Default for EditorDecorationService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorSnapshotService
// ---------------------------------------------------------------------------

/// A snapshot of a document at a point in time.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub id: u64,
    pub uri: String,
    pub content: String,
    pub version: u64,
    pub line_count: usize,
}

impl DocumentSnapshot {
    /// Get a specific line from the snapshot (0-based).
    pub fn get_line(&self, line: usize) -> Option<&str> {
        self.content.lines().nth(line)
    }

    /// Compute a simple checksum of the content.
    pub fn checksum(&self) -> u64 {
        let mut hash: u64 = 0;
        for byte in self.content.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }
}

/// Service for creating and managing document snapshots.
pub struct EditorSnapshotService {
    snapshots: Vec<DocumentSnapshot>,
    next_id: u64,
}

impl EditorSnapshotService {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            next_id: 1,
        }
    }

    /// Create a snapshot of the given content.
    pub fn create_snapshot(&mut self, uri: &str, content: &str, version: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let line_count = content.lines().count();
        self.snapshots.push(DocumentSnapshot {
            id,
            uri: uri.to_string(),
            content: content.to_string(),
            version,
            line_count,
        });
        id
    }

    /// Retrieve a snapshot by ID.
    pub fn get_snapshot(&self, id: u64) -> Option<&DocumentSnapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// Get the latest snapshot for a given URI.
    pub fn latest_for_uri(&self, uri: &str) -> Option<&DocumentSnapshot> {
        self.snapshots
            .iter()
            .filter(|s| s.uri == uri)
            .max_by_key(|s| s.version)
    }

    /// Remove a snapshot by ID.
    pub fn remove_snapshot(&mut self, id: u64) -> bool {
        let len = self.snapshots.len();
        self.snapshots.retain(|s| s.id != id);
        self.snapshots.len() != len
    }

    /// Number of stored snapshots.
    pub fn count(&self) -> usize {
        self.snapshots.len()
    }
}

impl Default for EditorSnapshotService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorDiffService
// ---------------------------------------------------------------------------

/// A single diff chunk between two versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffChunk {
    Equal(String),
    Added(String),
    Removed(String),
}

/// Compute a line-level diff between two strings.
pub struct EditorDiffService;

impl EditorDiffService {
    /// Compute line-by-line diff between `old` and `new` text.
    /// Uses a simple LCS-based approach.
    pub fn diff_lines(old: &str, new: &str) -> Vec<DiffChunk> {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        let mut chunks = Vec::new();
        let mut old_idx = 0;
        let mut new_idx = 0;

        while old_idx < old_lines.len() && new_idx < new_lines.len() {
            if old_lines[old_idx] == new_lines[new_idx] {
                chunks.push(DiffChunk::Equal(old_lines[old_idx].to_string()));
                old_idx += 1;
                new_idx += 1;
            } else {
                // Look ahead for a match in new
                let new_match = new_lines[new_idx..]
                    .iter()
                    .position(|l| old_idx < old_lines.len() && *l == old_lines[old_idx]);
                let old_match = old_lines[old_idx..]
                    .iter()
                    .position(|l| new_idx < new_lines.len() && *l == new_lines[new_idx]);

                match (new_match, old_match) {
                    (Some(nm), _) if nm > 0 => {
                        for i in 0..nm {
                            chunks.push(DiffChunk::Added(new_lines[new_idx + i].to_string()));
                        }
                        new_idx += nm;
                    }
                    (_, Some(om)) if om > 0 => {
                        for i in 0..om {
                            chunks.push(DiffChunk::Removed(old_lines[old_idx + i].to_string()));
                        }
                        old_idx += om;
                    }
                    _ => {
                        chunks.push(DiffChunk::Removed(old_lines[old_idx].to_string()));
                        chunks.push(DiffChunk::Added(new_lines[new_idx].to_string()));
                        old_idx += 1;
                        new_idx += 1;
                    }
                }
            }
        }

        while old_idx < old_lines.len() {
            chunks.push(DiffChunk::Removed(old_lines[old_idx].to_string()));
            old_idx += 1;
        }
        while new_idx < new_lines.len() {
            chunks.push(DiffChunk::Added(new_lines[new_idx].to_string()));
            new_idx += 1;
        }

        chunks
    }

    /// Count the number of added lines in a diff.
    pub fn count_added(chunks: &[DiffChunk]) -> usize {
        chunks
            .iter()
            .filter(|c| matches!(c, DiffChunk::Added(_)))
            .count()
    }

    /// Count the number of removed lines in a diff.
    pub fn count_removed(chunks: &[DiffChunk]) -> usize {
        chunks
            .iter()
            .filter(|c| matches!(c, DiffChunk::Removed(_)))
            .count()
    }

    /// Check if two texts are identical.
    pub fn is_identical(old: &str, new: &str) -> bool {
        old == new
    }
}

// ---------------------------------------------------------------------------
// EditorSelectionService – multi-cursor selection management
// ---------------------------------------------------------------------------

/// Represents a single text selection (or cursor position when start == end).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Selection {
    /// Create a cursor (zero-width selection).
    pub fn cursor(line: u32, col: u32) -> Self {
        Self {
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
        }
    }

    /// Create a range selection.
    pub fn range(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Returns `true` if the selection is a zero-width cursor.
    pub fn is_cursor(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    /// Returns `true` if the selection spans multiple lines.
    pub fn is_multiline(&self) -> bool {
        self.start_line != self.end_line
    }

    /// Number of lines the selection spans (at least 1).
    pub fn line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Returns true if the given position is inside this selection.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col > self.end_col {
            return false;
        }
        true
    }
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_cursor() {
            write!(f, "{}:{}", self.start_line, self.start_col)
        } else {
            write!(
                f,
                "{}:{}-{}:{}",
                self.start_line, self.start_col, self.end_line, self.end_col
            )
        }
    }
}

/// Manages multiple cursors/selections for an editor.
pub struct EditorSelectionService {
    selections: Vec<Selection>,
}

impl EditorSelectionService {
    pub fn new() -> Self {
        Self {
            selections: Vec::new(),
        }
    }

    /// Set a single primary selection (clears all others).
    pub fn set_primary(&mut self, sel: Selection) {
        self.selections.clear();
        self.selections.push(sel);
    }

    /// Add an additional cursor/selection.
    pub fn add_selection(&mut self, sel: Selection) {
        if !self.selections.contains(&sel) {
            self.selections.push(sel);
        }
    }

    /// Remove the selection at the given index.
    pub fn remove_selection(&mut self, index: usize) -> bool {
        if index < self.selections.len() {
            self.selections.remove(index);
            true
        } else {
            false
        }
    }

    /// Return the primary (first) selection.
    pub fn primary(&self) -> Option<&Selection> {
        self.selections.first()
    }

    /// Return all selections.
    pub fn all(&self) -> &[Selection] {
        &self.selections
    }

    /// Number of active selections/cursors.
    pub fn count(&self) -> usize {
        self.selections.len()
    }

    /// Clear all selections.
    pub fn clear(&mut self) {
        self.selections.clear();
    }

    /// Merge overlapping or adjacent selections.
    pub fn merge_overlapping(&mut self) {
        if self.selections.len() < 2 {
            return;
        }
        self.selections.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then(a.start_col.cmp(&b.start_col))
        });
        let mut merged = vec![self.selections[0].clone()];
        for sel in &self.selections[1..] {
            let last = merged.last_mut().unwrap();
            if sel.start_line < last.end_line
                || (sel.start_line == last.end_line && sel.start_col <= last.end_col)
            {
                if sel.end_line > last.end_line
                    || (sel.end_line == last.end_line && sel.end_col > last.end_col)
                {
                    last.end_line = sel.end_line;
                    last.end_col = sel.end_col;
                }
            } else {
                merged.push(sel.clone());
            }
        }
        self.selections = merged;
    }
}

impl Default for EditorSelectionService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorFoldingService – code folding regions
// ---------------------------------------------------------------------------

/// A foldable region in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRegion {
    pub start_line: u32,
    pub end_line: u32,
    pub collapsed: bool,
    pub kind: FoldingKind,
}

/// The kind of folding region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoldingKind {
    Comment,
    Import,
    Region,
    Code,
}

impl fmt::Display for FoldingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FoldingKind::Comment => write!(f, "comment"),
            FoldingKind::Import => write!(f, "import"),
            FoldingKind::Region => write!(f, "region"),
            FoldingKind::Code => write!(f, "code"),
        }
    }
}

/// Manages code folding regions.
pub struct EditorFoldingService {
    regions: Vec<FoldingRegion>,
}

impl EditorFoldingService {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Add a folding region.
    pub fn add_region(&mut self, start: u32, end: u32, kind: FoldingKind) {
        if end > start {
            self.regions.push(FoldingRegion {
                start_line: start,
                end_line: end,
                collapsed: false,
                kind,
            });
        }
    }

    /// Toggle the collapsed state of the region containing the given line.
    pub fn toggle_at_line(&mut self, line: u32) -> bool {
        for region in &mut self.regions {
            if region.start_line == line {
                region.collapsed = !region.collapsed;
                return true;
            }
        }
        false
    }

    /// Collapse all regions.
    pub fn collapse_all(&mut self) {
        for r in &mut self.regions {
            r.collapsed = true;
        }
    }

    /// Expand all regions.
    pub fn expand_all(&mut self) {
        for r in &mut self.regions {
            r.collapsed = false;
        }
    }

    /// Return the regions sorted by start line.
    pub fn regions(&self) -> Vec<&FoldingRegion> {
        let mut sorted: Vec<_> = self.regions.iter().collect();
        sorted.sort_by_key(|r| r.start_line);
        sorted
    }

    /// Count of folding regions.
    pub fn count(&self) -> usize {
        self.regions.len()
    }

    /// Count collapsed regions.
    pub fn collapsed_count(&self) -> usize {
        self.regions.iter().filter(|r| r.collapsed).count()
    }

    /// Check if a given line is hidden (inside a collapsed region).
    pub fn is_line_hidden(&self, line: u32) -> bool {
        self.regions
            .iter()
            .any(|r| r.collapsed && line > r.start_line && line <= r.end_line)
    }
}

impl Default for EditorFoldingService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorBookmarkService – bookmark management
// ---------------------------------------------------------------------------

/// A bookmark in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub uri: String,
    pub line: u32,
    pub label: Option<String>,
}

/// Manages editor bookmarks.
pub struct EditorBookmarkService {
    bookmarks: Vec<Bookmark>,
}

impl EditorBookmarkService {
    pub fn new() -> Self {
        Self {
            bookmarks: Vec::new(),
        }
    }

    /// Toggle a bookmark on the given line of the given URI.
    pub fn toggle(&mut self, uri: &str, line: u32) -> bool {
        if let Some(pos) = self
            .bookmarks
            .iter()
            .position(|b| b.uri == uri && b.line == line)
        {
            self.bookmarks.remove(pos);
            false
        } else {
            self.bookmarks.push(Bookmark {
                uri: uri.to_string(),
                line,
                label: None,
            });
            true
        }
    }

    /// Add a bookmark with a label.
    pub fn add_with_label(&mut self, uri: &str, line: u32, label: &str) {
        self.bookmarks.push(Bookmark {
            uri: uri.to_string(),
            line,
            label: Some(label.to_string()),
        });
    }

    /// Get all bookmarks for a URI.
    pub fn bookmarks_for(&self, uri: &str) -> Vec<&Bookmark> {
        self.bookmarks.iter().filter(|b| b.uri == uri).collect()
    }

    /// Get all bookmarks.
    pub fn all(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// Number of bookmarks.
    pub fn count(&self) -> usize {
        self.bookmarks.len()
    }

    /// Clear all bookmarks for a given URI.
    pub fn clear_for_uri(&mut self, uri: &str) {
        self.bookmarks.retain(|b| b.uri != uri);
    }

    /// Navigate to next bookmark after the given line in the same URI.
    pub fn next_in_uri(&self, uri: &str, current_line: u32) -> Option<&Bookmark> {
        self.bookmarks
            .iter()
            .filter(|b| b.uri == uri && b.line > current_line)
            .min_by_key(|b| b.line)
    }

    /// Navigate to previous bookmark before the given line in the same URI.
    pub fn prev_in_uri(&self, uri: &str, current_line: u32) -> Option<&Bookmark> {
        self.bookmarks
            .iter()
            .filter(|b| b.uri == uri && b.line < current_line)
            .max_by_key(|b| b.line)
    }
}

impl Default for EditorBookmarkService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorSnapshotService – diff comparison extension
// ---------------------------------------------------------------------------

impl EditorSnapshotService {
    /// Compare two snapshots by their IDs and return a diff.
    pub fn diff_snapshots(&self, id_a: u64, id_b: u64) -> Option<Vec<DiffChunk>> {
        let snap_a = self.get_snapshot(id_a)?;
        let snap_b = self.get_snapshot(id_b)?;
        Some(EditorDiffService::diff_lines(&snap_a.content, &snap_b.content))
    }

    /// Return true if two snapshots have identical content.
    pub fn snapshots_identical(&self, id_a: u64, id_b: u64) -> Option<bool> {
        let snap_a = self.get_snapshot(id_a)?;
        let snap_b = self.get_snapshot(id_b)?;
        Some(snap_a.content == snap_b.content)
    }
}
// ---------------------------------------------------------------------------
// CompletionItem – completion list filtering, sorting, and ranking
// ---------------------------------------------------------------------------

/// The kind of a completion item, modeled after LSP CompletionItemKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompletionItemKind {
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Keyword,
    Snippet,
    File,
    Constant,
    Enum,
    EnumMember,
    Struct,
    TypeParameter,
}

impl fmt::Display for CompletionItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Text => "text",
            Self::Method => "method",
            Self::Function => "function",
            Self::Constructor => "constructor",
            Self::Field => "field",
            Self::Variable => "variable",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Module => "module",
            Self::Property => "property",
            Self::Keyword => "keyword",
            Self::Snippet => "snippet",
            Self::File => "file",
            Self::Constant => "constant",
            Self::Enum => "enum",
            Self::EnumMember => "enum member",
            Self::Struct => "struct",
            Self::TypeParameter => "type parameter",
        };
        write!(f, "{s}")
    }
}

/// A single completion item with label, detail, sort text, and filter text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub insert_text: Option<String>,
    pub deprecated: bool,
    pub preselect: bool,
}

impl CompletionItem {
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            sort_text: None,
            filter_text: None,
            insert_text: None,
            deprecated: false,
            preselect: false,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_sort_text(mut self, sort_text: impl Into<String>) -> Self {
        self.sort_text = Some(sort_text.into());
        self
    }

    pub fn with_insert_text(mut self, text: impl Into<String>) -> Self {
        self.insert_text = Some(text.into());
        self
    }

    /// Returns the text used for filtering (filter_text if set, otherwise label).
    pub fn effective_filter_text(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }

    /// Returns the text used for sorting (sort_text if set, otherwise label).
    pub fn effective_sort_text(&self) -> &str {
        self.sort_text.as_deref().unwrap_or(&self.label)
    }
}

/// Service for managing and filtering completion lists.
pub struct CompletionService;

impl CompletionService {
    /// Filter completion items by a prefix string (case-insensitive).
    pub fn filter_by_prefix<'a>(items: &'a [CompletionItem], prefix: &str) -> Vec<&'a CompletionItem> {
        let prefix_lower = prefix.to_lowercase();
        items
            .iter()
            .filter(|item| {
                item.effective_filter_text()
                    .to_lowercase()
                    .starts_with(&prefix_lower)
            })
            .collect()
    }

    /// Fuzzy-filter items: every character in the query must appear in order
    /// within the filter text (case-insensitive).
    pub fn fuzzy_filter<'a>(items: &'a [CompletionItem], query: &str) -> Vec<&'a CompletionItem> {
        let query_lower: Vec<char> = query.to_lowercase().chars().collect();
        items
            .iter()
            .filter(|item| {
                let text: Vec<char> = item.effective_filter_text().to_lowercase().chars().collect();
                let mut qi = 0;
                for &ch in &text {
                    if qi < query_lower.len() && ch == query_lower[qi] {
                        qi += 1;
                    }
                }
                qi == query_lower.len()
            })
            .collect()
    }

    /// Sort items by their effective sort text.
    pub fn sort_by_sort_text(items: &mut [CompletionItem]) {
        items.sort_by(|a, b| a.effective_sort_text().cmp(b.effective_sort_text()));
    }

    /// Sort items putting preselected first, then by sort text.
    pub fn sort_with_preselect(items: &mut [CompletionItem]) {
        items.sort_by(|a, b| {
            b.preselect
                .cmp(&a.preselect)
                .then_with(|| a.effective_sort_text().cmp(b.effective_sort_text()))
        });
    }

    /// Group items by their kind.
    pub fn group_by_kind(items: &[CompletionItem]) -> std::collections::HashMap<CompletionItemKind, Vec<&CompletionItem>> {
        let mut map = std::collections::HashMap::new();
        for item in items {
            map.entry(item.kind).or_insert_with(Vec::new).push(item);
        }
        map
    }

    /// Count items that are not deprecated.
    pub fn count_non_deprecated(items: &[CompletionItem]) -> usize {
        items.iter().filter(|i| !i.deprecated).count()
    }
}

// ---------------------------------------------------------------------------
// DiagnosticSeverity & DiagnosticService – severity aggregation
// ---------------------------------------------------------------------------

/// Severity levels modeled after LSP DiagnosticSeverity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Information => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

/// A diagnostic message associated with a location in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub uri: String,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
}

impl Diagnostic {
    pub fn new(
        uri: impl Into<String>,
        line: u32,
        col: u32,
        severity: DiagnosticSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            line,
            col,
            end_line: line,
            end_col: col,
            severity,
            message: message.into(),
            source: None,
            code: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }
}

/// Service for collecting and querying diagnostics.
pub struct DiagnosticService {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticService {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    /// Push a new diagnostic.
    pub fn push(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    /// Replace all diagnostics for a given URI.
    pub fn set_for_uri(&mut self, uri: &str, diags: Vec<Diagnostic>) {
        self.diagnostics.retain(|d| d.uri != uri);
        self.diagnostics.extend(diags);
    }

    /// Get diagnostics for a specific URI.
    pub fn for_uri(&self, uri: &str) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.uri == uri).collect()
    }

    /// Get diagnostics for a specific line within a URI.
    pub fn for_line(&self, uri: &str, line: u32) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.uri == uri && d.line <= line && d.end_line >= line)
            .collect()
    }

    /// Count diagnostics grouped by severity for a given URI.
    pub fn severity_counts(&self, uri: &str) -> std::collections::HashMap<DiagnosticSeverity, usize> {
        let mut counts = std::collections::HashMap::new();
        for d in &self.diagnostics {
            if d.uri == uri {
                *counts.entry(d.severity).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Return the highest (most severe) severity found for a URI, if any.
    pub fn max_severity(&self, uri: &str) -> Option<DiagnosticSeverity> {
        self.diagnostics
            .iter()
            .filter(|d| d.uri == uri)
            .map(|d| d.severity)
            .min() // Error < Warning < Info < Hint in Ord
    }

    /// Return true if the URI has any error-level diagnostics.
    pub fn has_errors(&self, uri: &str) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.uri == uri && d.severity == DiagnosticSeverity::Error)
    }

    /// Total diagnostics count.
    pub fn total_count(&self) -> usize {
        self.diagnostics.len()
    }

    /// Clear all diagnostics.
    pub fn clear(&mut self) {
        self.diagnostics.clear();
    }

    /// List all distinct URIs that have diagnostics.
    pub fn affected_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.diagnostics.iter().map(|d| d.uri.as_str()).collect();
        uris.sort();
        uris.dedup();
        uris
    }
}

impl Default for DiagnosticService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DocumentHighlight – grouping highlights by kind
// ---------------------------------------------------------------------------

/// The kind of a document highlight (read, write, or text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentHighlightKind {
    Text,
    Read,
    Write,
}

/// A highlighted range in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub kind: DocumentHighlightKind,
}

/// Utilities for working with document highlights.
pub struct DocumentHighlightService;

impl DocumentHighlightService {
    /// Group highlights by kind.
    pub fn group_by_kind(
        highlights: &[DocumentHighlight],
    ) -> std::collections::HashMap<DocumentHighlightKind, Vec<&DocumentHighlight>> {
        let mut map = std::collections::HashMap::new();
        for h in highlights {
            map.entry(h.kind).or_insert_with(Vec::new).push(h);
        }
        map
    }

    /// Count write references.
    pub fn write_count(highlights: &[DocumentHighlight]) -> usize {
        highlights
            .iter()
            .filter(|h| h.kind == DocumentHighlightKind::Write)
            .count()
    }

    /// Count read references.
    pub fn read_count(highlights: &[DocumentHighlight]) -> usize {
        highlights
            .iter()
            .filter(|h| h.kind == DocumentHighlightKind::Read)
            .count()
    }

    /// Return highlights sorted by position.
    pub fn sorted_by_position(highlights: &mut [DocumentHighlight]) {
        highlights.sort_by(|a, b| a.line.cmp(&b.line).then(a.col.cmp(&b.col)));
    }
}

// ---------------------------------------------------------------------------
// InlayHint – inlay hint management
// ---------------------------------------------------------------------------

/// The kind of an inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    Type,
    Parameter,
}

/// An inlay hint rendered inline in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    pub line: u32,
    pub col: u32,
    pub label: String,
    pub kind: InlayHintKind,
    pub padding_left: bool,
    pub padding_right: bool,
}

impl InlayHint {
    pub fn type_hint(line: u32, col: u32, label: impl Into<String>) -> Self {
        Self {
            line,
            col,
            label: label.into(),
            kind: InlayHintKind::Type,
            padding_left: true,
            padding_right: false,
        }
    }

    pub fn parameter_hint(line: u32, col: u32, label: impl Into<String>) -> Self {
        Self {
            line,
            col,
            label: label.into(),
            kind: InlayHintKind::Parameter,
            padding_left: false,
            padding_right: true,
        }
    }
}

/// Service for managing inlay hints for a document.
pub struct InlayHintService {
    hints: Vec<InlayHint>,
}

impl InlayHintService {
    pub fn new() -> Self {
        Self { hints: Vec::new() }
    }

    /// Set all hints (replaces existing hints).
    pub fn set_hints(&mut self, hints: Vec<InlayHint>) {
        self.hints = hints;
    }

    /// Get hints within a line range (inclusive).
    pub fn hints_in_range(&self, start_line: u32, end_line: u32) -> Vec<&InlayHint> {
        self.hints
            .iter()
            .filter(|h| h.line >= start_line && h.line <= end_line)
            .collect()
    }

    /// Get all hints on a specific line, sorted by column.
    pub fn hints_on_line(&self, line: u32) -> Vec<&InlayHint> {
        let mut result: Vec<_> = self.hints.iter().filter(|h| h.line == line).collect();
        result.sort_by_key(|h| h.col);
        result
    }

    /// Total number of hints.
    pub fn count(&self) -> usize {
        self.hints.len()
    }

    /// Clear all hints.
    pub fn clear(&mut self) {
        self.hints.clear();
    }

    /// Count hints by kind.
    pub fn count_by_kind(&self, kind: InlayHintKind) -> usize {
        self.hints.iter().filter(|h| h.kind == kind).count()
    }
}

impl Default for InlayHintService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SignatureHelp – signature help with parameter highlighting
// ---------------------------------------------------------------------------

/// A single parameter in a function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParameter {
    pub label: String,
    pub documentation: Option<String>,
}

/// A function signature with its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInfo {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<SignatureParameter>,
}

impl SignatureInfo {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            documentation: None,
            parameters: Vec::new(),
        }
    }

    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }

    pub fn add_param(&mut self, label: impl Into<String>, doc: Option<String>) {
        self.parameters.push(SignatureParameter {
            label: label.into(),
            documentation: doc,
        });
    }

    /// Get the active parameter label at the given index.
    pub fn active_parameter(&self, index: usize) -> Option<&SignatureParameter> {
        self.parameters.get(index)
    }

    /// Format the signature with the active parameter highlighted using brackets.
    pub fn format_with_highlight(&self, active_param_index: usize) -> String {
        if self.parameters.is_empty() {
            return self.label.clone();
        }
        let parts: Vec<String> = self
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if i == active_param_index {
                    format!("[{}]", p.label)
                } else {
                    p.label.clone()
                }
            })
            .collect();
        // Find the function name (text before '(')
        let fn_name = self.label.split('(').next().unwrap_or(&self.label);
        format!("{}({})", fn_name, parts.join(", "))
    }

    /// Return the number of parameters.
    pub fn param_count(&self) -> usize {
        self.parameters.len()
    }
}

// ── EditorSessionTracker ─────────────────────────────────────────────────

/// Tracks open editor sessions with file association and modification state.
#[derive(Debug, Clone)]
pub struct EditorSession {
    pub id: u64,
    pub file: String,
    pub modified: bool,
    pub opened_at: u64,
}

#[derive(Debug, Clone)]
pub struct EditorSessionTracker {
    sessions: Vec<EditorSession>,
    next_id: u64,
}

impl EditorSessionTracker {
    pub fn new() -> Self { Self { sessions: Vec::new(), next_id: 1 } }

    pub fn add(&mut self, file: &str, opened_at: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.sessions.push(EditorSession { id, file: file.to_string(), modified: false, opened_at });
        id
    }

    pub fn remove(&mut self, id: u64) -> bool {
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            self.sessions.remove(pos);
            true
        } else { false }
    }

    pub fn find_by_file(&self, file: &str) -> Option<&EditorSession> {
        self.sessions.iter().find(|s| s.file == file)
    }

    pub fn set_modified(&mut self, id: u64, modified: bool) {
        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) { s.modified = modified; }
    }

    pub fn modified_count(&self) -> usize { self.sessions.iter().filter(|s| s.modified).count() }
    pub fn session_count(&self) -> usize { self.sessions.len() }

    /// Returns IDs of sessions opened before `threshold` timestamp.
    pub fn stale_sessions(&self, threshold: u64) -> Vec<u64> {
        self.sessions.iter().filter(|s| s.opened_at < threshold).map(|s| s.id).collect()
    }
}

// ── EditorCapabilitySet ──────────────────────────────────────────────────

/// A bitflag-like set of editor capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorCapabilitySet {
    bits: u32,
}

impl EditorCapabilitySet {
    pub const READONLY: u32 = 1 << 0;
    pub const DIFF: u32 = 1 << 1;
    pub const PREVIEW: u32 = 1 << 2;
    pub const PINNED: u32 = 1 << 3;

    pub fn empty() -> Self { Self { bits: 0 } }
    pub fn all() -> Self { Self { bits: Self::READONLY | Self::DIFF | Self::PREVIEW | Self::PINNED } }
    pub fn add(&mut self, flag: u32) { self.bits |= flag; }
    pub fn remove(&mut self, flag: u32) { self.bits &= !flag; }
    pub fn has(&self, flag: u32) -> bool { self.bits & flag != 0 }
    pub fn toggle(&mut self, flag: u32) { self.bits ^= flag; }
    pub fn is_empty(&self) -> bool { self.bits == 0 }
    pub fn raw(&self) -> u32 { self.bits }

    fn flag_name(flag: u32) -> &'static str {
        match flag {
            1 => "READONLY",
            2 => "DIFF",
            4 => "PREVIEW",
            8 => "PINNED",
            _ => "UNKNOWN",
        }
    }
}

impl fmt::Display for EditorCapabilitySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let flags = [Self::READONLY, Self::DIFF, Self::PREVIEW, Self::PINNED];
        let names: Vec<&str> = flags.iter().filter(|&&fl| self.has(fl)).map(|&fl| Self::flag_name(fl)).collect();
        if names.is_empty() { write!(f, "(none)") } else { write!(f, "{}", names.join("|")) }
    }
}

// ── EditorLayoutPreference ───────────────────────────────────────────────

/// Describes the preferred layout for editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorLayoutPreference {
    Single,
    SplitHorizontal,
    SplitVertical,
    Grid { columns: usize, rows: usize },
}

impl EditorLayoutPreference {
    pub fn column_count(&self) -> usize {
        match self {
            Self::Single => 1,
            Self::SplitHorizontal => 1,
            Self::SplitVertical => 2,
            Self::Grid { columns, .. } => *columns,
        }
    }

    pub fn row_count(&self) -> usize {
        match self {
            Self::Single => 1,
            Self::SplitHorizontal => 2,
            Self::SplitVertical => 1,
            Self::Grid { rows, .. } => *rows,
        }
    }

    pub fn is_split(&self) -> bool { !matches!(self, Self::Single) }

    pub fn to_grid_dimensions(&self) -> (usize, usize) {
        (self.column_count(), self.row_count())
    }

    pub fn total_panes(&self) -> usize {
        self.column_count() * self.row_count()
    }
}

impl fmt::Display for EditorLayoutPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single => write!(f, "Single"),
            Self::SplitHorizontal => write!(f, "Split Horizontal"),
            Self::SplitVertical => write!(f, "Split Vertical"),
            Self::Grid { columns, rows } => write!(f, "Grid {}x{}", columns, rows),
        }
    }
}


/// Editor service configuration manager.
#[derive(Debug, Clone)]
pub struct EditorServicesConfig {
    entries: Vec<EditorServicesEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single editor service entry.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorServicesEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl EditorServicesEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl EditorServicesConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: EditorServicesEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&EditorServicesEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut EditorServicesEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&EditorServicesEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&EditorServicesEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&EditorServicesEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<EditorServicesEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
// xa_ extended helpers for editor_services
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaEditorServicesRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaEditorServicesRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaEditorServicesCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaEditorServicesCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaEditorServicesCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_count() {
        let mut svc = EditorService::new();
        assert_eq!(svc.editor_count(), 0);
        let idx = svc.open_editor("file:///main.rs", Some("rust"));
        assert_eq!(idx, 0);
        assert_eq!(svc.editor_count(), 1);
        let active = svc.get_active().unwrap();
        assert_eq!(active.uri.as_deref(), Some("file:///main.rs"));
        assert_eq!(active.language_id.as_deref(), Some("rust"));
    }

    #[test]
    fn dirty_tracking() {
        let mut svc = EditorService::new();
        let idx = svc.open_editor("file:///lib.rs", None);
        assert!(!svc.get_active().unwrap().dirty);
        svc.mark_dirty(idx);
        assert!(svc.get_active().unwrap().dirty);
        svc.mark_clean(idx);
        assert!(!svc.get_active().unwrap().dirty);
    }

    #[test]
    fn close_editor_adjusts_active() {
        let mut svc = EditorService::new();
        svc.open_editor("a.rs", None);
        svc.open_editor("b.rs", None);
        svc.open_editor("c.rs", None);
        svc.set_active(2);
        svc.close_editor(0);
        assert_eq!(svc.editor_count(), 2);
        assert_eq!(svc.get_active().unwrap().uri.as_deref(), Some("c.rs"));
    }

    #[test]
    fn close_last_editor() {
        let mut svc = EditorService::new();
        let idx = svc.open_editor("only.rs", None);
        svc.close_editor(idx);
        assert_eq!(svc.editor_count(), 0);
        assert!(svc.get_active().is_none());
    }

    #[test]
    fn get_editor_by_index() {
        let mut svc = EditorService::new();
        svc.open_editor("a.rs", None);
        svc.open_editor("b.rs", Some("rust"));
        assert_eq!(svc.get_editor(0).unwrap().uri.as_deref(), Some("a.rs"));
        assert_eq!(svc.get_editor(1).unwrap().language_id.as_deref(), Some("rust"));
        assert!(svc.get_editor(5).is_none());
    }

    #[test]
    fn find_by_uri_found_and_missing() {
        let mut svc = EditorService::new();
        svc.open_editor("file:///main.rs", None);
        svc.open_editor("file:///lib.rs", None);
        assert_eq!(svc.find_by_uri("file:///lib.rs"), Some(1));
        assert_eq!(svc.find_by_uri("file:///nope.rs"), None);
    }

    #[test]
    fn dirty_editors_list() {
        let mut svc = EditorService::new();
        svc.open_editor("a.rs", None);
        svc.open_editor("b.rs", None);
        svc.open_editor("c.rs", None);
        svc.mark_dirty(0);
        svc.mark_dirty(2);
        assert_eq!(svc.dirty_editors(), vec![0, 2]);
    }

    #[test]
    fn close_all_editors() {
        let mut svc = EditorService::new();
        svc.open_editor("a.rs", None);
        svc.open_editor("b.rs", None);
        svc.close_all();
        assert_eq!(svc.editor_count(), 0);
        assert!(svc.get_active().is_none());
    }

    #[test]
    fn set_mode_for_active() {
        let mut svc = EditorService::new();
        assert!(svc.set_mode(EditorMode::Insert).is_err());
        svc.open_editor("a.rs", None);
        svc.set_mode(EditorMode::Insert).unwrap();
        assert_eq!(svc.get_active().unwrap().mode, EditorMode::Insert);
        svc.set_mode(EditorMode::Visual).unwrap();
        assert_eq!(svc.get_active().unwrap().mode, EditorMode::Visual);
    }

    #[test]
    fn move_cursor_updates_position() {
        let mut svc = EditorService::new();
        assert!(svc.move_cursor(5, 10).is_err());
        svc.open_editor("a.rs", None);
        svc.move_cursor(42, 7).unwrap();
        let active = svc.get_active().unwrap();
        assert_eq!(active.line, 42);
        assert_eq!(active.column, 7);
    }

    #[test]
    fn next_prev_editor_cycle() {
        let mut svc = EditorService::new();
        assert_eq!(svc.next_editor(), None);
        assert_eq!(svc.prev_editor(), None);

        svc.open_editor("a.rs", None);
        svc.open_editor("b.rs", None);
        svc.open_editor("c.rs", None);
        // active is 2 (last opened)
        assert_eq!(svc.next_editor(), Some(0)); // wraps around
        assert_eq!(svc.get_active().unwrap().uri.as_deref(), Some("a.rs"));
        assert_eq!(svc.next_editor(), Some(1));
        assert_eq!(svc.prev_editor(), Some(0));
        svc.set_active(0);
        assert_eq!(svc.prev_editor(), Some(2)); // wraps backward
    }

    #[test]
    fn display_name_returns_filename() {
        let mut svc = EditorService::new();
        svc.open_editor("file:///home/user/project/main.rs", None);
        assert_eq!(svc.get_active().unwrap().display_name(), "main.rs");
    }

    #[test]
    fn display_name_untitled_when_no_uri() {
        let state = EditorState {
            uri: None,
            line: 0,
            column: 0,
            mode: EditorMode::Normal,
            dirty: false,
            language_id: None,
        };
        assert_eq!(state.display_name(), "untitled");
    }

    #[test]
    fn editor_error_display() {
        let e1 = EditorError::EditorNotFound(3);
        assert_eq!(e1.to_string(), "editor not found at index 3");
        let e2 = EditorError::NoActiveEditor;
        assert_eq!(e2.to_string(), "no active editor");
        let e3 = EditorError::IndexOutOfBounds { index: 5, len: 2 };
        assert_eq!(e3.to_string(), "index 5 out of bounds (len 2)");
    }

    #[test]
    fn editor_mode_display() {
        assert_eq!(EditorMode::Normal.to_string(), "NORMAL");
        assert_eq!(EditorMode::Insert.to_string(), "INSERT");
        assert_eq!(EditorMode::Visual.to_string(), "VISUAL");
        assert_eq!(EditorMode::Command.to_string(), "COMMAND");
    }

    #[test]
    fn editor_event_display() {
        let e = EditorEvent::Opened("f.rs".into());
        assert_eq!(e.to_string(), "opened: f.rs");
        assert_eq!(EditorEvent::CursorMoved.to_string(), "cursor moved");
        assert_eq!(EditorEvent::SelectionChanged.to_string(), "selection changed");
    }

    #[test]
    fn eq_editorerror_same() {
        assert_eq!(EditorError::NoActiveEditor, EditorError::NoActiveEditor);
    }

    #[test]
    fn ne_editorerror_diff() {
        assert_ne!(EditorError::NoActiveEditor, EditorError::EditorNotFound(0));
    }

    #[test]
    fn eq_editormode_same() {
        assert_eq!(EditorMode::Normal, EditorMode::Normal);
    }

    #[test]
    fn ne_editormode_diff() {
        assert_ne!(EditorMode::Normal, EditorMode::Insert);
    }

    #[test]
    fn eq_editorevent_same() {
        assert_eq!(EditorEvent::SelectionChanged, EditorEvent::SelectionChanged);
    }

    #[test]
    fn ne_editorevent_diff() {
        assert_ne!(EditorEvent::SelectionChanged, EditorEvent::CursorMoved);
    }

    #[test]
    fn display_editorerror_variants() {
        assert!(!EditorError::NoActiveEditor.to_string().is_empty());
        assert!(!EditorError::NoActiveEditor.to_string().is_empty());
    }

    #[test]
    fn display_editormode_variants() {
        assert!(!EditorMode::Normal.to_string().is_empty());
        assert!(!EditorMode::Insert.to_string().is_empty());
        assert!(!EditorMode::Visual.to_string().is_empty());
        assert!(!EditorMode::Command.to_string().is_empty());
    }

    #[test]
    fn display_editorevent_variants() {
        assert!(!EditorEvent::SelectionChanged.to_string().is_empty());
        assert!(!EditorEvent::CursorMoved.to_string().is_empty());
    }

    #[test]
    fn decoration_service_add_and_query() {
        let mut svc = EditorDecorationService::new();
        let id = svc.add_decoration(DecorationType::ErrorSquiggly, 5, 0, 5, 10);
        assert_eq!(svc.count(), 1);
        let on_line = svc.decorations_on_line(5);
        assert_eq!(on_line.len(), 1);
        assert_eq!(on_line[0].id, id);
    }

    #[test]
    fn decoration_service_remove() {
        let mut svc = EditorDecorationService::new();
        let id = svc.add_decoration(DecorationType::Highlight, 1, 0, 1, 10);
        assert!(svc.remove_decoration(id));
        assert_eq!(svc.count(), 0);
        assert!(!svc.remove_decoration(id));
    }

    #[test]
    fn decoration_service_by_type() {
        let mut svc = EditorDecorationService::new();
        svc.add_decoration(DecorationType::ErrorSquiggly, 1, 0, 1, 5);
        svc.add_decoration(DecorationType::Highlight, 2, 0, 2, 5);
        svc.add_decoration(DecorationType::ErrorSquiggly, 3, 0, 3, 5);
        assert_eq!(svc.decorations_of_type(&DecorationType::ErrorSquiggly).len(), 2);
        assert_eq!(svc.decorations_of_type(&DecorationType::Highlight).len(), 1);
    }

    #[test]
    fn decoration_service_clear_type() {
        let mut svc = EditorDecorationService::new();
        svc.add_decoration(DecorationType::ErrorSquiggly, 1, 0, 1, 5);
        svc.add_decoration(DecorationType::Highlight, 2, 0, 2, 5);
        svc.clear_type(&DecorationType::ErrorSquiggly);
        assert_eq!(svc.count(), 1);
    }

    #[test]
    fn decoration_with_tooltip() {
        let mut svc = EditorDecorationService::new();
        let id = svc.add_decoration_with_tooltip(
            DecorationType::WarningSquiggly, 1, 0, 1, 10, "unused variable",
        );
        let dec = svc.decorations_on_line(1);
        assert_eq!(dec[0].tooltip.as_deref(), Some("unused variable"));
        assert_eq!(dec[0].id, id);
    }

    #[test]
    fn decoration_service_clear_all() {
        let mut svc = EditorDecorationService::new();
        svc.add_decoration(DecorationType::Highlight, 1, 0, 1, 5);
        svc.add_decoration(DecorationType::Highlight, 2, 0, 2, 5);
        svc.clear_all();
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn decoration_multiline_query() {
        let mut svc = EditorDecorationService::new();
        svc.add_decoration(DecorationType::Highlight, 3, 0, 7, 5);
        assert_eq!(svc.decorations_on_line(5).len(), 1);
        assert!(svc.decorations_on_line(2).is_empty());
        assert!(svc.decorations_on_line(8).is_empty());
    }

    #[test]
    fn snapshot_create_and_retrieve() {
        let mut svc = EditorSnapshotService::new();
        let id = svc.create_snapshot("file:///main.rs", "fn main() {}\n", 1);
        let snap = svc.get_snapshot(id).unwrap();
        assert_eq!(snap.uri, "file:///main.rs");
        assert_eq!(snap.version, 1);
        assert_eq!(snap.line_count, 1);
    }

    #[test]
    fn snapshot_get_line() {
        let mut svc = EditorSnapshotService::new();
        let id = svc.create_snapshot("test", "line0\nline1\nline2", 1);
        let snap = svc.get_snapshot(id).unwrap();
        assert_eq!(snap.get_line(0), Some("line0"));
        assert_eq!(snap.get_line(1), Some("line1"));
        assert_eq!(snap.get_line(2), Some("line2"));
        assert!(snap.get_line(3).is_none());
    }

    #[test]
    fn snapshot_checksum_differs() {
        let mut svc = EditorSnapshotService::new();
        let id1 = svc.create_snapshot("a", "hello", 1);
        let id2 = svc.create_snapshot("b", "world", 1);
        let c1 = svc.get_snapshot(id1).unwrap().checksum();
        let c2 = svc.get_snapshot(id2).unwrap().checksum();
        assert_ne!(c1, c2);
    }

    #[test]
    fn snapshot_latest_for_uri() {
        let mut svc = EditorSnapshotService::new();
        svc.create_snapshot("file:///a.rs", "v1", 1);
        svc.create_snapshot("file:///a.rs", "v2", 2);
        svc.create_snapshot("file:///b.rs", "other", 1);
        let latest = svc.latest_for_uri("file:///a.rs").unwrap();
        assert_eq!(latest.version, 2);
        assert_eq!(latest.content, "v2");
    }

    #[test]
    fn snapshot_remove() {
        let mut svc = EditorSnapshotService::new();
        let id = svc.create_snapshot("test", "content", 1);
        assert!(svc.remove_snapshot(id));
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn diff_identical() {
        let chunks = EditorDiffService::diff_lines("a\nb\nc", "a\nb\nc");
        assert!(chunks.iter().all(|c| matches!(c, DiffChunk::Equal(_))));
        assert!(EditorDiffService::is_identical("a", "a"));
    }

    #[test]
    fn diff_added_line() {
        let chunks = EditorDiffService::diff_lines("a\nc", "a\nb\nc");
        assert_eq!(EditorDiffService::count_added(&chunks), 1);
        assert_eq!(EditorDiffService::count_removed(&chunks), 0);
    }

    #[test]
    fn diff_removed_line() {
        let chunks = EditorDiffService::diff_lines("a\nb\nc", "a\nc");
        assert_eq!(EditorDiffService::count_removed(&chunks), 1);
        assert_eq!(EditorDiffService::count_added(&chunks), 0);
    }

    #[test]
    fn diff_changed_line() {
        let chunks = EditorDiffService::diff_lines("hello\nworld", "hello\nearth");
        assert!(EditorDiffService::count_removed(&chunks) >= 1);
        assert!(EditorDiffService::count_added(&chunks) >= 1);
    }

    #[test]
    fn diff_empty_to_content() {
        let chunks = EditorDiffService::diff_lines("", "a\nb");
        assert_eq!(EditorDiffService::count_added(&chunks), 2);
    }

    #[test]
    fn diff_content_to_empty() {
        let chunks = EditorDiffService::diff_lines("a\nb", "");
        assert_eq!(EditorDiffService::count_removed(&chunks), 2);
    }

    #[test]
    fn diff_not_identical() {
        assert!(!EditorDiffService::is_identical("a", "b"));
    }

    #[test]
    fn editor_services_stats_new_defaults() {
        let stats = EditorServicesStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn editor_services_stats_record_success() {
        let mut stats = EditorServicesStats::new();
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
    fn editor_services_stats_record_failure() {
        let mut stats = EditorServicesStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn editor_services_stats_reset() {
        let mut stats = EditorServicesStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn editor_services_stats_merge() {
        let mut a = EditorServicesStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = EditorServicesStats::new();
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
    fn editor_services_stats_display() {
        let mut stats = EditorServicesStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn editor_services_stats_default() {
        let stats = EditorServicesStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn editor_services_validator_accepts_valid_name() {
        let v = EditorServicesValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn editor_services_validator_rejects_empty() {
        let v = EditorServicesValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn editor_services_validator_rejects_too_long() {
        let v = EditorServicesValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn editor_services_validator_forbidden_prefix() {
        let v = EditorServicesValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn editor_services_validator_allowed_chars() {
        let v = EditorServicesValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn editor_services_validator_range() {
        let v = EditorServicesValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn editor_services_sanitize_removes_control() {
        let result = EditorServicesValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn editor_services_truncate_short_string() {
        assert_eq!(EditorServicesValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn editor_services_truncate_long_string() {
        let result = EditorServicesValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn editor_services_is_ascii_printable() {
        assert!(EditorServicesValidator::is_ascii_printable("Hello World 123"));
        assert!(!EditorServicesValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- EditorSelectionService tests --------------------------------------

    #[test]
    fn selection_service_primary_and_add() {
        let mut svc = EditorSelectionService::new();
        svc.set_primary(Selection::cursor(1, 0));
        assert_eq!(svc.count(), 1);
        assert!(svc.primary().unwrap().is_cursor());

        svc.add_selection(Selection::cursor(5, 3));
        assert_eq!(svc.count(), 2);
    }

    #[test]
    fn selection_merge_overlapping() {
        let mut svc = EditorSelectionService::new();
        svc.add_selection(Selection::range(1, 0, 3, 10));
        svc.add_selection(Selection::range(2, 5, 5, 0));
        svc.merge_overlapping();
        assert_eq!(svc.count(), 1);
        let merged = svc.primary().unwrap();
        assert_eq!(merged.start_line, 1);
        assert_eq!(merged.end_line, 5);
    }

    #[test]
    fn selection_contains() {
        let sel = Selection::range(2, 5, 4, 10);
        assert!(sel.contains(3, 0));
        assert!(!sel.contains(1, 0));
        assert!(!sel.contains(2, 3));
        assert!(sel.contains(2, 5));
    }

    #[test]
    fn selection_display() {
        assert_eq!(format!("{}", Selection::cursor(1, 5)), "1:5");
        assert_eq!(format!("{}", Selection::range(1, 0, 3, 10)), "1:0-3:10");
    }

    #[test]
    fn selection_is_multiline() {
        assert!(!Selection::cursor(1, 0).is_multiline());
        assert!(Selection::range(1, 0, 2, 0).is_multiline());
    }

    // -- EditorFoldingService tests ----------------------------------------

    #[test]
    fn folding_add_and_toggle() {
        let mut svc = EditorFoldingService::new();
        svc.add_region(1, 10, FoldingKind::Code);
        assert_eq!(svc.count(), 1);
        assert_eq!(svc.collapsed_count(), 0);

        assert!(svc.toggle_at_line(1));
        assert_eq!(svc.collapsed_count(), 1);
        assert!(svc.is_line_hidden(5));
        assert!(!svc.is_line_hidden(1)); // start line itself is visible
    }

    #[test]
    fn folding_collapse_expand_all() {
        let mut svc = EditorFoldingService::new();
        svc.add_region(1, 5, FoldingKind::Comment);
        svc.add_region(10, 20, FoldingKind::Import);
        svc.collapse_all();
        assert_eq!(svc.collapsed_count(), 2);
        svc.expand_all();
        assert_eq!(svc.collapsed_count(), 0);
    }

    #[test]
    fn folding_kind_display() {
        assert_eq!(format!("{}", FoldingKind::Region), "region");
    }

    // -- EditorBookmarkService tests ---------------------------------------

    #[test]
    fn bookmark_toggle() {
        let mut svc = EditorBookmarkService::new();
        assert!(svc.toggle("file:///main.rs", 10)); // added
        assert_eq!(svc.count(), 1);
        assert!(!svc.toggle("file:///main.rs", 10)); // removed
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn bookmark_navigation() {
        let mut svc = EditorBookmarkService::new();
        let uri = "file:///main.rs";
        svc.toggle(uri, 5);
        svc.toggle(uri, 15);
        svc.toggle(uri, 25);

        let next = svc.next_in_uri(uri, 10).unwrap();
        assert_eq!(next.line, 15);
        let prev = svc.prev_in_uri(uri, 20).unwrap();
        assert_eq!(prev.line, 15);
    }

    #[test]
    fn bookmark_clear_for_uri() {
        let mut svc = EditorBookmarkService::new();
        svc.toggle("file:///a.rs", 1);
        svc.toggle("file:///b.rs", 1);
        svc.clear_for_uri("file:///a.rs");
        assert_eq!(svc.count(), 1);
        assert_eq!(svc.bookmarks_for("file:///b.rs").len(), 1);
    }

    // -- EditorSnapshotService diff extension tests ------------------------

    #[test]
    fn snapshot_diff_comparison() {
        let mut svc = EditorSnapshotService::new();
        let id1 = svc.create_snapshot("file:///a.rs", "line1\nline2\n", 1);
        let id2 = svc.create_snapshot("file:///a.rs", "line1\nline3\n", 2);

        let identical = svc.snapshots_identical(id1, id2).unwrap();
        assert!(!identical);

        let diff = svc.diff_snapshots(id1, id2).unwrap();
        assert!(!diff.is_empty());
    }

    #[test]
    fn snapshot_diff_identical() {
        let mut svc = EditorSnapshotService::new();
        let id1 = svc.create_snapshot("file:///a.rs", "same content", 1);
        let id2 = svc.create_snapshot("file:///a.rs", "same content", 2);
        assert!(svc.snapshots_identical(id1, id2).unwrap());
    }

    // -- CompletionService tests -------------------------------------------

    #[test]
    fn completion_filter_by_prefix() {
        let items = vec![
            CompletionItem::new("println", CompletionItemKind::Function),
            CompletionItem::new("print", CompletionItemKind::Function),
            CompletionItem::new("format", CompletionItemKind::Function),
            CompletionItem::new("Parse", CompletionItemKind::Method),
        ];
        let filtered = CompletionService::filter_by_prefix(&items, "pri");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].label, "println");
        assert_eq!(filtered[1].label, "print");
        // case-insensitive
        let filtered_upper = CompletionService::filter_by_prefix(&items, "Par");
        assert_eq!(filtered_upper.len(), 1);
        assert_eq!(filtered_upper[0].label, "Parse");
    }

    #[test]
    fn completion_fuzzy_filter() {
        let items = vec![
            CompletionItem::new("HashMap", CompletionItemKind::Struct),
            CompletionItem::new("HashSet", CompletionItemKind::Struct),
            CompletionItem::new("Vec", CompletionItemKind::Struct),
            CompletionItem::new("BTreeMap", CompletionItemKind::Struct),
        ];
        let fuzzy = CompletionService::fuzzy_filter(&items, "hm");
        // "hm" case-insensitive: HashMap → h,a,s,h,m,a,p → matches h then m. HashSet → no 'm'.
        assert_eq!(fuzzy.len(), 1);
        assert_eq!(fuzzy[0].label, "HashMap");
    }

    #[test]
    fn completion_sort_by_sort_text() {
        let mut items = vec![
            CompletionItem::new("zebra", CompletionItemKind::Variable)
                .with_sort_text("0_zebra"),
            CompletionItem::new("alpha", CompletionItemKind::Variable),
            CompletionItem::new("beta", CompletionItemKind::Variable)
                .with_sort_text("1_beta"),
        ];
        CompletionService::sort_by_sort_text(&mut items);
        assert_eq!(items[0].label, "zebra"); // sort_text "0_zebra"
        assert_eq!(items[1].label, "beta");  // sort_text "1_beta"
        assert_eq!(items[2].label, "alpha"); // sort_text "alpha" (label)
    }

    #[test]
    fn completion_sort_with_preselect() {
        let mut items = vec![
            CompletionItem::new("alpha", CompletionItemKind::Variable),
            CompletionItem {
                preselect: true,
                ..CompletionItem::new("zeta", CompletionItemKind::Variable)
            },
            CompletionItem::new("beta", CompletionItemKind::Variable),
        ];
        CompletionService::sort_with_preselect(&mut items);
        assert_eq!(items[0].label, "zeta"); // preselect comes first
        assert_eq!(items[1].label, "alpha");
        assert_eq!(items[2].label, "beta");
    }

    #[test]
    fn completion_group_by_kind_and_deprecated() {
        let items = vec![
            CompletionItem::new("foo", CompletionItemKind::Function),
            CompletionItem::new("bar", CompletionItemKind::Function),
            CompletionItem::new("Baz", CompletionItemKind::Struct),
            CompletionItem {
                deprecated: true,
                ..CompletionItem::new("old_fn", CompletionItemKind::Function)
            },
        ];
        let groups = CompletionService::group_by_kind(&items);
        assert_eq!(groups[&CompletionItemKind::Function].len(), 3);
        assert_eq!(groups[&CompletionItemKind::Struct].len(), 1);
        assert_eq!(CompletionService::count_non_deprecated(&items), 3);
    }

    // -- DiagnosticService tests -------------------------------------------

    #[test]
    fn diagnostic_severity_counts() {
        let mut svc = DiagnosticService::new();
        svc.push(Diagnostic::new("a.rs", 1, 0, DiagnosticSeverity::Error, "err1"));
        svc.push(Diagnostic::new("a.rs", 2, 0, DiagnosticSeverity::Warning, "warn1"));
        svc.push(Diagnostic::new("a.rs", 3, 0, DiagnosticSeverity::Error, "err2"));
        svc.push(Diagnostic::new("b.rs", 1, 0, DiagnosticSeverity::Hint, "hint1"));

        let counts = svc.severity_counts("a.rs");
        assert_eq!(counts[&DiagnosticSeverity::Error], 2);
        assert_eq!(counts[&DiagnosticSeverity::Warning], 1);
        assert!(svc.has_errors("a.rs"));
        assert!(!svc.has_errors("b.rs"));
        assert_eq!(svc.max_severity("a.rs"), Some(DiagnosticSeverity::Error));
        assert_eq!(svc.total_count(), 4);
    }

    #[test]
    fn diagnostic_set_for_uri_replaces() {
        let mut svc = DiagnosticService::new();
        svc.push(Diagnostic::new("a.rs", 1, 0, DiagnosticSeverity::Error, "old"));
        svc.push(Diagnostic::new("b.rs", 1, 0, DiagnosticSeverity::Warning, "keep"));
        svc.set_for_uri("a.rs", vec![
            Diagnostic::new("a.rs", 5, 0, DiagnosticSeverity::Hint, "new"),
        ]);
        assert_eq!(svc.for_uri("a.rs").len(), 1);
        assert_eq!(svc.for_uri("a.rs")[0].message, "new");
        assert_eq!(svc.for_uri("b.rs").len(), 1);

        let affected = svc.affected_uris();
        assert_eq!(affected.len(), 2);
    }

    #[test]
    fn diagnostic_for_line() {
        let mut svc = DiagnosticService::new();
        svc.push(Diagnostic::new("a.rs", 5, 0, DiagnosticSeverity::Error, "msg"));
        assert_eq!(svc.for_line("a.rs", 5).len(), 1);
        assert!(svc.for_line("a.rs", 6).is_empty());
    }

    // -- DocumentHighlightService tests ------------------------------------

    #[test]
    fn document_highlight_grouping() {
        let highlights = vec![
            DocumentHighlight { line: 1, col: 0, end_line: 1, end_col: 5, kind: DocumentHighlightKind::Read },
            DocumentHighlight { line: 3, col: 0, end_line: 3, end_col: 5, kind: DocumentHighlightKind::Write },
            DocumentHighlight { line: 5, col: 0, end_line: 5, end_col: 5, kind: DocumentHighlightKind::Read },
        ];
        let groups = DocumentHighlightService::group_by_kind(&highlights);
        assert_eq!(groups[&DocumentHighlightKind::Read].len(), 2);
        assert_eq!(groups[&DocumentHighlightKind::Write].len(), 1);
        assert_eq!(DocumentHighlightService::write_count(&highlights), 1);
        assert_eq!(DocumentHighlightService::read_count(&highlights), 2);
    }

    // -- InlayHintService tests --------------------------------------------

    #[test]
    fn inlay_hint_service_basics() {
        let mut svc = InlayHintService::new();
        svc.set_hints(vec![
            InlayHint::type_hint(1, 10, ": i32"),
            InlayHint::type_hint(3, 5, ": String"),
            InlayHint::parameter_hint(5, 0, "name:"),
            InlayHint::parameter_hint(5, 10, "age:"),
        ]);
        assert_eq!(svc.count(), 4);
        assert_eq!(svc.count_by_kind(InlayHintKind::Type), 2);
        assert_eq!(svc.count_by_kind(InlayHintKind::Parameter), 2);

        let range = svc.hints_in_range(1, 3);
        assert_eq!(range.len(), 2);

        let on_line = svc.hints_on_line(5);
        assert_eq!(on_line.len(), 2);
        assert_eq!(on_line[0].col, 0); // sorted by column
        assert_eq!(on_line[1].col, 10);

        svc.clear();
        assert_eq!(svc.count(), 0);
    }

    // -- SignatureInfo tests -----------------------------------------------

    #[test]
    fn signature_format_with_highlight() {
        let mut sig = SignatureInfo::new("foo(a: i32, b: String)")
            .with_doc("Does stuff");
        sig.add_param("a: i32", None);
        sig.add_param("b: String", Some("the name".into()));

        assert_eq!(sig.param_count(), 2);
        assert_eq!(
            sig.format_with_highlight(0),
            "foo([a: i32], b: String)"
        );
        assert_eq!(
            sig.format_with_highlight(1),
            "foo(a: i32, [b: String])"
        );
        assert_eq!(sig.active_parameter(0).unwrap().label, "a: i32");
        assert_eq!(
            sig.active_parameter(1).unwrap().documentation.as_deref(),
            Some("the name")
        );
        assert!(sig.active_parameter(5).is_none());
    }

    // ── EditorSessionTracker tests ──

    #[test]
    fn session_tracker_add_and_find() {
        let mut tracker = EditorSessionTracker::new();
        let id = tracker.add("main.rs", 1000);
        assert_eq!(tracker.session_count(), 1);
        assert_eq!(tracker.find_by_file("main.rs").unwrap().id, id);
        assert!(tracker.find_by_file("other.rs").is_none());
    }

    #[test]
    fn session_tracker_remove() {
        let mut tracker = EditorSessionTracker::new();
        let id = tracker.add("main.rs", 1000);
        assert!(tracker.remove(id));
        assert!(!tracker.remove(id));
        assert_eq!(tracker.session_count(), 0);
    }

    #[test]
    fn session_tracker_modified_count() {
        let mut tracker = EditorSessionTracker::new();
        let id1 = tracker.add("a.rs", 1000);
        let _id2 = tracker.add("b.rs", 1000);
        assert_eq!(tracker.modified_count(), 0);
        tracker.set_modified(id1, true);
        assert_eq!(tracker.modified_count(), 1);
    }

    #[test]
    fn session_tracker_stale() {
        let mut tracker = EditorSessionTracker::new();
        tracker.add("old.rs", 100);
        tracker.add("new.rs", 500);
        let stale = tracker.stale_sessions(300);
        assert_eq!(stale.len(), 1);
    }

    // ── EditorCapabilitySet tests ──

    #[test]
    fn capability_set_add_has() {
        let mut caps = EditorCapabilitySet::empty();
        assert!(!caps.has(EditorCapabilitySet::READONLY));
        caps.add(EditorCapabilitySet::READONLY);
        assert!(caps.has(EditorCapabilitySet::READONLY));
    }

    #[test]
    fn capability_set_remove() {
        let mut caps = EditorCapabilitySet::all();
        caps.remove(EditorCapabilitySet::DIFF);
        assert!(!caps.has(EditorCapabilitySet::DIFF));
        assert!(caps.has(EditorCapabilitySet::READONLY));
    }

    #[test]
    fn capability_set_toggle() {
        let mut caps = EditorCapabilitySet::empty();
        caps.toggle(EditorCapabilitySet::PREVIEW);
        assert!(caps.has(EditorCapabilitySet::PREVIEW));
        caps.toggle(EditorCapabilitySet::PREVIEW);
        assert!(!caps.has(EditorCapabilitySet::PREVIEW));
    }

    #[test]
    fn capability_set_display() {
        let mut caps = EditorCapabilitySet::empty();
        assert_eq!(format!("{}", caps), "(none)");
        caps.add(EditorCapabilitySet::READONLY);
        caps.add(EditorCapabilitySet::PINNED);
        assert_eq!(format!("{}", caps), "READONLY|PINNED");
    }

    // ── EditorLayoutPreference tests ──

    #[test]
    fn layout_single() {
        let l = EditorLayoutPreference::Single;
        assert!(!l.is_split());
        assert_eq!(l.to_grid_dimensions(), (1, 1));
        assert_eq!(l.total_panes(), 1);
    }

    #[test]
    fn layout_split_horizontal() {
        let l = EditorLayoutPreference::SplitHorizontal;
        assert!(l.is_split());
        assert_eq!(l.column_count(), 1);
        assert_eq!(l.row_count(), 2);
    }

    #[test]
    fn layout_grid() {
        let l = EditorLayoutPreference::Grid { columns: 3, rows: 2 };
        assert_eq!(l.total_panes(), 6);
        assert_eq!(format!("{}", l), "Grid 3x2");
    }

    #[test]
    fn layout_display_variants() {
        assert_eq!(format!("{}", EditorLayoutPreference::Single), "Single");
        assert_eq!(format!("{}", EditorLayoutPreference::SplitVertical), "Split Vertical");
    }

    #[test]
    fn editor_services_entry_creation() {
        let e = EditorServicesEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn editor_services_entry_with_priority() {
        let e = EditorServicesEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn editor_services_entry_metadata() {
        let e = EditorServicesEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn editor_services_entry_remove_meta() {
        let mut e = EditorServicesEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn editor_services_entry_activate_deactivate() {
        let mut e = EditorServicesEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn editor_services_config_add_sorted() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("lo", "Lo").with_priority(1));
        c.add(EditorServicesEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn editor_services_config_capacity() {
        let mut c = EditorServicesConfig::new(1);
        assert!(c.add(EditorServicesEntry::new("a", "A")));
        assert!(!c.add(EditorServicesEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn editor_services_config_remove() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn editor_services_config_get() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn editor_services_config_active_entries() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "A"));
        c.add(EditorServicesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn editor_services_config_enable_disable() {
        let mut c = EditorServicesConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn editor_services_config_clear() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn editor_services_config_find_by_label() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn editor_services_config_top_n() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "A").with_priority(1));
        c.add(EditorServicesEntry::new("b", "B").with_priority(2));
        c.add(EditorServicesEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn editor_services_config_deactivate_activate_all() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "A"));
        c.add(EditorServicesEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn editor_services_config_highest_priority() {
        let mut c = EditorServicesConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(EditorServicesEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn editor_services_config_contains() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn editor_services_config_labels() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "Alpha"));
        c.add(EditorServicesEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn editor_services_config_drain_inactive() {
        let mut c = EditorServicesConfig::new(10);
        c.add(EditorServicesEntry::new("a", "A"));
        c.add(EditorServicesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for editor_services
    #[test]
    fn xa_editor_services_ring_new() {
        let rb = super::XaEditorServicesRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_editor_services_ring_push_len() {
        let mut rb = super::XaEditorServicesRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_editor_services_ring_wrap() {
        let mut rb = super::XaEditorServicesRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_editor_services_ring_mean_empty() {
        let rb = super::XaEditorServicesRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_editor_services_ring_mean_values() {
        let mut rb = super::XaEditorServicesRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_editor_services_ring_min_max() {
        let mut rb = super::XaEditorServicesRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_editor_services_ring_iter() {
        let mut rb = super::XaEditorServicesRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_editor_services_counter_new() {
        let c = super::XaEditorServicesCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_services_counter_inc() {
        let mut c = super::XaEditorServicesCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_editor_services_counter_inc_by() {
        let mut c = super::XaEditorServicesCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_editor_services_counter_reset() {
        let mut c = super::XaEditorServicesCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_editor_services_counter_clear() {
        let mut c = super::XaEditorServicesCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_editor_services_counter_default() {
        let c = super::XaEditorServicesCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }

}
