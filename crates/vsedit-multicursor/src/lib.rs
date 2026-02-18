//! Multi-cursor operations.
//!
//! Provides lightweight cursor position tracking, selection ranges,
//! and column-selection mode utilities that complement the lower-level
//! [`vsedit_cursor::CursorController`].

use std::fmt;

/// Errors that can occur during multi-cursor operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiCursorError {
    /// Position has a zero line or column (must be 1-based).
    InvalidPosition { line: u32, column: u32 },
    /// Cursor index is out of bounds.
    IndexOutOfBounds { index: usize, len: usize },
    /// Attempted to create an empty session when at least one cursor is required.
    EmptyCursors,
    /// Selection range is invalid (start_line > line_count, etc.).
    InvalidRange { start: u32, end: u32, max: u32 },
}

impl fmt::Display for MultiCursorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPosition { line, column } => {
                write!(f, "invalid position ({line}, {column}): line and column must be >= 1")
            }
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "cursor index {index} out of bounds (len {len})")
            }
            Self::EmptyCursors => write!(f, "at least one cursor is required"),
            Self::InvalidRange { start, end, max } => {
                write!(f, "invalid range {start}..={end} (max {max})")
            }
        }
    }
}

impl std::error::Error for MultiCursorError {}

/// A position in a text document (1-based line and column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CursorPosition {
    pub line: u32,
    pub column: u32,
}

impl CursorPosition {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    /// Create a position, returning an error if line or column is zero.
    pub fn try_new(line: u32, column: u32) -> Result<Self, MultiCursorError> {
        if line == 0 || column == 0 {
            Err(MultiCursorError::InvalidPosition { line, column })
        } else {
            Ok(Self { line, column })
        }
    }

    /// Offset this position by the given signed deltas, clamping to 1.
    pub fn offset(&self, line_delta: i64, column_delta: i64) -> Self {
        let new_line = (self.line as i64 + line_delta).max(1) as u32;
        let new_col = (self.column as i64 + column_delta).max(1) as u32;
        Self { line: new_line, column: new_col }
    }

    /// Manhattan distance between two positions (useful for heuristics).
    pub fn distance_to(&self, other: &CursorPosition) -> u64 {
        let dl = (self.line as i64 - other.line as i64).unsigned_abs();
        let dc = (self.column as i64 - other.column as i64).unsigned_abs();
        dl + dc
    }
}

impl fmt::Display for CursorPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A contiguous selection between two positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: CursorPosition,
    pub end: CursorPosition,
}

impl Selection {
    pub fn new(start: CursorPosition, end: CursorPosition) -> Self {
        Self { start, end }
    }

    /// Returns `true` when start equals end (no text is selected).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns a normalized selection where `start <= end`.
    pub fn normalized(&self) -> Self {
        if self.start <= self.end {
            *self
        } else {
            Self { start: self.end, end: self.start }
        }
    }

    /// Number of lines spanned by this selection (at least 1).
    pub fn line_span(&self) -> u32 {
        let n = self.normalized();
        n.end.line - n.start.line + 1
    }

    /// Returns `true` if the given position falls within this selection (inclusive).
    pub fn contains(&self, pos: &CursorPosition) -> bool {
        let n = self.normalized();
        *pos >= n.start && *pos <= n.end
    }

    /// Returns `true` if this selection overlaps with `other`.
    pub fn overlaps(&self, other: &Selection) -> bool {
        let a = self.normalized();
        let b = other.normalized();
        a.start <= b.end && b.start <= a.end
    }
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} -> {}]", self.start, self.end)
    }
}

/// Manages a set of cursors and their associated selections.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiCursorSession {
    pub cursors: Vec<CursorPosition>,
    pub selections: Vec<Selection>,
}

impl MultiCursorSession {
    /// Create a session with no cursors.
    pub fn new() -> Self {
        Self {
            cursors: Vec::new(),
            selections: Vec::new(),
        }
    }

    /// Add a cursor at the given position.
    pub fn add_cursor(&mut self, pos: CursorPosition) {
        self.cursors.push(pos);
    }

    /// Remove the cursor at `index`. Returns `None` if out of bounds.
    pub fn remove_cursor(&mut self, index: usize) -> Option<CursorPosition> {
        if index < self.cursors.len() {
            Some(self.cursors.remove(index))
        } else {
            None
        }
    }

    /// Add a cursor one line above the first cursor, keeping the same column.
    /// `max_column_fn` clamps the column to the target line's width.
    pub fn add_cursor_above(&mut self, max_column_fn: impl Fn(u32) -> u32) {
        if let Some(first) = self.cursors.first().copied() {
            if first.line > 1 {
                let new_line = first.line - 1;
                let col = first.column.min(max_column_fn(new_line));
                self.cursors.push(CursorPosition::new(new_line, col));
            }
        }
    }

    /// Add a cursor one line below the last cursor, keeping the same column.
    /// `max_column_fn` clamps the column; `line_count` is the total number of lines.
    pub fn add_cursor_below(
        &mut self,
        line_count: u32,
        max_column_fn: impl Fn(u32) -> u32,
    ) {
        if let Some(last) = self.cursors.last().copied() {
            if last.line < line_count {
                let new_line = last.line + 1;
                let col = last.column.min(max_column_fn(new_line));
                self.cursors.push(CursorPosition::new(new_line, col));
            }
        }
    }

    /// Sort cursors by position and remove duplicates.
    pub fn sort_and_deduplicate(&mut self) {
        self.cursors.sort();
        self.cursors.dedup();
    }

    /// Number of active cursors.
    pub fn cursor_count(&self) -> usize {
        self.cursors.len()
    }

    /// Remove all cursors and selections.
    pub fn clear(&mut self) {
        self.cursors.clear();
        self.selections.clear();
    }

    /// Returns `true` when more than one cursor is active.
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }

    /// Validated cursor addition — rejects zero line/column.
    pub fn try_add_cursor(&mut self, pos: CursorPosition) -> Result<(), MultiCursorError> {
        if pos.line == 0 || pos.column == 0 {
            return Err(MultiCursorError::InvalidPosition {
                line: pos.line,
                column: pos.column,
            });
        }
        self.cursors.push(pos);
        Ok(())
    }

    /// Validated cursor removal.
    pub fn try_remove_cursor(&mut self, index: usize) -> Result<CursorPosition, MultiCursorError> {
        if index >= self.cursors.len() {
            Err(MultiCursorError::IndexOutOfBounds {
                index,
                len: self.cursors.len(),
            })
        } else {
            Ok(self.cursors.remove(index))
        }
    }

    /// Move every cursor by the given signed deltas, clamping to 1.
    pub fn move_all(&mut self, line_delta: i64, column_delta: i64) {
        for c in &mut self.cursors {
            *c = c.offset(line_delta, column_delta);
        }
    }

    /// Returns the bounding box of all cursors as `(top_left, bottom_right)`,
    /// or `None` if there are no cursors.
    pub fn bounding_box(&self) -> Option<(CursorPosition, CursorPosition)> {
        if self.cursors.is_empty() {
            return None;
        }
        let min_line = self.cursors.iter().map(|c| c.line).min().unwrap();
        let max_line = self.cursors.iter().map(|c| c.line).max().unwrap();
        let min_col = self.cursors.iter().map(|c| c.column).min().unwrap();
        let max_col = self.cursors.iter().map(|c| c.column).max().unwrap();
        Some((
            CursorPosition::new(min_line, min_col),
            CursorPosition::new(max_line, max_col),
        ))
    }

    /// Merge overlapping selections in-place.
    pub fn merge_overlapping_selections(&mut self) {
        if self.selections.len() < 2 {
            return;
        }
        // Normalize and sort by start position.
        let mut sels: Vec<Selection> = self.selections.iter().map(|s| s.normalized()).collect();
        sels.sort_by_key(|s| (s.start.line, s.start.column));

        let mut merged: Vec<Selection> = vec![sels[0]];
        for s in &sels[1..] {
            let last = merged.last_mut().unwrap();
            if last.overlaps(s) || last.end >= s.start {
                if s.end > last.end {
                    last.end = s.end;
                }
            } else {
                merged.push(*s);
            }
        }
        self.selections = merged;
    }
}

impl fmt::Display for MultiCursorSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MultiCursorSession({} cursors, {} selections)",
            self.cursors.len(),
            self.selections.len(),
        )
    }
}

/// Builder for constructing a [`MultiCursorSession`] with validation.
#[derive(Debug, Clone, Default)]
pub struct MultiCursorSessionBuilder {
    cursors: Vec<CursorPosition>,
    selections: Vec<Selection>,
    deduplicate: bool,
}

impl MultiCursorSessionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cursor, returning an error if the position is invalid.
    pub fn cursor(mut self, line: u32, column: u32) -> Result<Self, MultiCursorError> {
        let pos = CursorPosition::try_new(line, column)?;
        self.cursors.push(pos);
        Ok(self)
    }

    /// Add a selection.
    pub fn selection(mut self, sel: Selection) -> Self {
        self.selections.push(sel);
        self
    }

    /// Enable automatic deduplication on build.
    pub fn deduplicate(mut self, yes: bool) -> Self {
        self.deduplicate = yes;
        self
    }

    /// Build the session.
    pub fn build(self) -> Result<MultiCursorSession, MultiCursorError> {
        if self.cursors.is_empty() {
            return Err(MultiCursorError::EmptyCursors);
        }
        let mut session = MultiCursorSession {
            cursors: self.cursors,
            selections: self.selections,
        };
        if self.deduplicate {
            session.sort_and_deduplicate();
        }
        Ok(session)
    }
}

impl Default for MultiCursorSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility for computing column-aligned selections across a range of lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSelectionMode {
    pub anchor_column: u32,
}

impl ColumnSelectionMode {
    pub fn new(anchor_column: u32) -> Self {
        Self { anchor_column }
    }

    /// Compute one [`Selection`] per line in `start_line..=end_line`.
    ///
    /// `max_column_fn` returns the maximum valid column for a given line.
    /// The selection on each line spans from `anchor_column` to `target_column`,
    /// both clamped to the line width.
    pub fn compute_selections(
        &self,
        start_line: u32,
        end_line: u32,
        target_column: u32,
        max_column_fn: impl Fn(u32) -> u32,
    ) -> Vec<Selection> {
        let (lo, hi) = if start_line <= end_line {
            (start_line, end_line)
        } else {
            (end_line, start_line)
        };

        (lo..=hi)
            .map(|line| {
                let max_col = max_column_fn(line);
                let a = self.anchor_column.min(max_col);
                let b = target_column.min(max_col);
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                Selection::new(
                    CursorPosition::new(line, start),
                    CursorPosition::new(line, end),
                )
            })
            .collect()
    }

    /// Like [`compute_selections`](Self::compute_selections) but validates inputs first.
    pub fn try_compute_selections(
        &self,
        start_line: u32,
        end_line: u32,
        target_column: u32,
        line_count: u32,
        max_column_fn: impl Fn(u32) -> u32,
    ) -> Result<Vec<Selection>, MultiCursorError> {
        let lo = start_line.min(end_line);
        let hi = start_line.max(end_line);
        if lo == 0 || hi > line_count {
            return Err(MultiCursorError::InvalidRange {
                start: lo,
                end: hi,
                max: line_count,
            });
        }
        Ok(self.compute_selections(start_line, end_line, target_column, max_column_fn))
    }

    /// Extract cursor positions (one per line at `target_column`, clamped).
    pub fn cursor_positions(
        &self,
        start_line: u32,
        end_line: u32,
        target_column: u32,
        max_column_fn: impl Fn(u32) -> u32,
    ) -> Vec<CursorPosition> {
        let (lo, hi) = if start_line <= end_line {
            (start_line, end_line)
        } else {
            (end_line, start_line)
        };
        (lo..=hi)
            .map(|line| {
                let col = target_column.min(max_column_fn(line));
                CursorPosition::new(line, col)
            })
            .collect()
    }
}

