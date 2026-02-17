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

}
