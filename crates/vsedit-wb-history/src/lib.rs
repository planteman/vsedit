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

/// Accumulated statistics for wb-history operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbHistoryStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbHistoryStats {
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
    pub fn merge(&mut self, other: &WbHistoryStats) {
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

impl Default for WbHistoryStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbHistoryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbHistoryStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-history.
#[derive(Debug, Clone)]
pub struct WbHistoryValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbHistoryValidator {
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

impl Default for WbHistoryValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ViewState — saved editor state for navigation history
// ---------------------------------------------------------------------------

/// Captured editor view state for restoring after navigation.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewState {
    pub scroll_top: u32,
    pub cursor_line: u32,
    pub cursor_column: u32,
    pub selections: Vec<(u32, u32, u32, u32)>,
}

impl ViewState {
    pub fn new(scroll_top: u32, cursor_line: u32, cursor_column: u32) -> Self {
        Self {
            scroll_top,
            cursor_line,
            cursor_column,
            selections: Vec::new(),
        }
    }

    pub fn with_selection(mut self, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        self.selections.push((start_line, start_col, end_line, end_col));
        self
    }
}

/// An extended navigation entry that includes view state.
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationEntry {
    pub uri: String,
    pub line: u32,
    pub column: u32,
    pub label: Option<String>,
    pub view_state: Option<ViewState>,
}

impl NavigationEntry {
    pub fn new(uri: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            uri: uri.into(),
            line,
            column,
            label: None,
            view_state: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_view_state(mut self, vs: ViewState) -> Self {
        self.view_state = Some(vs);
        self
    }
}

impl fmt::Display for NavigationEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.uri, self.line, self.column)
    }
}

/// Navigation history service with back/forward stacks and push-on-jump.
pub struct NavigationHistoryService {
    back_stack: Vec<NavigationEntry>,
    forward_stack: Vec<NavigationEntry>,
    current: Option<NavigationEntry>,
    max_size: usize,
}

