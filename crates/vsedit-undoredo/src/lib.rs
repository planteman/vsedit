//! Undo/redo stack service.
//!
//! Provides a generic [`UndoRedoStack<T>`] that tracks past and future states
//! for undo/redo operations, plus [`UndoRedoService`] with cursor-aware
//! grouped undo/redo matching VS Code's `UndoRedoService`.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// CursorState
// ---------------------------------------------------------------------------

/// Cursor state stored alongside each undo group, matching VS Code's
/// `ICursorStateComputer` return type.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorState {
    /// Cursor positions (one per cursor in multi-cursor mode).
    pub positions: Vec<CursorPosition>,
    /// Selections (one per cursor).
    pub selections: Vec<CursorSelection>,
}

/// A single 1-based cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub line: u32,
    pub column: u32,
}

impl CursorPosition {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// A selection range for one cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorSelection {
    pub anchor_line: u32,
    pub anchor_column: u32,
    pub active_line: u32,
    pub active_column: u32,
}

impl CursorSelection {
    pub fn new(
        anchor_line: u32,
        anchor_column: u32,
        active_line: u32,
        active_column: u32,
    ) -> Self {
        Self {
            anchor_line,
            anchor_column,
            active_line,
            active_column,
        }
    }

    pub fn collapsed(line: u32, column: u32) -> Self {
        Self::new(line, column, line, column)
    }
}

impl CursorState {
    pub fn single(line: u32, column: u32) -> Self {
        Self {
            positions: vec![CursorPosition::new(line, column)],
            selections: vec![CursorSelection::collapsed(line, column)],
        }
    }
}

// ---------------------------------------------------------------------------
// UndoRedoGroup — a grouped set of edits forming one undo step
// ---------------------------------------------------------------------------

/// A group of edits that form a single undo step, analogous to VS Code's
/// `StackElement`.
#[derive(Debug, Clone)]
pub struct UndoRedoGroup<T> {
    /// Unique group id.
    pub id: u64,
    /// The edits in this group.
    pub edits: Vec<T>,
    /// Cursor state *before* the group was applied.
    pub cursor_before: Option<CursorState>,
    /// Cursor state *after* the group was applied.
    pub cursor_after: Option<CursorState>,
}

// ---------------------------------------------------------------------------
// UndoRedoService — grouped undo/redo with cursor restoration
// ---------------------------------------------------------------------------

/// Grouped undo/redo service with cursor state tracking.
///
/// Each undo step is an [`UndoRedoGroup`] containing one or more edits plus
/// cursor state before/after. Supports open/close grouping for compound
/// operations (e.g. type + auto-close bracket = one undo step).
#[derive(Debug)]
pub struct UndoRedoService<T> {
    past: Vec<UndoRedoGroup<T>>,
    future: Vec<UndoRedoGroup<T>>,
    next_group_id: u64,
    /// While > 0, edits are accumulated into the current open group.
    open_group_depth: u32,
    /// The group being built while open.
    building_group: Option<UndoRedoGroup<T>>,
}

impl<T: Clone> UndoRedoService<T> {
    pub fn new() -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            next_group_id: 1,
            open_group_depth: 0,
            building_group: None,
        }
    }

    /// Open a new undo group. Edits pushed while the group is open are
    /// collected into a single undo step. Calls may be nested.
    pub fn open_group(&mut self, cursor_before: Option<CursorState>) {
        if self.open_group_depth == 0 {
            let id = self.next_group_id;
            self.next_group_id += 1;
            self.building_group = Some(UndoRedoGroup {
                id,
                edits: Vec::new(),
                cursor_before,
                cursor_after: None,
            });
        }
        self.open_group_depth += 1;
    }

    /// Close the current undo group. When all nested opens are matched, the
    /// group is pushed to the undo stack.
    pub fn close_group(&mut self, cursor_after: Option<CursorState>) {
        if self.open_group_depth == 0 {
            return;
        }
        self.open_group_depth -= 1;
        if self.open_group_depth == 0 {
            if let Some(mut group) = self.building_group.take() {
                group.cursor_after = cursor_after;
                self.past.push(group);
                self.future.clear();
            }
        }
    }

    /// Push a single edit. If a group is open it is appended; otherwise a
    /// new single-edit group is created.
    pub fn push_edit(
        &mut self,
        edit: T,
        cursor_before: Option<CursorState>,
        cursor_after: Option<CursorState>,
    ) {
        if self.open_group_depth > 0 {
            if let Some(ref mut g) = self.building_group {
                g.edits.push(edit);
            }
        } else {
            let id = self.next_group_id;
            self.next_group_id += 1;
            self.past.push(UndoRedoGroup {
                id,
                edits: vec![edit],
                cursor_before,
                cursor_after,
            });
            self.future.clear();
        }
    }

    /// Push an already-formed group.
    pub fn push_group(&mut self, mut group: UndoRedoGroup<T>) {
        group.id = self.next_group_id;
        self.next_group_id += 1;
        self.past.push(group);
        self.future.clear();
    }

    /// Undo the last group, returning it with cursor state for restoration.
    pub fn undo(&mut self) -> Option<&UndoRedoGroup<T>> {
        let group = self.past.pop()?;
        self.future.push(group);
        self.future.last()
    }

    /// Redo the last undone group.
    pub fn redo(&mut self) -> Option<&UndoRedoGroup<T>> {
        let group = self.future.pop()?;
        self.past.push(group);
        self.past.last()
    }

    /// Get cursor state to restore after an undo.
    pub fn undo_with_cursor(&mut self) -> Option<CursorState> {
        let group = self.past.pop()?;
        let cursor = group.cursor_before.clone();
        self.future.push(group);
        cursor
    }

    /// Get cursor state to restore after a redo.
    pub fn redo_with_cursor(&mut self) -> Option<CursorState> {
        let group = self.future.pop()?;
        let cursor = group.cursor_after.clone();
        self.past.push(group);
        cursor
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.open_group_depth = 0;
        self.building_group = None;
    }

    pub fn undo_count(&self) -> usize {
        self.past.len()
    }

    pub fn redo_count(&self) -> usize {
        self.future.len()
    }

    /// Peek at the last undo group.
    pub fn peek_undo(&self) -> Option<&UndoRedoGroup<T>> {
        self.past.last()
    }

    /// Returns true if a group is currently open.
    pub fn is_group_open(&self) -> bool {
        self.open_group_depth > 0
    }
}

impl<T: Clone> Default for UndoRedoService<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors returned by fallible undo/redo operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoRedoError {
    /// There is nothing to undo.
    NothingToUndo,
    /// There is nothing to redo.
    NothingToRedo,
    /// The undo stack has reached its maximum capacity.
    StackFull,
}

impl fmt::Display for UndoRedoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToUndo => write!(f, "nothing to undo"),
            Self::NothingToRedo => write!(f, "nothing to redo"),
            Self::StackFull => write!(f, "undo stack is full"),
        }
    }
}

/// A generic undo/redo stack.
///
/// Pushing a new item clears the redo history (any previously undone items are
/// discarded), matching standard editor behaviour.
///
/// An optional capacity limit can be set via [`with_capacity`](UndoRedoStack::with_capacity).
/// When the limit is reached the oldest undo entry is evicted.
#[derive(Debug, Clone)]
pub struct UndoRedoStack<T> {
    past: Vec<T>,
    future: Vec<T>,
    capacity: Option<usize>,
}

impl<T: Clone> UndoRedoStack<T> {
    /// Creates an empty undo/redo stack with no capacity limit.
    pub fn new() -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            capacity: None,
        }
    }

    /// Creates an empty undo/redo stack that holds at most `max` undo entries.
    ///
    /// When the stack is full the oldest entry is evicted on the next push.
    /// A `max` of `0` means every push immediately evicts (effectively no undo).
    pub fn with_capacity(max: usize) -> Self {
        Self {
            past: Vec::with_capacity(max),
            future: Vec::new(),
            capacity: Some(max),
        }
    }

    /// Pushes a new item onto the undo stack and clears the redo history.
    ///
    /// If a capacity limit is set and the stack is full, the oldest entry is
    /// removed before pushing.
    pub fn push(&mut self, item: T) {
        if let Some(cap) = self.capacity {
            if cap == 0 {
                self.future.clear();
                return;
            }
            if self.past.len() >= cap {
                self.past.remove(0);
            }
        }
        self.past.push(item);
        self.future.clear();
    }

    /// Undoes the last operation, returning the item that was undone.
    ///
    /// The item is moved to the redo stack so it can be redone later.
    pub fn undo(&mut self) -> Option<T> {
        let item = self.past.pop()?;
        self.future.push(item.clone());
        Some(item)
    }

    /// Redoes the last undone operation, returning the item that was redone.
    ///
    /// The item is moved back to the undo stack.
    pub fn redo(&mut self) -> Option<T> {
        let item = self.future.pop()?;
        self.past.push(item.clone());
        Some(item)
    }

    /// Returns `true` if there are items that can be undone.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Returns `true` if there are items that can be redone.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Returns the number of items that can be undone.
    pub fn undo_count(&self) -> usize {
        self.past.len()
    }

    /// Returns the number of items that can be redone.
    pub fn redo_count(&self) -> usize {
        self.future.len()
    }

    /// Peeks at the most recent undo entry without removing it.
    pub fn peek_undo(&self) -> Option<&T> {
        self.past.last()
    }

    /// Peeks at the most recent redo entry without removing it.
    pub fn peek_redo(&self) -> Option<&T> {
        self.future.last()
    }

    /// Tries to undo the last operation, returning an error if there is nothing to undo.
    pub fn try_undo(&mut self) -> Result<T, UndoRedoError> {
        self.undo().ok_or(UndoRedoError::NothingToUndo)
    }

    /// Tries to redo the last undone operation, returning an error if there is nothing to redo.
    pub fn try_redo(&mut self) -> Result<T, UndoRedoError> {
        self.redo().ok_or(UndoRedoError::NothingToRedo)
    }

    /// Clears both undo and redo history.
    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

