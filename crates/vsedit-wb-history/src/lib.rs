//! Navigation history.

use std::fmt;

/// Errors that can occur during history navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryError {
    EmptyHistory,
    AtBeginning,
    AtEnd,
    InvalidIndex(usize),
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyHistory => write!(f, "history is empty"),
            Self::AtBeginning => write!(f, "already at the beginning of history"),
            Self::AtEnd => write!(f, "already at the end of history"),
            Self::InvalidIndex(i) => write!(f, "invalid history index: {i}"),
        }
    }
}

/// A single entry in the navigation history.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryNavigationEntry {
    pub uri: String,
    pub line: u32,
    pub column: u32,
    pub label: Option<String>,
}

impl HistoryNavigationEntry {
    /// Builder method to attach a label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl fmt::Display for HistoryNavigationEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.uri, self.line)
    }
}

/// Maintains a bounded navigation history with back/forward support.
#[derive(Debug, Clone)]
pub struct NavigationHistory {
    pub entries: Vec<HistoryNavigationEntry>,
    pub current: Option<usize>,
    pub max_size: usize,
}

impl NavigationHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            current: None,
            max_size,
        }
    }

    /// Push a new entry, discarding any forward history and enforcing `max_size`.
    pub fn push(&mut self, entry: HistoryNavigationEntry) {
        // Trim forward history beyond current position.
        if let Some(idx) = self.current {
            self.entries.truncate(idx + 1);
        }

        self.entries.push(entry);

        // Evict oldest entries when exceeding max_size.
        while self.entries.len() > self.max_size {
            self.entries.remove(0);
        }

        self.current = Some(self.entries.len() - 1);
    }

    pub fn go_back(&mut self) -> Option<&HistoryNavigationEntry> {
        let idx = self.current?;
        if idx == 0 {
            return None;
        }
        self.current = Some(idx - 1);
        self.entries.get(idx - 1)
    }

    pub fn go_forward(&mut self) -> Option<&HistoryNavigationEntry> {
        let idx = self.current?;
        if idx + 1 >= self.entries.len() {
            return None;
        }
        self.current = Some(idx + 1);
        self.entries.get(idx + 1)
    }

    pub fn can_go_back(&self) -> bool {
        matches!(self.current, Some(idx) if idx > 0)
    }

    pub fn can_go_forward(&self) -> bool {
        matches!(self.current, Some(idx) if idx + 1 < self.entries.len())
    }

    pub fn current(&self) -> Option<&HistoryNavigationEntry> {
        self.entries.get(self.current?)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = None;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Like `go_back` but returns a `Result` with a descriptive error.
    pub fn try_go_back(&mut self) -> Result<&HistoryNavigationEntry, HistoryError> {
        let idx = self.current.ok_or(HistoryError::EmptyHistory)?;
        if idx == 0 {
            return Err(HistoryError::AtBeginning);
        }
        self.current = Some(idx - 1);
        Ok(&self.entries[idx - 1])
    }

    /// Like `go_forward` but returns a `Result` with a descriptive error.
    pub fn try_go_forward(&mut self) -> Result<&HistoryNavigationEntry, HistoryError> {
        let idx = self.current.ok_or(HistoryError::EmptyHistory)?;
        if idx + 1 >= self.entries.len() {
            return Err(HistoryError::AtEnd);
        }
        self.current = Some(idx + 1);
        Ok(&self.entries[idx + 1])
    }

    /// Jump to a specific index in the history.
    pub fn go_to_index(&mut self, index: usize) -> Result<&HistoryNavigationEntry, HistoryError> {
        if self.entries.is_empty() {
            return Err(HistoryError::EmptyHistory);
        }
        if index >= self.entries.len() {
            return Err(HistoryError::InvalidIndex(index));
        }
        self.current = Some(index);
        Ok(&self.entries[index])
    }

    /// Remove all entries whose URI matches the given value.
    pub fn remove_entries_for_uri(&mut self, uri: &str) {
        let current_uri = self.current().map(|e| e.uri.clone());
        self.entries.retain(|e| e.uri != uri);
        if self.entries.is_empty() {
            self.current = None;
        } else if let Some(cur) = current_uri {
            // Try to keep current pointing at the same entry, or clamp.
            if cur == uri {
                self.current = Some(self.entries.len().saturating_sub(1));
            } else {
                // Find the first entry with the previous current URI.
                let pos = self
                    .entries
                    .iter()
                    .position(|e| e.uri == cur)
                    .unwrap_or(self.entries.len() - 1);
                self.current = Some(pos);
            }
        }
    }

    /// Return references to all entries matching the given URI.
    pub fn entries_for_uri(&self, uri: &str) -> Vec<&HistoryNavigationEntry> {
        self.entries.iter().filter(|e| e.uri == uri).collect()
    }

    /// Remove consecutive duplicate entries (same URI and line).
    pub fn deduplicate(&mut self) {
        if self.entries.len() < 2 {
            return;
        }
        let mut i = 1;
        while i < self.entries.len() {
            if self.entries[i].uri == self.entries[i - 1].uri
                && self.entries[i].line == self.entries[i - 1].line
            {
                self.entries.remove(i);
                // Adjust current index if it was at or after the removed entry.
                if let Some(cur) = self.current {
                    if cur >= i {
                        self.current = Some(cur.saturating_sub(1));
                    }
                }
            } else {
                i += 1;
            }
        }
        if self.entries.is_empty() {
            self.current = None;
        }
    }

    /// Number of entries behind the current position.
    pub fn back_stack_size(&self) -> usize {
        self.current.unwrap_or(0)
    }

    /// Number of entries ahead of the current position.
    pub fn forward_stack_size(&self) -> usize {
        match self.current {
            Some(idx) => self.entries.len().saturating_sub(idx + 1),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uri: &str, line: u32) -> HistoryNavigationEntry {
        HistoryNavigationEntry {
            uri: uri.to_string(),
            line,
            column: 0,
            label: None,
        }
    }

    #[test]
    fn push_and_navigate_back_forward() {
        let mut h = NavigationHistory::new(10);
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());

        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));
        assert_eq!(h.len(), 3);
        assert_eq!(h.current().unwrap().uri, "c.rs");

        let back = h.go_back().unwrap();
        assert_eq!(back.uri, "b.rs");
        assert!(h.can_go_forward());

        let fwd = h.go_forward().unwrap();
        assert_eq!(fwd.uri, "c.rs");
        assert!(!h.can_go_forward());
    }

    #[test]
    fn push_truncates_forward_history() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));

        h.go_back();
        h.go_back(); // now at "a.rs"

        h.push(entry("d.rs", 4));
        assert_eq!(h.len(), 2); // "a.rs" and "d.rs"
        assert_eq!(h.current().unwrap().uri, "d.rs");
        assert!(!h.can_go_forward());
    }

    #[test]
    fn max_size_evicts_oldest() {
        let mut h = NavigationHistory::new(3);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));
        h.push(entry("d.rs", 4));

        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0].uri, "b.rs");
        assert_eq!(h.current().unwrap().uri, "d.rs");
    }

    #[test]
    fn clear_resets_history() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.clear();
        assert_eq!(h.len(), 0);
        assert!(h.current().is_none());
    }

    #[test]
    fn display_entry() {
        let e = entry("main.rs", 42);
        assert_eq!(format!("{e}"), "main.rs:42");
    }

    #[test]
    fn with_label_builder() {
        let e = entry("lib.rs", 10).with_label("fn main");
        assert_eq!(e.label.as_deref(), Some("fn main"));
    }

    #[test]
    fn history_error_display() {
        assert_eq!(HistoryError::EmptyHistory.to_string(), "history is empty");
        assert_eq!(
            HistoryError::AtBeginning.to_string(),
            "already at the beginning of history"
        );
        assert_eq!(
            HistoryError::AtEnd.to_string(),
            "already at the end of history"
        );
        assert_eq!(
            HistoryError::InvalidIndex(5).to_string(),
            "invalid history index: 5"
        );
    }

    #[test]
    fn try_go_back_and_forward_errors() {
        let mut h = NavigationHistory::new(10);
        assert_eq!(h.try_go_back(), Err(HistoryError::EmptyHistory));
        assert_eq!(h.try_go_forward(), Err(HistoryError::EmptyHistory));

        h.push(entry("a.rs", 1));
        assert_eq!(h.try_go_back(), Err(HistoryError::AtBeginning));
        assert_eq!(h.try_go_forward(), Err(HistoryError::AtEnd));
    }

    #[test]
    fn try_go_back_and_forward_success() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));

        let back = h.try_go_back().unwrap();
        assert_eq!(back.uri, "a.rs");

        let fwd = h.try_go_forward().unwrap();
        assert_eq!(fwd.uri, "b.rs");
    }

    #[test]
    fn go_to_index_valid() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));

        let e = h.go_to_index(0).unwrap();
        assert_eq!(e.uri, "a.rs");
        assert_eq!(h.current().unwrap().uri, "a.rs");
    }

    #[test]
    fn go_to_index_errors() {
        let mut h = NavigationHistory::new(10);
        assert_eq!(h.go_to_index(0), Err(HistoryError::EmptyHistory));

        h.push(entry("a.rs", 1));
        assert_eq!(h.go_to_index(5), Err(HistoryError::InvalidIndex(5)));
    }

    #[test]
    fn is_empty() {
        let mut h = NavigationHistory::new(10);
        assert!(h.is_empty());
        h.push(entry("a.rs", 1));
        assert!(!h.is_empty());
    }

    #[test]
    fn remove_entries_for_uri() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("a.rs", 3));
        h.push(entry("c.rs", 4));

        h.remove_entries_for_uri("a.rs");
        assert_eq!(h.len(), 2);
        assert!(h.entries.iter().all(|e| e.uri != "a.rs"));
    }

    #[test]
    fn entries_for_uri() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("a.rs", 10));

        let matches = h.entries_for_uri("a.rs");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[1].line, 10);
    }

    #[test]
    fn deduplicate_removes_consecutive_dupes() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("b.rs", 2));
        h.push(entry("a.rs", 1));

        h.deduplicate();
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0].uri, "a.rs");
        assert_eq!(h.entries[1].uri, "b.rs");
        assert_eq!(h.entries[2].uri, "a.rs");
    }

    #[test]
    fn back_and_forward_stack_sizes() {
        let mut h = NavigationHistory::new(10);
        assert_eq!(h.back_stack_size(), 0);
        assert_eq!(h.forward_stack_size(), 0);

        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));

        assert_eq!(h.back_stack_size(), 2);
        assert_eq!(h.forward_stack_size(), 0);

        h.go_back();
        assert_eq!(h.back_stack_size(), 1);
        assert_eq!(h.forward_stack_size(), 1);
    }
}
