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
}
