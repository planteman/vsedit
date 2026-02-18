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


/// A double-ended queue backed by a ring buffer (variant 188).
pub struct Xi188Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi188Deque<T> {
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
pub struct Xi188Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi188Interval {
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

/// A simple interval tree (variant 188).
pub struct Xi188IntervalTree {
    xi_intervals: Vec<Xi188Interval>,
}

impl Xi188IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi188Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi188Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi188Interval) -> Vec<&Xi188Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi188Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi188Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi188Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi188Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi188Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi188Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 188) ---

/// Disjoint set / union-find for crate 188.
pub struct Xj188UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj188UnionFind {
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

const XJ188_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 188.
pub struct Xj188BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj188BTreeNode<K, V>>>,
    len: usize,
}

struct Xj188BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj188BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj188BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ188_BTREE_ORDER - 1
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
        let mid = XJ188_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj188BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj188BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj188BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj188BTreeNode::xj_new_leaf();
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


// --- xk_188 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk188SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk188SegmentTree {
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
pub struct Xk188DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk188DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_188).
#[derive(Debug, Clone)]
pub struct Xl188Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl188Rope {
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

/// Suffix array for efficient string searching (xl_188).
#[derive(Debug, Clone)]
pub struct Xl188SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl188SuffixArray {
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
pub struct Xm188MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm188MatrixSparse {
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
pub struct Xm188Tokenizer {
    text: String,
}

impl Xm188Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 188.
pub struct Xn188Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn188Fenwick {
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

// ----- AVL tree map — crate 188 -----

#[derive(Debug, Clone)]
struct Xn188AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn188AvlNode<K, V>>>,
    right: Option<Box<Xn188AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 188.
#[derive(Debug, Clone)]
pub struct Xn188AVL<K, V> {
    root: Option<Box<Xn188AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn188AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn188AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn188AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn188AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn188AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn188AvlNode<K, V>>) -> Box<Xn188AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn188AvlNode<K, V>>) -> Box<Xn188AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn188AvlNode<K, V>>) -> Box<Xn188AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn188AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn188AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn188AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn188AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn188AvlNode<K, V>>) -> &Xn188AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn188AvlNode<K, V>>) -> (Box<Xn188AvlNode<K, V>>, Option<Box<Xn188AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn188AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn188AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn188AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn188AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn188AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn188AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn188AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo188RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo188Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo188RBNode<K, V> {
    key: K,
    value: V,
    color: Xo188Color,
    left: Option<Box<Xo188RBNode<K, V>>>,
    right: Option<Box<Xo188RBNode<K, V>>>,
}

/// A red-black tree map for crate 188.
#[derive(Debug, Clone)]
pub struct Xo188RedBlack<K, V> {
    root: Option<Box<Xo188RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo188RedBlack<K, V> {
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
            r.color = Xo188Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo188RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo188RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo188RBNode {
                    key, value, color: Xo188Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo188RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo188Color::Red)
    }

    fn xo_balance(mut h: Box<Xo188RBNode<K, V>>) -> Box<Xo188RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo188Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo188RBNode<K, V>>) -> Box<Xo188RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo188Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo188RBNode<K, V>>) -> Box<Xo188RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo188Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo188RBNode<K, V>>) {
        h.color = Xo188Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo188Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo188Color::Black; }
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
            r.color = Xo188Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo188RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo188RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo188RBNode<K, V>) -> (K, V, Option<Box<Xo188RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo188RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo188Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo188RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo188ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 188.
#[derive(Debug, Clone)]
pub struct Xo188ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo188ConsistentHash {
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
            let vkey = format!("{}#xo188#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo188#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 188).
#[derive(Debug)]
pub struct Xp188SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp188Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp188Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp188Node<K, V>>>,
    xp_right: Option<Box<Xp188Node<K, V>>>,
}

impl<K: Ord, V> Xp188Node<K, V> {
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

impl<K: Ord, V> Default for Xp188SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp188SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp188Node<K, V>>>, key: &K) -> Option<Box<Xp188Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp188Node<K, V>>) -> Box<Xp188Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp188Node<K, V>>) -> Box<Xp188Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp188Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp188Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp188Node::xp_new(key, val));
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