/// Accumulated statistics for multicursor operations.
#[derive(Debug, Clone, PartialEq)]
pub struct MulticursorStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl MulticursorStats {
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
    pub fn merge(&mut self, other: &MulticursorStats) {
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

impl Default for MulticursorStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MulticursorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MulticursorStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for multicursor.
#[derive(Debug, Clone)]
pub struct MulticursorValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl MulticursorValidator {
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

impl Default for MulticursorValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cursor merging
// ---------------------------------------------------------------------------

/// Merge overlapping or adjacent selections in a list.
///
/// Selections are first sorted by start position, then merged when they
/// overlap or are directly adjacent (end of one == start of next).
/// Returns a new list with no overlaps.
pub fn cursor_merge_overlapping(selections: &[Selection]) -> Vec<Selection> {
    if selections.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<Selection> = selections.to_vec();
    sorted.sort_by(|a, b| {
        a.start.line.cmp(&b.start.line)
            .then(a.start.column.cmp(&b.start.column))
    });

    let mut merged: Vec<Selection> = Vec::new();
    merged.push(sorted[0]);

    for sel in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        if sel.start.line < last.end.line
            || (sel.start.line == last.end.line && sel.start.column <= last.end.column)
        {
            // Overlapping or adjacent – extend the end if needed
            if sel.end.line > last.end.line
                || (sel.end.line == last.end.line && sel.end.column > last.end.column)
            {
                last.end = sel.end;
            }
        } else {
            merged.push(*sel);
        }
    }

    merged
}

/// Deduplicate cursor positions (same line+column), preserving order of first occurrence.
pub fn cursor_deduplicate(positions: &[CursorPosition]) -> Vec<CursorPosition> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for p in positions {
        let key = (p.line, p.column);
        if seen.insert(key) {
            result.push(*p);
        }
    }
    result
}

/// Check if any two selections in the list overlap.
pub fn has_overlapping_selections(selections: &[Selection]) -> bool {
    let merged = cursor_merge_overlapping(selections);
    merged.len() < selections.len()
}

// ---------------------------------------------------------------------------
// CursorPosition extensions
// ---------------------------------------------------------------------------

impl CursorPosition {
    pub fn is_before(&self, other: &CursorPosition) -> bool {
        (self.line, self.column) < (other.line, other.column)
    }

    pub fn pos_min(&self, other: &Self) -> Self {
        if self <= other { *self } else { *other }
    }

    pub fn pos_max(&self, other: &Self) -> Self {
        if self >= other { *self } else { *other }
    }

    pub fn signed_distance_to(&self, other: &CursorPosition) -> (i64, i64) {
        let dl = other.line as i64 - self.line as i64;
        let dc = other.column as i64 - self.column as i64;
        (dl, dc)
    }
}

// ---------------------------------------------------------------------------
// Selection extensions
// ---------------------------------------------------------------------------

impl Selection {
    pub fn char_count(&self) -> u32 {
        let n = self.normalized();
        if n.start.line == n.end.line {
            n.end.column.saturating_sub(n.start.column)
        } else {
            n.end.column + n.start.column
        }
    }

    pub fn is_reversed(&self) -> bool {
        self.start > self.end
    }

    pub fn merge_with(&self, other: &Selection) -> Option<Selection> {
        if !self.overlaps(other) {
            return None;
        }
        let a = self.normalized();
        let b = other.normalized();
        Some(Selection::new(
            a.start.pos_min(&b.start),
            a.end.pos_max(&b.end),
        ))
    }
}

// ---------------------------------------------------------------------------
// MultiCursorSession extensions
// ---------------------------------------------------------------------------

impl MultiCursorSession {
    pub fn total_selected_lines(&self) -> u32 {
        self.selections.iter().map(|s| s.line_span()).sum()
    }

    pub fn bounding_range(&self) -> Option<Selection> {
        if self.cursors.is_empty() && self.selections.is_empty() {
            return None;
        }
        let mut min_pos = CursorPosition::new(u32::MAX, u32::MAX);
        let mut max_pos = CursorPosition::new(1, 1);

        for c in &self.cursors {
            min_pos = min_pos.pos_min(c);
            max_pos = max_pos.pos_max(c);
        }
        for s in &self.selections {
            let n = s.normalized();
            min_pos = min_pos.pos_min(&n.start);
            max_pos = max_pos.pos_max(&n.end);
        }
        Some(Selection::new(min_pos, max_pos))
    }

    pub fn iter_cursors(&self) -> std::slice::Iter<'_, CursorPosition> {
        self.cursors.iter()
    }

    pub fn find_at_line(&self, line: u32) -> Vec<&CursorPosition> {
        self.cursors.iter().filter(|c| c.line == line).collect()
    }
}

impl<'a> IntoIterator for &'a MultiCursorSession {
    type Item = &'a CursorPosition;
    type IntoIter = std::slice::Iter<'a, CursorPosition>;

    fn into_iter(self) -> Self::IntoIter {
        self.cursors.iter()
    }
}

// ---------------------------------------------------------------------------
// ColumnSelectionMode extensions
// ---------------------------------------------------------------------------

impl ColumnSelectionMode {
    pub fn is_block(&self) -> bool {
        self.anchor_column > 0
    }
}

impl fmt::Display for ColumnSelectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ColumnSelectionMode(anchor={})", self.anchor_column)
    }
}

// ---------------------------------------------------------------------------
// SelectionSummary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionSummary {
    pub total_selections: usize,
    pub overlapping_count: usize,
    pub total_lines: u32,
}

impl SelectionSummary {
    pub fn from_session(session: &MultiCursorSession) -> Self {
        let total_selections = session.selections.len();
        let total_lines = session.total_selected_lines();
        let merged = cursor_merge_overlapping(&session.selections);
        let overlapping_count = if total_selections > merged.len() {
            total_selections - merged.len()
        } else {
            0
        };
        Self {
            total_selections,
            overlapping_count,
            total_lines,
        }
    }
}

impl fmt::Display for SelectionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SelectionSummary(selections={}, overlapping={}, lines={})",
            self.total_selections, self.overlapping_count, self.total_lines
        )
    }
}

// ---------------------------------------------------------------------------
// Selection sorting helpers
// ---------------------------------------------------------------------------

pub fn sort_selections(selections: &mut [Selection]) {
    selections.sort_by(|a, b| {
        let an = a.normalized();
        let bn = b.normalized();
        (an.start.line, an.start.column).cmp(&(bn.start.line, bn.start.column))
    });
}

pub fn deduplicate_selections(selections: &[Selection]) -> Vec<Selection> {
    let mut normalized: Vec<Selection> = selections.iter().map(|s| s.normalized()).collect();
    normalized.sort_by(|a, b| {
        (a.start.line, a.start.column, a.end.line, a.end.column)
            .cmp(&(b.start.line, b.start.column, b.end.line, b.end.column))
    });
    normalized.dedup();
    normalized
}

// ---------------------------------------------------------------------------
// Text transformation for multi-cursor edits
// ---------------------------------------------------------------------------

/// Kinds of text transformation that can be applied at each cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTransform {
    Uppercase,
    Lowercase,
    /// Convert `snake_case` or space-separated words to `camelCase`.
    CamelCase,
    /// Convert `camelCase` or space-separated words to `snake_case`.
    SnakeCase,
    /// Reverse the characters in the string.
    Reverse,
}

impl TextTransform {
    /// Apply this transformation to the given text.
    pub fn apply(&self, text: &str) -> String {
        match self {
            Self::Uppercase => text.to_uppercase(),
            Self::Lowercase => text.to_lowercase(),
            Self::CamelCase => to_camel_case(text),
            Self::SnakeCase => to_snake_case(text),
            Self::Reverse => text.chars().rev().collect(),
        }
    }
}

/// Split text on underscores or spaces and join as camelCase.
fn to_camel_case(text: &str) -> String {
    let words: Vec<&str> = text.split(|c: char| c == '_' || c == ' ')
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return String::new();
    }
    let mut result = words[0].to_lowercase();
    for word in &words[1..] {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            result.push(first.to_uppercase().next().unwrap_or(first));
            result.extend(chars.flat_map(|c| c.to_lowercase()));
        }
    }
    result
}

/// Split camelCase boundaries, underscores, or spaces and join with underscores.
fn to_snake_case(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 4);
    let mut prev_was_upper = false;
    for (i, ch) in text.chars().enumerate() {
        if ch == '_' || ch == ' ' {
            if !result.ends_with('_') {
                result.push('_');
            }
            prev_was_upper = false;
            continue;
        }
        if ch.is_uppercase() && i > 0 && !prev_was_upper && !result.ends_with('_') {
            result.push('_');
        }
        result.extend(ch.to_lowercase());
        prev_was_upper = ch.is_uppercase();
    }
    result
}

/// Result of applying a multi-cursor text transformation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformResult {
    pub original: String,
    pub transformed: String,
    pub cursor: CursorPosition,
}

/// Apply a [`TextTransform`] to a list of `(cursor, selected_text)` pairs.
pub fn apply_transform_at_cursors(
    pairs: &[(CursorPosition, &str)],
    transform: TextTransform,
) -> Vec<TransformResult> {
    pairs.iter().map(|(pos, text)| TransformResult {
        original: (*text).to_string(),
        transformed: transform.apply(text),
        cursor: *pos,
    }).collect()
}

// ---------------------------------------------------------------------------
// Cursor grouping by column alignment
// ---------------------------------------------------------------------------

/// Group cursors that share the same column value.
///
/// Returns a map from column number to the list of cursor positions at that
/// column, sorted by line within each group.
pub fn group_cursors_by_column(
    cursors: &[CursorPosition],
) -> std::collections::BTreeMap<u32, Vec<CursorPosition>> {
    let mut groups: std::collections::BTreeMap<u32, Vec<CursorPosition>> =
        std::collections::BTreeMap::new();
    for c in cursors {
        groups.entry(c.column).or_default().push(*c);
    }
    for positions in groups.values_mut() {
        positions.sort_by_key(|p| p.line);
    }
    groups
}

// ---------------------------------------------------------------------------
// Rectangular / block selection
// ---------------------------------------------------------------------------

/// A rectangular (block) selection defined by two corner positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockSelection {
    pub top_left: CursorPosition,
    pub bottom_right: CursorPosition,
}

impl BlockSelection {
    /// Create a block selection from any two corner positions.
    pub fn from_corners(a: CursorPosition, b: CursorPosition) -> Self {
        let top = a.line.min(b.line);
        let bottom = a.line.max(b.line);
        let left = a.column.min(b.column);
        let right = a.column.max(b.column);
        Self {
            top_left: CursorPosition::new(top, left),
            bottom_right: CursorPosition::new(bottom, right),
        }
    }

    /// Number of lines in this block.
    pub fn height(&self) -> u32 {
        self.bottom_right.line - self.top_left.line + 1
    }

    /// Width of this block in columns.
    pub fn width(&self) -> u32 {
        self.bottom_right.column - self.top_left.column + 1
    }

    /// Returns `true` if the given position is inside the block (inclusive).
    pub fn contains(&self, pos: &CursorPosition) -> bool {
        pos.line >= self.top_left.line
            && pos.line <= self.bottom_right.line
            && pos.column >= self.top_left.column
            && pos.column <= self.bottom_right.column
    }

    /// Expand the per-line selections from this block, clamping each line's
    /// right edge to `max_column_fn(line)`.
    pub fn to_line_selections(
        &self,
        max_column_fn: impl Fn(u32) -> u32,
    ) -> Vec<Selection> {
        (self.top_left.line..=self.bottom_right.line)
            .map(|line| {
                let max_col = max_column_fn(line);
                let left = self.top_left.column.min(max_col);
                let right = self.bottom_right.column.min(max_col);
                Selection::new(
                    CursorPosition::new(line, left),
                    CursorPosition::new(line, right),
                )
            })
            .collect()
    }

    /// Generate one cursor per line at the right edge of the block, clamped.
    pub fn right_edge_cursors(
        &self,
        max_column_fn: impl Fn(u32) -> u32,
    ) -> Vec<CursorPosition> {
        (self.top_left.line..=self.bottom_right.line)
            .map(|line| {
                let col = self.bottom_right.column.min(max_column_fn(line));
                CursorPosition::new(line, col)
            })
            .collect()
    }
}

impl fmt::Display for BlockSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Block[{},{} -> {},{}]",
            self.top_left.line, self.top_left.column,
            self.bottom_right.line, self.bottom_right.column,
        )
    }
}

// ---------------------------------------------------------------------------
// Multi-cursor undo / redo history
// ---------------------------------------------------------------------------

/// A snapshot of cursor and selection state for undo/redo.
#[derive(Debug, Clone, PartialEq)]
struct CursorSnapshot {
    cursors: Vec<CursorPosition>,
    selections: Vec<Selection>,
}

/// Tracks undo/redo history for a [`MultiCursorSession`].
#[derive(Debug, Clone)]
pub struct CursorHistory {
    undo_stack: Vec<CursorSnapshot>,
    redo_stack: Vec<CursorSnapshot>,
    max_entries: usize,
}

