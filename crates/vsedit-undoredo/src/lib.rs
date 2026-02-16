//! Undo/redo stack service.
//!
//! Provides a generic [`UndoRedoStack<T>`] that tracks past and future states
//! for undo/redo operations.

use std::fmt;

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
}
