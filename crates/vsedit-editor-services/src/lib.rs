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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 37
// ---------------------------------------------------------------------------

/// Generic object pool `Xc37Pool<T>`.
pub struct Xc37Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc37Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc37PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc37Pool<T> {
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
    pub fn stats(&self) -> Xc37PoolStats {
        Xc37PoolStats {
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

impl<T> Default for Xc37Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc37Scheduler`.
pub struct Xc37Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc37Scheduler {
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

impl Default for Xc37Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_37 hash for the given byte slice.
pub fn xc_37_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_37 convention.
pub fn xc_37_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_79 deepening: state machine + event bus ---

/// States for the Xd79 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd79State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd79State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd79Transition {
    pub from: Xd79State,
    pub to: Xd79State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd79StateMachine {
    current: Xd79State,
    history: Vec<Xd79Transition>,
    step_counter: usize,
}

impl Xd79StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd79State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd79State {
        self.current
    }

    pub fn history(&self) -> &[Xd79Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd79State) -> Result<Xd79State, String> {
        let allowed = match (self.current, target) {
            (Xd79State::Idle, Xd79State::Running) => true,
            (Xd79State::Running, Xd79State::Paused) => true,
            (Xd79State::Running, Xd79State::Done) => true,
            (Xd79State::Paused, Xd79State::Running) => true,
            (Xd79State::Paused, Xd79State::Done) => true,
            (Xd79State::Done, Xd79State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_79: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd79Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd79SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd79State> {
        let prefix = "Xd79SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd79State::Idle),
            "Running" => Some(Xd79State::Running),
            "Paused" => Some(Xd79State::Paused),
            "Done" => Some(Xd79State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd79State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd79 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd79Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd79Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd79HandlerFn = Box<dyn Fn(&Xd79Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd79EventBus {
    handlers: Vec<(usize, Option<String>, Xd79HandlerFn)>,
    next_id: usize,
    published: Vec<Xd79Event>,
}

impl Xd79EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd79Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd79Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd79Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd79Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #99
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf99Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf99TrieNode {
    children: std::collections::HashMap<char, Xf99TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf99Trie {
    root: Xf99TrieNode,
    count: usize,
}

impl Xf99Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf99TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf99TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf99TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf99BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf99BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 36).
pub struct Xh36SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh36SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 78 as u64,
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

/// A compact bit set supporting boolean operations (variant 36).
pub struct Xh36BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh36BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 36).
pub struct Xi36Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi36Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi36Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi36Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 36).
pub struct Xi36IntervalTree {
    xi_intervals: Vec<Xi36Interval>,
}

impl Xi36IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi36Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi36Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi36Interval) -> Vec<&Xi36Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi36Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi36Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi36Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi36Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi36Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi36Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 36) ---

/// Disjoint set / union-find for crate 36.
pub struct Xj36UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj36UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ36_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 36.
pub struct Xj36BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj36BTreeNode<K, V>>>,
    len: usize,
}

struct Xj36BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj36BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj36BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ36_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ36_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj36BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj36BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj36BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj36BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_36 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk36SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk36SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk36DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk36DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
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


    // ---- xc_ pool / scheduler tests – block 37 ----

    #[test]
    fn xc_37_pool_new_empty() {
        let pool: super::Xc37Pool<i32> = super::Xc37Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_37_pool_release_acquire() {
        let mut pool = super::Xc37Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_37_pool_acquire_empty() {
        let mut pool: super::Xc37Pool<i32> = super::Xc37Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_37_pool_full() {
        let mut pool = super::Xc37Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_37_pool_drain() {
        let mut pool = super::Xc37Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_37_pool_stats() {
        let mut pool = super::Xc37Pool::new(8);
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
    fn xc_37_pool_clear() {
        let mut pool = super::Xc37Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_37_pool_shrink() {
        let mut pool = super::Xc37Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_37_pool_default() {
        let pool: super::Xc37Pool<String> = super::Xc37Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_37_pool_extend() {
        let mut pool = super::Xc37Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_37_pool_retain() {
        let mut pool = super::Xc37Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_37_scheduler_round_robin() {
        let mut sched = super::Xc37Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_37_scheduler_empty() {
        let mut sched = super::Xc37Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_37_scheduler_reset() {
        let mut sched = super::Xc37Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_37_scheduler_add_remove() {
        let mut sched = super::Xc37Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_37_scheduler_targets() {
        let sched = super::Xc37Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_37_hash_empty() {
        assert_eq!(super::xc_37_hash(b""), 5381);
    }

    #[test]
    fn xc_37_hash_data() {
        let h = super::xc_37_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_37_hash(b"hello"), h);
    }

    #[test]
    fn xc_37_reverse_str() {
        assert_eq!(super::xc_37_reverse("abc"), "cba");
        assert_eq!(super::xc_37_reverse(""), "");
    }


    // --- xd_79 deepening tests ---

    #[test]
    fn xd_79_sm_initial_state() {
        let sm = Xd79StateMachine::new();
        assert_eq!(sm.current_state(), Xd79State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_79_sm_valid_idle_to_running() {
        let mut sm = Xd79StateMachine::new();
        assert!(sm.transition(Xd79State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd79State::Running);
    }

    #[test]
    fn xd_79_sm_valid_running_to_paused() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        assert!(sm.transition(Xd79State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd79State::Paused);
    }

    #[test]
    fn xd_79_sm_valid_running_to_done() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        assert!(sm.transition(Xd79State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd79State::Done);
    }

    #[test]
    fn xd_79_sm_valid_paused_to_running() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        sm.transition(Xd79State::Paused).unwrap();
        assert!(sm.transition(Xd79State::Running).is_ok());
    }

    #[test]
    fn xd_79_sm_valid_done_to_idle() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        sm.transition(Xd79State::Done).unwrap();
        assert!(sm.transition(Xd79State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd79State::Idle);
    }

    #[test]
    fn xd_79_sm_invalid_idle_to_done() {
        let mut sm = Xd79StateMachine::new();
        assert!(sm.transition(Xd79State::Done).is_err());
    }

    #[test]
    fn xd_79_sm_invalid_idle_to_paused() {
        let mut sm = Xd79StateMachine::new();
        assert!(sm.transition(Xd79State::Paused).is_err());
    }

    #[test]
    fn xd_79_sm_history_tracking() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        sm.transition(Xd79State::Paused).unwrap();
        sm.transition(Xd79State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd79State::Idle);
        assert_eq!(sm.history()[0].to, Xd79State::Running);
        assert_eq!(sm.history()[1].from, Xd79State::Running);
        assert_eq!(sm.history()[2].to, Xd79State::Done);
    }

    #[test]
    fn xd_79_sm_serialize_deserialize() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd79StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd79State::Running));
    }

    #[test]
    fn xd_79_sm_deserialize_invalid() {
        assert_eq!(Xd79StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_79_sm_reset() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd79State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_79_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd79EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd79Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_79_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd79EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd79Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd79Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_79_bus_unsubscribe() {
        let mut bus = Xd79EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_79_event_kind_and_payload() {
        let e = Xd79Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd79Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_79_bus_clear_history() {
        let mut bus = Xd79EventBus::new();
        bus.publish(Xd79Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_79_sm_step_counter_increments() {
        let mut sm = Xd79StateMachine::new();
        sm.transition(Xd79State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd79State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #99 --

    #[test]
    fn xf99_trie_insert_search() {
        let mut t = Xf99Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf99_trie_starts_with() {
        let mut t = Xf99Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf99_trie_remove() {
        let mut t = Xf99Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf99_trie_word_count() {
        let mut t = Xf99Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf99_trie_longest_prefix() {
        let mut t = Xf99Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf99_trie_all_words() {
        let mut t = Xf99Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf99_trie_autocomplete() {
        let mut t = Xf99Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf99_trie_empty_search() {
        let t = Xf99Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf99_bloom_add_contains() {
        let mut bf = Xf99BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf99_bloom_probably_absent() {
        let bf = Xf99BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf99_bloom_false_positive_rate() {
        let mut bf = Xf99BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf99_bloom_clear() {
        let mut bf = Xf99BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf99_bloom_union() {
        let mut a = Xf99BloomFilter::xf_new(512, 2);
        let mut b = Xf99BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf99_bloom_intersection_estimate() {
        let mut a = Xf99BloomFilter::xf_new(512, 2);
        let mut b = Xf99BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf99_bloom_union_size_mismatch() {
        let a = Xf99BloomFilter::xf_new(256, 2);
        let b = Xf99BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh36_skip_insert_contains() {
        let mut sl = super::Xh36SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh36_skip_remove() {
        let mut sl = super::Xh36SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh36_skip_len() {
        let mut sl = super::Xh36SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh36_skip_range_query() {
        let mut sl = super::Xh36SkipList::xh_new(4);
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
    fn xh36_skip_floor_ceiling() {
        let mut sl = super::Xh36SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh36_skip_rank() {
        let mut sl = super::Xh36SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh36_skip_empty() {
        let sl = super::Xh36SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh36_skip_duplicates() {
        let mut sl = super::Xh36SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh36_bitset_set_test() {
        let mut bs = super::Xh36BitSet::xh_new(256);
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
    fn xh36_bitset_clear_count() {
        let mut bs = super::Xh36BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh36_bitset_and_or_xor() {
        let mut a = super::Xh36BitSet::xh_new(128);
        let mut b = super::Xh36BitSet::xh_new(128);
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
    fn xh36_bitset_iter_ones() {
        let mut bs = super::Xh36BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh36_bitset_first_last() {
        let mut bs = super::Xh36BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh36_bitset_empty() {
        let bs = super::Xh36BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi36_deque_push_pop_back() {
        let mut dq = super::Xi36Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi36_deque_push_pop_front() {
        let mut dq = super::Xi36Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi36_deque_mixed_ops() {
        let mut dq = super::Xi36Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi36_deque_get_and_split() {
        let mut dq = super::Xi36Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi36_deque_rotate_left() {
        let mut dq = super::Xi36Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi36_deque_rotate_right() {
        let mut dq = super::Xi36Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi36_deque_grow() {
        let mut dq = super::Xi36Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi36_deque_empty() {
        let dq = super::Xi36Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi36_interval_tree_insert_query() {
        let mut tree = super::Xi36IntervalTree::xi_new();
        tree.xi_insert(super::Xi36Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi36Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi36Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi36_interval_tree_overlap() {
        let mut tree = super::Xi36IntervalTree::xi_new();
        tree.xi_insert(super::Xi36Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi36Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi36Interval::xi_new(12, 20));
        let q = super::Xi36Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi36_interval_tree_remove() {
        let mut tree = super::Xi36IntervalTree::xi_new();
        tree.xi_insert(super::Xi36Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi36Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi36_interval_tree_gaps() {
        let mut tree = super::Xi36IntervalTree::xi_new();
        tree.xi_insert(super::Xi36Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi36Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi36Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi36Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi36Interval::xi_new(8, 10));
    }

    #[test]
    fn xi36_interval_tree_merge() {
        let mut tree = super::Xi36IntervalTree::xi_new();
        tree.xi_insert(super::Xi36Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi36Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi36Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi36Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi36Interval::xi_new(10, 15));
    }

    #[test]
    fn xi36_interval_tree_all() {
        let mut tree = super::Xi36IntervalTree::xi_new();
        tree.xi_insert(super::Xi36Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi36Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi36_interval_tree_empty() {
        let tree = super::Xi36IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi36_interval_tree_contains_point() {
        let iv = super::Xi36Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 36) ---

    #[test]
    fn xj_36_uf_make_and_find() {
        let mut uf = super::Xj36UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_36_uf_union_connected() {
        let mut uf = super::Xj36UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_36_uf_component_count() {
        let mut uf = super::Xj36UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_36_uf_component_size() {
        let mut uf = super::Xj36UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_36_uf_largest_component() {
        let mut uf = super::Xj36UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_36_uf_many_elements() {
        let mut uf = super::Xj36UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_36_uf_separate_components() {
        let mut uf = super::Xj36UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_36_uf_path_compression() {
        let mut uf = super::Xj36UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_36_bt_insert_get() {
        let mut bt = super::Xj36BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_36_bt_contains_len() {
        let mut bt = super::Xj36BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_36_bt_replace() {
        let mut bt = super::Xj36BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_36_bt_remove() {
        let mut bt = super::Xj36BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_36_bt_keys_values() {
        let mut bt = super::Xj36BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_36_bt_range() {
        let mut bt = super::Xj36BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_36_bt_min_max() {
        let mut bt = super::Xj36BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_36_bt_many_inserts() {
        let mut bt = super::Xj36BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_36 segment tree tests ---

    #[test]
    fn xk_36_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk36SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_36_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk36SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_36_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk36SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_36_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk36SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_36_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk36SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_36_st_single_element() {
        let data = vec![42];
        let st = super::Xk36SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_36_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk36SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_36_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk36SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_36 disjoint intervals tests ---

    #[test]
    fn xk_36_di_add_and_count() {
        let mut di = super::Xk36DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_36_di_merge_overlap() {
        let mut di = super::Xk36DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_36_di_contains() {
        let mut di = super::Xk36DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_36_di_remove() {
        let mut di = super::Xk36DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_36_di_covered_length() {
        let mut di = super::Xk36DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_36_di_gaps() {
        let mut di = super::Xk36DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_36_di_merge_adjacent() {
        let mut di = super::Xk36DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_36_di_empty() {
        let di = super::Xk36DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