impl CursorHistory {
    /// Create a new history tracker with the given maximum number of undo entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries,
        }
    }

    /// Save the current session state before a mutation.
    pub fn save(&mut self, session: &MultiCursorSession) {
        if self.undo_stack.len() >= self.max_entries {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(CursorSnapshot {
            cursors: session.cursors.clone(),
            selections: session.selections.clone(),
        });
        self.redo_stack.clear();
    }

    /// Undo the last change, restoring `session` to its previous state.
    /// Returns `true` if an undo was performed.
    pub fn undo(&mut self, session: &mut MultiCursorSession) -> bool {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(CursorSnapshot {
                cursors: session.cursors.clone(),
                selections: session.selections.clone(),
            });
            session.cursors = snapshot.cursors;
            session.selections = snapshot.selections;
            true
        } else {
            false
        }
    }

    /// Redo the last undone change. Returns `true` if a redo was performed.
    pub fn redo(&mut self, session: &mut MultiCursorSession) -> bool {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(CursorSnapshot {
                cursors: session.cursors.clone(),
                selections: session.selections.clone(),
            });
            session.cursors = snapshot.cursors;
            session.selections = snapshot.selections;
            true
        } else {
            false
        }
    }

    /// Number of undo entries available.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of redo entries available.
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

// ---------------------------------------------------------------------------
// MultiCursorColumns – column/box selection
// ---------------------------------------------------------------------------

/// Manages column (rectangular/box) selection across multiple lines.
#[derive(Debug, Clone)]
pub struct MultiCursorColumns {
    pub anchor_line: u32,
    pub anchor_column: u32,
    pub active_line: u32,
    pub active_column: u32,
}

impl MultiCursorColumns {
    pub fn new(anchor_line: u32, anchor_column: u32) -> Self {
        Self {
            anchor_line,
            anchor_column,
            active_line: anchor_line,
            active_column: anchor_column,
        }
    }

    /// Extend the selection to a new line and column.
    pub fn extend_to(&mut self, line: u32, column: u32) {
        self.active_line = line;
        self.active_column = column;
    }

    /// Get the range of lines covered (inclusive, sorted).
    pub fn line_range(&self) -> (u32, u32) {
        let min = self.anchor_line.min(self.active_line);
        let max = self.anchor_line.max(self.active_line);
        (min, max)
    }

    /// Get the column range (inclusive, sorted).
    pub fn column_range(&self) -> (u32, u32) {
        let min = self.anchor_column.min(self.active_column);
        let max = self.anchor_column.max(self.active_column);
        (min, max)
    }

    /// Number of lines in the selection.
    pub fn line_count(&self) -> u32 {
        let (min, max) = self.line_range();
        max - min + 1
    }

    /// Generate cursor positions for each line in the column selection.
    pub fn cursor_positions(&self) -> Vec<CursorPosition> {
        let (min_line, max_line) = self.line_range();
        let col = self.active_column;
        (min_line..=max_line)
            .map(|line| CursorPosition::new(line, col))
            .collect()
    }

    /// Width of the selection in columns.
    pub fn width(&self) -> u32 {
        let (min, max) = self.column_range();
        max.saturating_sub(min)
    }
}

impl fmt::Display for MultiCursorColumns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ColumnSel({}:{}-{}:{}, {} lines)",
            self.anchor_line, self.anchor_column,
            self.active_line, self.active_column,
            self.line_count()
        )
    }
}

// ---------------------------------------------------------------------------
// MultiCursorMatch – pattern-based cursor placement
// ---------------------------------------------------------------------------

/// Places cursors at all positions matching a pattern in text.
#[derive(Debug, Clone)]
pub struct MultiCursorMatch {
    pub pattern: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

impl MultiCursorMatch {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            case_sensitive: true,
            whole_word: false,
        }
    }

    pub fn case_insensitive(mut self) -> Self {
        self.case_sensitive = false;
        self
    }

    pub fn whole_word(mut self) -> Self {
        self.whole_word = true;
        self
    }

    /// Find all match positions (line, column) in the given lines. Returns 1-based positions.
    pub fn find_all(&self, lines: &[&str]) -> Vec<CursorPosition> {
        let mut positions = Vec::new();
        let pat = if self.case_sensitive {
            self.pattern.clone()
        } else {
            self.pattern.to_lowercase()
        };

        for (li, line) in lines.iter().enumerate() {
            let search_line = if self.case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };

            let mut start = 0;
            while let Some(idx) = search_line[start..].find(&pat) {
                let col = start + idx;
                if self.whole_word {
                    let before_ok = col == 0 || !search_line.as_bytes()[col - 1].is_ascii_alphanumeric();
                    let after_ok = col + pat.len() >= search_line.len()
                        || !search_line.as_bytes()[col + pat.len()].is_ascii_alphanumeric();
                    if before_ok && after_ok {
                        positions.push(CursorPosition::new((li + 1) as u32, (col + 1) as u32));
                    }
                } else {
                    positions.push(CursorPosition::new((li + 1) as u32, (col + 1) as u32));
                }
                start = col + 1;
            }
        }
        positions
    }

    /// Count total matches.
    pub fn count_matches(&self, lines: &[&str]) -> usize {
        self.find_all(lines).len()
    }
}

impl fmt::Display for MultiCursorMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MultiCursorMatch(\"{}\")", self.pattern)
    }
}

// ---------------------------------------------------------------------------
// Cursor merge strategies
// ---------------------------------------------------------------------------

/// Strategy for merging overlapping cursors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMergeStrategy {
    /// Keep the first cursor in each overlapping group.
    KeepFirst,
    /// Keep the last cursor in each overlapping group.
    KeepLast,
    /// Merge overlapping cursors into a single selection spanning all.
    Union,
}

impl fmt::Display for CursorMergeStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeepFirst => write!(f, "KeepFirst"),
            Self::KeepLast => write!(f, "KeepLast"),
            Self::Union => write!(f, "Union"),
        }
    }
}

/// Merge cursor positions that are on the same line and column.
pub fn merge_duplicate_cursors(positions: &[CursorPosition], strategy: CursorMergeStrategy) -> Vec<CursorPosition> {
    if positions.is_empty() {
        return Vec::new();
    }
    let mut sorted = positions.to_vec();
    sorted.sort();
    sorted.dedup();

    match strategy {
        CursorMergeStrategy::KeepFirst | CursorMergeStrategy::Union => sorted,
        CursorMergeStrategy::KeepLast => {
            sorted.reverse();
            sorted.dedup();
            sorted.reverse();
            sorted
        }
    }
}

// ---------------------------------------------------------------------------
// Multi-cursor clipboard – per-cursor paste content
// ---------------------------------------------------------------------------

/// Clipboard that tracks per-cursor content for multi-cursor paste.
#[derive(Debug, Clone)]
pub struct MultiCursorClipboard {
    /// One entry per cursor.
    entries: Vec<String>,
}

impl MultiCursorClipboard {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Copy text from each cursor (one string per cursor).
    pub fn copy_from_cursors(&mut self, texts: Vec<String>) {
        self.entries = texts;
    }

    /// Get the paste text for cursor at `index`. Falls back to full content if
    /// index is out of range.
    pub fn paste_for_cursor(&self, index: usize) -> &str {
        self.entries.get(index).map(|s| s.as_str()).unwrap_or_else(|| {
            self.entries.last().map(|s| s.as_str()).unwrap_or("")
        })
    }

    /// Number of clipboard entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Whether the clipboard has per-cursor entries matching cursor count.
    pub fn matches_cursor_count(&self, cursor_count: usize) -> bool {
        self.entries.len() == cursor_count
    }

    /// Get all entries joined with a separator.
    pub fn joined(&self, separator: &str) -> String {
        self.entries.join(separator)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for MultiCursorClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MultiCursorClipboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MultiCursorClipboard({} entries)", self.entries.len())
    }
}


// ---------------------------------------------------------------------------
// MulticursorColumnExtender
// ---------------------------------------------------------------------------

/// Extends cursors to fill a rectangular column selection area.
///
/// Given an anchor position and a target position, this computes a list of
/// cursor positions that fill every line in the range at the specified column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticursorColumnExtender {
    /// The column at which cursors are placed.
    column: u32,
    /// Starting line (inclusive, 1-based).
    start_line: u32,
    /// Ending line (inclusive, 1-based).
    end_line: u32,
    /// Generated cursor positions.
    positions: Vec<CursorPosition>,
}

impl MulticursorColumnExtender {
    /// Create a new column extender from an anchor and a target line.
    pub fn new(column: u32, start_line: u32, end_line: u32) -> Result<Self, MultiCursorError> {
        if column == 0 {
            return Err(MultiCursorError::InvalidPosition { line: start_line, column });
        }
        if start_line == 0 || end_line == 0 {
            return Err(MultiCursorError::InvalidPosition { line: 0, column });
        }
        let (lo, hi) = if start_line <= end_line {
            (start_line, end_line)
        } else {
            (end_line, start_line)
        };
        let positions: Vec<CursorPosition> = (lo..=hi)
            .map(|line| CursorPosition::new(line, column))
            .collect();
        Ok(Self {
            column,
            start_line: lo,
            end_line: hi,
            positions,
        })
    }

    /// Return the number of cursors generated.
    pub fn cursor_count(&self) -> usize {
        self.positions.len()
    }

    /// Return a slice of the generated positions.
    pub fn positions(&self) -> &[CursorPosition] {
        &self.positions
    }

    /// The column all cursors share.
    pub fn column(&self) -> u32 {
        self.column
    }

    /// The start line of the column selection.
    pub fn start_line(&self) -> u32 {
        self.start_line
    }

    /// The end line of the column selection.
    pub fn end_line(&self) -> u32 {
        self.end_line
    }

    /// Line span covered by this extender.
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Shift all cursor positions by a line delta.
    pub fn shift_lines(&mut self, delta: i64) {
        let new_start = (self.start_line as i64 + delta).max(1) as u32;
        let new_end = (self.end_line as i64 + delta).max(1) as u32;
        self.start_line = new_start;
        self.end_line = new_end;
        self.positions = (new_start..=new_end)
            .map(|line| CursorPosition::new(line, self.column))
            .collect();
    }

    /// Shift the column by a delta.
    pub fn shift_column(&mut self, delta: i64) {
        let new_col = (self.column as i64 + delta).max(1) as u32;
        self.column = new_col;
        for pos in &mut self.positions {
            *pos = CursorPosition::new(pos.line, new_col);
        }
    }

    /// Expand the selection by one line in each direction (clamped at 1).
    pub fn expand(&mut self) {
        if self.start_line > 1 {
            self.start_line -= 1;
        }
        self.end_line += 1;
        self.positions = (self.start_line..=self.end_line)
            .map(|line| CursorPosition::new(line, self.column))
            .collect();
    }

    /// Shrink the selection by one line on each side, if possible.
    pub fn shrink(&mut self) {
        if self.end_line - self.start_line >= 2 {
            self.start_line += 1;
            self.end_line -= 1;
            self.positions = (self.start_line..=self.end_line)
                .map(|line| CursorPosition::new(line, self.column))
                .collect();
        }
    }

    /// Check if a given position is within the column selection.
    pub fn contains(&self, pos: &CursorPosition) -> bool {
        pos.column == self.column && pos.line >= self.start_line && pos.line <= self.end_line
    }

    /// Merge two column extenders that share the same column.
    pub fn merge(&self, other: &Self) -> Result<Self, MultiCursorError> {
        if self.column != other.column {
            return Err(MultiCursorError::InvalidPosition {
                line: other.start_line,
                column: other.column,
            });
        }
        let lo = self.start_line.min(other.start_line);
        let hi = self.end_line.max(other.end_line);
        Self::new(self.column, lo, hi)
    }
}

impl fmt::Display for MulticursorColumnExtender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ColumnExtender(col={}, lines={}..={}, count={})",
            self.column,
            self.start_line,
            self.end_line,
            self.cursor_count()
        )
    }
}

// ---------------------------------------------------------------------------
// MulticursorTypeFilter
// ---------------------------------------------------------------------------

/// Criteria for filtering cursors by position attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorFilterCriterion {
    /// Keep cursors on a specific line.
    OnLine(u32),
    /// Keep cursors within a line range (inclusive).
    InLineRange(u32, u32),
    /// Keep cursors at a specific column.
    AtColumn(u32),
    /// Keep cursors where column >= threshold.
    MinColumn(u32),
    /// Keep cursors where column <= threshold.
    MaxColumn(u32),
    /// Keep cursors where line is even.
    EvenLines,
    /// Keep cursors where line is odd.
    OddLines,
}

/// Filters a set of cursor positions by one or more criteria.
#[derive(Debug, Clone)]
pub struct MulticursorTypeFilter {
    criteria: Vec<CursorFilterCriterion>,
    /// When true, ALL criteria must match (AND). When false, ANY criterion suffices (OR).
    require_all: bool,
}

impl MulticursorTypeFilter {
    /// Create a new filter that requires all criteria to match.
    pub fn all_of(criteria: Vec<CursorFilterCriterion>) -> Self {
        Self { criteria, require_all: true }
    }

