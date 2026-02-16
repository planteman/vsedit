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
}