impl<T: Clone> Default for UndoRedoStack<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> fmt::Display for UndoRedoStack<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UndoRedoStack(undo: {}, redo: {})",
            self.past.len(),
            self.future.len()
        )
    }
}

/// A named undo/redo entry that wraps a value with a description.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoEntry<T> {
    pub label: String,
    pub value: T,
    pub timestamp: u64,
}

impl<T> UndoEntry<T> {
    pub fn new(label: impl Into<String>, value: T, timestamp: u64) -> Self {
        Self {
            label: label.into(),
            value,
            timestamp,
        }
    }
}

impl<T: fmt::Display> fmt::Display for UndoEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} (t={})", self.label, self.value, self.timestamp)
    }
}

/// A transaction groups multiple undo entries into a single undoable unit.
#[derive(Debug, Clone)]
pub struct Transaction<T> {
    pub label: String,
    entries: Vec<T>,
    committed: bool,
}

impl<T: Clone> Transaction<T> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            entries: Vec::new(),
            committed: false,
        }
    }

    /// Add an entry to this transaction.
    pub fn add(&mut self, entry: T) {
        self.entries.push(entry);
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether this transaction has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Mark this transaction as committed.
    pub fn commit(&mut self) {
        self.committed = true;
    }

    /// Check whether this transaction has been committed.
    pub fn is_committed(&self) -> bool {
        self.committed
    }

    /// Return the entries in this transaction.
    pub fn entries(&self) -> &[T] {
        &self.entries
    }
}

impl<T: Clone> fmt::Display for Transaction<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Transaction({}, {} entries, {})",
            self.label,
            self.entries.len(),
            if self.committed {
                "committed"
            } else {
                "pending"
            }
        )
    }
}

impl<T: Clone> UndoRedoStack<T> {
    /// Push multiple items as a batch, clearing redo after all are pushed.
    pub fn push_batch(&mut self, items: impl IntoIterator<Item = T>) {
        for item in items {
            if let Some(cap) = self.capacity {
                if cap == 0 {
                    continue;
                }
                if self.past.len() >= cap {
                    self.past.remove(0);
                }
            }
            self.past.push(item);
        }
        self.future.clear();
    }

    /// Undo `n` operations at once, returning all undone items.
    pub fn undo_n(&mut self, n: usize) -> Vec<T> {
        let mut results = Vec::new();
        for _ in 0..n {
            match self.undo() {
                Some(item) => results.push(item),
                None => break,
            }
        }
        results
    }

    /// Redo `n` operations at once, returning all redone items.
    pub fn redo_n(&mut self, n: usize) -> Vec<T> {
        let mut results = Vec::new();
        for _ in 0..n {
            match self.redo() {
                Some(item) => results.push(item),
                None => break,
            }
        }
        results
    }

    /// Returns a slice of the undo history (oldest first).
    pub fn undo_history(&self) -> &[T] {
        &self.past
    }

    /// Returns a slice of the redo history (oldest first).
    pub fn redo_history(&self) -> &[T] {
        &self.future
    }

    /// Returns the configured capacity, if any.
    pub fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    /// Returns total items in both stacks.
    pub fn total_entries(&self) -> usize {
        self.past.len() + self.future.len()
    }

    /// Squash the top `n` undo entries into a single entry using a combining function.
    /// Returns `None` if there are fewer than `n` entries.
    pub fn squash_top(&mut self, n: usize, combine: impl Fn(Vec<T>) -> T) -> Option<T> {
        if self.past.len() < n {
            return None;
        }
        let mut items = Vec::with_capacity(n);
        for _ in 0..n {
            items.push(self.past.pop().unwrap());
        }
        items.reverse();
        let combined = combine(items);
        self.past.push(combined.clone());
        Some(combined)
    }

    /// Remove all redo entries, keeping only the undo stack.
    pub fn discard_redo(&mut self) {
        self.future.clear();
    }
}

impl<T: Clone + PartialEq> UndoRedoStack<T> {
    /// Push only if the new item differs from the top of the undo stack.
    pub fn push_if_changed(&mut self, item: T) -> bool {
        if self.past.last() == Some(&item) {
            return false;
        }
        self.push(item);
        true
    }
}

impl std::error::Error for UndoRedoError {}

// ---------------------------------------------------------------------------
// undo_group — group multiple edits into one undo step
// ---------------------------------------------------------------------------

/// Execute a closure while grouping all edits into a single undo step.
/// Any edits pushed to the service inside `f` will be part of one group.
pub fn undo_group<T: Clone, F>(
    service: &mut UndoRedoService<T>,
    cursor_before: Option<CursorState>,
    cursor_after: Option<CursorState>,
    f: F,
) where
    F: FnOnce(&mut UndoRedoService<T>),
{
    service.open_group(cursor_before);
    f(service);
    service.close_group(cursor_after);
}

/// Group builder for constructing undo groups incrementally.
#[derive(Debug)]
pub struct UndoGroupBuilder<T: Clone> {
    edits: Vec<T>,
    cursor_before: Option<CursorState>,
    cursor_after: Option<CursorState>,
}

impl<T: Clone> UndoGroupBuilder<T> {
    pub fn new() -> Self {
        Self {
            edits: Vec::new(),
            cursor_before: None,
            cursor_after: None,
        }
    }

    pub fn cursor_before(mut self, state: CursorState) -> Self {
        self.cursor_before = Some(state);
        self
    }

    pub fn cursor_after(mut self, state: CursorState) -> Self {
        self.cursor_after = Some(state);
        self
    }

    pub fn add_edit(mut self, edit: T) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Commit the builder contents as a single undo group.
    pub fn commit(self, service: &mut UndoRedoService<T>) {
        if self.edits.is_empty() {
            return;
        }
        service.open_group(self.cursor_before);
        for edit in self.edits {
            service.push_edit(edit, None, None);
        }
        service.close_group(self.cursor_after);
    }
}

impl<T: Clone> Default for UndoGroupBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// UndoRedoMetrics — tracks undo/redo usage statistics
// ---------------------------------------------------------------------------

/// Tracks frequency and usage statistics for undo/redo operations.
#[derive(Debug, Clone, Default)]
pub struct UndoRedoMetrics {
    /// Total number of undo operations performed.
    pub undo_count: u64,
    /// Total number of redo operations performed.
    pub redo_count: u64,
    /// Total number of push operations performed.
    pub push_count: u64,
    /// Total number of clear operations performed.
    pub clear_count: u64,
    /// Peak undo depth ever reached.
    pub peak_undo_depth: usize,
}

impl UndoRedoMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an undo operation.
    pub fn record_undo(&mut self) {
        self.undo_count += 1;
    }

    /// Record a redo operation.
    pub fn record_redo(&mut self) {
        self.redo_count += 1;
    }

    /// Record a push operation and update peak depth if needed.
    pub fn record_push(&mut self, current_depth: usize) {
        self.push_count += 1;
        if current_depth > self.peak_undo_depth {
            self.peak_undo_depth = current_depth;
        }
    }

    /// Record a clear operation.
    pub fn record_clear(&mut self) {
        self.clear_count += 1;
    }

    /// Returns the total number of all recorded operations.
    pub fn total_operations(&self) -> u64 {
        self.undo_count + self.redo_count + self.push_count + self.clear_count
    }

    /// Reset all metrics to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl fmt::Display for UndoRedoMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Metrics(push: {}, undo: {}, redo: {}, clear: {}, peak: {})",
            self.push_count,
            self.undo_count,
            self.redo_count,
            self.clear_count,
            self.peak_undo_depth,
        )
    }
}

// ---------------------------------------------------------------------------
// Checkpoint — named snapshot of undo/redo stack state
// ---------------------------------------------------------------------------

/// A checkpoint captures the lengths of the undo and redo stacks at a moment
/// in time, allowing callers to detect whether changes have been made since
/// the checkpoint was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// Label for the checkpoint.
    pub label: String,
    /// Length of the undo stack when the checkpoint was taken.
    pub undo_len: usize,
    /// Length of the redo stack when the checkpoint was taken.
    pub redo_len: usize,
    /// Monotonic sequence number at checkpoint time.
    pub seq: u64,
}

impl fmt::Display for Checkpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Checkpoint('{}', undo={}, redo={}, seq={})",
            self.label, self.undo_len, self.redo_len, self.seq,
        )
    }
}

impl<T: Clone> UndoRedoStack<T> {
    /// Create a named checkpoint of the current stack state.
    pub fn checkpoint(&self, label: impl Into<String>, seq: u64) -> Checkpoint {
        Checkpoint {
            label: label.into(),
            undo_len: self.past.len(),
            redo_len: self.future.len(),
            seq,
        }
    }

    /// Returns `true` if the stack state has changed since the given checkpoint.
    pub fn changed_since(&self, cp: &Checkpoint) -> bool {
        self.past.len() != cp.undo_len || self.future.len() != cp.redo_len
    }

    /// Truncate the undo stack back to the depth recorded in the checkpoint,
    /// discarding any entries pushed after the checkpoint was taken.
    /// Returns the number of entries removed.
    pub fn restore_to_checkpoint(&mut self, cp: &Checkpoint) -> usize {
        let removed = self.past.len().saturating_sub(cp.undo_len);
        self.past.truncate(cp.undo_len);
        self.future.truncate(cp.redo_len);
        removed
    }
}

// ---------------------------------------------------------------------------
// HistoryCompactor — compact consecutive similar entries
// ---------------------------------------------------------------------------

/// Compacts an undo history by merging consecutive entries that satisfy a
/// caller-provided predicate.
pub struct HistoryCompactor;