    /// Create a new filter that requires any criterion to match.
    pub fn any_of(criteria: Vec<CursorFilterCriterion>) -> Self {
        Self { criteria, require_all: false }
    }

    /// Create an empty filter that passes everything.
    pub fn pass_all() -> Self {
        Self { criteria: Vec::new(), require_all: true }
    }

    /// Add a criterion to this filter.
    pub fn add(&mut self, criterion: CursorFilterCriterion) {
        self.criteria.push(criterion);
    }

    /// Number of criteria in this filter.
    pub fn criteria_count(&self) -> usize {
        self.criteria.len()
    }

    /// Check if a single position matches a single criterion.
    fn matches_criterion(pos: &CursorPosition, criterion: &CursorFilterCriterion) -> bool {
        match criterion {
            CursorFilterCriterion::OnLine(line) => pos.line == *line,
            CursorFilterCriterion::InLineRange(lo, hi) => pos.line >= *lo && pos.line <= *hi,
            CursorFilterCriterion::AtColumn(col) => pos.column == *col,
            CursorFilterCriterion::MinColumn(min) => pos.column >= *min,
            CursorFilterCriterion::MaxColumn(max) => pos.column <= *max,
            CursorFilterCriterion::EvenLines => pos.line % 2 == 0,
            CursorFilterCriterion::OddLines => pos.line % 2 == 1,
        }
    }

    /// Check if a position passes this filter.
    pub fn matches(&self, pos: &CursorPosition) -> bool {
        if self.criteria.is_empty() {
            return true;
        }
        if self.require_all {
            self.criteria.iter().all(|c| Self::matches_criterion(pos, c))
        } else {
            self.criteria.iter().any(|c| Self::matches_criterion(pos, c))
        }
    }

    /// Filter a slice of positions, returning those that match.
    pub fn apply(&self, positions: &[CursorPosition]) -> Vec<CursorPosition> {
        positions.iter().filter(|p| self.matches(p)).cloned().collect()
    }

    /// Count how many positions match.
    pub fn count_matches(&self, positions: &[CursorPosition]) -> usize {
        positions.iter().filter(|p| self.matches(p)).count()
    }

    /// Partition positions into (matched, unmatched).
    pub fn partition(&self, positions: &[CursorPosition]) -> (Vec<CursorPosition>, Vec<CursorPosition>) {
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for p in positions {
            if self.matches(p) {
                matched.push(p.clone());
            } else {
                unmatched.push(p.clone());
            }
        }
        (matched, unmatched)
    }

    /// Return true if the filter is in AND mode.
    pub fn is_require_all(&self) -> bool {
        self.require_all
    }

    /// Clear all criteria.
    pub fn clear(&mut self) {
        self.criteria.clear();
    }
}

impl fmt::Display for MulticursorTypeFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode = if self.require_all { "ALL" } else { "ANY" };
        write!(f, "TypeFilter({}, {} criteria)", mode, self.criteria.len())
    }
}



// ---------------------------------------------------------------------------
// vsedit-multicursor: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticursorXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl MulticursorXConfig {
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

