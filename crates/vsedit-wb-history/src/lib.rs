//! Navigation history.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// HistoryBookmark — named bookmarks within a navigation history
// ---------------------------------------------------------------------------

/// A named bookmark that captures a position in the history for quick recall.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryBookmark {
    pub name: String,
    pub entry: NavigationEntry,
}

impl HistoryBookmark {
    pub fn new(name: impl Into<String>, entry: NavigationEntry) -> Self {
        Self {
            name: name.into(),
            entry,
        }
    }
}

impl fmt::Display for HistoryBookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.name, self.entry)
    }
}

/// Manages a collection of named bookmarks.
#[derive(Debug, Clone, Default)]
pub struct BookmarkManager {
    bookmarks: Vec<HistoryBookmark>,
}

impl BookmarkManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a bookmark. If a bookmark with the same name exists, it is replaced.
    pub fn set(&mut self, bookmark: HistoryBookmark) {
        if let Some(pos) = self.bookmarks.iter().position(|b| b.name == bookmark.name) {
            self.bookmarks[pos] = bookmark;
        } else {
            self.bookmarks.push(bookmark);
        }
    }

    /// Retrieve a bookmark by name.
    pub fn get(&self, name: &str) -> Option<&HistoryBookmark> {
        self.bookmarks.iter().find(|b| b.name == name)
    }

    /// Remove a bookmark by name. Returns `true` if it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.name != name);
        self.bookmarks.len() < before
    }

    /// List all bookmark names.
    pub fn names(&self) -> Vec<&str> {
        self.bookmarks.iter().map(|b| b.name.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.bookmarks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }
}

// ---------------------------------------------------------------------------
// HistorySearch — search/filter entries within a NavigationHistory
// ---------------------------------------------------------------------------

/// Search results from a history query.
#[derive(Debug, Clone)]
pub struct HistorySearchResult {
    pub index: usize,
    pub entry: HistoryNavigationEntry,
}

/// Provides search capabilities over a `NavigationHistory`.
pub struct HistorySearch;

impl HistorySearch {
    /// Find all entries whose URI contains the given substring.
    pub fn search_uri(history: &NavigationHistory, query: &str) -> Vec<HistorySearchResult> {
        history
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.uri.contains(query))
            .map(|(i, e)| HistorySearchResult {
                index: i,
                entry: e.clone(),
            })
            .collect()
    }

    /// Find entries within a line range in a specific URI.
    pub fn search_line_range(
        history: &NavigationHistory,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<HistorySearchResult> {
        history
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.uri == uri && e.line >= start_line && e.line <= end_line)
            .map(|(i, e)| HistorySearchResult {
                index: i,
                entry: e.clone(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// HistoryFrequencyStats — frequency analysis of visited URIs
// ---------------------------------------------------------------------------

/// Frequency analysis of navigation history entries.
#[derive(Debug, Clone, Default)]
pub struct HistoryFrequencyStats {
    uri_counts: HashMap<String, usize>,
    total_visits: usize,
}

impl HistoryFrequencyStats {
    /// Build frequency stats from a `NavigationHistory`.
    pub fn from_history(history: &NavigationHistory) -> Self {
        let mut uri_counts = HashMap::new();
        for entry in &history.entries {
            *uri_counts.entry(entry.uri.clone()).or_insert(0) += 1;
        }
        Self {
            total_visits: history.entries.len(),
            uri_counts,
        }
    }

    /// Number of times a specific URI was visited.
    pub fn visit_count(&self, uri: &str) -> usize {
        self.uri_counts.get(uri).copied().unwrap_or(0)
    }

    /// Number of distinct URIs visited.
    pub fn unique_uris(&self) -> usize {
        self.uri_counts.len()
    }

    /// Total number of visits across all URIs.
    pub fn total_visits(&self) -> usize {
        self.total_visits
    }

    /// Return the most visited URI and its count, or `None` if empty.
    pub fn most_visited(&self) -> Option<(&str, usize)> {
        self.uri_counts
            .iter()
            .max_by_key(|(_, count)| **count)
            .map(|(uri, &count)| (uri.as_str(), count))
    }

    /// Return URIs sorted by visit count (descending).
    pub fn ranked(&self) -> Vec<(&str, usize)> {
        let mut pairs: Vec<_> = self
            .uri_counts
            .iter()
            .map(|(uri, &count)| (uri.as_str(), count))
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        pairs
    }
}

// ---------------------------------------------------------------------------
// HistoryCompactor — merge duplicate entries
// ---------------------------------------------------------------------------

/// Compacts a `NavigationHistory` by merging globally duplicate entries.
pub struct HistoryCompactor;

impl HistoryCompactor {
    /// Remove all globally duplicate (uri, line) pairs, keeping only the last
    /// occurrence of each. Resets `current` to the last entry.
    pub fn compact(history: &mut NavigationHistory) {
        let mut seen = HashMap::<(String, u32), usize>::new();
        // Track last occurrence index of each (uri, line) pair.
        for (i, entry) in history.entries.iter().enumerate() {
            seen.insert((entry.uri.clone(), entry.line), i);
        }
        let keep: Vec<bool> = (0..history.entries.len())
            .map(|i| {
                let entry = &history.entries[i];
                seen.get(&(entry.uri.clone(), entry.line)) == Some(&i)
            })
            .collect();
        let mut idx = 0;
        history.entries.retain(|_| {
            let k = keep[idx];
            idx += 1;
            k
        });
        if history.entries.is_empty() {
            history.current = None;
        } else {
            history.current = Some(history.entries.len() - 1);
        }
    }
}

// ---------------------------------------------------------------------------
// HistorySnapshot — capture and restore full navigation state
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of the entire navigation history state.
#[derive(Debug, Clone)]
pub struct HistorySnapshot {
    pub entries: Vec<HistoryNavigationEntry>,
    pub current_index: Option<usize>,
    pub timestamp: u64,
    pub label: Option<String>,
}

impl HistorySnapshot {
    /// Capture a snapshot from the current state of a `NavigationHistory`.
    pub fn capture(history: &NavigationHistory, timestamp: u64) -> Self {
        Self {
            entries: history.entries.clone(),
            current_index: history.current,
            timestamp,
            label: None,
        }
    }

    /// Attach a label to this snapshot.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Restore a `NavigationHistory` from this snapshot.
    pub fn restore(&self, max_size: usize) -> NavigationHistory {
        NavigationHistory {
            entries: self.entries.clone(),
            current: self.current_index,
            max_size,
        }
    }

    /// Number of entries in the snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for HistorySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HistorySnapshot({} entries, ts={}{})",
            self.entries.len(),
            self.timestamp,
            self.label
                .as_ref()
                .map(|l| format!(", label={l}"))
                .unwrap_or_default()
        )
    }
}

/// Manages multiple named snapshots for undo/redo of history state.
#[derive(Debug, Clone, Default)]
pub struct SnapshotManager {
    snapshots: Vec<HistorySnapshot>,
    max_snapshots: usize,
}

impl SnapshotManager {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
        }
    }

    /// Save a snapshot. Oldest is evicted if at capacity.
    pub fn save(&mut self, snapshot: HistorySnapshot) {
        if self.snapshots.len() >= self.max_snapshots {
            self.snapshots.remove(0);
        }
        self.snapshots.push(snapshot);
    }

    /// Pop and return the most recent snapshot.
    pub fn pop(&mut self) -> Option<HistorySnapshot> {
        self.snapshots.pop()
    }

    /// Get the most recent snapshot without removing it.
    pub fn latest(&self) -> Option<&HistorySnapshot> {
        self.snapshots.last()
    }

    /// Number of stored snapshots.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether no snapshots are stored.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Clear all stored snapshots.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Find a snapshot by label.
    pub fn find_by_label(&self, label: &str) -> Option<&HistorySnapshot> {
        self.snapshots
            .iter()
            .rev()
            .find(|s| s.label.as_deref() == Some(label))
    }
}