impl HistoryCompactor {
    /// Compact a vector of entries by merging consecutive runs where `should_merge`
    /// returns `true`. Merged runs are combined using `merge_fn`.
    pub fn compact<T>(
        entries: Vec<T>,
        should_merge: impl Fn(&T, &T) -> bool,
        merge_fn: impl Fn(Vec<T>) -> T,
    ) -> Vec<T> {
        if entries.is_empty() {
            return Vec::new();
        }
        let mut result: Vec<Vec<T>> = Vec::new();
        let mut current_run: Vec<T> = Vec::new();

        for entry in entries {
            if current_run.is_empty() {
                current_run.push(entry);
            } else if should_merge(current_run.last().unwrap(), &entry) {
                current_run.push(entry);
            } else {
                result.push(std::mem::take(&mut current_run));
                current_run.push(entry);
            }
        }
        if !current_run.is_empty() {
            result.push(current_run);
        }

        result.into_iter().map(merge_fn).collect()
    }
}

// ---------------------------------------------------------------------------
// TimelineEntry — helper for visualising undo history
// ---------------------------------------------------------------------------

/// A single entry in an undo/redo timeline suitable for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    /// Index within the combined timeline (0 = oldest).
    pub index: usize,
    /// Whether this entry is in the undo (past) or redo (future) portion.
    pub kind: TimelineKind,
    /// Human-readable description.
    pub label: String,
}

/// Whether a timeline entry is part of the undo or redo stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineKind {
    Undo,
    Redo,
}

impl fmt::Display for TimelineEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let marker = match self.kind {
            TimelineKind::Undo => "←",
            TimelineKind::Redo => "→",
        };
        write!(f, "{:>3} {} {}", self.index, marker, self.label)
    }
}

impl<T: Clone + fmt::Display> UndoRedoStack<T> {
    /// Build a timeline of all undo and redo entries for display.
    ///
    /// Undo entries come first (oldest to newest), then redo entries (oldest
    /// to newest — i.e. the next redo first).
    pub fn timeline(&self) -> Vec<TimelineEntry> {
        let mut out = Vec::with_capacity(self.past.len() + self.future.len());
        for (i, item) in self.past.iter().enumerate() {
            out.push(TimelineEntry {
                index: i,
                kind: TimelineKind::Undo,
                label: item.to_string(),
            });
        }
        // Redo stack is stored newest-first internally; reverse for display.
        for (i, item) in self.future.iter().rev().enumerate() {
            out.push(TimelineEntry {
                index: self.past.len() + i,
                kind: TimelineKind::Redo,
                label: item.to_string(),
            });
        }
        out
    }
}

// ---------------------------------------------------------------------------
// EditKind — categorise edits for selective undo
// ---------------------------------------------------------------------------

/// Categorises an edit operation for selective undo filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditKind {
    /// A character or text insertion.
    Insert,
    /// A deletion (backspace, delete, cut).
    Delete,
    /// A replacement (select-then-type, find-and-replace).
    Replace,
    /// Formatting-only change (indent, whitespace normalisation).
    Format,
    /// Any other edit that does not fit the above categories.
    Other,
}

impl fmt::Display for EditKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Insert => write!(f, "insert"),
            Self::Delete => write!(f, "delete"),
            Self::Replace => write!(f, "replace"),
            Self::Format => write!(f, "format"),
            Self::Other => write!(f, "other"),
        }
    }
}

// ---------------------------------------------------------------------------
// TaggedEdit — an edit value paired with its kind
// ---------------------------------------------------------------------------

/// Wraps an edit value together with an [`EditKind`] tag so that the undo
/// system can reason about edit categories without knowing `T`.
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedEdit<T> {
    pub kind: EditKind,
    pub value: T,
    /// Optional human-readable description of this edit.
    pub description: Option<String>,
}

impl<T> TaggedEdit<T> {
    pub fn new(kind: EditKind, value: T) -> Self {
        Self {
            kind,
            value,
            description: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

impl<T: fmt::Display> fmt::Display for TaggedEdit<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.description {
            Some(d) => write!(f, "[{}] {} ({})", self.kind, self.value, d),
            None => write!(f, "[{}] {}", self.kind, self.value),
        }
    }
}

// ---------------------------------------------------------------------------
// Selective undo — filter undo history by EditKind
// ---------------------------------------------------------------------------

impl<T: Clone> UndoRedoStack<TaggedEdit<T>> {
    /// Remove and return the most recent undo entry whose kind matches `kind`,
    /// shifting later entries to preserve order. Returns `None` if no matching
    /// entry exists.
    pub fn selective_undo(&mut self, kind: EditKind) -> Option<TaggedEdit<T>> {
        let pos = self.past.iter().rposition(|e| e.kind == kind)?;
        let entry = self.past.remove(pos);
        Some(entry)
    }

    /// Count how many undo entries match the given `kind`.
    pub fn count_by_kind(&self, kind: EditKind) -> usize {
        self.past.iter().filter(|e| e.kind == kind).count()
    }
}

// ---------------------------------------------------------------------------
// MemoryBudget — track estimated memory consumption of the undo stack
// ---------------------------------------------------------------------------

/// Trait for types that can estimate their in-memory size in bytes.
pub trait EstimateSize {
    fn estimated_size(&self) -> usize;
}

impl EstimateSize for String {
    fn estimated_size(&self) -> usize {
        std::mem::size_of::<String>() + self.capacity()
    }
}

impl EstimateSize for Vec<u8> {
    fn estimated_size(&self) -> usize {
        std::mem::size_of::<Vec<u8>>() + self.capacity()
    }
}

/// Tracks the memory budget consumed by an undo stack.
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// Maximum allowed bytes.
    pub limit: usize,
    /// Currently consumed bytes.
    pub used: usize,
}

impl MemoryBudget {
    pub fn new(limit: usize) -> Self {
        Self { limit, used: 0 }
    }

    /// Record that `bytes` have been added to the budget.
    pub fn add(&mut self, bytes: usize) {
        self.used = self.used.saturating_add(bytes);
    }

    /// Record that `bytes` have been freed from the budget.
    pub fn free(&mut self, bytes: usize) {
        self.used = self.used.saturating_sub(bytes);
    }

    /// Returns `true` if `additional` bytes would exceed the budget.
    pub fn would_exceed(&self, additional: usize) -> bool {
        self.used.saturating_add(additional) > self.limit
    }

    /// Remaining bytes before the budget is exhausted.
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.used)
    }

    /// Fraction of the budget consumed, in the range `0.0..=1.0`.
    pub fn utilisation(&self) -> f64 {
        if self.limit == 0 {
            return 1.0;
        }
        self.used as f64 / self.limit as f64
    }
}

impl fmt::Display for MemoryBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemoryBudget({}/{} bytes, {:.1}%)",
            self.used,
            self.limit,
            self.utilisation() * 100.0,
        )
    }
}

impl<T: Clone + EstimateSize> UndoRedoStack<T> {
    /// Push an item while respecting a memory budget, evicting oldest entries
    /// as needed. Returns the number of entries evicted.
    pub fn push_budgeted(&mut self, item: T, budget: &mut MemoryBudget) -> usize {
        let item_size = item.estimated_size();
        let mut evicted = 0;
        while budget.would_exceed(item_size) && !self.past.is_empty() {
            let old = self.past.remove(0);
            budget.free(old.estimated_size());
            evicted += 1;
        }
        budget.add(item_size);
        self.past.push(item);
        self.future.clear();
        evicted
    }

    /// Compute the total estimated memory used by all undo entries.
    pub fn estimated_memory(&self) -> usize {
        self.past.iter().map(|e| e.estimated_size()).sum::<usize>()
            + self.future.iter().map(|e| e.estimated_size()).sum::<usize>()
    }
}

// ---------------------------------------------------------------------------
// UndoStats — summary statistics about the undo stack
// ---------------------------------------------------------------------------

/// Snapshot of undo/redo stack statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoStats {
    pub undo_depth: usize,
    pub redo_depth: usize,
    pub total_entries: usize,
    pub has_capacity_limit: bool,
}

impl<T: Clone> UndoRedoStack<T> {
    /// Gather a snapshot of the current stack statistics.
    pub fn stats(&self) -> UndoStats {
        UndoStats {
            undo_depth: self.past.len(),
            redo_depth: self.future.len(),
            total_entries: self.past.len() + self.future.len(),
            has_capacity_limit: self.capacity.is_some(),
        }
    }
}

impl fmt::Display for UndoStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UndoStats(undo={}, redo={}, total={}, capped={})",
            self.undo_depth, self.redo_depth, self.total_entries, self.has_capacity_limit,
        )
    }
}

// ---------------------------------------------------------------------------
// UndoRedoSizeLimit – memory-aware undo management
// ---------------------------------------------------------------------------

/// Tracks memory usage of the undo/redo stack and enforces limits.
#[derive(Debug, Clone)]
pub struct UndoRedoSizeLimit {
    /// Maximum memory budget in bytes.
    pub max_bytes: usize,
    /// Current estimated memory usage in bytes.
    pub current_bytes: usize,
    /// Number of entries evicted due to memory pressure.
    pub eviction_count: usize,
}

impl UndoRedoSizeLimit {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            current_bytes: 0,
            eviction_count: 0,
        }
    }

    /// Record that `size` bytes have been added.
    pub fn record_add(&mut self, size: usize) {
        self.current_bytes += size;
    }

    /// Record that `size` bytes have been removed.
    pub fn record_remove(&mut self, size: usize) {
        self.current_bytes = self.current_bytes.saturating_sub(size);
    }

    /// Whether the limit has been exceeded.
    pub fn is_exceeded(&self) -> bool {
        self.current_bytes > self.max_bytes
    }

    /// How many bytes remain before the limit is reached.
    pub fn remaining(&self) -> usize {
        self.max_bytes.saturating_sub(self.current_bytes)
    }

    /// Usage as a percentage 0.0..=100.0+.
    pub fn usage_pct(&self) -> f64 {
        if self.max_bytes == 0 {
            return 100.0;
        }
        (self.current_bytes as f64 / self.max_bytes as f64) * 100.0
    }

    /// Record an eviction.
    pub fn record_eviction(&mut self, freed_bytes: usize) {
        self.record_remove(freed_bytes);
        self.eviction_count += 1;
    }
}

impl fmt::Display for UndoRedoSizeLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SizeLimit({}/{} bytes, {:.1}%, {} evictions)",
            self.current_bytes, self.max_bytes, self.usage_pct(), self.eviction_count
        )
    }
}

