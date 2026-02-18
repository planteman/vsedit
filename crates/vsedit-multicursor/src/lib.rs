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

}