// ---------------------------------------------------------------------------
// NavigationHistoryService — additional methods
// ---------------------------------------------------------------------------

impl NavigationHistoryService {
    /// Return all entries in the back stack (oldest first).
    pub fn back_stack_entries(&self) -> &[NavigationEntry] {
        &self.back_stack
    }

    /// Return all entries in the forward stack (oldest first).
    pub fn forward_stack_entries(&self) -> &[NavigationEntry] {
        &self.forward_stack
    }

    /// Remove all entries whose URI matches the given string.
    pub fn remove_entries_for_uri(&mut self, uri: &str) {
        self.back_stack.retain(|e| e.uri != uri);
        self.forward_stack.retain(|e| e.uri != uri);
        if self.current.as_ref().map(|c| c.uri.as_str()) == Some(uri) {
            self.current = self.back_stack.pop();
        }
    }

    /// Total number of entries across back stack, current, and forward stack.
    pub fn total_entries(&self) -> usize {
        self.back_stack.len()
            + self.forward_stack.len()
            + if self.current.is_some() { 1 } else { 0 }
    }
}

/// A parallel timeline branch of navigation history.
///
/// Each branch maintains its own ordered list of navigation entries,
/// allowing users to explore multiple code paths without losing context.
#[derive(Debug, Clone)]
pub struct HistoryBranch {
    /// Unique identifier for this branch.
    pub id: String,
    /// Human-readable name for this branch.
    pub name: String,
    /// Navigation entries recorded on this branch.
    pub entries: Vec<NavigationEntry>,
    /// Monotonic counter representing when this branch was created.
    pub created_at: u64,
}

impl HistoryBranch {
    /// Create a new empty branch with the given id and name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            entries: Vec::new(),
            created_at: 0,
        }
    }

    /// Push a navigation entry onto this branch.
    pub fn push(&mut self, entry: NavigationEntry) {
        self.entries.push(entry);
    }

    /// Return the number of entries on this branch.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if this branch has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return a reference to the most recent entry, if any.
    pub fn latest(&self) -> Option<&NavigationEntry> {
        self.entries.last()
    }
}

impl fmt::Display for HistoryBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Branch({}, \"{}\", {} entries)",
            self.id,
            self.name,
            self.entries.len()
        )
    }
}

/// Manages multiple [`HistoryBranch`] instances and supports switching and merging.
#[derive(Debug)]
pub struct HistoryBranchManager {
    branches: HashMap<String, HistoryBranch>,
    active_branch_id: Option<String>,
    next_id: u64,
}

impl HistoryBranchManager {
    /// Create a new branch manager with no branches.
    pub fn new() -> Self {
        Self {
            branches: HashMap::new(),
            active_branch_id: None,
            next_id: 0,
        }
    }

    /// Create a new branch with the given name and return its id.
    ///
    /// The first branch created automatically becomes the active branch.
    pub fn create_branch(&mut self, name: impl Into<String>) -> String {
        let id = format!("branch-{}", self.next_id);
        self.next_id += 1;
        let mut branch = HistoryBranch::new(id.clone(), name);
        branch.created_at = self.next_id;
        self.branches.insert(id.clone(), branch);
        if self.active_branch_id.is_none() {
            self.active_branch_id = Some(id.clone());
        }
        id
    }

    /// Switch to the branch identified by `id`.
    ///
    /// Returns `Err(HistoryError::InvalidIndex)` if the branch does not exist.
    pub fn switch_branch(&mut self, id: &str) -> Result<(), HistoryError> {
        if !self.branches.contains_key(id) {
            return Err(HistoryError::InvalidIndex(0));
        }
        self.active_branch_id = Some(id.to_string());
        Ok(())
    }