// --------------- Xq188Treap ---------------

use std::cmp::Ordering as Xq188Ord;

struct Xq188TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq188TreapNode<K, V>>>,
    right: Option<Box<Xq188TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq188Treap<K, V> {
    root: Option<Box<Xq188TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq188TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_188_size<K, V>(node: &Option<Box<Xq188TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_188_update_size<K, V>(node: &mut Xq188TreapNode<K, V>) {
    node.size = 1 + xq_188_size(&node.left) + xq_188_size(&node.right);
}

fn xq_188_rotate_right<K, V>(mut node: Box<Xq188TreapNode<K, V>>) -> Box<Xq188TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_188_update_size(&mut node);
    left.right = Some(node);
    xq_188_update_size(&mut left);
    left
}

fn xq_188_rotate_left<K, V>(mut node: Box<Xq188TreapNode<K, V>>) -> Box<Xq188TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_188_update_size(&mut node);
    right.left = Some(node);
    xq_188_update_size(&mut right);
    right
}

fn xq_188_insert_node<K: Ord, V>(
    node: Option<Box<Xq188TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq188TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq188TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq188Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq188Ord::Less => {
                let (new_left, old) = xq_188_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_188_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_188_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq188Ord::Greater => {
                let (new_right, old) = xq_188_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_188_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_188_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_188_remove_node<K: Ord, V>(
    node: Option<Box<Xq188TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq188TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq188Ord::Less => {
                let (new_left, old) = xq_188_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_188_update_size(&mut n);
                (Some(n), old)
            }
            Xq188Ord::Greater => {
                let (new_right, old) = xq_188_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_188_update_size(&mut n);
                (Some(n), old)
            }
            Xq188Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_188_rotate_right(n);
                    let (new_right, old) = xq_188_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_188_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_188_rotate_left(n);
                    let (new_left, old) = xq_188_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_188_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_188_find_min<K, V>(node: &Option<Box<Xq188TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_188_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_188_find_max<K, V>(node: &Option<Box<Xq188TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_188_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_188_rank<K: Ord, V>(node: &Option<Box<Xq188TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq188Ord::Less => xq_188_rank(&n.left, key),
            Xq188Ord::Equal => xq_188_size(&n.left),
            Xq188Ord::Greater => 1 + xq_188_size(&n.left) + xq_188_rank(&n.right, key),
        },
    }
}

fn xq_188_kth<K, V>(node: &Option<Box<Xq188TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_188_size(&n.left);
        if k < left_size {
            xq_188_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_188_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_188_in_order<K: Clone, V>(node: &Option<Box<Xq188TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_188_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_188_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq188Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 188 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_188_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq188Ord::Equal => return Some(&n.value),
                Xq188Ord::Less => cur = &n.left,
                Xq188Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_188_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_188_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_188_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_188_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_188_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_188_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_188_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq188VEBTree ---------------

pub struct Xq188VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq188VEBTree>>,
    clusters: Vec<Option<Box<Xq188VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq188VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq188VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq188VEBTree::xq_new(self.sqrt_lo)));
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


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr188KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr188KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr188BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr188KDNode {
    xr_point: Xr188KDPoint,
    xr_left: Option<Box<Xr188KDNode>>,
    xr_right: Option<Box<Xr188KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr188KDTree {
    xr_root: Option<Box<Xr188KDNode>>,
    xr_size: usize,
}

impl Xr188KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr188KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr188KDNode>>,
        point: Xr188KDPoint,
        depth: usize,
    ) -> Box<Xr188KDNode> {
        match node {
            None => Box::new(Xr188KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr188KDPoint) -> Option<Xr188KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr188KDNode>,
        query: &Xr188KDPoint,
        depth: usize,
        best: &mut Xr188KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr188KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr188KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr188KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr188KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr188KDNode>>, pts: &mut Vec<Xr188KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr188KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr188BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr188BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
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


    #[test]
    fn xi188_deque_push_pop_back() {
        let mut dq = super::Xi188Deque::xi_new(4);
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
    fn xi188_deque_push_pop_front() {
        let mut dq = super::Xi188Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi188_deque_mixed_ops() {
        let mut dq = super::Xi188Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi188_deque_get_and_split() {
        let mut dq = super::Xi188Deque::xi_new(8);
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
    fn xi188_deque_rotate_left() {
        let mut dq = super::Xi188Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi188_deque_rotate_right() {
        let mut dq = super::Xi188Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi188_deque_grow() {
        let mut dq = super::Xi188Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi188_deque_empty() {
        let dq = super::Xi188Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi188_interval_tree_insert_query() {
        let mut tree = super::Xi188IntervalTree::xi_new();
        tree.xi_insert(super::Xi188Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi188Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi188Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi188_interval_tree_overlap() {
        let mut tree = super::Xi188IntervalTree::xi_new();
        tree.xi_insert(super::Xi188Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi188Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi188Interval::xi_new(12, 20));
        let q = super::Xi188Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi188_interval_tree_remove() {
        let mut tree = super::Xi188IntervalTree::xi_new();
        tree.xi_insert(super::Xi188Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi188Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi188_interval_tree_gaps() {
        let mut tree = super::Xi188IntervalTree::xi_new();
        tree.xi_insert(super::Xi188Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi188Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi188Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi188Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi188Interval::xi_new(8, 10));
    }

    #[test]
    fn xi188_interval_tree_merge() {
        let mut tree = super::Xi188IntervalTree::xi_new();
        tree.xi_insert(super::Xi188Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi188Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi188Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi188Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi188Interval::xi_new(10, 15));
    }

    #[test]
    fn xi188_interval_tree_all() {
        let mut tree = super::Xi188IntervalTree::xi_new();
        tree.xi_insert(super::Xi188Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi188Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi188_interval_tree_empty() {
        let tree = super::Xi188IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi188_interval_tree_contains_point() {
        let iv = super::Xi188Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 188) ---

    #[test]
    fn xj_188_uf_make_and_find() {
        let mut uf = super::Xj188UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_188_uf_union_connected() {
        let mut uf = super::Xj188UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_188_uf_component_count() {
        let mut uf = super::Xj188UnionFind::xj_new();
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
    fn xj_188_uf_component_size() {
        let mut uf = super::Xj188UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_188_uf_largest_component() {
        let mut uf = super::Xj188UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_188_uf_many_elements() {
        let mut uf = super::Xj188UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_188_uf_separate_components() {
        let mut uf = super::Xj188UnionFind::xj_new();
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
    fn xj_188_uf_path_compression() {
        let mut uf = super::Xj188UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_188_bt_insert_get() {
        let mut bt = super::Xj188BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_188_bt_contains_len() {
        let mut bt = super::Xj188BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_188_bt_replace() {
        let mut bt = super::Xj188BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_188_bt_remove() {
        let mut bt = super::Xj188BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_188_bt_keys_values() {
        let mut bt = super::Xj188BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_188_bt_range() {
        let mut bt = super::Xj188BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_188_bt_min_max() {
        let mut bt = super::Xj188BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_188_bt_many_inserts() {
        let mut bt = super::Xj188BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_188 segment tree tests ---

    #[test]
    fn xk_188_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk188SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_188_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk188SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_188_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk188SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_188_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk188SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_188_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk188SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_188_st_single_element() {
        let data = vec![42];
        let st = super::Xk188SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_188_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk188SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_188_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk188SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_188 disjoint intervals tests ---

    #[test]
    fn xk_188_di_add_and_count() {
        let mut di = super::Xk188DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_188_di_merge_overlap() {
        let mut di = super::Xk188DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_188_di_contains() {
        let mut di = super::Xk188DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_188_di_remove() {
        let mut di = super::Xk188DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_188_di_covered_length() {
        let mut di = super::Xk188DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_188_di_gaps() {
        let mut di = super::Xk188DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_188_di_merge_adjacent() {
        let mut di = super::Xk188DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_188_di_empty() {
        let di = super::Xk188DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_188_rope_new_empty() {
        let rope = super::Xl188Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_188_rope_from_str() {
        let rope = super::Xl188Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_188_rope_insert_at() {
        let mut rope = super::Xl188Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_188_rope_delete_range() {
        let mut rope = super::Xl188Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_188_rope_char_at() {
        let rope = super::Xl188Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_188_rope_split_concat() {
        let rope = super::Xl188Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_188_rope_line_count() {
        let rope = super::Xl188Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_188_rope_line_at() {
        let rope = super::Xl188Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_188_sa_build_and_search() {
        let sa = super::Xl188SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_188_sa_count() {
        let sa = super::Xl188SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_188_sa_longest_repeated() {
        let sa = super::Xl188SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_188_sa_all_positions() {
        let sa = super::Xl188SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_188_sa_len() {
        let sa = super::Xl188SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_188_sa_empty() {
        let sa = super::Xl188SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_188_rope_slice() {
        let rope = super::Xl188Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_188_sa_search_start() {
        let sa = super::Xl188SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_188_sparse_set_get() {
        let mut m = super::Xm188MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_188_sparse_row_col() {
        let mut m = super::Xm188MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_188_sparse_transpose() {
        let mut m = super::Xm188MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_188_sparse_multiply_vec() {
        let mut m = super::Xm188MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_188_sparse_nnz_density() {
        let mut m = super::Xm188MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_188_sparse_clear() {
        let mut m = super::Xm188MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_188_sparse_overwrite_zero() {
        let mut m = super::Xm188MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_188_tokenizer_basic() {
        let t = super::Xm188Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_188_tokenizer_count() {
        let t = super::Xm188Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_188_tokenizer_unique() {
        let t = super::Xm188Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_188_tokenizer_frequency() {
        let t = super::Xm188Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_188_tokenizer_delimiter() {
        let t = super::Xm188Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_188_tokenizer_whitespace() {
        let t = super::Xm188Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_188_tokenizer_empty() {
        let t = super::Xm188Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 188 ----

    #[test]
    fn xn_188_fenwick_prefix_sum() {
        let mut ft = super::Xn188Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_188_fenwick_range_sum() {
        let mut ft = super::Xn188Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_188_fenwick_point_query() {
        let mut ft = super::Xn188Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_188_fenwick_len() {
        let ft = super::Xn188Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_188_fenwick_multiple_updates() {
        let mut ft = super::Xn188Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_188_fenwick_single_element() {
        let mut ft = super::Xn188Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_188_fenwick_find_kth() {
        let mut ft = super::Xn188Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_188_fenwick_negative_delta() {
        let mut ft = super::Xn188Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 188 ----

    #[test]
    fn xn_188_avl_insert_get() {
        let mut m = super::Xn188AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_188_avl_remove() {
        let mut m = super::Xn188AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_188_avl_in_order() {
        let mut m = super::Xn188AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_188_avl_min_max() {
        let mut m = super::Xn188AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_188_avl_floor_ceiling() {
        let mut m = super::Xn188AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_188_avl_height_balanced() {
        let mut m = super::Xn188AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_188_avl_overwrite() {
        let mut m = super::Xn188AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_188_avl_empty() {
        let m: super::Xn188AVL<i32, i32> = super::Xn188AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo188RedBlack tests ---

    #[test]
    fn xo_188_rb_insert_and_get() {
        let mut tree = super::Xo188RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_188_rb_len_and_empty() {
        let mut tree = super::Xo188RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_188_rb_min_max() {
        let mut tree = super::Xo188RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_188_rb_contains() {
        let mut tree = super::Xo188RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_188_rb_remove() {
        let mut tree = super::Xo188RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_188_rb_in_order() {
        let mut tree = super::Xo188RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_188_rb_black_height() {
        let mut tree = super::Xo188RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_188_rb_overwrite() {
        let mut tree = super::Xo188RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo188ConsistentHash tests ---

    #[test]
    fn xo_188_ch_add_and_count() {
        let mut ring = super::Xo188ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_188_ch_remove_node() {
        let mut ring = super::Xo188ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_188_ch_get_node() {
        let mut ring = super::Xo188ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_188_ch_empty_ring() {
        let ring = super::Xo188ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_188_ch_distribution() {
        let mut ring = super::Xo188ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_188_ch_rebalance() {
        let mut ring = super::Xo188ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_188_ch_virtual_nodes() {
        let mut ring = super::Xo188ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_188_ch_consistent_lookup() {
        let mut ring = super::Xo188ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_188_splay_insert_get() {
        let mut t = super::Xp188SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_188_splay_remove() {
        let mut t = super::Xp188SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_188_splay_count_increases() {
        let mut t = super::Xp188SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_188_splay_depth() {
        let mut t = super::Xp188SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_188_splay_len_empty() {
        let t = super::Xp188SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_188_splay_min_max() {
        let mut t = super::Xp188SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_188_splay_overwrite() {
        let mut t = super::Xp188SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_188_splay_remove_missing() {
        let mut t = super::Xp188SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_188 treap tests ----
    #[test]
    fn xq_188_treap_empty() {
        let t = super::Xq188Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_188_treap_insert_get() {
        let mut t = super::Xq188Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_188_treap_overwrite() {
        let mut t = super::Xq188Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_188_treap_remove() {
        let mut t = super::Xq188Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_188_treap_min_max() {
        let mut t = super::Xq188Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_188_treap_rank() {
        let mut t = super::Xq188Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_188_treap_kth() {
        let mut t = super::Xq188Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_188_treap_in_order() {
        let mut t = super::Xq188Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_188 VEB tree tests ----
    #[test]
    fn xq_188_veb_empty() {
        let v = super::Xq188VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_188_veb_insert_contains() {
        let mut v = super::Xq188VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_188_veb_min_max() {
        let mut v = super::Xq188VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_188_veb_delete() {
        let mut v = super::Xq188VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_188_veb_successor() {
        let mut v = super::Xq188VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_188_veb_predecessor() {
        let mut v = super::Xq188VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_188_veb_count() {
        let mut v = super::Xq188VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_188_veb_duplicate_insert() {
        let mut v = super::Xq188VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_188_kdtree_empty() {
        let tree = super::Xr188KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_188_kdtree_insert_one() {
        let mut tree = super::Xr188KDTree::xr_new();
        tree.xr_insert(super::Xr188KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_188_kdtree_insert_multiple() {
        let mut tree = super::Xr188KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr188KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_188_kdtree_nearest_neighbor() {
        let mut tree = super::Xr188KDTree::xr_new();
        tree.xr_insert(super::Xr188KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr188KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr188KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_188_kdtree_nn_empty() {
        let tree = super::Xr188KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr188KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_188_kdtree_range_search() {
        let mut tree = super::Xr188KDTree::xr_new();
        tree.xr_insert(super::Xr188KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr188KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr188KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_188_kdtree_range_empty() {
        let mut tree = super::Xr188KDTree::xr_new();
        tree.xr_insert(super::Xr188KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_188_kdtree_all_points() {
        let mut tree = super::Xr188KDTree::xr_new();
        tree.xr_insert(super::Xr188KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr188KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_188_kdtree_depth() {
        let mut tree = super::Xr188KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr188KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_188_kdtree_bounding_box() {
        let mut tree = super::Xr188KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr188KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr188KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
