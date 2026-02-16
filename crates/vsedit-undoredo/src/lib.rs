//! Undo/redo stack service.
//!
//! Provides a generic [`UndoRedoStack<T>`] that tracks past and future states
//! for undo/redo operations, plus [`UndoRedoService`] with cursor-aware
//! grouped undo/redo matching VS Code's `UndoRedoService`.

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
}