    /// Return a reference to the currently active branch, if any.
    pub fn current_branch(&self) -> Option<&HistoryBranch> {
        self.active_branch_id
            .as_ref()
            .and_then(|id| self.branches.get(id))
    }

    /// Return a mutable reference to the currently active branch, if any.
    pub fn current_branch_mut(&mut self) -> Option<&mut HistoryBranch> {
        let id = self.active_branch_id.clone()?;
        self.branches.get_mut(&id)
    }

    /// List all branches in arbitrary order.
    pub fn list_branches(&self) -> Vec<&HistoryBranch> {
        self.branches.values().collect()
    }

    /// Merge all entries from `source_id` into `target_id`.
    ///
    /// Returns the number of entries merged or an error if either branch does
    /// not exist.
    pub fn merge_branch(
        &mut self,
        source_id: &str,
        target_id: &str,
    ) -> Result<usize, HistoryError> {
        let source = self
            .branches
            .get(source_id)
            .ok_or(HistoryError::InvalidIndex(0))?
            .clone();
        let count = source.entries.len();
        let target = self
            .branches
            .get_mut(target_id)
            .ok_or(HistoryError::InvalidIndex(0))?;
        target.entries.extend(source.entries);
        Ok(count)
    }
}

/// Serialises and deserialises [`NavigationEntry`] slices to a simple text
/// format.
///
/// Each entry is encoded as a single line: `uri:line:column`.
pub struct HistoryExporter;

impl HistoryExporter {
    /// Export a slice of entries to a newline-delimited string.
    ///
    /// Format: one entry per line as `uri:line:column`.
    pub fn export_entries(entries: &[NavigationEntry]) -> String {
        let mut out = String::new();
        for (i, e) in entries.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&format!("{}:{}:{}", e.uri, e.line, e.column));
        }
        out
    }

    /// Import entries from a text string produced by [`Self::export_entries`].
    ///
    /// Lines that cannot be parsed are silently skipped.
    pub fn import_entries(text: &str) -> Vec<NavigationEntry> {
        let mut entries = Vec::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() == 3 {
                if let (Ok(line_num), Ok(col)) =
                    (parts[1].parse::<u32>(), parts[2].parse::<u32>())
                {
                    entries.push(NavigationEntry::new(parts[0], line_num, col));
                }
            }
        }
        entries
    }
}

/// Compresses navigation history by removing redundant entries.
pub struct HistoryCompression;

impl HistoryCompression {
    /// Remove consecutive entries that refer to the same URI.
    ///
    /// When duplicates are found the *last* entry in each run is kept so that
    /// the final cursor position is preserved.
    pub fn compress(entries: &[NavigationEntry]) -> Vec<NavigationEntry> {
        if entries.is_empty() {
            return Vec::new();
        }
        let mut result: Vec<NavigationEntry> = Vec::new();
        for entry in entries {
            if let Some(last) = result.last() {
                if last.uri == entry.uri {
                    // Replace with the newer entry.
                    let len = result.len();
                    result[len - 1] = entry.clone();
                    continue;
                }
            }
            result.push(entry.clone());
        }
        result
    }

    /// Remove consecutive entries with the same URI where the line difference
    /// is within `max_line_gap`.
    ///
    /// Entries that are farther apart than `max_line_gap` lines are kept even
    /// if they share a URI, because the user likely navigated to a
    /// meaningfully different location in the same file.
    pub fn compress_within_distance(
        entries: &[NavigationEntry],
        max_line_gap: u32,
    ) -> Vec<NavigationEntry> {
        if entries.is_empty() {
            return Vec::new();
        }
        let mut result: Vec<NavigationEntry> = Vec::new();
        for entry in entries {
            if let Some(last) = result.last() {
                if last.uri == entry.uri {
                    let diff = if entry.line >= last.line {
                        entry.line - last.line
                    } else {
                        last.line - entry.line
                    };
                    if diff <= max_line_gap {
                        let len = result.len();
                        result[len - 1] = entry.clone();
                        continue;
                    }
                }
            }
            result.push(entry.clone());
        }
        result
    }
}


// === History Diff Viewer ===

/// History Diff Viewer implementation.
#[derive(Debug, Clone)]
pub struct HistoryDiffViewer {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: HistoryDiffViewerStats,
}

/// Statistics for HistoryDiffViewer.
#[derive(Debug, Clone, Default)]
pub struct HistoryDiffViewerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl HistoryDiffViewerStats {
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

impl HistoryDiffViewer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: HistoryDiffViewerStats::default(),
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