impl std::fmt::Display for MulticursorXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct MulticursorXRegistry {
    entries: Vec<MulticursorXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl MulticursorXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: MulticursorXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&MulticursorXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut MulticursorXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<MulticursorXConfig> {
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

    pub fn active_entries(&self) -> Vec<&MulticursorXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&MulticursorXConfig> {
        let mut sorted: Vec<&MulticursorXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&MulticursorXConfig> {
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

    pub fn iter(&self) -> MulticursorXIterator<'_> {
        MulticursorXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct MulticursorXIterator<'a> {
    inner: std::slice::Iter<'a, MulticursorXConfig>,
}

impl<'a> Iterator for MulticursorXIterator<'a> {
    type Item = &'a MulticursorXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct MulticursorXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl MulticursorXCache {
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
pub struct MulticursorXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl MulticursorXFormatter {
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

    pub fn format_entry(&self, entry: &MulticursorXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &MulticursorXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &MulticursorXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for MulticursorXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct MulticursorXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl MulticursorXValidator {
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

    pub fn validate(&self, entry: &MulticursorXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &MulticursorXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for MulticursorXValidator {
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
// xb_ utilities – batch 73
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer73 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer73 {
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
pub fn xb_fnv1a_73(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_73<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_73<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_73(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_73(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 125
// ---------------------------------------------------------------------------

/// Generic object pool `Xc125Pool<T>`.
pub struct Xc125Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc125Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc125PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc125Pool<T> {
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
    pub fn stats(&self) -> Xc125PoolStats {
        Xc125PoolStats {
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

impl<T> Default for Xc125Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc125Scheduler`.
pub struct Xc125Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc125Scheduler {
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

impl Default for Xc125Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_125 hash for the given byte slice.
pub fn xc_125_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_125 convention.
pub fn xc_125_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe86 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe86Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe86PipelineError {
    pub stage: Xe86Stage,
    pub message: String,
}

impl std::fmt::Display for Xe86PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe86Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe86Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError>>>,
    stage_names: Vec<Xe86Stage>,
}

impl Xe86Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe86Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe86Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe86Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe86Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> {
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

    pub fn compose(mut self, other: Xe86Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe86CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe86CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe86Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe86CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe86CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe86Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe86CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_86_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe86CacheEntry {
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

    fn xe_86_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe86CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_86_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> {
    Ok(data)
}

pub fn xe_86_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_86_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_86_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_86_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe86PipelineError> {
    Err(Xe86PipelineError {
        stage: Xe86Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_84: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg84Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg84Graph {
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

impl Default for Xg84Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_84: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg84Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg84Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg84Heap<T>) {
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

impl<T: Ord> Default for Xg84Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 124).
pub struct Xh124SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh124SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 166 as u64,
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

/// A compact bit set supporting boolean operations (variant 124).
pub struct Xh124BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh124BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 124).
pub struct Xi124Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi124Deque<T> {
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
pub struct Xi124Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi124Interval {
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

/// A simple interval tree (variant 124).
pub struct Xi124IntervalTree {
    xi_intervals: Vec<Xi124Interval>,
}

impl Xi124IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi124Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi124Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi124Interval) -> Vec<&Xi124Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi124Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi124Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi124Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi124Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi124Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi124Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 124) ---

/// Disjoint set / union-find for crate 124.
pub struct Xj124UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj124UnionFind {
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

const XJ124_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 124.
pub struct Xj124BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj124BTreeNode<K, V>>>,
    len: usize,
}

struct Xj124BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj124BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj124BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ124_BTREE_ORDER - 1
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
        let mid = XJ124_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj124BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj124BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj124BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj124BTreeNode::xj_new_leaf();
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


// --- xk_124 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk124SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk124SegmentTree {
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
pub struct Xk124DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk124DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_124).
#[derive(Debug, Clone)]
pub struct Xl124Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl124Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_124).
#[derive(Debug, Clone)]
pub struct Xl124SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl124SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm124MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm124MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm124Tokenizer {
    text: String,
}

impl Xm124Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 124.
pub struct Xn124Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn124Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 124 -----

#[derive(Debug, Clone)]
struct Xn124AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn124AvlNode<K, V>>>,
    right: Option<Box<Xn124AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 124.
#[derive(Debug, Clone)]
pub struct Xn124AVL<K, V> {
    root: Option<Box<Xn124AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn124AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn124AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn124AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn124AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn124AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn124AvlNode<K, V>>) -> Box<Xn124AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn124AvlNode<K, V>>) -> Box<Xn124AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn124AvlNode<K, V>>) -> Box<Xn124AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn124AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn124AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn124AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn124AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn124AvlNode<K, V>>) -> &Xn124AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn124AvlNode<K, V>>) -> (Box<Xn124AvlNode<K, V>>, Option<Box<Xn124AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn124AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn124AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn124AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn124AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn124AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn124AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn124AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo124RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo124Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo124RBNode<K, V> {
    key: K,
    value: V,
    color: Xo124Color,
    left: Option<Box<Xo124RBNode<K, V>>>,
    right: Option<Box<Xo124RBNode<K, V>>>,
}

/// A red-black tree map for crate 124.
#[derive(Debug, Clone)]
pub struct Xo124RedBlack<K, V> {
    root: Option<Box<Xo124RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo124RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo124Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo124RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo124RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo124RBNode {
                    key, value, color: Xo124Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo124RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo124Color::Red)
    }

    fn xo_balance(mut h: Box<Xo124RBNode<K, V>>) -> Box<Xo124RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo124Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo124RBNode<K, V>>) -> Box<Xo124RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo124Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo124RBNode<K, V>>) -> Box<Xo124RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo124Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo124RBNode<K, V>>) {
        h.color = Xo124Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo124Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo124Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo124Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo124RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo124RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo124RBNode<K, V>) -> (K, V, Option<Box<Xo124RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo124RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo124Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo124RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo124ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 124.
#[derive(Debug, Clone)]
pub struct Xo124ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo124ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo124#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo124#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 124).
#[derive(Debug)]
pub struct Xp124SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp124Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp124Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp124Node<K, V>>>,
    xp_right: Option<Box<Xp124Node<K, V>>>,
}

impl<K: Ord, V> Xp124Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp124SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp124SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp124Node<K, V>>>, key: &K) -> Option<Box<Xp124Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp124Node<K, V>>) -> Box<Xp124Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp124Node<K, V>>) -> Box<Xp124Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp124Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp124Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp124Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq124Treap ---------------

use std::cmp::Ordering as Xq124Ord;

struct Xq124TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq124TreapNode<K, V>>>,
    right: Option<Box<Xq124TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq124Treap<K, V> {
    root: Option<Box<Xq124TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq124TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_124_size<K, V>(node: &Option<Box<Xq124TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_124_update_size<K, V>(node: &mut Xq124TreapNode<K, V>) {
    node.size = 1 + xq_124_size(&node.left) + xq_124_size(&node.right);
}

fn xq_124_rotate_right<K, V>(mut node: Box<Xq124TreapNode<K, V>>) -> Box<Xq124TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_124_update_size(&mut node);
    left.right = Some(node);
    xq_124_update_size(&mut left);
    left
}

fn xq_124_rotate_left<K, V>(mut node: Box<Xq124TreapNode<K, V>>) -> Box<Xq124TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_124_update_size(&mut node);
    right.left = Some(node);
    xq_124_update_size(&mut right);
    right
}

fn xq_124_insert_node<K: Ord, V>(
    node: Option<Box<Xq124TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq124TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq124TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq124Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq124Ord::Less => {
                let (new_left, old) = xq_124_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_124_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_124_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq124Ord::Greater => {
                let (new_right, old) = xq_124_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_124_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_124_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_124_remove_node<K: Ord, V>(
    node: Option<Box<Xq124TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq124TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq124Ord::Less => {
                let (new_left, old) = xq_124_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_124_update_size(&mut n);
                (Some(n), old)
            }
            Xq124Ord::Greater => {
                let (new_right, old) = xq_124_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_124_update_size(&mut n);
                (Some(n), old)
            }
            Xq124Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_124_rotate_right(n);
                    let (new_right, old) = xq_124_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_124_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_124_rotate_left(n);
                    let (new_left, old) = xq_124_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_124_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_124_find_min<K, V>(node: &Option<Box<Xq124TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_124_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_124_find_max<K, V>(node: &Option<Box<Xq124TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_124_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_124_rank<K: Ord, V>(node: &Option<Box<Xq124TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq124Ord::Less => xq_124_rank(&n.left, key),
            Xq124Ord::Equal => xq_124_size(&n.left),
            Xq124Ord::Greater => 1 + xq_124_size(&n.left) + xq_124_rank(&n.right, key),
        },
    }
}

fn xq_124_kth<K, V>(node: &Option<Box<Xq124TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_124_size(&n.left);
        if k < left_size {
            xq_124_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_124_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_124_in_order<K: Clone, V>(node: &Option<Box<Xq124TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_124_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_124_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq124Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 124 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_124_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq124Ord::Equal => return Some(&n.value),
                Xq124Ord::Less => cur = &n.left,
                Xq124Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_124_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_124_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_124_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_124_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_124_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_124_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_124_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq124VEBTree ---------------

pub struct Xq124VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq124VEBTree>>,
    clusters: Vec<Option<Box<Xq124VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq124VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq124VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq124VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_cursors() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        session.add_cursor(CursorPosition::new(2, 5));
        assert_eq!(session.cursor_count(), 2);
        assert!(session.has_multiple_cursors());

        let removed = session.remove_cursor(0);
        assert_eq!(removed, Some(CursorPosition::new(1, 1)));
        assert_eq!(session.cursor_count(), 1);
        assert!(!session.has_multiple_cursors());
    }

    #[test]
    fn sort_and_deduplicate_removes_dups() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(3, 1));
        session.add_cursor(CursorPosition::new(1, 1));
        session.add_cursor(CursorPosition::new(3, 1)); // duplicate
        session.sort_and_deduplicate();
        assert_eq!(session.cursor_count(), 2);
        assert_eq!(session.cursors[0], CursorPosition::new(1, 1));
        assert_eq!(session.cursors[1], CursorPosition::new(3, 1));
    }

    #[test]
    fn add_cursor_above_and_below() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(3, 10));

        // Line widths: line 2 has max col 5, line 4 has max col 15
        session.add_cursor_above(|_| 5);
        assert_eq!(session.cursor_count(), 2);
        assert_eq!(session.cursors[1], CursorPosition::new(2, 5));

        // add_cursor_below uses the last cursor (line 2, col 5)
        session.add_cursor_below(10, |_| 15);
        assert_eq!(session.cursor_count(), 3);
        assert_eq!(session.cursors[2], CursorPosition::new(3, 5));
    }

    #[test]
    fn column_selection_mode() {
        let csm = ColumnSelectionMode::new(3);
        // 3 lines, target column 8, all lines have max col 10
        let sels = csm.compute_selections(1, 3, 8, |_| 10);
        assert_eq!(sels.len(), 3);
        for sel in &sels {
            assert_eq!(sel.start.column, 3);
            assert_eq!(sel.end.column, 8);
        }

        // Short line clamps both anchor and target
        let sels = csm.compute_selections(1, 1, 8, |_| 4);
        assert_eq!(sels[0].start.column, 3);
        assert_eq!(sels[0].end.column, 4);
    }

    #[test]
    fn clear_removes_everything() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        session.selections.push(Selection::new(
            CursorPosition::new(1, 1),
            CursorPosition::new(1, 5),
        ));
        session.clear();
        assert_eq!(session.cursor_count(), 0);
        assert!(session.selections.is_empty());
    }

    #[test]
    fn cursor_position_try_new_rejects_zero() {
        assert!(CursorPosition::try_new(0, 1).is_err());
        assert!(CursorPosition::try_new(1, 0).is_err());
        assert!(CursorPosition::try_new(0, 0).is_err());
        assert_eq!(CursorPosition::try_new(1, 1).unwrap(), CursorPosition::new(1, 1));
    }

    #[test]
    fn cursor_position_offset_clamps() {
        let p = CursorPosition::new(3, 5);
        assert_eq!(p.offset(-10, -10), CursorPosition::new(1, 1));
        assert_eq!(p.offset(2, 3), CursorPosition::new(5, 8));
    }

    #[test]
    fn cursor_position_distance() {
        let a = CursorPosition::new(1, 1);
        let b = CursorPosition::new(4, 6);
        assert_eq!(a.distance_to(&b), 8);
        assert_eq!(b.distance_to(&a), 8);
    }

    #[test]
    fn cursor_position_display() {
        assert_eq!(format!("{}", CursorPosition::new(10, 25)), "10:25");
    }

    #[test]
    fn selection_normalized_and_contains() {
        let sel = Selection::new(CursorPosition::new(3, 8), CursorPosition::new(1, 2));
        let n = sel.normalized();
        assert_eq!(n.start, CursorPosition::new(1, 2));
        assert_eq!(n.end, CursorPosition::new(3, 8));
        assert!(sel.contains(&CursorPosition::new(2, 5)));
        assert!(!sel.contains(&CursorPosition::new(4, 1)));
    }

    #[test]
    fn selection_line_span() {
        let sel = Selection::new(CursorPosition::new(2, 1), CursorPosition::new(5, 3));
        assert_eq!(sel.line_span(), 4);
        let single = Selection::new(CursorPosition::new(7, 1), CursorPosition::new(7, 10));
        assert_eq!(single.line_span(), 1);
    }

    #[test]
    fn selection_overlaps() {
        let a = Selection::new(CursorPosition::new(1, 1), CursorPosition::new(3, 5));
        let b = Selection::new(CursorPosition::new(3, 3), CursorPosition::new(5, 1));
        assert!(a.overlaps(&b));
        let c = Selection::new(CursorPosition::new(4, 1), CursorPosition::new(6, 1));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn selection_display() {
        let sel = Selection::new(CursorPosition::new(1, 1), CursorPosition::new(2, 3));
        assert_eq!(format!("{sel}"), "[1:1 -> 2:3]");
    }

    #[test]
    fn try_add_cursor_validates() {
        let mut session = MultiCursorSession::new();
        assert!(session.try_add_cursor(CursorPosition::new(0, 5)).is_err());
        assert!(session.try_add_cursor(CursorPosition::new(1, 1)).is_ok());
        assert_eq!(session.cursor_count(), 1);
    }

    #[test]
    fn try_remove_cursor_out_of_bounds() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        assert!(session.try_remove_cursor(5).is_err());
        assert!(session.try_remove_cursor(0).is_ok());
    }

    #[test]
    fn move_all_cursors() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(2, 3));
        session.add_cursor(CursorPosition::new(4, 1));
        session.move_all(1, 2);
        assert_eq!(session.cursors[0], CursorPosition::new(3, 5));
        assert_eq!(session.cursors[1], CursorPosition::new(5, 3));
    }

    #[test]
    fn bounding_box_computation() {
        let mut session = MultiCursorSession::new();
        assert!(session.bounding_box().is_none());
        session.add_cursor(CursorPosition::new(5, 10));
        session.add_cursor(CursorPosition::new(2, 3));
        session.add_cursor(CursorPosition::new(8, 7));
        let (tl, br) = session.bounding_box().unwrap();
        assert_eq!(tl, CursorPosition::new(2, 3));
        assert_eq!(br, CursorPosition::new(8, 10));
    }

    #[test]
    fn merge_overlapping_selections() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        session.selections.push(Selection::new(
            CursorPosition::new(1, 1),
            CursorPosition::new(3, 5),
        ));
        session.selections.push(Selection::new(
            CursorPosition::new(3, 3),
            CursorPosition::new(5, 2),
        ));
        session.selections.push(Selection::new(
            CursorPosition::new(7, 1),
            CursorPosition::new(8, 1),
        ));
        session.merge_overlapping_selections();
        assert_eq!(session.selections.len(), 2);
        assert_eq!(session.selections[0].end, CursorPosition::new(5, 2));
        assert_eq!(session.selections[1].start, CursorPosition::new(7, 1));
    }

    #[test]
    fn builder_validates_and_builds() {
        // Empty builder should fail.
        let res = MultiCursorSessionBuilder::new().build();
        assert!(res.is_err());

        // Invalid position should fail.
        let res = MultiCursorSessionBuilder::new().cursor(0, 1);
        assert!(res.is_err());

        // Valid build with dedup.
        let session = MultiCursorSessionBuilder::new()
            .cursor(3, 1).unwrap()
            .cursor(1, 1).unwrap()
            .cursor(3, 1).unwrap()
            .deduplicate(true)
            .build()
            .unwrap();
        assert_eq!(session.cursor_count(), 2);
        assert_eq!(session.cursors[0], CursorPosition::new(1, 1));
    }

    #[test]
    fn session_display() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        assert_eq!(
            format!("{session}"),
            "MultiCursorSession(1 cursors, 0 selections)"
        );
    }

    #[test]
    fn column_selection_try_compute_validates() {
        let csm = ColumnSelectionMode::new(3);
        // start_line 0 is invalid.
        assert!(csm.try_compute_selections(0, 3, 5, 10, |_| 10).is_err());
        // end_line exceeds line_count.
        assert!(csm.try_compute_selections(1, 11, 5, 10, |_| 10).is_err());
        // Valid.
        let sels = csm.try_compute_selections(1, 3, 5, 10, |_| 10).unwrap();
        assert_eq!(sels.len(), 3);
    }

    #[test]
    fn column_selection_cursor_positions() {
        let csm = ColumnSelectionMode::new(1);
        let positions = csm.cursor_positions(2, 5, 8, |line| if line == 3 { 4 } else { 10 });
        assert_eq!(positions.len(), 4);
        assert_eq!(positions[1], CursorPosition::new(3, 4)); // clamped
        assert_eq!(positions[0], CursorPosition::new(2, 8));
    }

    #[test]
    fn error_display_messages() {
        let e = MultiCursorError::InvalidPosition { line: 0, column: 5 };
        assert!(format!("{e}").contains("invalid position"));
        let e = MultiCursorError::IndexOutOfBounds { index: 3, len: 2 };
        assert!(format!("{e}").contains("out of bounds"));
        let e = MultiCursorError::EmptyCursors;
        assert!(format!("{e}").contains("at least one cursor"));
        let e = MultiCursorError::InvalidRange { start: 0, end: 5, max: 10 };
        assert!(format!("{e}").contains("invalid range"));
    }

    #[test]
    fn multicursor_stats_new_defaults() {
        let stats = MulticursorStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn multicursor_stats_record_success() {
        let mut stats = MulticursorStats::new();
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
    fn multicursor_stats_record_failure() {
        let mut stats = MulticursorStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn multicursor_stats_reset() {
        let mut stats = MulticursorStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn multicursor_stats_merge() {
        let mut a = MulticursorStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = MulticursorStats::new();
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
    fn multicursor_stats_display() {
        let mut stats = MulticursorStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn multicursor_stats_default() {
        let stats = MulticursorStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn multicursor_validator_accepts_valid_name() {
        let v = MulticursorValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn multicursor_validator_rejects_empty() {
        let v = MulticursorValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn multicursor_validator_rejects_too_long() {
        let v = MulticursorValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn multicursor_validator_forbidden_prefix() {
        let v = MulticursorValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn multicursor_validator_allowed_chars() {
        let v = MulticursorValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn multicursor_validator_range() {
        let v = MulticursorValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn multicursor_sanitize_removes_control() {
        let result = MulticursorValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn multicursor_truncate_short_string() {
        assert_eq!(MulticursorValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn multicursor_truncate_long_string() {
        let result = MulticursorValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn multicursor_is_ascii_printable() {
        assert!(MulticursorValidator::is_ascii_printable("Hello World 123"));
        assert!(!MulticursorValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- cursor_merge_overlapping --

    #[test]
    fn merge_non_overlapping() {
        let sels = vec![
            Selection { start: CursorPosition { line: 1, column: 1 }, end: CursorPosition { line: 1, column: 5 } },
            Selection { start: CursorPosition { line: 2, column: 1 }, end: CursorPosition { line: 2, column: 5 } },
        ];
        let merged = cursor_merge_overlapping(&sels);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_overlapping_same_line() {
        let sels = vec![
            Selection { start: CursorPosition { line: 1, column: 1 }, end: CursorPosition { line: 1, column: 10 } },
            Selection { start: CursorPosition { line: 1, column: 5 }, end: CursorPosition { line: 1, column: 15 } },
        ];
        let merged = cursor_merge_overlapping(&sels);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start.column, 1);
        assert_eq!(merged[0].end.column, 15);
    }

    #[test]
    fn merge_adjacent() {
        let sels = vec![
            Selection { start: CursorPosition { line: 1, column: 1 }, end: CursorPosition { line: 1, column: 5 } },
            Selection { start: CursorPosition { line: 1, column: 5 }, end: CursorPosition { line: 1, column: 10 } },
        ];
        let merged = cursor_merge_overlapping(&sels);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_empty_list() {
        let merged = cursor_merge_overlapping(&[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_unsorted_input() {
        let sels = vec![
            Selection { start: CursorPosition { line: 3, column: 1 }, end: CursorPosition { line: 3, column: 5 } },
            Selection { start: CursorPosition { line: 1, column: 1 }, end: CursorPosition { line: 1, column: 5 } },
        ];
        let merged = cursor_merge_overlapping(&sels);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start.line, 1);
    }

    #[test]
    fn deduplicate_cursors() {
        let positions = vec![
            CursorPosition { line: 1, column: 5 },
            CursorPosition { line: 2, column: 3 },
            CursorPosition { line: 1, column: 5 },
        ];
        let deduped = cursor_deduplicate(&positions);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn has_overlapping_detects() {
        let sels = vec![
            Selection { start: CursorPosition { line: 1, column: 1 }, end: CursorPosition { line: 1, column: 10 } },
            Selection { start: CursorPosition { line: 1, column: 5 }, end: CursorPosition { line: 1, column: 15 } },
        ];
        assert!(has_overlapping_selections(&sels));
    }

    // -- new functionality tests --

    #[test]
    fn cursor_position_is_before() {
        let a = CursorPosition::new(1, 5);
        let b = CursorPosition::new(2, 1);
        let c = CursorPosition::new(1, 5);
        assert!(a.is_before(&b));
        assert!(!b.is_before(&a));
        assert!(!a.is_before(&c));
    }

    #[test]
    fn cursor_position_min_max() {
        let a = CursorPosition::new(2, 10);
        let b = CursorPosition::new(3, 1);
        assert_eq!(a.pos_min(&b), a);
        assert_eq!(a.pos_max(&b), b);
        assert_eq!(b.pos_min(&a), a);
        assert_eq!(b.pos_max(&a), b);
    }

    #[test]
    fn cursor_position_signed_distance() {
        let a = CursorPosition::new(1, 5);
        let b = CursorPosition::new(4, 2);
        let (dl, dc) = a.signed_distance_to(&b);
        assert_eq!(dl, 3);
        assert_eq!(dc, -3);
        let (dl2, dc2) = b.signed_distance_to(&a);
        assert_eq!(dl2, -3);
        assert_eq!(dc2, 3);
    }

    #[test]
    fn selection_char_count_and_reversed() {
        let sel = Selection::new(CursorPosition::new(1, 3), CursorPosition::new(1, 10));
        assert_eq!(sel.char_count(), 7);
        assert!(!sel.is_reversed());

        let rev = Selection::new(CursorPosition::new(3, 5), CursorPosition::new(1, 2));
        assert!(rev.is_reversed());
    }

    #[test]
    fn selection_merge_with() {
        let a = Selection::new(CursorPosition::new(1, 1), CursorPosition::new(2, 5));
        let b = Selection::new(CursorPosition::new(2, 3), CursorPosition::new(3, 8));
        let merged = a.merge_with(&b).unwrap();
        assert_eq!(merged.start, CursorPosition::new(1, 1));
        assert_eq!(merged.end, CursorPosition::new(3, 8));

        let c = Selection::new(CursorPosition::new(5, 1), CursorPosition::new(6, 1));
        assert!(a.merge_with(&c).is_none());
    }

    #[test]
    fn session_bounding_range_and_total_lines() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(2, 3));
        session.add_cursor(CursorPosition::new(8, 12));
        session.selections.push(Selection::new(
            CursorPosition::new(1, 1),
            CursorPosition::new(4, 5),
        ));
        session.selections.push(Selection::new(
            CursorPosition::new(6, 1),
            CursorPosition::new(7, 3),
        ));

        let br = session.bounding_range().unwrap();
        assert_eq!(br.start, CursorPosition::new(1, 1));
        assert_eq!(br.end, CursorPosition::new(8, 12));

        assert_eq!(session.total_selected_lines(), 6); // 4 + 2

        let at_line_2: Vec<_> = session.find_at_line(2);
        assert_eq!(at_line_2.len(), 1);
        assert_eq!(at_line_2[0].column, 3);
    }

    #[test]
    fn into_iterator_for_session() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        session.add_cursor(CursorPosition::new(2, 2));
        let collected: Vec<_> = (&session).into_iter().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(*collected[0], CursorPosition::new(1, 1));
    }

    #[test]
    fn column_selection_mode_display_and_is_block() {
        let csm = ColumnSelectionMode::new(5);
        assert!(csm.is_block());
        assert_eq!(format!("{csm}"), "ColumnSelectionMode(anchor=5)");
    }

    #[test]
    fn selection_summary_from_session() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        session.selections.push(Selection::new(
            CursorPosition::new(1, 1),
            CursorPosition::new(3, 5),
        ));
        session.selections.push(Selection::new(
            CursorPosition::new(2, 1),
            CursorPosition::new(4, 1),
        ));
        let summary = SelectionSummary::from_session(&session);
        assert_eq!(summary.total_selections, 2);
        assert_eq!(summary.overlapping_count, 1);
        assert!(format!("{summary}").contains("overlapping=1"));
    }

    #[test]
    fn sort_and_deduplicate_selections_helpers() {
        let mut sels = vec![
            Selection::new(CursorPosition::new(3, 1), CursorPosition::new(3, 5)),
            Selection::new(CursorPosition::new(1, 1), CursorPosition::new(1, 5)),
            Selection::new(CursorPosition::new(3, 1), CursorPosition::new(3, 5)),
        ];
        sort_selections(&mut sels);
        assert_eq!(sels[0].start.line, 1);
        assert_eq!(sels[1].start.line, 3);

        let deduped = deduplicate_selections(&sels);
        assert_eq!(deduped.len(), 2);
    }

    // -- text transform tests --

    #[test]
    fn text_transform_uppercase_lowercase() {
        assert_eq!(TextTransform::Uppercase.apply("hello"), "HELLO");
        assert_eq!(TextTransform::Lowercase.apply("HELLO"), "hello");
        assert_eq!(TextTransform::Reverse.apply("abc"), "cba");
    }

    #[test]
    fn text_transform_camel_and_snake() {
        assert_eq!(TextTransform::CamelCase.apply("my_variable_name"), "myVariableName");
        assert_eq!(TextTransform::CamelCase.apply("get data"), "getData");
        assert_eq!(TextTransform::SnakeCase.apply("myVariableName"), "my_variable_name");
        assert_eq!(TextTransform::SnakeCase.apply("getData"), "get_data");
    }

    #[test]
    fn apply_transform_at_cursors_batch() {
        let pairs = vec![
            (CursorPosition::new(1, 1), "hello_world"),
            (CursorPosition::new(3, 5), "foo_bar"),
        ];
        let results = apply_transform_at_cursors(&pairs, TextTransform::CamelCase);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].transformed, "helloWorld");
        assert_eq!(results[0].original, "hello_world");
        assert_eq!(results[1].transformed, "fooBar");
        assert_eq!(results[1].cursor, CursorPosition::new(3, 5));
    }

    // -- cursor grouping tests --

    #[test]
    fn group_cursors_by_column_groups_correctly() {
        let cursors = vec![
            CursorPosition::new(1, 5),
            CursorPosition::new(3, 10),
            CursorPosition::new(2, 5),
            CursorPosition::new(4, 10),
            CursorPosition::new(6, 1),
        ];
        let groups = group_cursors_by_column(&cursors);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[&5].len(), 2);
        assert_eq!(groups[&5][0].line, 1);
        assert_eq!(groups[&5][1].line, 2);
        assert_eq!(groups[&10].len(), 2);
        assert_eq!(groups[&1].len(), 1);
    }

    // -- block selection tests --

    #[test]
    fn block_selection_from_any_corners() {
        let block = BlockSelection::from_corners(
            CursorPosition::new(5, 10),
            CursorPosition::new(2, 3),
        );
        assert_eq!(block.top_left, CursorPosition::new(2, 3));
        assert_eq!(block.bottom_right, CursorPosition::new(5, 10));
        assert_eq!(block.height(), 4);
        assert_eq!(block.width(), 8);
        assert!(block.contains(&CursorPosition::new(3, 5)));
        assert!(!block.contains(&CursorPosition::new(1, 5)));
        assert!(!block.contains(&CursorPosition::new(3, 11)));

        let sels = block.to_line_selections(|_| 20);
        assert_eq!(sels.len(), 4);
        assert_eq!(sels[0].start, CursorPosition::new(2, 3));
        assert_eq!(sels[0].end, CursorPosition::new(2, 10));

        // Clamped line
        let sels_clamped = block.to_line_selections(|line| if line == 3 { 6 } else { 20 });
        assert_eq!(sels_clamped[1].end.column, 6);

        let edge = block.right_edge_cursors(|_| 20);
        assert_eq!(edge.len(), 4);
        assert_eq!(edge[0], CursorPosition::new(2, 10));

        assert!(format!("{block}").contains("Block["));
    }

    // -- cursor history undo/redo tests --

    #[test]
    fn cursor_history_undo_redo() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        let mut history = CursorHistory::new(10);

        // Save state, then mutate
        history.save(&session);
        session.add_cursor(CursorPosition::new(2, 2));
        assert_eq!(session.cursor_count(), 2);
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.redo_len(), 0);

        // Undo restores to 1 cursor
        assert!(history.undo(&mut session));
        assert_eq!(session.cursor_count(), 1);
        assert_eq!(session.cursors[0], CursorPosition::new(1, 1));
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.redo_len(), 1);

        // Redo restores to 2 cursors
        assert!(history.redo(&mut session));
        assert_eq!(session.cursor_count(), 2);
        assert_eq!(history.redo_len(), 0);

        // Undo with empty stack returns false
        history.clear();
        assert!(!history.undo(&mut session));
        assert!(!history.redo(&mut session));
    }

    #[test]
    fn cursor_history_max_entries_evicts() {
        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition::new(1, 1));
        let mut history = CursorHistory::new(3);

        for i in 0..5u32 {
            history.save(&session);
            session.move_all(1, 0);
            // After 4th save the stack should be capped at 3
            assert!(history.undo_len() <= 3, "iteration {i}");
        }
        assert_eq!(history.undo_len(), 3);
    }

    // -- MultiCursorColumns ------------------------------------------------

    #[test]
    fn column_selection_basic() {
        let mut cs = MultiCursorColumns::new(1, 5);
        cs.extend_to(5, 10);
        assert_eq!(cs.line_range(), (1, 5));
        assert_eq!(cs.column_range(), (5, 10));
        assert_eq!(cs.line_count(), 5);
        assert_eq!(cs.width(), 5);
    }

    #[test]
    fn column_selection_generates_positions() {
    }

    #[test]
    fn column_selection_display() {
        let cs = MultiCursorColumns::new(1, 1);
        let s = format!("{cs}");
        assert!(s.contains("ColumnSel"));
    }

    // -- MultiCursorMatch --------------------------------------------------

    #[test]
    fn cursor_match_find_all() {
        let m = MultiCursorMatch::new("foo");
        let lines = vec!["foo bar foo", "baz", "foo"];
        let positions = m.find_all(&lines);
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], CursorPosition::new(1, 1));
        assert_eq!(positions[1], CursorPosition::new(1, 9));
        assert_eq!(positions[2], CursorPosition::new(3, 1));
    }

    #[test]
    fn cursor_match_case_insensitive() {
        let m = MultiCursorMatch::new("FOO").case_insensitive();
        let lines = vec!["foo Foo FOO"];
        let positions = m.find_all(&lines);
        assert_eq!(positions.len(), 3);
    }

    #[test]
    fn cursor_match_whole_word() {
        let m = MultiCursorMatch::new("foo").whole_word();
        let lines = vec!["foo foobar foo"];
        let positions = m.find_all(&lines);
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn cursor_match_count() {
        let m = MultiCursorMatch::new("x");
        let lines = vec!["x x x"];
        assert_eq!(m.count_matches(&lines), 3);
    }

    // -- CursorMergeStrategy -----------------------------------------------

    #[test]
    fn merge_duplicate_cursors_dedup() {
        let positions = vec![
            CursorPosition::new(1, 1),
            CursorPosition::new(1, 1),
            CursorPosition::new(2, 1),
        ];
        let merged = merge_duplicate_cursors(&positions, CursorMergeStrategy::KeepFirst);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_strategy_display() {
        assert_eq!(format!("{}", CursorMergeStrategy::Union), "Union");
    }

    // -- MultiCursorClipboard ----------------------------------------------

    #[test]
    fn clipboard_per_cursor_paste() {
        let mut cb = MultiCursorClipboard::new();
        cb.copy_from_cursors(vec!["aaa".into(), "bbb".into(), "ccc".into()]);
        assert_eq!(cb.paste_for_cursor(0), "aaa");
        assert_eq!(cb.paste_for_cursor(1), "bbb");
        assert_eq!(cb.paste_for_cursor(2), "ccc");
        assert_eq!(cb.paste_for_cursor(99), "ccc"); // fallback to last
    }

    #[test]
    fn clipboard_matches_cursor_count() {
        let mut cb = MultiCursorClipboard::new();
        cb.copy_from_cursors(vec!["a".into(), "b".into()]);
        assert!(cb.matches_cursor_count(2));
        assert!(!cb.matches_cursor_count(3));
    }

    #[test]
    fn clipboard_joined() {
        let mut cb = MultiCursorClipboard::new();
        cb.copy_from_cursors(vec!["a".into(), "b".into()]);
        assert_eq!(cb.joined("\n"), "a\nb");
    }

    #[test]
    fn clipboard_clear_and_empty() {
        let mut cb = MultiCursorClipboard::new();
        assert!(cb.is_empty());
        cb.copy_from_cursors(vec!["x".into()]);
        assert!(!cb.is_empty());
        cb.clear();
        assert!(cb.is_empty());
    }

    #[test]
    fn column_extender_basic() {
        let ext = MulticursorColumnExtender::new(5, 1, 3).unwrap();
        assert_eq!(ext.cursor_count(), 3);
        assert_eq!(ext.column(), 5);
        assert_eq!(ext.start_line(), 1);
        assert_eq!(ext.end_line(), 3);
    }

    #[test]
    fn column_extender_reversed_range() {
        let ext = MulticursorColumnExtender::new(10, 5, 2).unwrap();
        assert_eq!(ext.start_line(), 2);
        assert_eq!(ext.end_line(), 5);
        assert_eq!(ext.cursor_count(), 4);
    }

    #[test]
    fn column_extender_single_line() {
        let ext = MulticursorColumnExtender::new(3, 7, 7).unwrap();
        assert_eq!(ext.cursor_count(), 1);
        assert_eq!(ext.line_span(), 1);
    }

    #[test]
    fn column_extender_invalid_column() {
        let result = MulticursorColumnExtender::new(0, 1, 3);
        assert!(result.is_err());
    }

    #[test]
    fn column_extender_shift_lines() {
        let mut ext = MulticursorColumnExtender::new(5, 2, 4).unwrap();
        ext.shift_lines(3);
        assert_eq!(ext.start_line(), 5);
        assert_eq!(ext.end_line(), 7);
        assert_eq!(ext.cursor_count(), 3);
    }

    #[test]
    fn column_extender_shift_column() {
        let mut ext = MulticursorColumnExtender::new(5, 1, 3).unwrap();
        ext.shift_column(2);
        assert_eq!(ext.column(), 7);
        assert!(ext.positions().iter().all(|p| p.column == 7));
    }

    #[test]
    fn column_extender_expand_shrink() {
        let mut ext = MulticursorColumnExtender::new(5, 3, 5).unwrap();
        ext.expand();
        assert_eq!(ext.start_line(), 2);
        assert_eq!(ext.end_line(), 6);
        ext.shrink();
        assert_eq!(ext.start_line(), 3);
        assert_eq!(ext.end_line(), 5);
    }

    #[test]
    fn column_extender_contains() {
        let ext = MulticursorColumnExtender::new(5, 2, 4).unwrap();
        assert!(ext.contains(&CursorPosition::new(3, 5)));
        assert!(!ext.contains(&CursorPosition::new(3, 6)));
        assert!(!ext.contains(&CursorPosition::new(1, 5)));
    }

    #[test]
    fn column_extender_merge() {
        let a = MulticursorColumnExtender::new(5, 1, 3).unwrap();
        let b = MulticursorColumnExtender::new(5, 5, 7).unwrap();
        let merged = a.merge(&b).unwrap();
        assert_eq!(merged.start_line(), 1);
        assert_eq!(merged.end_line(), 7);
        assert_eq!(merged.cursor_count(), 7);
    }

    #[test]
    fn column_extender_merge_different_col_fails() {
        let a = MulticursorColumnExtender::new(5, 1, 3).unwrap();
        let b = MulticursorColumnExtender::new(6, 5, 7).unwrap();
        assert!(a.merge(&b).is_err());
    }

    #[test]
    fn column_extender_display() {
        let ext = MulticursorColumnExtender::new(5, 1, 3).unwrap();
        let s = format!("{ext}");
        assert!(s.contains("col=5"));
        assert!(s.contains("count=3"));
    }

    #[test]
    fn type_filter_on_line() {
        let filter = MulticursorTypeFilter::all_of(vec![CursorFilterCriterion::OnLine(3)]);
        let positions = vec![
            CursorPosition::new(1, 1),
            CursorPosition::new(3, 5),
            CursorPosition::new(3, 10),
        ];
        let result = filter.apply(&positions);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn type_filter_in_line_range() {
        let filter = MulticursorTypeFilter::all_of(vec![CursorFilterCriterion::InLineRange(2, 4)]);
        let positions: Vec<CursorPosition> = (1..=6).map(|l| CursorPosition::new(l, 1)).collect();
        assert_eq!(filter.count_matches(&positions), 3);
    }

    #[test]
    fn type_filter_any_of() {
        let filter = MulticursorTypeFilter::any_of(vec![
            CursorFilterCriterion::OnLine(1),
            CursorFilterCriterion::OnLine(5),
        ]);
        let positions: Vec<CursorPosition> = (1..=5).map(|l| CursorPosition::new(l, 1)).collect();
        assert_eq!(filter.count_matches(&positions), 2);
    }

    #[test]
    fn type_filter_all_of_combined() {
        let filter = MulticursorTypeFilter::all_of(vec![
            CursorFilterCriterion::InLineRange(1, 10),
            CursorFilterCriterion::MinColumn(5),
        ]);
        let positions = vec![
            CursorPosition::new(1, 3),
            CursorPosition::new(5, 8),
            CursorPosition::new(15, 8),
        ];
        assert_eq!(filter.count_matches(&positions), 1);
    }

    #[test]
    fn type_filter_even_odd() {
        let filter_even = MulticursorTypeFilter::all_of(vec![CursorFilterCriterion::EvenLines]);
        let filter_odd = MulticursorTypeFilter::all_of(vec![CursorFilterCriterion::OddLines]);
        let positions: Vec<CursorPosition> = (1..=6).map(|l| CursorPosition::new(l, 1)).collect();
        assert_eq!(filter_even.count_matches(&positions), 3);
        assert_eq!(filter_odd.count_matches(&positions), 3);
    }

    #[test]
    fn type_filter_partition() {
        let filter = MulticursorTypeFilter::all_of(vec![CursorFilterCriterion::MaxColumn(5)]);
        let positions = vec![
            CursorPosition::new(1, 3),
            CursorPosition::new(2, 8),
            CursorPosition::new(3, 5),
        ];
        let (matched, unmatched) = filter.partition(&positions);
        assert_eq!(matched.len(), 2);
        assert_eq!(unmatched.len(), 1);
    }

    #[test]
    fn type_filter_pass_all() {
        let filter = MulticursorTypeFilter::pass_all();
        let positions: Vec<CursorPosition> = (1..=10).map(|l| CursorPosition::new(l, 1)).collect();
        assert_eq!(filter.count_matches(&positions), 10);
    }

    #[test]
    fn type_filter_display() {
        let filter = MulticursorTypeFilter::all_of(vec![CursorFilterCriterion::OnLine(1)]);
        let s = format!("{filter}");
        assert!(s.contains("ALL"));
        assert!(s.contains("1 criteria"));
    }

    #[test]
    fn type_filter_clear() {
        let mut filter = MulticursorTypeFilter::all_of(vec![CursorFilterCriterion::OnLine(1)]);
        assert_eq!(filter.criteria_count(), 1);
        filter.clear();
        assert_eq!(filter.criteria_count(), 0);
    }



    #[test]
    fn multicursor_x_config_new() {
        let c = MulticursorXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn multicursor_x_config_builder() {
        let c = MulticursorXConfig::new("k")
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
    fn multicursor_x_config_display() {
        let c = MulticursorXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn multicursor_x_registry_insert_get() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn multicursor_x_registry_duplicate() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a")).unwrap();
        assert!(reg.insert(MulticursorXConfig::new("a")).is_err());
    }

    #[test]
    fn multicursor_x_registry_remove() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a")).unwrap();
        reg.insert(MulticursorXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn multicursor_x_registry_active_entries() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a")).unwrap();
        reg.insert(MulticursorXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn multicursor_x_registry_by_weight() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(MulticursorXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn multicursor_x_registry_tags() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(MulticursorXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn multicursor_x_registry_total_weight() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(MulticursorXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn multicursor_x_registry_iterator() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a")).unwrap();
        reg.insert(MulticursorXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn multicursor_x_cache_put_get() {
        let mut cache = MulticursorXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn multicursor_x_cache_eviction() {
        let mut cache = MulticursorXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn multicursor_x_cache_lru_order() {
        let mut cache = MulticursorXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn multicursor_x_cache_most_least_recent() {
        let mut cache = MulticursorXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn multicursor_x_formatter_entry() {
        let e = MulticursorXConfig::new("k").with_value("v");
        let fmt = MulticursorXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn multicursor_x_formatter_summary() {
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("a").with_weight(5)).unwrap();
        let fmt = MulticursorXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn multicursor_x_validator_valid() {
        let v = MulticursorXValidator::new();
        let c = MulticursorXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn multicursor_x_validator_empty_key() {
        let v = MulticursorXValidator::new();
        let c = MulticursorXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn multicursor_x_validator_require_value() {
        let v = MulticursorXValidator::new().require_value(true);
        let c = MulticursorXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn multicursor_x_validator_allowed_tags() {
        let v = MulticursorXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = MulticursorXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn multicursor_x_validator_validate_all() {
        let v = MulticursorXValidator::new();
        let mut reg = MulticursorXRegistry::new();
        reg.insert(MulticursorXConfig::new("ok")).unwrap();
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
    fn xb_ring_buffer_73_push_and_len() {
        let mut rb = super::XbRingBuffer73::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_73_overwrite() {
        let mut rb = super::XbRingBuffer73::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_73_get_out_of_bounds() {
        let rb = super::XbRingBuffer73::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_73_drain_all() {
        let mut rb = super::XbRingBuffer73::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_73_peek_front_back() {
        let mut rb = super::XbRingBuffer73::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_73_clear() {
        let mut rb = super::XbRingBuffer73::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_73_capacity() {
        let rb = super::XbRingBuffer73::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_73_basic() {
        let h = super::xb_fnv1a_73(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_73(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_73_different_inputs() {
        let h1 = super::xb_fnv1a_73(b"abc");
        let h2 = super::xb_fnv1a_73(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_73_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_73(&data);
        let dec = super::xb_rle_decode_73(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_73_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_73(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_73(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_73_values() {
        assert!((super::xb_clamp_73(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_73(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_73(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_73_values() {
        assert!((super::xb_lerp_73(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_73(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_73(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_73_wrap_around_twice() {
        let mut rb = super::XbRingBuffer73::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 125 ----

    #[test]
    fn xc_125_pool_new_empty() {
        let pool: super::Xc125Pool<i32> = super::Xc125Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_125_pool_release_acquire() {
        let mut pool = super::Xc125Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_125_pool_acquire_empty() {
        let mut pool: super::Xc125Pool<i32> = super::Xc125Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_125_pool_full() {
        let mut pool = super::Xc125Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_125_pool_drain() {
        let mut pool = super::Xc125Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_125_pool_stats() {
        let mut pool = super::Xc125Pool::new(8);
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
    fn xc_125_pool_clear() {
        let mut pool = super::Xc125Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_125_pool_shrink() {
        let mut pool = super::Xc125Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_125_pool_default() {
        let pool: super::Xc125Pool<String> = super::Xc125Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_125_pool_extend() {
        let mut pool = super::Xc125Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_125_pool_retain() {
        let mut pool = super::Xc125Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_125_scheduler_round_robin() {
        let mut sched = super::Xc125Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_125_scheduler_empty() {
        let mut sched = super::Xc125Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_125_scheduler_reset() {
        let mut sched = super::Xc125Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_125_scheduler_add_remove() {
        let mut sched = super::Xc125Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_125_scheduler_targets() {
        let sched = super::Xc125Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_125_hash_empty() {
        assert_eq!(super::xc_125_hash(b""), 5381);
    }

    #[test]
    fn xc_125_hash_data() {
        let h = super::xc_125_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_125_hash(b"hello"), h);
    }

    #[test]
    fn xc_125_reverse_str() {
        assert_eq!(super::xc_125_reverse("abc"), "cba");
        assert_eq!(super::xc_125_reverse(""), "");
    }


    #[test]
    fn xe_86_pipeline_empty() {
        let p = super::Xe86Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_86_pipeline_parse_stage() {
        let p = super::Xe86Pipeline::new()
            .add_parse(super::xe_86_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_86_pipeline_transform_double() {
        let p = super::Xe86Pipeline::new()
            .add_transform(super::xe_86_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_86_pipeline_validate_reverse() {
        let p = super::Xe86Pipeline::new()
            .add_validate(super::xe_86_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_86_pipeline_emit_filter() {
        let p = super::Xe86Pipeline::new()
            .add_emit(super::xe_86_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_86_pipeline_multi_stage() {
        let p = super::Xe86Pipeline::new()
            .add_parse(super::xe_86_pipeline_identity)
            .add_transform(super::xe_86_pipeline_double)
            .add_validate(super::xe_86_pipeline_reverse)
            .add_emit(super::xe_86_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_86_pipeline_error_propagation() {
        let p = super::Xe86Pipeline::new()
            .add_parse(super::xe_86_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe86Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_86_pipeline_compose() {
        let p1 = super::Xe86Pipeline::new()
            .add_parse(super::xe_86_pipeline_identity);
        let p2 = super::Xe86Pipeline::new()
            .add_transform(super::xe_86_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_86_pipeline_error_display() {
        let e = super::Xe86PipelineError {
            stage: super::Xe86Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_86_cache_put_get() {
        let mut c = super::Xe86Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_86_cache_miss() {
        let mut c: super::Xe86Cache<&str, i32> = super::Xe86Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_86_cache_ttl_expiry() {
        let mut c = super::Xe86Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_86_cache_evict() {
        let mut c = super::Xe86Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_86_cache_capacity() {
        let mut c = super::Xe86Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_86_cache_stats() {
        let mut c = super::Xe86Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_86_cache_clear() {
        let mut c = super::Xe86Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_84 graph tests ------------------------------------------------

    #[test]
    fn xg_84_graph_empty() {
        let g = super::Xg84Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_84_graph_add_node() {
        let mut g = super::Xg84Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_84_graph_add_edge() {
        let mut g = super::Xg84Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_84_graph_neighbors() {
        let mut g = super::Xg84Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_84_graph_has_path() {
        let mut g = super::Xg84Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_84_graph_self_path() {
        let g = super::Xg84Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_84_graph_topo_sort() {
        let mut g = super::Xg84Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_84_graph_cycle_detect_false() {
        let mut g = super::Xg84Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_84_graph_cycle_detect_true() {
        let mut g = super::Xg84Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_84 heap tests -------------------------------------------------

    #[test]
    fn xg_84_heap_empty() {
        let h: super::Xg84Heap<i32> = super::Xg84Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_84_heap_push_pop() {
        let mut h = super::Xg84Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_84_heap_peek() {
        let mut h = super::Xg84Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_84_heap_drain_sorted() {
        let mut h = super::Xg84Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_84_heap_merge() {
        let mut a = super::Xg84Heap::new();
        let mut b = super::Xg84Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_84_heap_default() {
        let h: super::Xg84Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_84_graph_default() {
        let g: super::Xg84Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh124_skip_insert_contains() {
        let mut sl = super::Xh124SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh124_skip_remove() {
        let mut sl = super::Xh124SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh124_skip_len() {
        let mut sl = super::Xh124SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh124_skip_range_query() {
        let mut sl = super::Xh124SkipList::xh_new(4);
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
    fn xh124_skip_floor_ceiling() {
        let mut sl = super::Xh124SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh124_skip_rank() {
        let mut sl = super::Xh124SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh124_skip_empty() {
        let sl = super::Xh124SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh124_skip_duplicates() {
        let mut sl = super::Xh124SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh124_bitset_set_test() {
        let mut bs = super::Xh124BitSet::xh_new(256);
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
    fn xh124_bitset_clear_count() {
        let mut bs = super::Xh124BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh124_bitset_and_or_xor() {
        let mut a = super::Xh124BitSet::xh_new(128);
        let mut b = super::Xh124BitSet::xh_new(128);
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
    fn xh124_bitset_iter_ones() {
        let mut bs = super::Xh124BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh124_bitset_first_last() {
        let mut bs = super::Xh124BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh124_bitset_empty() {
        let bs = super::Xh124BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi124_deque_push_pop_back() {
        let mut dq = super::Xi124Deque::xi_new(4);
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
    fn xi124_deque_push_pop_front() {
        let mut dq = super::Xi124Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi124_deque_mixed_ops() {
        let mut dq = super::Xi124Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi124_deque_get_and_split() {
        let mut dq = super::Xi124Deque::xi_new(8);
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
    fn xi124_deque_rotate_left() {
        let mut dq = super::Xi124Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi124_deque_rotate_right() {
        let mut dq = super::Xi124Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi124_deque_grow() {
        let mut dq = super::Xi124Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi124_deque_empty() {
        let dq = super::Xi124Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi124_interval_tree_insert_query() {
        let mut tree = super::Xi124IntervalTree::xi_new();
        tree.xi_insert(super::Xi124Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi124Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi124Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi124_interval_tree_overlap() {
        let mut tree = super::Xi124IntervalTree::xi_new();
        tree.xi_insert(super::Xi124Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi124Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi124Interval::xi_new(12, 20));
        let q = super::Xi124Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi124_interval_tree_remove() {
        let mut tree = super::Xi124IntervalTree::xi_new();
        tree.xi_insert(super::Xi124Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi124Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi124_interval_tree_gaps() {
        let mut tree = super::Xi124IntervalTree::xi_new();
        tree.xi_insert(super::Xi124Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi124Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi124Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi124Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi124Interval::xi_new(8, 10));
    }

    #[test]
    fn xi124_interval_tree_merge() {
        let mut tree = super::Xi124IntervalTree::xi_new();
        tree.xi_insert(super::Xi124Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi124Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi124Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi124Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi124Interval::xi_new(10, 15));
    }

    #[test]
    fn xi124_interval_tree_all() {
        let mut tree = super::Xi124IntervalTree::xi_new();
        tree.xi_insert(super::Xi124Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi124Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi124_interval_tree_empty() {
        let tree = super::Xi124IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi124_interval_tree_contains_point() {
        let iv = super::Xi124Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 124) ---

    #[test]
    fn xj_124_uf_make_and_find() {
        let mut uf = super::Xj124UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_124_uf_union_connected() {
        let mut uf = super::Xj124UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_124_uf_component_count() {
        let mut uf = super::Xj124UnionFind::xj_new();
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
    fn xj_124_uf_component_size() {
        let mut uf = super::Xj124UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_124_uf_largest_component() {
        let mut uf = super::Xj124UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_124_uf_many_elements() {
        let mut uf = super::Xj124UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_124_uf_separate_components() {
        let mut uf = super::Xj124UnionFind::xj_new();
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
    fn xj_124_uf_path_compression() {
        let mut uf = super::Xj124UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_124_bt_insert_get() {
        let mut bt = super::Xj124BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_124_bt_contains_len() {
        let mut bt = super::Xj124BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_124_bt_replace() {
        let mut bt = super::Xj124BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_124_bt_remove() {
        let mut bt = super::Xj124BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_124_bt_keys_values() {
        let mut bt = super::Xj124BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_124_bt_range() {
        let mut bt = super::Xj124BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_124_bt_min_max() {
        let mut bt = super::Xj124BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_124_bt_many_inserts() {
        let mut bt = super::Xj124BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_124 segment tree tests ---

    #[test]
    fn xk_124_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk124SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_124_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk124SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_124_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk124SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_124_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk124SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_124_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk124SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_124_st_single_element() {
        let data = vec![42];
        let st = super::Xk124SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_124_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk124SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_124_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk124SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_124 disjoint intervals tests ---

    #[test]
    fn xk_124_di_add_and_count() {
        let mut di = super::Xk124DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_124_di_merge_overlap() {
        let mut di = super::Xk124DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_124_di_contains() {
        let mut di = super::Xk124DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_124_di_remove() {
        let mut di = super::Xk124DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_124_di_covered_length() {
        let mut di = super::Xk124DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_124_di_gaps() {
        let mut di = super::Xk124DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_124_di_merge_adjacent() {
        let mut di = super::Xk124DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_124_di_empty() {
        let di = super::Xk124DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_124_rope_new_empty() {
        let rope = super::Xl124Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_124_rope_from_str() {
        let rope = super::Xl124Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_124_rope_insert_at() {
        let mut rope = super::Xl124Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_124_rope_delete_range() {
        let mut rope = super::Xl124Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_124_rope_char_at() {
        let rope = super::Xl124Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_124_rope_split_concat() {
        let rope = super::Xl124Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_124_rope_line_count() {
        let rope = super::Xl124Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_124_rope_line_at() {
        let rope = super::Xl124Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_124_sa_build_and_search() {
        let sa = super::Xl124SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_124_sa_count() {
        let sa = super::Xl124SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_124_sa_longest_repeated() {
        let sa = super::Xl124SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_124_sa_all_positions() {
        let sa = super::Xl124SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_124_sa_len() {
        let sa = super::Xl124SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_124_sa_empty() {
        let sa = super::Xl124SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_124_rope_slice() {
        let rope = super::Xl124Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_124_sa_search_start() {
        let sa = super::Xl124SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_124_sparse_set_get() {
        let mut m = super::Xm124MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_124_sparse_row_col() {
        let mut m = super::Xm124MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_124_sparse_transpose() {
        let mut m = super::Xm124MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_124_sparse_multiply_vec() {
        let mut m = super::Xm124MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_124_sparse_nnz_density() {
        let mut m = super::Xm124MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_124_sparse_clear() {
        let mut m = super::Xm124MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_124_sparse_overwrite_zero() {
        let mut m = super::Xm124MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_124_tokenizer_basic() {
        let t = super::Xm124Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_124_tokenizer_count() {
        let t = super::Xm124Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_124_tokenizer_unique() {
        let t = super::Xm124Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_124_tokenizer_frequency() {
        let t = super::Xm124Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_124_tokenizer_delimiter() {
        let t = super::Xm124Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_124_tokenizer_whitespace() {
        let t = super::Xm124Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_124_tokenizer_empty() {
        let t = super::Xm124Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 124 ----

    #[test]
    fn xn_124_fenwick_prefix_sum() {
        let mut ft = super::Xn124Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_124_fenwick_range_sum() {
        let mut ft = super::Xn124Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_124_fenwick_point_query() {
        let mut ft = super::Xn124Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_124_fenwick_len() {
        let ft = super::Xn124Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_124_fenwick_multiple_updates() {
        let mut ft = super::Xn124Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_124_fenwick_single_element() {
        let mut ft = super::Xn124Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_124_fenwick_find_kth() {
        let mut ft = super::Xn124Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_124_fenwick_negative_delta() {
        let mut ft = super::Xn124Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 124 ----

    #[test]
    fn xn_124_avl_insert_get() {
        let mut m = super::Xn124AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_124_avl_remove() {
        let mut m = super::Xn124AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_124_avl_in_order() {
        let mut m = super::Xn124AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_124_avl_min_max() {
        let mut m = super::Xn124AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_124_avl_floor_ceiling() {
        let mut m = super::Xn124AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_124_avl_height_balanced() {
        let mut m = super::Xn124AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_124_avl_overwrite() {
        let mut m = super::Xn124AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_124_avl_empty() {
        let m: super::Xn124AVL<i32, i32> = super::Xn124AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo124RedBlack tests ---

    #[test]
    fn xo_124_rb_insert_and_get() {
        let mut tree = super::Xo124RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_124_rb_len_and_empty() {
        let mut tree = super::Xo124RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_124_rb_min_max() {
        let mut tree = super::Xo124RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_124_rb_contains() {
        let mut tree = super::Xo124RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_124_rb_remove() {
        let mut tree = super::Xo124RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_124_rb_in_order() {
        let mut tree = super::Xo124RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_124_rb_black_height() {
        let mut tree = super::Xo124RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_124_rb_overwrite() {
        let mut tree = super::Xo124RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo124ConsistentHash tests ---

    #[test]
    fn xo_124_ch_add_and_count() {
        let mut ring = super::Xo124ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_124_ch_remove_node() {
        let mut ring = super::Xo124ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_124_ch_get_node() {
        let mut ring = super::Xo124ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_124_ch_empty_ring() {
        let ring = super::Xo124ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_124_ch_distribution() {
        let mut ring = super::Xo124ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_124_ch_rebalance() {
        let mut ring = super::Xo124ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_124_ch_virtual_nodes() {
        let mut ring = super::Xo124ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_124_ch_consistent_lookup() {
        let mut ring = super::Xo124ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_124_splay_insert_get() {
        let mut t = super::Xp124SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_124_splay_remove() {
        let mut t = super::Xp124SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_124_splay_count_increases() {
        let mut t = super::Xp124SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_124_splay_depth() {
        let mut t = super::Xp124SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_124_splay_len_empty() {
        let t = super::Xp124SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_124_splay_min_max() {
        let mut t = super::Xp124SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_124_splay_overwrite() {
        let mut t = super::Xp124SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_124_splay_remove_missing() {
        let mut t = super::Xp124SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_124 treap tests ----
    #[test]
    fn xq_124_treap_empty() {
        let t = super::Xq124Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_124_treap_insert_get() {
        let mut t = super::Xq124Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_124_treap_overwrite() {
        let mut t = super::Xq124Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_124_treap_remove() {
        let mut t = super::Xq124Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_124_treap_min_max() {
        let mut t = super::Xq124Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_124_treap_rank() {
        let mut t = super::Xq124Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_124_treap_kth() {
        let mut t = super::Xq124Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_124_treap_in_order() {
        let mut t = super::Xq124Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_124 VEB tree tests ----
    #[test]
    fn xq_124_veb_empty() {
        let v = super::Xq124VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_124_veb_insert_contains() {
        let mut v = super::Xq124VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_124_veb_min_max() {
        let mut v = super::Xq124VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_124_veb_delete() {
        let mut v = super::Xq124VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_124_veb_successor() {
        let mut v = super::Xq124VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_124_veb_predecessor() {
        let mut v = super::Xq124VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_124_veb_count() {
        let mut v = super::Xq124VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_124_veb_duplicate_insert() {
        let mut v = super::Xq124VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