// ---------------------------------------------------------------------------
// UndoRedoHistory – serializable undo history
// ---------------------------------------------------------------------------

/// A serializable snapshot of the undo/redo history.
#[derive(Debug, Clone)]
pub struct UndoRedoHistory {
    /// Descriptions of undo entries (oldest first).
    pub undo_descriptions: Vec<String>,
    /// Descriptions of redo entries (oldest first).
    pub redo_descriptions: Vec<String>,
    /// Timestamp (epoch seconds) when the snapshot was taken.
    pub snapshot_time: u64,
}

impl UndoRedoHistory {
    pub fn new() -> Self {
        Self {
            undo_descriptions: Vec::new(),
            redo_descriptions: Vec::new(),
            snapshot_time: 0,
        }
    }

    /// Total number of entries in the history.
    pub fn total_entries(&self) -> usize {
        self.undo_descriptions.len() + self.redo_descriptions.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.undo_descriptions.is_empty() && self.redo_descriptions.is_empty()
    }

    /// Serialize to a simple text format: one description per line, section-separated.
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("[undo]\n");
        for d in &self.undo_descriptions {
            out.push_str(d);
            out.push('\n');
        }
        out.push_str("[redo]\n");
        for d in &self.redo_descriptions {
            out.push_str(d);
            out.push('\n');
        }
        out
    }

    /// Deserialize from the text format produced by `serialize`.
    pub fn deserialize(input: &str) -> Self {
        let mut history = Self::new();
        let mut in_redo = false;
        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed == "[undo]" {
                in_redo = false;
            } else if trimmed == "[redo]" {
                in_redo = true;
            } else if !trimmed.is_empty() {
                if in_redo {
                    history.redo_descriptions.push(trimmed.to_string());
                } else {
                    history.undo_descriptions.push(trimmed.to_string());
                }
            }
        }
        history
    }
}

impl Default for UndoRedoHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UndoRedoHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UndoRedoHistory(undo={}, redo={})",
            self.undo_descriptions.len(),
            self.redo_descriptions.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Undo branch navigation
// ---------------------------------------------------------------------------

/// A branch in the undo tree. Each branch has a parent index and a list of entries.
#[derive(Debug, Clone)]
pub struct UndoBranch {
    pub id: u32,
    pub parent_id: Option<u32>,
    pub entries: Vec<String>,
}