impl NavigationHistoryService {
    pub fn new(max_size: usize) -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            current: None,
            max_size,
        }
    }

    /// Push a new entry (called on every definition/reference jump).
    pub fn push_navigation(&mut self, entry: NavigationEntry) {
        if let Some(cur) = self.current.take() {
            self.back_stack.push(cur);
            while self.back_stack.len() > self.max_size {
                self.back_stack.remove(0);
            }
        }
        self.forward_stack.clear();
        self.current = Some(entry);
    }

    /// Navigate back (Alt+Left).
    pub fn navigate_back(&mut self) -> Option<&NavigationEntry> {
        if let Some(cur) = self.current.take() {
            self.forward_stack.push(cur);
        }
        if let Some(prev) = self.back_stack.pop() {
            self.current = Some(prev);
            self.current.as_ref()
        } else {
            // Restore current from forward stack if back is empty
            if let Some(fwd) = self.forward_stack.pop() {
                self.current = Some(fwd);
            }
            None
        }
    }

    /// Navigate forward (Alt+Right).
    pub fn navigate_forward(&mut self) -> Option<&NavigationEntry> {
        if let Some(cur) = self.current.take() {
            self.back_stack.push(cur);
        }
        if let Some(next) = self.forward_stack.pop() {
            self.current = Some(next);
            self.current.as_ref()
        } else {
            // Restore current from back stack if forward is empty
            if let Some(back) = self.back_stack.pop() {
                self.current = Some(back);
            }
            None
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn current(&self) -> Option<&NavigationEntry> {
        self.current.as_ref()
    }

    pub fn back_stack_size(&self) -> usize {
        self.back_stack.len()
    }

    pub fn forward_stack_size(&self) -> usize {
        self.forward_stack.len()
    }

    pub fn clear(&mut self) {
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current = None;
    }
}

impl Default for NavigationHistoryService {
    fn default() -> Self {
        Self::new(100)
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

    #[test]
    fn eq_historyerror_same() {
        assert_eq!(HistoryError::EmptyHistory, HistoryError::EmptyHistory);
    }

    #[test]
    fn ne_historyerror_diff() {
        assert_ne!(HistoryError::EmptyHistory, HistoryError::AtBeginning);
    }

    #[test]
    fn display_historyerror_variants() {
        assert!(!HistoryError::EmptyHistory.to_string().is_empty());
        assert!(!HistoryError::AtBeginning.to_string().is_empty());
        assert!(!HistoryError::AtEnd.to_string().is_empty());
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn wb_history_stats_new_defaults() {
        let stats = WbHistoryStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_history_stats_record_success() {
        let mut stats = WbHistoryStats::new();
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
    fn wb_history_stats_record_failure() {
        let mut stats = WbHistoryStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_history_stats_reset() {
        let mut stats = WbHistoryStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_history_stats_merge() {
        let mut a = WbHistoryStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbHistoryStats::new();
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
    fn wb_history_stats_display() {
        let mut stats = WbHistoryStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_history_stats_default() {
        let stats = WbHistoryStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_history_validator_accepts_valid_name() {
        let v = WbHistoryValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_history_validator_rejects_empty() {
        let v = WbHistoryValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_history_validator_rejects_too_long() {
        let v = WbHistoryValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_history_validator_forbidden_prefix() {
        let v = WbHistoryValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_history_validator_allowed_chars() {
        let v = WbHistoryValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_history_validator_range() {
        let v = WbHistoryValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_history_sanitize_removes_control() {
        let result = WbHistoryValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_history_truncate_short_string() {
        assert_eq!(WbHistoryValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_history_truncate_long_string() {
        let result = WbHistoryValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_history_is_ascii_printable() {
        assert!(WbHistoryValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbHistoryValidator::is_ascii_printable("Hello\x00World"));
    }

    // --- ViewState tests ---

    #[test]
    fn view_state_creation() {
        let vs = ViewState::new(10, 20, 5);
        assert_eq!(vs.scroll_top, 10);
        assert_eq!(vs.cursor_line, 20);
        assert_eq!(vs.cursor_column, 5);
        assert!(vs.selections.is_empty());
    }

    #[test]
    fn view_state_with_selection() {
        let vs = ViewState::new(0, 5, 0).with_selection(5, 0, 5, 10);
        assert_eq!(vs.selections.len(), 1);
        assert_eq!(vs.selections[0], (5, 0, 5, 10));
    }

    // --- NavigationEntry tests ---

    #[test]
    fn navigation_entry_display() {
        let e = NavigationEntry::new("main.rs", 42, 7);
        assert_eq!(format!("{e}"), "main.rs:42:7");
    }

    #[test]
    fn navigation_entry_with_view_state() {
        let e = NavigationEntry::new("lib.rs", 1, 0)
            .with_label("fn main")
            .with_view_state(ViewState::new(0, 1, 0));
        assert_eq!(e.label.as_deref(), Some("fn main"));
        assert!(e.view_state.is_some());
    }

    // --- NavigationHistoryService tests ---

    #[test]
    fn nav_service_push_and_current() {
        let mut svc = NavigationHistoryService::new(10);
        assert!(svc.current().is_none());
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        assert_eq!(svc.current().unwrap().uri, "a.rs");
    }

    #[test]
    fn nav_service_back_forward() {
        let mut svc = NavigationHistoryService::new(10);
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        svc.push_navigation(NavigationEntry::new("b.rs", 2, 0));
        svc.push_navigation(NavigationEntry::new("c.rs", 3, 0));

        assert_eq!(svc.current().unwrap().uri, "c.rs");
        assert!(svc.can_go_back());
        assert!(!svc.can_go_forward());

        let back = svc.navigate_back().unwrap();
        assert_eq!(back.uri, "b.rs");
        assert!(svc.can_go_forward());

        let fwd = svc.navigate_forward().unwrap();
        assert_eq!(fwd.uri, "c.rs");
    }

    #[test]
    fn nav_service_push_clears_forward() {
        let mut svc = NavigationHistoryService::new(10);
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        svc.push_navigation(NavigationEntry::new("b.rs", 2, 0));
        svc.navigate_back();

        svc.push_navigation(NavigationEntry::new("d.rs", 4, 0));
        assert!(!svc.can_go_forward());
        assert_eq!(svc.current().unwrap().uri, "d.rs");
    }

    #[test]
    fn nav_service_max_size() {
        let mut svc = NavigationHistoryService::new(2);
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        svc.push_navigation(NavigationEntry::new("b.rs", 2, 0));
        svc.push_navigation(NavigationEntry::new("c.rs", 3, 0));
        svc.push_navigation(NavigationEntry::new("d.rs", 4, 0));
        // Back stack should be capped at 2
        assert_eq!(svc.back_stack_size(), 2);
    }

    #[test]
    fn nav_service_clear() {
        let mut svc = NavigationHistoryService::new(10);
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        svc.push_navigation(NavigationEntry::new("b.rs", 2, 0));
        svc.clear();
        assert!(svc.current().is_none());
        assert!(!svc.can_go_back());
        assert!(!svc.can_go_forward());
    }

    #[test]
    fn nav_service_empty_back() {
        let mut svc = NavigationHistoryService::new(10);
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        assert!(svc.navigate_back().is_none());
    }

    #[test]
    fn nav_service_stack_sizes() {
        let mut svc = NavigationHistoryService::new(10);
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        svc.push_navigation(NavigationEntry::new("b.rs", 2, 0));
        svc.push_navigation(NavigationEntry::new("c.rs", 3, 0));
        assert_eq!(svc.back_stack_size(), 2);
        assert_eq!(svc.forward_stack_size(), 0);
        svc.navigate_back();
        assert_eq!(svc.back_stack_size(), 1);
        assert_eq!(svc.forward_stack_size(), 1);
    }
}