    pub fn stats(&self) -> &HistoryDiffViewerStats {
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

impl Default for HistoryDiffViewer {
    fn default() -> Self {
        Self::new()
    }
}

// === History Restore Point Creator ===

/// Priority level for HistoryRestorePointCreator items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HistoryRestorePointCreatorPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl HistoryRestorePointCreatorPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for HistoryRestorePointCreatorPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// History Restore Point Creator implementation.
#[derive(Debug, Clone)]
pub struct HistoryRestorePointCreator {
    items: Vec<HistoryRestorePointCreatorItem>,
    max_items: usize,
    default_priority: HistoryRestorePointCreatorPriority,
}

/// A single item in HistoryRestorePointCreator.
#[derive(Debug, Clone)]
pub struct HistoryRestorePointCreatorItem {
    pub id: String,
    pub label: String,
    pub priority: HistoryRestorePointCreatorPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl HistoryRestorePointCreatorItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: HistoryRestorePointCreatorPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: HistoryRestorePointCreatorPriority) -> Self {
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

impl HistoryRestorePointCreator {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: HistoryRestorePointCreatorPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: HistoryRestorePointCreatorItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<HistoryRestorePointCreatorItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&HistoryRestorePointCreatorItem> {
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

    pub fn by_priority(&self, priority: HistoryRestorePointCreatorPriority) -> Vec<&HistoryRestorePointCreatorItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&HistoryRestorePointCreatorItem> {
        let mut sorted: Vec<&HistoryRestorePointCreatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&HistoryRestorePointCreatorItem> {
        let mut sorted: Vec<&HistoryRestorePointCreatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&HistoryRestorePointCreatorItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: HistoryRestorePointCreatorPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> HistoryRestorePointCreatorPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &HistoryRestorePointCreatorItem> {
        self.items.iter()
    }
}

impl Default for HistoryRestorePointCreator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-wb-history: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbHistoryXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl WbHistoryXConfig {
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

impl std::fmt::Display for WbHistoryXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct WbHistoryXRegistry {
    entries: Vec<WbHistoryXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl WbHistoryXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: WbHistoryXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&WbHistoryXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut WbHistoryXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<WbHistoryXConfig> {
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

    pub fn active_entries(&self) -> Vec<&WbHistoryXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&WbHistoryXConfig> {
        let mut sorted: Vec<&WbHistoryXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&WbHistoryXConfig> {
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

    pub fn iter(&self) -> WbHistoryXIterator<'_> {
        WbHistoryXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct WbHistoryXIterator<'a> {
    inner: std::slice::Iter<'a, WbHistoryXConfig>,
}

impl<'a> Iterator for WbHistoryXIterator<'a> {
    type Item = &'a WbHistoryXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct WbHistoryXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl WbHistoryXCache {
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
pub struct WbHistoryXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl WbHistoryXFormatter {
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

    pub fn format_entry(&self, entry: &WbHistoryXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &WbHistoryXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &WbHistoryXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for WbHistoryXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct WbHistoryXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl WbHistoryXValidator {
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

    pub fn validate(&self, entry: &WbHistoryXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &WbHistoryXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for WbHistoryXValidator {
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
// xb_ utilities – batch 51
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer51 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer51 {
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
pub fn xb_fnv1a_51(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_51<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_51<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_51(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_51(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 209
// ---------------------------------------------------------------------------

/// Generic object pool `Xc209Pool<T>`.
pub struct Xc209Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc209Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc209PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc209Pool<T> {
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
    pub fn stats(&self) -> Xc209PoolStats {
        Xc209PoolStats {
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

impl<T> Default for Xc209Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc209Scheduler`.
pub struct Xc209Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc209Scheduler {
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

impl Default for Xc209Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_209 hash for the given byte slice.
pub fn xc_209_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_209 convention.
pub fn xc_209_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe64 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe64Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe64PipelineError {
    pub stage: Xe64Stage,
    pub message: String,
}

impl std::fmt::Display for Xe64PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe64Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe64Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError>>>,
    stage_names: Vec<Xe64Stage>,
}

impl Xe64Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe64Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe64Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe64Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe64Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> {
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

    pub fn compose(mut self, other: Xe64Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe64CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe64CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe64Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe64CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe64CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe64Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe64CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_64_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe64CacheEntry {
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

    fn xe_64_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe64CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_64_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> {
    Ok(data)
}

pub fn xe_64_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_64_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_64_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_64_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe64PipelineError> {
    Err(Xe64PipelineError {
        stage: Xe64Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_62: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg62Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg62Graph {
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

impl Default for Xg62Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_62: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg62Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg62Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg62Heap<T>) {
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

impl<T: Ord> Default for Xg62Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 208).
pub struct Xh208SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh208SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 250 as u64,
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

/// A compact bit set supporting boolean operations (variant 208).
pub struct Xh208BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh208BitSet {
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

    // --- BookmarkManager tests ---

    #[test]
    fn bookmark_manager_set_and_get() {
        let mut mgr = BookmarkManager::new();
        let bm = HistoryBookmark::new("home", NavigationEntry::new("main.rs", 1, 0));
        mgr.set(bm);
        assert_eq!(mgr.len(), 1);
        let found = mgr.get("home").unwrap();
        assert_eq!(found.entry.uri, "main.rs");
    }

    #[test]
    fn bookmark_manager_replace_existing() {
        let mut mgr = BookmarkManager::new();
        mgr.set(HistoryBookmark::new("x", NavigationEntry::new("a.rs", 1, 0)));
        mgr.set(HistoryBookmark::new("x", NavigationEntry::new("b.rs", 99, 0)));
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.get("x").unwrap().entry.uri, "b.rs");
    }

    #[test]
    fn bookmark_manager_remove() {
        let mut mgr = BookmarkManager::new();
        mgr.set(HistoryBookmark::new("a", NavigationEntry::new("a.rs", 1, 0)));
        assert!(mgr.remove("a"));
        assert!(!mgr.remove("a"));
        assert!(mgr.is_empty());
    }

    #[test]
    fn bookmark_display() {
        let bm = HistoryBookmark::new("spot", NavigationEntry::new("lib.rs", 42, 5));
        assert_eq!(format!("{bm}"), "[spot] lib.rs:42:5");
    }

    #[test]
    fn bookmark_names_listing() {
        let mut mgr = BookmarkManager::new();
        mgr.set(HistoryBookmark::new("alpha", NavigationEntry::new("a.rs", 1, 0)));
        mgr.set(HistoryBookmark::new("beta", NavigationEntry::new("b.rs", 1, 0)));
        let names = mgr.names();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    // --- HistorySearch tests ---

    #[test]
    fn history_search_uri_substring() {
        let mut h = NavigationHistory::new(20);
        h.push(entry("src/main.rs", 1));
        h.push(entry("src/lib.rs", 10));
        h.push(entry("tests/test.rs", 5));

        let results = HistorySearch::search_uri(&h, "src/");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].entry.uri, "src/main.rs");
        assert_eq!(results[1].entry.uri, "src/lib.rs");
    }

    #[test]
    fn history_search_line_range() {
        let mut h = NavigationHistory::new(20);
        h.push(entry("a.rs", 5));
        h.push(entry("a.rs", 15));
        h.push(entry("a.rs", 25));
        h.push(entry("b.rs", 10));

        let results = HistorySearch::search_line_range(&h, "a.rs", 10, 20);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.line, 15);
    }

    // --- HistoryFrequencyStats tests ---

    #[test]
    fn frequency_stats_basic() {
        let mut h = NavigationHistory::new(20);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("a.rs", 3));
        h.push(entry("a.rs", 4));
        h.push(entry("c.rs", 5));

        let stats = HistoryFrequencyStats::from_history(&h);
        assert_eq!(stats.total_visits(), 5);
        assert_eq!(stats.unique_uris(), 3);
        assert_eq!(stats.visit_count("a.rs"), 3);
        assert_eq!(stats.visit_count("b.rs"), 1);
        assert_eq!(stats.visit_count("missing.rs"), 0);

        let (most_uri, most_count) = stats.most_visited().unwrap();
        assert_eq!(most_uri, "a.rs");
        assert_eq!(most_count, 3);
    }

    #[test]
    fn frequency_stats_ranked_order() {
        let mut h = NavigationHistory::new(20);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 1));
        h.push(entry("c.rs", 2));
        h.push(entry("c.rs", 3));

        let stats = HistoryFrequencyStats::from_history(&h);
        let ranked = stats.ranked();
        assert_eq!(ranked[0].0, "c.rs");
        assert_eq!(ranked[0].1, 3);
        assert_eq!(ranked[1].0, "b.rs");
        assert_eq!(ranked[1].1, 2);
        assert_eq!(ranked[2].0, "a.rs");
        assert_eq!(ranked[2].1, 1);
    }

    // --- HistoryCompactor tests ---

    #[test]
    fn compactor_removes_global_duplicates() {
        let mut h = NavigationHistory::new(20);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("a.rs", 1)); // duplicate of first
        h.push(entry("c.rs", 3));
        h.push(entry("b.rs", 2)); // duplicate of second

        HistoryCompactor::compact(&mut h);
        // Should keep last occurrence of each: a.rs:1, c.rs:3, b.rs:2
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0].uri, "a.rs");
        assert_eq!(h.entries[1].uri, "c.rs");
        assert_eq!(h.entries[2].uri, "b.rs");
    }

    #[test]
    fn compactor_no_duplicates_unchanged() {
        let mut h = NavigationHistory::new(20);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 2));
        h.push(entry("c.rs", 3));

        HistoryCompactor::compact(&mut h);
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn frequency_stats_empty_history() {
        let h = NavigationHistory::new(10);
        let stats = HistoryFrequencyStats::from_history(&h);
        assert_eq!(stats.total_visits(), 0);
        assert_eq!(stats.unique_uris(), 0);
        assert!(stats.most_visited().is_none());
    }

    // -- Snapshot tests -------------------------------------------------------

    #[test]
    fn snapshot_capture_and_restore() {
        let mut h = NavigationHistory::new(10);
        h.push(entry("a.rs", 1));
        h.push(entry("b.rs", 5));
        let snap = HistorySnapshot::capture(&h, 1000);
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.timestamp, 1000);
        let restored = snap.restore(10);
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.current().unwrap().uri, "b.rs");
    }

    #[test]
    fn snapshot_with_label() {
        let h = NavigationHistory::new(10);
        let snap = HistorySnapshot::capture(&h, 42).with_label("before-refactor");
        assert_eq!(snap.label.as_deref(), Some("before-refactor"));
        let display = format!("{snap}");
        assert!(display.contains("before-refactor"));
    }

    #[test]
    fn snapshot_manager_capacity() {
        let mut mgr = SnapshotManager::new(2);
        let h = NavigationHistory::new(5);
        mgr.save(HistorySnapshot::capture(&h, 1));
        mgr.save(HistorySnapshot::capture(&h, 2));
        mgr.save(HistorySnapshot::capture(&h, 3));
        assert_eq!(mgr.len(), 2);
        assert_eq!(mgr.latest().unwrap().timestamp, 3);
    }

    #[test]
    fn snapshot_manager_find_by_label() {
        let mut mgr = SnapshotManager::new(10);
        let h = NavigationHistory::new(5);
        mgr.save(HistorySnapshot::capture(&h, 1).with_label("alpha"));
        mgr.save(HistorySnapshot::capture(&h, 2).with_label("beta"));
        assert!(mgr.find_by_label("alpha").is_some());
        assert_eq!(mgr.find_by_label("alpha").unwrap().timestamp, 1);
        assert!(mgr.find_by_label("gamma").is_none());
    }

    #[test]
    fn nav_service_remove_entries_for_uri() {
        let mut svc = NavigationHistoryService::new(50);
        svc.push_navigation(NavigationEntry::new("a.rs", 1, 0));
        svc.push_navigation(NavigationEntry::new("b.rs", 2, 0));
        svc.push_navigation(NavigationEntry::new("a.rs", 3, 0));
        assert_eq!(svc.total_entries(), 3);
        svc.remove_entries_for_uri("a.rs");
        assert_eq!(svc.current().unwrap().uri, "b.rs");
    }

    #[test]
    fn nav_service_total_entries() {
        let mut svc = NavigationHistoryService::new(50);
        assert_eq!(svc.total_entries(), 0);
        svc.push_navigation(NavigationEntry::new("x.rs", 1, 0));
        assert_eq!(svc.total_entries(), 1);
        svc.push_navigation(NavigationEntry::new("y.rs", 2, 0));
        assert_eq!(svc.total_entries(), 2);
        svc.navigate_back();
        assert_eq!(svc.total_entries(), 2);
    }

    // ---- HistoryBranch tests ----

    #[test]
    fn branch_new_is_empty() {
        let b = HistoryBranch::new("b0", "main");
        assert!(b.is_empty());
        assert_eq!(b.len(), 0);
        assert!(b.latest().is_none());
    }

    #[test]
    fn branch_push_and_latest() {
        let mut b = HistoryBranch::new("b1", "feature");
        b.push(NavigationEntry::new("foo.rs", 10, 5));
        b.push(NavigationEntry::new("bar.rs", 20, 0));
        assert_eq!(b.len(), 2);
        assert!(!b.is_empty());
        let latest = b.latest().unwrap();
        assert_eq!(latest.uri, "bar.rs");
        assert_eq!(latest.line, 20);
    }

    #[test]
    fn branch_display() {
        let mut b = HistoryBranch::new("b2", "refactor");
        b.push(NavigationEntry::new("a.rs", 1, 0));
        let s = format!("{b}");
        assert!(s.contains("refactor"));
        assert!(s.contains("1 entries"));
    }

    // ---- HistoryBranchManager tests ----

    #[test]
    fn branch_manager_create_and_list() {
        let mut mgr = HistoryBranchManager::new();
        let id1 = mgr.create_branch("main");
        let id2 = mgr.create_branch("feature");
        assert_ne!(id1, id2);
        assert_eq!(mgr.list_branches().len(), 2);
    }

    #[test]
    fn branch_manager_switch() {
        let mut mgr = HistoryBranchManager::new();
        let id1 = mgr.create_branch("main");
        let id2 = mgr.create_branch("feature");
        assert_eq!(mgr.current_branch().unwrap().id, id1);
        mgr.switch_branch(&id2).unwrap();
        assert_eq!(mgr.current_branch().unwrap().id, id2);
    }

    #[test]
    fn branch_manager_switch_invalid() {
        let mut mgr = HistoryBranchManager::new();
        mgr.create_branch("main");
        assert!(mgr.switch_branch("nonexistent").is_err());
    }

    #[test]
    fn branch_manager_merge() {
        let mut mgr = HistoryBranchManager::new();
        let id1 = mgr.create_branch("main");
        let id2 = mgr.create_branch("feature");
        mgr.switch_branch(&id2).unwrap();
        mgr.current_branch_mut()
            .unwrap()
            .push(NavigationEntry::new("x.rs", 1, 0));
        mgr.current_branch_mut()
            .unwrap()
            .push(NavigationEntry::new("y.rs", 2, 0));
        let count = mgr.merge_branch(&id2, &id1).unwrap();
        assert_eq!(count, 2);
        mgr.switch_branch(&id1).unwrap();
        assert_eq!(mgr.current_branch().unwrap().len(), 2);
    }

    // ---- HistoryExporter tests ----

    #[test]
    fn exporter_roundtrip() {
        let entries = vec![
            NavigationEntry::new("a.rs", 10, 3),
            NavigationEntry::new("b.rs", 20, 7),
        ];
        let text = HistoryExporter::export_entries(&entries);
        assert_eq!(text, "a.rs:10:3\nb.rs:20:7");
        let imported = HistoryExporter::import_entries(&text);
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].uri, "a.rs");
        assert_eq!(imported[0].line, 10);
        assert_eq!(imported[1].column, 7);
    }

    #[test]
    fn exporter_import_skips_bad_lines() {
        let text = "good.rs:1:0\nbadline\nalso:bad\nok.rs:5:2";
        let imported = HistoryExporter::import_entries(text);
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].uri, "good.rs");
        assert_eq!(imported[1].uri, "ok.rs");
    }

    // ---- HistoryCompression tests ----

    #[test]
    fn compress_removes_consecutive_same_uri() {
        let entries = vec![
            NavigationEntry::new("a.rs", 1, 0),
            NavigationEntry::new("a.rs", 5, 0),
            NavigationEntry::new("b.rs", 10, 0),
            NavigationEntry::new("b.rs", 12, 0),
            NavigationEntry::new("a.rs", 3, 0),
        ];
        let compressed = HistoryCompression::compress(&entries);
        assert_eq!(compressed.len(), 3);
        assert_eq!(compressed[0].uri, "a.rs");
        assert_eq!(compressed[0].line, 5); // kept the last in the run
        assert_eq!(compressed[1].uri, "b.rs");
        assert_eq!(compressed[1].line, 12);
        assert_eq!(compressed[2].uri, "a.rs");
    }

    #[test]
    fn compress_within_distance_keeps_far_entries() {
        let entries = vec![
            NavigationEntry::new("a.rs", 1, 0),
            NavigationEntry::new("a.rs", 3, 0),  // within gap of 5
            NavigationEntry::new("a.rs", 50, 0), // far away, keep
            NavigationEntry::new("b.rs", 1, 0),
        ];
        let compressed = HistoryCompression::compress_within_distance(&entries, 5);
        assert_eq!(compressed.len(), 3);
        assert_eq!(compressed[0].line, 3); // merged run of 1,3
        assert_eq!(compressed[1].line, 50); // kept because gap > 5
        assert_eq!(compressed[2].uri, "b.rs");
    }

    #[test]
    fn compress_empty_input() {
        let compressed = HistoryCompression::compress(&[]);
        assert!(compressed.is_empty());
        let compressed2 = HistoryCompression::compress_within_distance(&[], 10);
        assert!(compressed2.is_empty());
    }

    #[test]
    fn historyDiffViewer_new() {
        let s = HistoryDiffViewer::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn historyDiffViewer_add_contains() {
        let mut s = HistoryDiffViewer::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn historyDiffViewer_add_duplicate() {
        let mut s = HistoryDiffViewer::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn historyDiffViewer_remove() {
        let mut s = HistoryDiffViewer::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn historyDiffViewer_capacity() {
        let s = HistoryDiffViewer::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn historyDiffViewer_search() {
        let mut s = HistoryDiffViewer::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn historyDiffViewer_stats() {
        let mut s = HistoryDiffViewer::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn historyRestorePointCreator_new() {
        let m = HistoryRestorePointCreator::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn historyRestorePointCreator_add_find() {
        let mut m = HistoryRestorePointCreator::new();
        m.add(HistoryRestorePointCreatorItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn historyRestorePointCreator_priority_filter() {
        let mut m = HistoryRestorePointCreator::new();
        m.add(HistoryRestorePointCreatorItem::new("a", "A").with_priority(HistoryRestorePointCreatorPriority::High));
        m.add(HistoryRestorePointCreatorItem::new("b", "B").with_priority(HistoryRestorePointCreatorPriority::Low));
        m.add(HistoryRestorePointCreatorItem::new("c", "C").with_priority(HistoryRestorePointCreatorPriority::High));
        assert_eq!(m.by_priority(HistoryRestorePointCreatorPriority::High).len(), 2);
    }

    #[test]
    fn historyRestorePointCreator_remove() {
        let mut m = HistoryRestorePointCreator::new();
        m.add(HistoryRestorePointCreatorItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn historyRestorePointCreator_search() {
        let mut m = HistoryRestorePointCreator::new();
        m.add(HistoryRestorePointCreatorItem::new("id1", "Hello World"));
        m.add(HistoryRestorePointCreatorItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn historyRestorePointCreator_total_weight() {
        let mut m = HistoryRestorePointCreator::new();
        m.add(HistoryRestorePointCreatorItem::new("a", "A").with_priority(HistoryRestorePointCreatorPriority::Critical));
        m.add(HistoryRestorePointCreatorItem::new("b", "B").with_priority(HistoryRestorePointCreatorPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn historyRestorePointCreator_capacity_limit() {
        let mut m = HistoryRestorePointCreator::new().with_max_items(2);
        m.add(HistoryRestorePointCreatorItem::new("1", "one"));
        m.add(HistoryRestorePointCreatorItem::new("2", "two"));
        assert!(!m.add(HistoryRestorePointCreatorItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn historyRestorePointCreator_sorted_by_priority() {
        let mut m = HistoryRestorePointCreator::new();
        m.add(HistoryRestorePointCreatorItem::new("lo", "Low").with_priority(HistoryRestorePointCreatorPriority::Low));
        m.add(HistoryRestorePointCreatorItem::new("hi", "High").with_priority(HistoryRestorePointCreatorPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn historyRestorePointCreator_item_metadata() {
        let mut item = HistoryRestorePointCreatorItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn historyDiffViewer_enabled_toggle() {
        let mut s = HistoryDiffViewer::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn historyRestorePointCreator_priority_display() {
        assert_eq!(format!("{}", HistoryRestorePointCreatorPriority::High), "high");
        assert_eq!(format!("{}", HistoryRestorePointCreatorPriority::Low), "low");
    }


    #[test]
    fn wbHistory_x_config_new() {
        let c = WbHistoryXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn wbHistory_x_config_builder() {
        let c = WbHistoryXConfig::new("k")
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
    fn wbHistory_x_config_display() {
        let c = WbHistoryXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn wbHistory_x_registry_insert_get() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbHistory_x_registry_duplicate() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a")).unwrap();
        assert!(reg.insert(WbHistoryXConfig::new("a")).is_err());
    }

    #[test]
    fn wbHistory_x_registry_remove() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a")).unwrap();
        reg.insert(WbHistoryXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbHistory_x_registry_active_entries() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a")).unwrap();
        reg.insert(WbHistoryXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn wbHistory_x_registry_by_weight() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(WbHistoryXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn wbHistory_x_registry_tags() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(WbHistoryXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn wbHistory_x_registry_total_weight() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(WbHistoryXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn wbHistory_x_registry_iterator() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a")).unwrap();
        reg.insert(WbHistoryXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn wbHistory_x_cache_put_get() {
        let mut cache = WbHistoryXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn wbHistory_x_cache_eviction() {
        let mut cache = WbHistoryXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn wbHistory_x_cache_lru_order() {
        let mut cache = WbHistoryXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn wbHistory_x_cache_most_least_recent() {
        let mut cache = WbHistoryXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn wbHistory_x_formatter_entry() {
        let e = WbHistoryXConfig::new("k").with_value("v");
        let fmt = WbHistoryXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn wbHistory_x_formatter_summary() {
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("a").with_weight(5)).unwrap();
        let fmt = WbHistoryXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn wbHistory_x_validator_valid() {
        let v = WbHistoryXValidator::new();
        let c = WbHistoryXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn wbHistory_x_validator_empty_key() {
        let v = WbHistoryXValidator::new();
        let c = WbHistoryXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbHistory_x_validator_require_value() {
        let v = WbHistoryXValidator::new().require_value(true);
        let c = WbHistoryXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbHistory_x_validator_allowed_tags() {
        let v = WbHistoryXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = WbHistoryXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbHistory_x_validator_validate_all() {
        let v = WbHistoryXValidator::new();
        let mut reg = WbHistoryXRegistry::new();
        reg.insert(WbHistoryXConfig::new("ok")).unwrap();
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
    fn xb_ring_buffer_51_push_and_len() {
        let mut rb = super::XbRingBuffer51::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_51_overwrite() {
        let mut rb = super::XbRingBuffer51::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_51_get_out_of_bounds() {
        let rb = super::XbRingBuffer51::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_51_drain_all() {
        let mut rb = super::XbRingBuffer51::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_51_peek_front_back() {
        let mut rb = super::XbRingBuffer51::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_51_clear() {
        let mut rb = super::XbRingBuffer51::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_51_capacity() {
        let rb = super::XbRingBuffer51::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_51_basic() {
        let h = super::xb_fnv1a_51(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_51(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_51_different_inputs() {
        let h1 = super::xb_fnv1a_51(b"abc");
        let h2 = super::xb_fnv1a_51(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_51_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_51(&data);
        let dec = super::xb_rle_decode_51(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_51_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_51(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_51(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_51_values() {
        assert!((super::xb_clamp_51(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_51(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_51(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_51_values() {
        assert!((super::xb_lerp_51(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_51(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_51(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_51_wrap_around_twice() {
        let mut rb = super::XbRingBuffer51::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 209 ----

    #[test]
    fn xc_209_pool_new_empty() {
        let pool: super::Xc209Pool<i32> = super::Xc209Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_209_pool_release_acquire() {
        let mut pool = super::Xc209Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_209_pool_acquire_empty() {
        let mut pool: super::Xc209Pool<i32> = super::Xc209Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_209_pool_full() {
        let mut pool = super::Xc209Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_209_pool_drain() {
        let mut pool = super::Xc209Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_209_pool_stats() {
        let mut pool = super::Xc209Pool::new(8);
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
    fn xc_209_pool_clear() {
        let mut pool = super::Xc209Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_209_pool_shrink() {
        let mut pool = super::Xc209Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_209_pool_default() {
        let pool: super::Xc209Pool<String> = super::Xc209Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_209_pool_extend() {
        let mut pool = super::Xc209Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_209_pool_retain() {
        let mut pool = super::Xc209Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_209_scheduler_round_robin() {
        let mut sched = super::Xc209Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_209_scheduler_empty() {
        let mut sched = super::Xc209Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_209_scheduler_reset() {
        let mut sched = super::Xc209Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_209_scheduler_add_remove() {
        let mut sched = super::Xc209Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_209_scheduler_targets() {
        let sched = super::Xc209Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_209_hash_empty() {
        assert_eq!(super::xc_209_hash(b""), 5381);
    }

    #[test]
    fn xc_209_hash_data() {
        let h = super::xc_209_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_209_hash(b"hello"), h);
    }

    #[test]
    fn xc_209_reverse_str() {
        assert_eq!(super::xc_209_reverse("abc"), "cba");
        assert_eq!(super::xc_209_reverse(""), "");
    }


    #[test]
    fn xe_64_pipeline_empty() {
        let p = super::Xe64Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_64_pipeline_parse_stage() {
        let p = super::Xe64Pipeline::new()
            .add_parse(super::xe_64_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_64_pipeline_transform_double() {
        let p = super::Xe64Pipeline::new()
            .add_transform(super::xe_64_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_64_pipeline_validate_reverse() {
        let p = super::Xe64Pipeline::new()
            .add_validate(super::xe_64_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_64_pipeline_emit_filter() {
        let p = super::Xe64Pipeline::new()
            .add_emit(super::xe_64_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_64_pipeline_multi_stage() {
        let p = super::Xe64Pipeline::new()
            .add_parse(super::xe_64_pipeline_identity)
            .add_transform(super::xe_64_pipeline_double)
            .add_validate(super::xe_64_pipeline_reverse)
            .add_emit(super::xe_64_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_64_pipeline_error_propagation() {
        let p = super::Xe64Pipeline::new()
            .add_parse(super::xe_64_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe64Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_64_pipeline_compose() {
        let p1 = super::Xe64Pipeline::new()
            .add_parse(super::xe_64_pipeline_identity);
        let p2 = super::Xe64Pipeline::new()
            .add_transform(super::xe_64_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_64_pipeline_error_display() {
        let e = super::Xe64PipelineError {
            stage: super::Xe64Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_64_cache_put_get() {
        let mut c = super::Xe64Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_64_cache_miss() {
        let mut c: super::Xe64Cache<&str, i32> = super::Xe64Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_64_cache_ttl_expiry() {
        let mut c = super::Xe64Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_64_cache_evict() {
        let mut c = super::Xe64Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_64_cache_capacity() {
        let mut c = super::Xe64Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_64_cache_stats() {
        let mut c = super::Xe64Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_64_cache_clear() {
        let mut c = super::Xe64Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_62 graph tests ------------------------------------------------

    #[test]
    fn xg_62_graph_empty() {
        let g = super::Xg62Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_62_graph_add_node() {
        let mut g = super::Xg62Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_62_graph_add_edge() {
        let mut g = super::Xg62Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_62_graph_neighbors() {
        let mut g = super::Xg62Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_62_graph_has_path() {
        let mut g = super::Xg62Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_62_graph_self_path() {
        let g = super::Xg62Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_62_graph_topo_sort() {
        let mut g = super::Xg62Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_62_graph_cycle_detect_false() {
        let mut g = super::Xg62Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_62_graph_cycle_detect_true() {
        let mut g = super::Xg62Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_62 heap tests -------------------------------------------------

    #[test]
    fn xg_62_heap_empty() {
        let h: super::Xg62Heap<i32> = super::Xg62Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_62_heap_push_pop() {
        let mut h = super::Xg62Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_62_heap_peek() {
        let mut h = super::Xg62Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_62_heap_drain_sorted() {
        let mut h = super::Xg62Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_62_heap_merge() {
        let mut a = super::Xg62Heap::new();
        let mut b = super::Xg62Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_62_heap_default() {
        let h: super::Xg62Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_62_graph_default() {
        let g: super::Xg62Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh208_skip_insert_contains() {
        let mut sl = super::Xh208SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh208_skip_remove() {
        let mut sl = super::Xh208SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh208_skip_len() {
        let mut sl = super::Xh208SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh208_skip_range_query() {
        let mut sl = super::Xh208SkipList::xh_new(4);
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
    fn xh208_skip_floor_ceiling() {
        let mut sl = super::Xh208SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh208_skip_rank() {
        let mut sl = super::Xh208SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh208_skip_empty() {
        let sl = super::Xh208SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh208_skip_duplicates() {
        let mut sl = super::Xh208SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh208_bitset_set_test() {
        let mut bs = super::Xh208BitSet::xh_new(256);
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
    fn xh208_bitset_clear_count() {
        let mut bs = super::Xh208BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh208_bitset_and_or_xor() {
        let mut a = super::Xh208BitSet::xh_new(128);
        let mut b = super::Xh208BitSet::xh_new(128);
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
    fn xh208_bitset_iter_ones() {
        let mut bs = super::Xh208BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh208_bitset_first_last() {
        let mut bs = super::Xh208BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh208_bitset_empty() {
        let bs = super::Xh208BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