impl UndoBranch {
    pub fn new(id: u32, parent_id: Option<u32>) -> Self {
        Self {
            id,
            parent_id,
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, entry: String) {
        self.entries.push(entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Navigator for undo branches (tree-structured undo).
#[derive(Debug, Clone)]
pub struct UndoBranchNavigator {
    branches: Vec<UndoBranch>,
    active_branch: u32,
    next_id: u32,
}

impl UndoBranchNavigator {
    pub fn new() -> Self {
        let root = UndoBranch::new(0, None);
        Self {
            branches: vec![root],
            active_branch: 0,
            next_id: 1,
        }
    }

    /// Create a new branch from the current active branch.
    pub fn fork(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let branch = UndoBranch::new(id, Some(self.active_branch));
        self.branches.push(branch);
        self.active_branch = id;
        id
    }

    /// Switch to a different branch by ID.
    pub fn switch_to(&mut self, branch_id: u32) -> bool {
        if self.branches.iter().any(|b| b.id == branch_id) {
            self.active_branch = branch_id;
            true
        } else {
            false
        }
    }

    /// Push an entry onto the active branch.
    pub fn push_entry(&mut self, entry: String) {
        if let Some(b) = self.branches.iter_mut().find(|b| b.id == self.active_branch) {
            b.push(entry);
        }
    }

    /// Get the active branch.
    pub fn active(&self) -> Option<&UndoBranch> {
        self.branches.iter().find(|b| b.id == self.active_branch)
    }

    /// Total number of branches.
    pub fn branch_count(&self) -> usize {
        self.branches.len()
    }

    /// List all branch IDs.
    pub fn branch_ids(&self) -> Vec<u32> {
        self.branches.iter().map(|b| b.id).collect()
    }

    /// Get the parent chain of the active branch.
    pub fn ancestry(&self) -> Vec<u32> {
        let mut chain = Vec::new();
        let mut current = self.active_branch;
        loop {
            chain.push(current);
            if let Some(b) = self.branches.iter().find(|b| b.id == current) {
                if let Some(parent) = b.parent_id {
                    current = parent;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        chain.reverse();
        chain
    }
}

impl Default for UndoBranchNavigator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UndoBranchNavigator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UndoBranchNavigator(branches={}, active={})",
            self.branches.len(),
            self.active_branch
        )
    }
}


// === Undo Redo Group Namer ===

/// Undo Redo Group Namer implementation.
#[derive(Debug, Clone)]
pub struct UndoRedoGroupNamer {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: UndoRedoGroupNamerStats,
}

/// Statistics for UndoRedoGroupNamer.
#[derive(Debug, Clone, Default)]
pub struct UndoRedoGroupNamerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl UndoRedoGroupNamerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl UndoRedoGroupNamer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: UndoRedoGroupNamerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &UndoRedoGroupNamerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for UndoRedoGroupNamer {
    fn default() -> Self {
        Self::new()
    }
}

// === Undo Redo Memory Tracker ===

/// Priority level for UndoRedoMemoryTracker items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UndoRedoMemoryTrackerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl UndoRedoMemoryTrackerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for UndoRedoMemoryTrackerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Undo Redo Memory Tracker implementation.
#[derive(Debug, Clone)]
pub struct UndoRedoMemoryTracker {
    items: Vec<UndoRedoMemoryTrackerItem>,
    max_items: usize,
    default_priority: UndoRedoMemoryTrackerPriority,
}

/// A single item in UndoRedoMemoryTracker.
#[derive(Debug, Clone)]
pub struct UndoRedoMemoryTrackerItem {
    pub id: String,
    pub label: String,
    pub priority: UndoRedoMemoryTrackerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl UndoRedoMemoryTrackerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: UndoRedoMemoryTrackerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: UndoRedoMemoryTrackerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl UndoRedoMemoryTracker {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: UndoRedoMemoryTrackerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: UndoRedoMemoryTrackerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<UndoRedoMemoryTrackerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&UndoRedoMemoryTrackerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: UndoRedoMemoryTrackerPriority) -> Vec<&UndoRedoMemoryTrackerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&UndoRedoMemoryTrackerItem> {
        let mut sorted: Vec<&UndoRedoMemoryTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&UndoRedoMemoryTrackerItem> {
        let mut sorted: Vec<&UndoRedoMemoryTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&UndoRedoMemoryTrackerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: UndoRedoMemoryTrackerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> UndoRedoMemoryTrackerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &UndoRedoMemoryTrackerItem> {
        self.items.iter()
    }
}

impl Default for UndoRedoMemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-undoredo: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoredoXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl UndoredoXConfig {
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

impl std::fmt::Display for UndoredoXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct UndoredoXRegistry {
    entries: Vec<UndoredoXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl UndoredoXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: UndoredoXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&UndoredoXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut UndoredoXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<UndoredoXConfig> {
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

    pub fn active_entries(&self) -> Vec<&UndoredoXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&UndoredoXConfig> {
        let mut sorted: Vec<&UndoredoXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&UndoredoXConfig> {
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

    pub fn iter(&self) -> UndoredoXIterator<'_> {
        UndoredoXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct UndoredoXIterator<'a> {
    inner: std::slice::Iter<'a, UndoredoXConfig>,
}

impl<'a> Iterator for UndoredoXIterator<'a> {
    type Item = &'a UndoredoXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct UndoredoXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl UndoredoXCache {
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
pub struct UndoredoXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl UndoredoXFormatter {
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

    pub fn format_entry(&self, entry: &UndoredoXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &UndoredoXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &UndoredoXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for UndoredoXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct UndoredoXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl UndoredoXValidator {
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

    pub fn validate(&self, entry: &UndoredoXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &UndoredoXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for UndoredoXValidator {
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
// xc_ pool and scheduler – generated block 189
// ---------------------------------------------------------------------------

/// Generic object pool `Xc189Pool<T>`.
pub struct Xc189Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc189Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc189PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc189Pool<T> {
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
    pub fn stats(&self) -> Xc189PoolStats {
        Xc189PoolStats {
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

impl<T> Default for Xc189Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc189Scheduler`.
pub struct Xc189Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc189Scheduler {
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

impl Default for Xc189Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_189 hash for the given byte slice.
pub fn xc_189_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_189 convention.
pub fn xc_189_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_12 deepening: state machine + event bus ---

/// States for the Xd12 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd12State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd12State {
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
pub struct Xd12Transition {
    pub from: Xd12State,
    pub to: Xd12State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd12StateMachine {
    current: Xd12State,
    history: Vec<Xd12Transition>,
    step_counter: usize,
}

impl Xd12StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd12State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd12State {
        self.current
    }

    pub fn history(&self) -> &[Xd12Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd12State) -> Result<Xd12State, String> {
        let allowed = match (self.current, target) {
            (Xd12State::Idle, Xd12State::Running) => true,
            (Xd12State::Running, Xd12State::Paused) => true,
            (Xd12State::Running, Xd12State::Done) => true,
            (Xd12State::Paused, Xd12State::Running) => true,
            (Xd12State::Paused, Xd12State::Done) => true,
            (Xd12State::Done, Xd12State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_12: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd12Transition {
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
            "Xd12SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd12State> {
        let prefix = "Xd12SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd12State::Idle),
            "Running" => Some(Xd12State::Running),
            "Paused" => Some(Xd12State::Paused),
            "Done" => Some(Xd12State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd12State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd12 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd12Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd12Event {
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

type Xd12HandlerFn = Box<dyn Fn(&Xd12Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd12EventBus {
    handlers: Vec<(usize, Option<String>, Xd12HandlerFn)>,
    next_id: usize,
    published: Vec<Xd12Event>,
}

impl Xd12EventBus {
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
        F: Fn(&Xd12Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd12Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd12Event) {
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

    pub fn published_events(&self) -> &[Xd12Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #10
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf10Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf10TrieNode {
    children: std::collections::HashMap<char, Xf10TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf10Trie {
    root: Xf10TrieNode,
    count: usize,
}

impl Xf10Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf10TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf10TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf10TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf10BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf10BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 188).
pub struct Xh188SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh188SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 230 as u64,
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

/// A compact bit set supporting boolean operations (variant 188).
pub struct Xh188BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh188BitSet {
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
    fn new_stack_is_empty() {
        let stack: UndoRedoStack<i32> = UndoRedoStack::new();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn push_and_undo() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        assert!(stack.can_undo());
        assert_eq!(stack.undo(), Some(2));
        assert_eq!(stack.undo(), Some(1));
        assert!(!stack.can_undo());
    }

    #[test]
    fn undo_and_redo() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.undo();
        assert!(stack.can_redo());
        assert_eq!(stack.redo(), Some(2));
        assert!(!stack.can_redo());
    }

    #[test]
    fn push_clears_redo() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.undo();
        assert!(stack.can_redo());
        stack.push(3);
        assert!(!stack.can_redo());
    }

    #[test]
    fn clear_empties_both() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.undo();
        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_empty_returns_none() {
        let mut stack: UndoRedoStack<i32> = UndoRedoStack::new();
        assert_eq!(stack.undo(), None);
    }

    #[test]
    fn redo_empty_returns_none() {
        let mut stack: UndoRedoStack<i32> = UndoRedoStack::new();
        assert_eq!(stack.redo(), None);
    }

    #[test]
    fn default_is_new() {
        let stack: UndoRedoStack<String> = UndoRedoStack::default();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn with_capacity_evicts_oldest() {
        let mut stack = UndoRedoStack::with_capacity(3);
        stack.push(1);
        stack.push(2);
        stack.push(3);
        stack.push(4); // evicts 1
        assert_eq!(stack.undo_count(), 3);
        assert_eq!(stack.undo(), Some(4));
        assert_eq!(stack.undo(), Some(3));
        assert_eq!(stack.undo(), Some(2));
        assert_eq!(stack.undo(), None);
    }

    #[test]
    fn with_capacity_zero_keeps_nothing() {
        let mut stack = UndoRedoStack::with_capacity(0);
        stack.push(1);
        assert!(!stack.can_undo());
    }

    #[test]
    fn undo_count_and_redo_count() {
        let mut stack = UndoRedoStack::new();
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 0);
        stack.push(10);
        stack.push(20);
        assert_eq!(stack.undo_count(), 2);
        stack.undo();
        assert_eq!(stack.undo_count(), 1);
        assert_eq!(stack.redo_count(), 1);
    }

    #[test]
    fn peek_undo_and_peek_redo() {
        let mut stack = UndoRedoStack::new();
        assert_eq!(stack.peek_undo(), None);
        assert_eq!(stack.peek_redo(), None);
        stack.push(5);
        stack.push(10);
        assert_eq!(stack.peek_undo(), Some(&10));
        stack.undo();
        assert_eq!(stack.peek_redo(), Some(&10));
        assert_eq!(stack.peek_undo(), Some(&5));
    }

    #[test]
    fn try_undo_success() {
        let mut stack = UndoRedoStack::new();
        stack.push(42);
        assert_eq!(stack.try_undo(), Ok(42));
    }

    #[test]
    fn try_undo_error() {
        let mut stack: UndoRedoStack<i32> = UndoRedoStack::new();
        assert_eq!(stack.try_undo(), Err(UndoRedoError::NothingToUndo));
    }

    #[test]
    fn try_redo_success() {
        let mut stack = UndoRedoStack::new();
        stack.push(7);
        stack.undo();
        assert_eq!(stack.try_redo(), Ok(7));
    }

    #[test]
    fn try_redo_error() {
        let mut stack: UndoRedoStack<i32> = UndoRedoStack::new();
        assert_eq!(stack.try_redo(), Err(UndoRedoError::NothingToRedo));
    }

    #[test]
    fn display_impl() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);
        stack.undo();
        assert_eq!(format!("{stack}"), "UndoRedoStack(undo: 2, redo: 1)");
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(UndoRedoError::NothingToUndo.to_string(), "nothing to undo");
        assert_eq!(UndoRedoError::NothingToRedo.to_string(), "nothing to redo");
        assert_eq!(UndoRedoError::StackFull.to_string(), "undo stack is full");
    }

    #[test]
    fn undo_entry_creation_and_display() {
        let entry = UndoEntry::new("edit", 42, 1000);
        assert_eq!(entry.label, "edit");
        assert_eq!(entry.value, 42);
        assert_eq!(entry.timestamp, 1000);
        assert_eq!(format!("{entry}"), "[edit] 42 (t=1000)");
    }

    #[test]
    fn transaction_lifecycle() {
        let mut tx: Transaction<String> = Transaction::new("batch edit");
        assert!(tx.is_empty());
        assert!(!tx.is_committed());
        tx.add("change1".to_string());
        tx.add("change2".to_string());
        assert_eq!(tx.len(), 2);
        assert!(!tx.is_empty());
        tx.commit();
        assert!(tx.is_committed());
        assert_eq!(tx.entries().len(), 2);
    }

    #[test]
    fn transaction_display() {
        let mut tx: Transaction<i32> = Transaction::new("test");
        tx.add(1);
        tx.add(2);
        let s = format!("{tx}");
        assert!(s.contains("test"));
        assert!(s.contains("2 entries"));
        assert!(s.contains("pending"));
        tx.commit();
        let s2 = format!("{tx}");
        assert!(s2.contains("committed"));
    }

    #[test]
    fn push_batch_operation() {
        let mut stack = UndoRedoStack::new();
        stack.push_batch(vec![1, 2, 3]);
        assert_eq!(stack.undo_count(), 3);
        assert!(!stack.can_redo());
        assert_eq!(stack.undo(), Some(3));
    }

    #[test]
    fn push_batch_with_capacity() {
        let mut stack = UndoRedoStack::with_capacity(2);
        stack.push_batch(vec![1, 2, 3]);
        assert_eq!(stack.undo_count(), 2);
        assert_eq!(stack.undo(), Some(3));
        assert_eq!(stack.undo(), Some(2));
    }

    #[test]
    fn undo_n_multiple() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);
        let undone = stack.undo_n(2);
        assert_eq!(undone, vec![3, 2]);
        assert_eq!(stack.undo_count(), 1);
        assert_eq!(stack.redo_count(), 2);
    }

    #[test]
    fn undo_n_more_than_available() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        let undone = stack.undo_n(5);
        assert_eq!(undone, vec![1]);
    }

    #[test]
    fn redo_n_multiple() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);
        stack.undo_n(3);
        let redone = stack.redo_n(2);
        assert_eq!(redone, vec![1, 2]);
        assert_eq!(stack.redo_count(), 1);
    }

    #[test]
    fn undo_and_redo_history_slices() {
        let mut stack = UndoRedoStack::new();
        stack.push(10);
        stack.push(20);
        stack.push(30);
        stack.undo();
        assert_eq!(stack.undo_history(), &[10, 20]);
        assert_eq!(stack.redo_history(), &[30]);
    }

    #[test]
    fn capacity_accessor() {
        let stack: UndoRedoStack<i32> = UndoRedoStack::new();
        assert_eq!(stack.capacity(), None);
        let stack2: UndoRedoStack<i32> = UndoRedoStack::with_capacity(10);
        assert_eq!(stack2.capacity(), Some(10));
    }

    #[test]
    fn total_entries_count() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);
        stack.undo();
        assert_eq!(stack.total_entries(), 3);
    }

    #[test]
    fn squash_top_combines() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);
        let combined = stack.squash_top(3, |items| items.iter().sum());
        assert_eq!(combined, Some(6));
        assert_eq!(stack.undo_count(), 1);
        assert_eq!(stack.undo(), Some(6));
    }

    #[test]
    fn squash_top_not_enough_entries() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        assert_eq!(stack.squash_top(5, |_| 0), None);
    }

    #[test]
    fn discard_redo_clears_future() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.undo();
        assert!(stack.can_redo());
        stack.discard_redo();
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 1);
    }

    #[test]
    fn push_if_changed_skips_duplicate() {
        let mut stack = UndoRedoStack::new();
        assert!(stack.push_if_changed(1));
        assert!(!stack.push_if_changed(1));
        assert!(stack.push_if_changed(2));
        assert_eq!(stack.undo_count(), 2);
    }

    #[test]
    fn error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(UndoRedoError::NothingToUndo);
        assert_eq!(err.to_string(), "nothing to undo");
    }

    #[test]
    fn undo_entry_partial_eq() {
        let a = UndoEntry::new("x", 1, 100);
        let b = UndoEntry::new("x", 1, 100);
        let c = UndoEntry::new("y", 1, 100);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // -- UndoRedoService tests -----------------------------------------------

    #[test]
    fn service_push_edit_and_undo() {
        let mut svc = UndoRedoService::<String>::new();
        svc.push_edit("edit1".into(), None, None);
        svc.push_edit("edit2".into(), None, None);
        assert_eq!(svc.undo_count(), 2);
        let g = svc.undo().unwrap();
        assert_eq!(g.edits, vec!["edit2".to_string()]);
        assert_eq!(svc.undo_count(), 1);
    }

    #[test]
    fn service_redo_after_undo() {
        let mut svc = UndoRedoService::<i32>::new();
        svc.push_edit(1, None, None);
        svc.undo();
        assert!(svc.can_redo());
        let g = svc.redo().unwrap();
        assert_eq!(g.edits, vec![1]);
        assert!(!svc.can_redo());
    }

    #[test]
    fn service_open_close_group() {
        let mut svc = UndoRedoService::<String>::new();
        svc.open_group(None);
        svc.push_edit("a".into(), None, None);
        svc.push_edit("b".into(), None, None);
        svc.close_group(None);
        // Both edits are in one group
        assert_eq!(svc.undo_count(), 1);
        let g = svc.undo().unwrap();
        assert_eq!(g.edits.len(), 2);
    }

    #[test]
    fn service_nested_groups() {
        let mut svc = UndoRedoService::<i32>::new();
        svc.open_group(None);
        svc.push_edit(1, None, None);
        svc.open_group(None);
        svc.push_edit(2, None, None);
        svc.close_group(None);
        svc.push_edit(3, None, None);
        svc.close_group(None);
        assert_eq!(svc.undo_count(), 1);
        let g = svc.undo().unwrap();
        assert_eq!(g.edits, vec![1, 2, 3]);
    }

    #[test]
    fn service_cursor_state_roundtrip() {
        let mut svc = UndoRedoService::<i32>::new();
        let before = CursorState::single(1, 1);
        let after = CursorState::single(1, 5);
        svc.push_edit(42, Some(before.clone()), Some(after.clone()));

        let restored = svc.undo_with_cursor().unwrap();
        assert_eq!(restored, before);

        let restored = svc.redo_with_cursor().unwrap();
        assert_eq!(restored, after);
    }

    #[test]
    fn service_clear() {
        let mut svc = UndoRedoService::<i32>::new();
        svc.push_edit(1, None, None);
        svc.push_edit(2, None, None);
        svc.undo();
        svc.clear();
        assert!(!svc.can_undo());
        assert!(!svc.can_redo());
    }

    #[test]
    fn service_push_clears_redo() {
        let mut svc = UndoRedoService::<i32>::new();
        svc.push_edit(1, None, None);
        svc.undo();
        assert!(svc.can_redo());
        svc.push_edit(2, None, None);
        assert!(!svc.can_redo());
    }

    #[test]
    fn service_is_group_open() {
        let mut svc = UndoRedoService::<i32>::new();
        assert!(!svc.is_group_open());
        svc.open_group(None);
        assert!(svc.is_group_open());
        svc.close_group(None);
        assert!(!svc.is_group_open());
    }

    #[test]
    fn service_undo_empty_returns_none() {
        let mut svc = UndoRedoService::<i32>::new();
        assert!(svc.undo().is_none());
        assert!(svc.undo_with_cursor().is_none());
    }

    #[test]
    fn service_redo_empty_returns_none() {
        let mut svc = UndoRedoService::<i32>::new();
        assert!(svc.redo().is_none());
        assert!(svc.redo_with_cursor().is_none());
    }

    #[test]
    fn cursor_state_single() {
        let cs = CursorState::single(3, 7);
        assert_eq!(cs.positions.len(), 1);
        assert_eq!(cs.positions[0].line, 3);
        assert_eq!(cs.positions[0].column, 7);
    }

    #[test]
    fn cursor_selection_collapsed() {
        let sel = CursorSelection::collapsed(2, 4);
        assert_eq!(sel.anchor_line, 2);
        assert_eq!(sel.anchor_column, 4);
        assert_eq!(sel.active_line, 2);
        assert_eq!(sel.active_column, 4);
    }

    #[test]
    fn service_push_group() {
        let mut svc = UndoRedoService::<i32>::new();
        let group = UndoRedoGroup {
            id: 0,
            edits: vec![10, 20, 30],
            cursor_before: Some(CursorState::single(1, 1)),
            cursor_after: Some(CursorState::single(1, 4)),
        };
        svc.push_group(group);
        assert_eq!(svc.undo_count(), 1);
        let g = svc.peek_undo().unwrap();
        assert_eq!(g.edits.len(), 3);
    }

    #[test]
    fn service_group_with_cursor_in_open_close() {
        let mut svc = UndoRedoService::<&str>::new();
        let before = CursorState::single(1, 1);
        let after = CursorState::single(1, 10);
        svc.open_group(Some(before.clone()));
        svc.push_edit("type a", None, None);
        svc.push_edit("auto-close bracket", None, None);
        svc.close_group(Some(after.clone()));

        let restored = svc.undo_with_cursor().unwrap();
        assert_eq!(restored, before);
    }

    // -- undo_group tests ---------------------------------------------------

    #[test]
    fn undo_group_combines_edits() {
        let mut svc = UndoRedoService::<i32>::new();
        undo_group(&mut svc, None, None, |s| {
            s.push_edit(1, None, None);
            s.push_edit(2, None, None);
            s.push_edit(3, None, None);
        });
        assert_eq!(svc.undo_count(), 1);
        let g = svc.undo().unwrap();
        assert_eq!(g.edits, vec![1, 2, 3]);
    }

    #[test]
    fn undo_group_with_cursors() {
        let mut svc = UndoRedoService::<&str>::new();
        let before = CursorState::single(1, 1);
        let after = CursorState::single(1, 10);
        undo_group(&mut svc, Some(before.clone()), Some(after.clone()), |s| {
            s.push_edit("a", None, None);
            s.push_edit("b", None, None);
        });
        let restored = svc.undo_with_cursor().unwrap();
        assert_eq!(restored, before);
    }

    #[test]
    fn undo_group_empty_closure() {
        let mut svc = UndoRedoService::<i32>::new();
        undo_group(&mut svc, None, None, |_| {});
        // Empty group still created (service handles empty groups)
        assert!(svc.undo_count() <= 1);
    }

    #[test]
    fn builder_commit() {
        let mut svc = UndoRedoService::<i32>::new();
        UndoGroupBuilder::new()
            .add_edit(10)
            .add_edit(20)
            .add_edit(30)
            .commit(&mut svc);
        assert_eq!(svc.undo_count(), 1);
        let g = svc.undo().unwrap();
        assert_eq!(g.edits, vec![10, 20, 30]);
    }

    #[test]
    fn builder_empty_does_not_commit() {
        let mut svc = UndoRedoService::<i32>::new();
        UndoGroupBuilder::new().commit(&mut svc);
        assert_eq!(svc.undo_count(), 0);
    }

    #[test]
    fn builder_with_cursors() {
        let mut svc = UndoRedoService::<i32>::new();
        let before = CursorState::single(5, 1);
        let after = CursorState::single(5, 20);
        UndoGroupBuilder::new()
            .cursor_before(before.clone())
            .cursor_after(after.clone())
            .add_edit(42)
            .commit(&mut svc);
        let restored = svc.undo_with_cursor().unwrap();
        assert_eq!(restored, before);
    }

    #[test]
    fn builder_edit_count_and_is_empty() {
        let builder = UndoGroupBuilder::<i32>::new();
        assert!(builder.is_empty());
        assert_eq!(builder.edit_count(), 0);
        let builder = builder.add_edit(1).add_edit(2);
        assert!(!builder.is_empty());
        assert_eq!(builder.edit_count(), 2);
    }

    // -- UndoRedoMetrics tests -----------------------------------------------

    #[test]
    fn metrics_tracks_operations() {
        let mut m = UndoRedoMetrics::new();
        assert_eq!(m.total_operations(), 0);

        m.record_push(1);
        m.record_push(2);
        m.record_push(3);
        m.record_undo();
        m.record_redo();
        m.record_clear();

        assert_eq!(m.push_count, 3);
        assert_eq!(m.undo_count, 1);
        assert_eq!(m.redo_count, 1);
        assert_eq!(m.clear_count, 1);
        assert_eq!(m.total_operations(), 6);
        assert_eq!(m.peak_undo_depth, 3);

        let display = format!("{m}");
        assert!(display.contains("push: 3"));
        assert!(display.contains("peak: 3"));
    }

    #[test]
    fn metrics_reset() {
        let mut m = UndoRedoMetrics::new();
        m.record_push(5);
        m.record_undo();
        m.reset();
        assert_eq!(m.total_operations(), 0);
        assert_eq!(m.peak_undo_depth, 0);
    }

    // -- Checkpoint tests ----------------------------------------------------

    #[test]
    fn checkpoint_detects_changes() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        let cp = stack.checkpoint("save", 1);
        assert!(!stack.changed_since(&cp));

        stack.push(3);
        assert!(stack.changed_since(&cp));

        let display = format!("{cp}");
        assert!(display.contains("save"));
        assert!(display.contains("seq=1"));
    }

    #[test]
    fn checkpoint_restore_truncates() {
        let mut stack = UndoRedoStack::new();
        stack.push(10);
        stack.push(20);
        let cp = stack.checkpoint("before-batch", 0);
        stack.push(30);
        stack.push(40);
        assert_eq!(stack.undo_count(), 4);

        let removed = stack.restore_to_checkpoint(&cp);
        assert_eq!(removed, 2);
        assert_eq!(stack.undo_count(), 2);
        assert_eq!(stack.peek_undo(), Some(&20));
    }

    // -- HistoryCompactor tests ----------------------------------------------

    #[test]
    fn compactor_merges_consecutive_runs() {
        // Compact consecutive equal values by summing them
        let entries = vec![1, 1, 1, 2, 2, 3];
        let compacted = HistoryCompactor::compact(
            entries,
            |a, b| a == b,
            |run| run.iter().sum(),
        );
        assert_eq!(compacted, vec![3, 4, 3]);
    }

    // -- Timeline tests ------------------------------------------------------

    #[test]
    fn timeline_shows_undo_and_redo() {
        let mut stack = UndoRedoStack::new();
        stack.push("alpha".to_string());
        stack.push("beta".to_string());
        stack.push("gamma".to_string());
        stack.undo(); // gamma moves to redo

        let tl = stack.timeline();
        assert_eq!(tl.len(), 3);
        assert_eq!(tl[0].kind, TimelineKind::Undo);
        assert_eq!(tl[0].label, "alpha");
        assert_eq!(tl[1].kind, TimelineKind::Undo);
        assert_eq!(tl[1].label, "beta");
        assert_eq!(tl[2].kind, TimelineKind::Redo);
        assert_eq!(tl[2].label, "gamma");

        // Display format
        let display = format!("{}", tl[0]);
        assert!(display.contains("←"));
        assert!(display.contains("alpha"));
    }

    // -- EditKind & TaggedEdit tests -----------------------------------------

    #[test]
    fn edit_kind_display() {
        assert_eq!(EditKind::Insert.to_string(), "insert");
        assert_eq!(EditKind::Delete.to_string(), "delete");
        assert_eq!(EditKind::Replace.to_string(), "replace");
        assert_eq!(EditKind::Format.to_string(), "format");
        assert_eq!(EditKind::Other.to_string(), "other");
    }

    #[test]
    fn tagged_edit_with_description() {
        let edit = TaggedEdit::new(EditKind::Insert, "hello")
            .with_description("typed greeting");
        assert_eq!(edit.kind, EditKind::Insert);
        assert_eq!(edit.value, "hello");
        assert_eq!(edit.description.as_deref(), Some("typed greeting"));
        let display = format!("{edit}");
        assert!(display.contains("[insert]"));
        assert!(display.contains("hello"));
        assert!(display.contains("typed greeting"));
    }

    // -- Selective undo tests ------------------------------------------------

    #[test]
    fn selective_undo_removes_matching_kind() {
        let mut stack: UndoRedoStack<TaggedEdit<&str>> = UndoRedoStack::new();
        stack.push(TaggedEdit::new(EditKind::Insert, "a"));
        stack.push(TaggedEdit::new(EditKind::Delete, "b"));
        stack.push(TaggedEdit::new(EditKind::Insert, "c"));

        assert_eq!(stack.count_by_kind(EditKind::Insert), 2);
        let removed = stack.selective_undo(EditKind::Insert).unwrap();
        assert_eq!(removed.value, "c");
        assert_eq!(stack.count_by_kind(EditKind::Insert), 1);
        assert_eq!(stack.undo_count(), 2);
    }

    #[test]
    fn selective_undo_returns_none_when_no_match() {
        let mut stack: UndoRedoStack<TaggedEdit<i32>> = UndoRedoStack::new();
        stack.push(TaggedEdit::new(EditKind::Insert, 1));
        assert!(stack.selective_undo(EditKind::Format).is_none());
        assert_eq!(stack.undo_count(), 1);
    }

    // -- MemoryBudget tests --------------------------------------------------

    #[test]
    fn memory_budget_tracking() {
        let mut budget = MemoryBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
        assert!(!budget.would_exceed(500));

        budget.add(600);
        assert_eq!(budget.used, 600);
        assert_eq!(budget.remaining(), 400);
        assert!(budget.would_exceed(500));
        assert!(!budget.would_exceed(400));

        budget.free(200);
        assert_eq!(budget.used, 400);

        let display = format!("{budget}");
        assert!(display.contains("400/1000 bytes"));
    }

    #[test]
    fn push_budgeted_evicts_oldest() {
        let mut stack: UndoRedoStack<String> = UndoRedoStack::new();
        // Each String has overhead (~24 bytes on 64-bit) plus content capacity.
        // Use a small budget to force evictions.
        let mut budget = MemoryBudget::new(200);

        // Push strings that will eventually exceed the budget.
        let s1 = "a".repeat(60);
        let s2 = "b".repeat(60);
        let s3 = "c".repeat(60);
        let evicted1 = stack.push_budgeted(s1, &mut budget);
        assert_eq!(evicted1, 0);
        let evicted2 = stack.push_budgeted(s2, &mut budget);
        assert_eq!(evicted2, 0);
        // Third push should evict at least one entry to stay within budget.
        let evicted3 = stack.push_budgeted(s3, &mut budget);
        assert!(evicted3 >= 1);
        // Stack should still have entries and budget should be within limit.
        assert!(stack.undo_count() >= 1);
        assert!(budget.used <= budget.limit);
    }

    // -- UndoStats tests -----------------------------------------------------

    #[test]
    fn stats_snapshot() {
        let mut stack = UndoRedoStack::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);
        stack.undo();

        let s = stack.stats();
        assert_eq!(s.undo_depth, 2);
        assert_eq!(s.redo_depth, 1);
        assert_eq!(s.total_entries, 3);
        assert!(!s.has_capacity_limit);

        let display = format!("{s}");
        assert!(display.contains("undo=2"));
        assert!(display.contains("redo=1"));

        let capped: UndoRedoStack<i32> = UndoRedoStack::with_capacity(5);
        assert!(capped.stats().has_capacity_limit);
    }

    // -- UndoRedoSizeLimit -------------------------------------------------

    #[test]
    fn size_limit_basic() {
        let mut sl = UndoRedoSizeLimit::new(1024);
        assert!(!sl.is_exceeded());
        assert_eq!(sl.remaining(), 1024);
        sl.record_add(512);
        assert_eq!(sl.remaining(), 512);
        assert!(!sl.is_exceeded());
        assert!((sl.usage_pct() - 50.0).abs() < 0.1);
    }

    #[test]
    fn size_limit_exceeded() {
        let mut sl = UndoRedoSizeLimit::new(100);
        sl.record_add(150);
        assert!(sl.is_exceeded());
    }

    #[test]
    fn size_limit_eviction() {
        let mut sl = UndoRedoSizeLimit::new(100);
        sl.record_add(80);
        sl.record_eviction(50);
        assert_eq!(sl.current_bytes, 30);
        assert_eq!(sl.eviction_count, 1);
    }

    #[test]
    fn size_limit_display() {
        let sl = UndoRedoSizeLimit::new(1000);
        let s = format!("{sl}");
        assert!(s.contains("1000"));
        assert!(s.contains("evictions"));
    }

    // -- UndoRedoHistory ---------------------------------------------------

    #[test]
    fn history_serialize_deserialize() {
        let mut h = UndoRedoHistory::new();
        h.undo_descriptions.push("insert text".into());
        h.undo_descriptions.push("delete line".into());
        h.redo_descriptions.push("paste".into());

        let serialized = h.serialize();
        let restored = UndoRedoHistory::deserialize(&serialized);
        assert_eq!(restored.undo_descriptions, h.undo_descriptions);
        assert_eq!(restored.redo_descriptions, h.redo_descriptions);
    }

    #[test]
    fn history_total_and_empty() {
        let h = UndoRedoHistory::new();
        assert!(h.is_empty());
        assert_eq!(h.total_entries(), 0);
    }

    #[test]
    fn history_display() {
        let h = UndoRedoHistory::default();
        let s = format!("{h}");
        assert!(s.contains("undo=0"));
    }

    // -- UndoBranchNavigator -----------------------------------------------

    #[test]
    fn branch_navigator_new_has_root() {
        let nav = UndoBranchNavigator::new();
        assert_eq!(nav.branch_count(), 1);
        assert_eq!(nav.active().unwrap().id, 0);
    }

    #[test]
    fn branch_navigator_fork() {
        let mut nav = UndoBranchNavigator::new();
        nav.push_entry("edit1".into());
        let branch_id = nav.fork();
        assert_eq!(nav.branch_count(), 2);
        assert_eq!(nav.active().unwrap().id, branch_id);
        nav.push_entry("edit2".into());
        assert_eq!(nav.active().unwrap().len(), 1);
    }

    #[test]
    fn branch_navigator_switch() {
        let mut nav = UndoBranchNavigator::new();
        let _b1 = nav.fork();
        assert!(nav.switch_to(0));
        assert_eq!(nav.active().unwrap().id, 0);
        assert!(!nav.switch_to(999));
    }

    #[test]
    fn branch_navigator_ancestry() {
        let mut nav = UndoBranchNavigator::new();
        let b1 = nav.fork();
        let _b2 = nav.fork();
        let ancestry = nav.ancestry();
        assert_eq!(ancestry, vec![0, b1, _b2]);
    }

    #[test]
    fn branch_navigator_display() {
        let nav = UndoBranchNavigator::default();
        let s = format!("{nav}");
        assert!(s.contains("branches=1"));
    }

    #[test]
    fn branch_is_empty() {
        let b = UndoBranch::new(0, None);
        assert!(b.is_empty());
        assert_eq!(b.len(), 0);
    }

    #[test]
    fn undoRedoGroupNamer_new() {
        let s = UndoRedoGroupNamer::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn undoRedoGroupNamer_add_contains() {
        let mut s = UndoRedoGroupNamer::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn undoRedoGroupNamer_add_duplicate() {
        let mut s = UndoRedoGroupNamer::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn undoRedoGroupNamer_remove() {
        let mut s = UndoRedoGroupNamer::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn undoRedoGroupNamer_capacity() {
        let s = UndoRedoGroupNamer::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn undoRedoGroupNamer_search() {
        let mut s = UndoRedoGroupNamer::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn undoRedoGroupNamer_stats() {
        let mut s = UndoRedoGroupNamer::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn undoRedoMemoryTracker_new() {
        let m = UndoRedoMemoryTracker::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn undoRedoMemoryTracker_add_find() {
        let mut m = UndoRedoMemoryTracker::new();
        m.add(UndoRedoMemoryTrackerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn undoRedoMemoryTracker_priority_filter() {
        let mut m = UndoRedoMemoryTracker::new();
        m.add(UndoRedoMemoryTrackerItem::new("a", "A").with_priority(UndoRedoMemoryTrackerPriority::High));
        m.add(UndoRedoMemoryTrackerItem::new("b", "B").with_priority(UndoRedoMemoryTrackerPriority::Low));
        m.add(UndoRedoMemoryTrackerItem::new("c", "C").with_priority(UndoRedoMemoryTrackerPriority::High));
        assert_eq!(m.by_priority(UndoRedoMemoryTrackerPriority::High).len(), 2);
    }

    #[test]
    fn undoRedoMemoryTracker_remove() {
        let mut m = UndoRedoMemoryTracker::new();
        m.add(UndoRedoMemoryTrackerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn undoRedoMemoryTracker_search() {
        let mut m = UndoRedoMemoryTracker::new();
        m.add(UndoRedoMemoryTrackerItem::new("id1", "Hello World"));
        m.add(UndoRedoMemoryTrackerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn undoRedoMemoryTracker_total_weight() {
        let mut m = UndoRedoMemoryTracker::new();
        m.add(UndoRedoMemoryTrackerItem::new("a", "A").with_priority(UndoRedoMemoryTrackerPriority::Critical));
        m.add(UndoRedoMemoryTrackerItem::new("b", "B").with_priority(UndoRedoMemoryTrackerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn undoRedoMemoryTracker_capacity_limit() {
        let mut m = UndoRedoMemoryTracker::new().with_max_items(2);
        m.add(UndoRedoMemoryTrackerItem::new("1", "one"));
        m.add(UndoRedoMemoryTrackerItem::new("2", "two"));
        assert!(!m.add(UndoRedoMemoryTrackerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn undoRedoMemoryTracker_sorted_by_priority() {
        let mut m = UndoRedoMemoryTracker::new();
        m.add(UndoRedoMemoryTrackerItem::new("lo", "Low").with_priority(UndoRedoMemoryTrackerPriority::Low));
        m.add(UndoRedoMemoryTrackerItem::new("hi", "High").with_priority(UndoRedoMemoryTrackerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn undoRedoMemoryTracker_item_metadata() {
        let mut item = UndoRedoMemoryTrackerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn undoRedoGroupNamer_enabled_toggle() {
        let mut s = UndoRedoGroupNamer::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn undoRedoMemoryTracker_priority_display() {
        assert_eq!(format!("{}", UndoRedoMemoryTrackerPriority::High), "high");
        assert_eq!(format!("{}", UndoRedoMemoryTrackerPriority::Low), "low");
    }


    #[test]
    fn undoredo_x_config_new() {
        let c = UndoredoXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn undoredo_x_config_builder() {
        let c = UndoredoXConfig::new("k")
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
    fn undoredo_x_config_display() {
        let c = UndoredoXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn undoredo_x_registry_insert_get() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn undoredo_x_registry_duplicate() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a")).unwrap();
        assert!(reg.insert(UndoredoXConfig::new("a")).is_err());
    }

    #[test]
    fn undoredo_x_registry_remove() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a")).unwrap();
        reg.insert(UndoredoXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn undoredo_x_registry_active_entries() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a")).unwrap();
        reg.insert(UndoredoXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn undoredo_x_registry_by_weight() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(UndoredoXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn undoredo_x_registry_tags() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(UndoredoXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn undoredo_x_registry_total_weight() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(UndoredoXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn undoredo_x_registry_iterator() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a")).unwrap();
        reg.insert(UndoredoXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn undoredo_x_cache_put_get() {
        let mut cache = UndoredoXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn undoredo_x_cache_eviction() {
        let mut cache = UndoredoXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn undoredo_x_cache_lru_order() {
        let mut cache = UndoredoXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn undoredo_x_cache_most_least_recent() {
        let mut cache = UndoredoXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn undoredo_x_formatter_entry() {
        let e = UndoredoXConfig::new("k").with_value("v");
        let fmt = UndoredoXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn undoredo_x_formatter_summary() {
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("a").with_weight(5)).unwrap();
        let fmt = UndoredoXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn undoredo_x_validator_valid() {
        let v = UndoredoXValidator::new();
        let c = UndoredoXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn undoredo_x_validator_empty_key() {
        let v = UndoredoXValidator::new();
        let c = UndoredoXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn undoredo_x_validator_require_value() {
        let v = UndoredoXValidator::new().require_value(true);
        let c = UndoredoXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn undoredo_x_validator_allowed_tags() {
        let v = UndoredoXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = UndoredoXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn undoredo_x_validator_validate_all() {
        let v = UndoredoXValidator::new();
        let mut reg = UndoredoXRegistry::new();
        reg.insert(UndoredoXConfig::new("ok")).unwrap();
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


    // ---- xc_ pool / scheduler tests – block 189 ----

    #[test]
    fn xc_189_pool_new_empty() {
        let pool: super::Xc189Pool<i32> = super::Xc189Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_189_pool_release_acquire() {
        let mut pool = super::Xc189Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_189_pool_acquire_empty() {
        let mut pool: super::Xc189Pool<i32> = super::Xc189Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_189_pool_full() {
        let mut pool = super::Xc189Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_189_pool_drain() {
        let mut pool = super::Xc189Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_189_pool_stats() {
        let mut pool = super::Xc189Pool::new(8);
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
    fn xc_189_pool_clear() {
        let mut pool = super::Xc189Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_189_pool_shrink() {
        let mut pool = super::Xc189Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_189_pool_default() {
        let pool: super::Xc189Pool<String> = super::Xc189Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_189_pool_extend() {
        let mut pool = super::Xc189Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_189_pool_retain() {
        let mut pool = super::Xc189Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_189_scheduler_round_robin() {
        let mut sched = super::Xc189Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_189_scheduler_empty() {
        let mut sched = super::Xc189Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_189_scheduler_reset() {
        let mut sched = super::Xc189Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_189_scheduler_add_remove() {
        let mut sched = super::Xc189Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_189_scheduler_targets() {
        let sched = super::Xc189Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_189_hash_empty() {
        assert_eq!(super::xc_189_hash(b""), 5381);
    }

    #[test]
    fn xc_189_hash_data() {
        let h = super::xc_189_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_189_hash(b"hello"), h);
    }

    #[test]
    fn xc_189_reverse_str() {
        assert_eq!(super::xc_189_reverse("abc"), "cba");
        assert_eq!(super::xc_189_reverse(""), "");
    }


    // --- xd_12 deepening tests ---

    #[test]
    fn xd_12_sm_initial_state() {
        let sm = Xd12StateMachine::new();
        assert_eq!(sm.current_state(), Xd12State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_12_sm_valid_idle_to_running() {
        let mut sm = Xd12StateMachine::new();
        assert!(sm.transition(Xd12State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd12State::Running);
    }

    #[test]
    fn xd_12_sm_valid_running_to_paused() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        assert!(sm.transition(Xd12State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd12State::Paused);
    }

    #[test]
    fn xd_12_sm_valid_running_to_done() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        assert!(sm.transition(Xd12State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd12State::Done);
    }

    #[test]
    fn xd_12_sm_valid_paused_to_running() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        sm.transition(Xd12State::Paused).unwrap();
        assert!(sm.transition(Xd12State::Running).is_ok());
    }

    #[test]
    fn xd_12_sm_valid_done_to_idle() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        sm.transition(Xd12State::Done).unwrap();
        assert!(sm.transition(Xd12State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd12State::Idle);
    }

    #[test]
    fn xd_12_sm_invalid_idle_to_done() {
        let mut sm = Xd12StateMachine::new();
        assert!(sm.transition(Xd12State::Done).is_err());
    }

    #[test]
    fn xd_12_sm_invalid_idle_to_paused() {
        let mut sm = Xd12StateMachine::new();
        assert!(sm.transition(Xd12State::Paused).is_err());
    }

    #[test]
    fn xd_12_sm_history_tracking() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        sm.transition(Xd12State::Paused).unwrap();
        sm.transition(Xd12State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd12State::Idle);
        assert_eq!(sm.history()[0].to, Xd12State::Running);
        assert_eq!(sm.history()[1].from, Xd12State::Running);
        assert_eq!(sm.history()[2].to, Xd12State::Done);
    }

    #[test]
    fn xd_12_sm_serialize_deserialize() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd12StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd12State::Running));
    }

    #[test]
    fn xd_12_sm_deserialize_invalid() {
        assert_eq!(Xd12StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_12_sm_reset() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd12State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_12_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd12EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd12Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_12_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd12EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd12Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd12Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_12_bus_unsubscribe() {
        let mut bus = Xd12EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_12_event_kind_and_payload() {
        let e = Xd12Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd12Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_12_bus_clear_history() {
        let mut bus = Xd12EventBus::new();
        bus.publish(Xd12Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_12_sm_step_counter_increments() {
        let mut sm = Xd12StateMachine::new();
        sm.transition(Xd12State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd12State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #10 --

    #[test]
    fn xf10_trie_insert_search() {
        let mut t = Xf10Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf10_trie_starts_with() {
        let mut t = Xf10Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf10_trie_remove() {
        let mut t = Xf10Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf10_trie_word_count() {
        let mut t = Xf10Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf10_trie_longest_prefix() {
        let mut t = Xf10Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf10_trie_all_words() {
        let mut t = Xf10Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf10_trie_autocomplete() {
        let mut t = Xf10Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf10_trie_empty_search() {
        let t = Xf10Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf10_bloom_add_contains() {
        let mut bf = Xf10BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf10_bloom_probably_absent() {
        let bf = Xf10BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf10_bloom_false_positive_rate() {
        let mut bf = Xf10BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf10_bloom_clear() {
        let mut bf = Xf10BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf10_bloom_union() {
        let mut a = Xf10BloomFilter::xf_new(512, 2);
        let mut b = Xf10BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf10_bloom_intersection_estimate() {
        let mut a = Xf10BloomFilter::xf_new(512, 2);
        let mut b = Xf10BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf10_bloom_union_size_mismatch() {
        let a = Xf10BloomFilter::xf_new(256, 2);
        let b = Xf10BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh188_skip_insert_contains() {
        let mut sl = super::Xh188SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh188_skip_remove() {
        let mut sl = super::Xh188SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh188_skip_len() {
        let mut sl = super::Xh188SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh188_skip_range_query() {
        let mut sl = super::Xh188SkipList::xh_new(4);
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
    fn xh188_skip_floor_ceiling() {
        let mut sl = super::Xh188SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh188_skip_rank() {
        let mut sl = super::Xh188SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh188_skip_empty() {
        let sl = super::Xh188SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh188_skip_duplicates() {
        let mut sl = super::Xh188SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh188_bitset_set_test() {
        let mut bs = super::Xh188BitSet::xh_new(256);
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
    fn xh188_bitset_clear_count() {
        let mut bs = super::Xh188BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh188_bitset_and_or_xor() {
        let mut a = super::Xh188BitSet::xh_new(128);
        let mut b = super::Xh188BitSet::xh_new(128);
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
    fn xh188_bitset_iter_ones() {
        let mut bs = super::Xh188BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh188_bitset_first_last() {
        let mut bs = super::Xh188BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh188_bitset_empty() {
        let bs = super::Xh188BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
