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
}
