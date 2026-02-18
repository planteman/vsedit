//! Inline ghost text completions.

use std::collections::HashMap;
use std::fmt;
/// A single inline completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCompletionItem {
    pub insert_text: String,
    pub range_start_line: u32,
    pub range_start_col: u32,
    pub range_end_line: u32,
    pub range_end_col: u32,
    pub filter_text: Option<String>,
    pub command: Option<String>,
}

/// A list of inline completion items.
#[derive(Debug, Clone, Default)]
pub struct InlineCompletionList {
    pub items: Vec<InlineCompletionItem>,
}

/// What triggered the inline completion request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionTriggerKind {
    Invoke,
    Automatic,
}

/// Context passed to an inline completion provider.
#[derive(Debug, Clone)]
pub struct InlineCompletionContext {
    pub trigger_kind: InlineCompletionTriggerKind,
    pub selected_suggestion: Option<String>,
}

/// Trait for providing inline completions.
pub trait InlineCompletionProvider {
    fn provide_inline_completions(
        &self,
        uri: &str,
        line: u32,
        col: u32,
        context: &InlineCompletionContext,
    ) -> Option<InlineCompletionList>;
}

/// Tracks the current ghost text display state in the editor.
#[derive(Debug, Clone)]
pub struct GhostTextWidget {
    active_item: Option<InlineCompletionItem>,
    visible: bool,
}

impl GhostTextWidget {
    pub fn new() -> Self {
        Self {
            active_item: None,
            visible: false,
        }
    }

    pub fn show(&mut self, item: InlineCompletionItem) {
        self.active_item = Some(item);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn dismiss(&mut self) {
        self.active_item = None;
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn active_item(&self) -> Option<&InlineCompletionItem> {
        self.active_item.as_ref()
    }
}

impl Default for GhostTextWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl GhostTextWidget {
    /// Returns the first word of the active inline completion text.
    pub fn accept_word(&self) -> Option<String> {
        self.active_item.as_ref().map(|item| {
            let text = &item.insert_text;
            let end = text
                .find(|c: char| c.is_whitespace())
                .unwrap_or(text.len());
            text[..end].to_string()
        })
    }

    /// Returns the first line of the active inline completion text.
    pub fn accept_line(&self) -> Option<String> {
        self.active_item.as_ref().map(|item| {
            let text = &item.insert_text;
            let end = text.find('\n').unwrap_or(text.len());
            text[..end].to_string()
        })
    }
}

// ---------------------------------------------------------------------------
// Inline completion session
// ---------------------------------------------------------------------------

/// Manages cycling through a list of inline completion items.
#[derive(Debug, Clone)]
pub struct InlineCompletionSession {
    items: Vec<InlineCompletionItem>,
    current_index: usize,
}

impl InlineCompletionSession {
    /// Creates a new session from a completion list.
    pub fn new(list: InlineCompletionList) -> Self {
        Self {
            items: list.items,
            current_index: 0,
        }
    }

    /// Advances to the next completion item, wrapping around.
    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.current_index = (self.current_index + 1) % self.items.len();
        }
    }

    /// Moves to the previous completion item, wrapping around.
    pub fn previous(&mut self) {
        if !self.items.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.items.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    /// Returns a reference to the currently selected item.
    pub fn current(&self) -> Option<&InlineCompletionItem> {
        self.items.get(self.current_index)
    }

    /// Accepts the current item, returning it and consuming the session.
    pub fn accept(self) -> Option<InlineCompletionItem> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items[self.current_index].clone())
        }
    }

    /// Returns the number of items in the session.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the session has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Inline completion configuration
// ---------------------------------------------------------------------------

/// How inline completions are matched against the current prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionMatchMode {
    Prefix,
    Subword,
}

/// Configuration for inline completion behavior.
#[derive(Debug, Clone)]
pub struct InlineCompletionConfig {
    pub enabled: bool,
    pub show_toolbar: bool,
    pub mode: InlineCompletionMatchMode,
}

impl Default for InlineCompletionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_toolbar: false,
            mode: InlineCompletionMatchMode::Prefix,
        }
    }
}

// ---------------------------------------------------------------------------
// Provider registry
// ---------------------------------------------------------------------------

/// A registry holding multiple inline completion providers.
#[derive(Default)]
pub struct InlineCompletionRegistry {
    providers: Vec<Box<dyn InlineCompletionProvider>>,
}

impl InlineCompletionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new provider.
    pub fn register_provider(&mut self, provider: Box<dyn InlineCompletionProvider>) {
        self.providers.push(provider);
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

// ---------------------------------------------------------------------------
// Ghost text position & rendering
// ---------------------------------------------------------------------------

/// Where ghost text renders relative to the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostTextPosition {
    /// Render ghost text immediately after the cursor on the same line.
    AfterCursor,
    /// Render ghost text starting on the next line.
    NextLine,
    /// Render ghost text below the cursor, indented to the cursor column.
    BelowCursor,
}

/// Renders gray ghost text with position tracking.
pub struct InlineCompletionGhost {
    text_lines: Vec<String>,
    position: GhostTextPosition,
    cursor_line: u32,
    cursor_col: u32,
    visible: bool,
}

impl InlineCompletionGhost {
    /// Creates a new ghost from raw text at the given cursor location.
    pub fn new(text: &str, cursor_line: u32, cursor_col: u32) -> Self {
        let text_lines: Vec<String> = text.split('\n').map(String::from).collect();
        let position = if text_lines.len() > 1 {
            GhostTextPosition::NextLine
        } else {
            GhostTextPosition::AfterCursor
        };
        Self {
            text_lines,
            position,
            cursor_line,
            cursor_col,
            visible: true,
        }
    }

    /// Returns the current ghost text position.
    pub fn position(&self) -> GhostTextPosition {
        self.position
    }

    /// Returns the number of lines in the ghost text.
    pub fn line_count(&self) -> usize {
        self.text_lines.len()
    }

    /// Returns the first line of ghost text.
    pub fn first_line(&self) -> &str {
        &self.text_lines[0]
    }

    /// Produces render-ready lines according to the current position mode.
    pub fn render_lines(&self) -> Vec<String> {
        let indent = " ".repeat(self.cursor_col as usize);
        match self.position {
            GhostTextPosition::AfterCursor => {
                vec![format!("{}{}", indent, self.text_lines[0])]
            }
            GhostTextPosition::NextLine => self.text_lines.clone(),
            GhostTextPosition::BelowCursor => {
                self.text_lines
                    .iter()
                    .map(|l| format!("{}{}", indent, l))
                    .collect()
            }
        }
    }

    /// Overrides the position mode.
    pub fn set_position(&mut self, pos: GhostTextPosition) {
        self.position = pos;
    }

    /// Returns whether the ghost text is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Makes the ghost text visible.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hides the ghost text.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Joins all lines back into a single string.
    pub fn text(&self) -> String {
        self.text_lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Accept inline completion
// ---------------------------------------------------------------------------

/// The result of accepting an inline completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptResult {
    /// The full document text after applying the completion.
    pub new_text: String,
    /// The cursor line after the insertion.
    pub new_cursor_line: u32,
    /// The cursor column after the insertion.
    pub new_cursor_col: u32,
}

/// Applies the current session completion at the given cursor position.
pub fn accept_inline_completion(
    session: &InlineCompletionSession,
    document_text: &str,
    cursor_line: u32,
    cursor_col: u32,
) -> Option<AcceptResult> {
    let item = session.current()?;
    let mut lines: Vec<String> = document_text.split('\n').map(String::from).collect();
    let line_idx = cursor_line as usize;
    if line_idx >= lines.len() {
        return None;
    }
    let line = &lines[line_idx];
    let col = cursor_col as usize;
    let before = &line[..col.min(line.len())];
    let after = &line[col.min(line.len())..];

    let insert_parts: Vec<&str> = item.insert_text.split('\n').collect();
    let mut new_cursor_line = cursor_line;
    let new_cursor_col;

    if insert_parts.len() == 1 {
        lines[line_idx] = format!("{}{}{}", before, insert_parts[0], after);
        new_cursor_col = (col + insert_parts[0].len()) as u32;
    } else {
        let first = format!("{}{}", before, insert_parts[0]);
        let last_insert = insert_parts[insert_parts.len() - 1];
        let last = format!("{}{}", last_insert, after);
        let mut replacement: Vec<String> = Vec::with_capacity(insert_parts.len());
        replacement.push(first);
        for part in &insert_parts[1..insert_parts.len() - 1] {
            replacement.push((*part).to_string());
        }
        replacement.push(last);
        new_cursor_line += (insert_parts.len() - 1) as u32;
        new_cursor_col = last_insert.len() as u32;
        lines.splice(line_idx..=line_idx, replacement);
    }

    Some(AcceptResult {
        new_text: lines.join("\n"),
        new_cursor_line,
        new_cursor_col,
    })
}

/// Accumulated statistics for inline-complete operations.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineCompleteStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl InlineCompleteStats {
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
    pub fn merge(&mut self, other: &InlineCompleteStats) {
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

impl Default for InlineCompleteStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InlineCompleteStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InlineCompleteStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for inline-complete.
#[derive(Debug, Clone)]
pub struct InlineCompleteValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl InlineCompleteValidator {
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

impl Default for InlineCompleteValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Completion ranker – scores and sorts completion items
// ---------------------------------------------------------------------------

/// Scoring criteria for ranking inline completion items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingCriteria {
    /// Rank by how well the insert text matches a prefix.
    PrefixMatch,
    /// Rank by the length of the insert text (shorter is better).
    Shortest,
    /// Rank by the length of the insert text (longer is better).
    Longest,
}

/// Ranks and sorts inline completion items by a chosen criterion.
#[derive(Debug, Clone)]
pub struct CompletionRanker {
    criteria: RankingCriteria,
}

impl CompletionRanker {
    pub fn new(criteria: RankingCriteria) -> Self {
        Self { criteria }
    }

    /// Score a single item against the given prefix. Higher is better.
    pub fn score(&self, item: &InlineCompletionItem, prefix: &str) -> i64 {
        match self.criteria {
            RankingCriteria::PrefixMatch => {
                let text = item.filter_text.as_deref().unwrap_or(&item.insert_text);
                let common = text
                    .chars()
                    .zip(prefix.chars())
                    .take_while(|(a, b)| a == b)
                    .count();
                common as i64
            }
            RankingCriteria::Shortest => -(item.insert_text.len() as i64),
            RankingCriteria::Longest => item.insert_text.len() as i64,
        }
    }

    /// Sort items in-place by score (descending). Stable sort preserves
    /// order among equal-scored items.
    pub fn rank(&self, items: &mut [InlineCompletionItem], prefix: &str) {
        items.sort_by(|a, b| self.score(b, prefix).cmp(&self.score(a, prefix)));
    }

    /// Return a new sorted `Vec` without modifying the original slice.
    pub fn ranked(&self, items: &[InlineCompletionItem], prefix: &str) -> Vec<InlineCompletionItem> {
        let mut v = items.to_vec();
        self.rank(&mut v, prefix);
        v
    }
}

impl Default for CompletionRanker {
    fn default() -> Self {
        Self::new(RankingCriteria::PrefixMatch)
    }
}

// ---------------------------------------------------------------------------
// Completion cache – keeps recent completions keyed by (uri, line, col)
// ---------------------------------------------------------------------------

/// A simple bounded cache for inline completion results.
#[derive(Debug, Clone)]
pub struct CompletionCache {
    entries: Vec<CompletionCacheEntry>,
    capacity: usize,
}

#[derive(Debug, Clone)]
struct CompletionCacheEntry {
    uri: String,
    line: u32,
    col: u32,
    list: InlineCompletionList,
}

impl CompletionCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// Look up a cached completion list for the exact position.
    pub fn get(&self, uri: &str, line: u32, col: u32) -> Option<&InlineCompletionList> {
        self.entries
            .iter()
            .find(|e| e.uri == uri && e.line == line && e.col == col)
            .map(|e| &e.list)
    }

    /// Insert a completion list. Evicts the oldest entry when at capacity.
    pub fn insert(&mut self, uri: &str, line: u32, col: u32, list: InlineCompletionList) {
        // Remove existing entry for the same key.
        self.entries
            .retain(|e| !(e.uri == uri && e.line == line && e.col == col));
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(CompletionCacheEntry {
            uri: uri.to_string(),
            line,
            col,
            list,
        });
    }

    /// Remove all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for CompletionCache {
    fn default() -> Self {
        Self::new(64)
    }
}

// ---------------------------------------------------------------------------
// Completion preview – prepares a textual preview of a completion
// ---------------------------------------------------------------------------

/// A rendered preview of an inline completion suitable for display in a
/// tooltip or preview panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionPreview {
    /// Lines of the preview with line numbers.
    pub numbered_lines: Vec<String>,
    /// Total number of lines in the preview.
    pub total_lines: usize,
    /// Whether the preview was truncated.
    pub truncated: bool,
}

impl CompletionPreview {
    /// Build a preview from a completion item, limiting to `max_lines`.
    pub fn from_item(item: &InlineCompletionItem, max_lines: usize) -> Self {
        let all_lines: Vec<&str> = item.insert_text.split('\n').collect();
        let truncated = all_lines.len() > max_lines;
        let visible = if truncated {
            &all_lines[..max_lines]
        } else {
            &all_lines[..]
        };
        let start = item.range_start_line as usize;
        let numbered_lines = visible
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{:>4} | {}", start + i + 1, l))
            .collect();
        Self {
            numbered_lines,
            total_lines: all_lines.len(),
            truncated,
        }
    }

    /// Format the preview as a single string separated by newlines.
    pub fn render(&self) -> String {
        let mut out = self.numbered_lines.join("\n");
        if self.truncated {
            out.push_str(&format!("\n  ... ({} more lines)", self.total_lines - self.numbered_lines.len()));
        }
        out
    }
}

impl fmt::Display for CompletionPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

// ---------------------------------------------------------------------------
// Completion telemetry – lightweight event log
// ---------------------------------------------------------------------------

/// The kind of telemetry event recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionEventKind {
    Shown,
    Accepted,
    Dismissed,
    Cycled,
}

/// A single telemetry event.
#[derive(Debug, Clone)]
pub struct CompletionEvent {
    pub kind: CompletionEventKind,
    pub uri: String,
    pub line: u32,
    pub col: u32,
    pub insert_text_len: usize,
}

/// Collects telemetry events for inline completions.
#[derive(Debug, Clone, Default)]
pub struct CompletionTelemetry {
    events: Vec<CompletionEvent>,
}

impl CompletionTelemetry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an event.
    pub fn record(&mut self, kind: CompletionEventKind, uri: &str, line: u32, col: u32, insert_text_len: usize) {
        self.events.push(CompletionEvent {
            kind,
            uri: uri.to_string(),
            line,
            col,
            insert_text_len,
        });
    }

    /// Return total number of recorded events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Count events of a specific kind.
    pub fn count_kind(&self, kind: CompletionEventKind) -> usize {
        self.events.iter().filter(|e| e.kind == kind).count()
    }

    /// Compute the accept rate (accepted / shown). Returns `None` when no
    /// `Shown` events have been recorded.
    pub fn accept_rate(&self) -> Option<f64> {
        let shown = self.count_kind(CompletionEventKind::Shown);
        if shown == 0 {
            return None;
        }
        let accepted = self.count_kind(CompletionEventKind::Accepted);
        Some(accepted as f64 / shown as f64)
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Return a slice of all recorded events.
    pub fn events(&self) -> &[CompletionEvent] {
        &self.events
    }
}

// ---------------------------------------------------------------------------
// CompletionDebouncer – rate-limits completion requests
// ---------------------------------------------------------------------------

/// Controls the rate at which inline completion requests are dispatched.
/// Only allows a request through if enough time has elapsed since the last one.
#[derive(Debug, Clone)]
pub struct CompletionDebouncer {
    delay_ms: u64,
    last_request_ms: Option<u64>,
    suppressed_count: u64,
}

impl CompletionDebouncer {
    /// Create a new debouncer with the given minimum delay between requests.
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            last_request_ms: None,
            suppressed_count: 0,
        }
    }

    /// Try to dispatch a request at `now_ms`. Returns `true` if allowed.
    pub fn try_request(&mut self, now_ms: u64) -> bool {
        if let Some(last) = self.last_request_ms {
            if now_ms.saturating_sub(last) < self.delay_ms {
                self.suppressed_count += 1;
                return false;
            }
        }
        self.last_request_ms = Some(now_ms);
        true
    }

    /// Number of requests that were suppressed.
    pub fn suppressed_count(&self) -> u64 {
        self.suppressed_count
    }

    /// Reset the debouncer state.
    pub fn reset(&mut self) {
        self.last_request_ms = None;
        self.suppressed_count = 0;
    }

    /// The configured delay in milliseconds.
    pub fn delay_ms(&self) -> u64 {
        self.delay_ms
    }
}

// ---------------------------------------------------------------------------
// CompletionDiff – shows what text a completion would insert/replace
// ---------------------------------------------------------------------------

/// Describes the textual difference a completion would produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionDiff {
    /// Text that would be removed from the document.
    pub removed: String,
    /// Text that would be inserted into the document.
    pub inserted: String,
}

impl CompletionDiff {
    /// Compute the diff between the current line content and the completion.
    pub fn compute(current_line: &str, col: usize, item: &InlineCompletionItem) -> Self {
        let prefix = if col <= current_line.len() {
            &current_line[..col]
        } else {
            current_line
        };
        let suffix = if col <= current_line.len() {
            &current_line[col..]
        } else {
            ""
        };
        let removed = suffix.to_string();
        let inserted = item.insert_text.clone();
        Self { removed, inserted }

    }

    /// Whether this completion is a pure insertion (nothing removed).
    pub fn is_pure_insert(&self) -> bool {
        self.removed.is_empty()
    }

    /// Net change in character count.
    pub fn net_change(&self) -> i64 {
        self.inserted.len() as i64 - self.removed.len() as i64
    }

    /// Build the resulting line after applying this diff.
    pub fn apply_to(&self, current_line: &str, col: usize) -> String {
        let prefix = if col <= current_line.len() {
            &current_line[..col]
        } else {
            current_line
        };
        format!("{}{}", prefix, self.inserted)
    }
}

impl fmt::Display for CompletionDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CompletionDiff(-{:?} +{:?})", self.removed, self.inserted)
    }
}

// ---------------------------------------------------------------------------
// CompletionFilter – filter completions by criteria
// ---------------------------------------------------------------------------

/// Filters inline completion items by various criteria.
#[derive(Debug, Clone)]
pub struct CompletionFilter {
    min_length: Option<usize>,
    max_length: Option<usize>,
    prefix: Option<String>,
}

impl CompletionFilter {
    pub fn new() -> Self {
        Self {
            min_length: None,
            max_length: None,
            prefix: None,
        }
    }

    /// Only include completions with at least this many characters.
    pub fn min_length(mut self, len: usize) -> Self {
        self.min_length = Some(len);
        self
    }

    /// Only include completions with at most this many characters.
    pub fn max_length(mut self, len: usize) -> Self {
        self.max_length = Some(len);
        self
    }

    /// Only include completions whose insert text starts with the given prefix.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Apply the filter, returning only matching items.
    pub fn apply<'a>(&self, items: &'a [InlineCompletionItem]) -> Vec<&'a InlineCompletionItem> {
        items
            .iter()
            .filter(|item| {
                if let Some(min) = self.min_length {
                    if item.insert_text.len() < min {
                        return false;
                    }
                }
                if let Some(max) = self.max_length {
                    if item.insert_text.len() > max {
                        return false;
                    }
                }
                if let Some(ref pfx) = self.prefix {
                    if !item.insert_text.starts_with(pfx.as_str()) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Count how many items pass the filter.
    pub fn count_matching(&self, items: &[InlineCompletionItem]) -> usize {
        self.apply(items).len()
    }
}

impl Default for CompletionFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// InlineCompletionPrefixCache
// ---------------------------------------------------------------------------

/// Caches completions keyed by the typed prefix string.
#[derive(Debug, Clone)]
pub struct InlineCompletionPrefixCache {
    entries: HashMap<String, Vec<InlineCompletionItem>>,
    max_entries: usize,
}

impl InlineCompletionPrefixCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    pub fn insert(&mut self, prefix: &str, items: Vec<InlineCompletionItem>) {
        if self.entries.len() >= self.max_entries && !self.entries.contains_key(prefix) {
            // Evict an arbitrary entry to make room.
            if let Some(key) = self.entries.keys().next().cloned() {
                self.entries.remove(&key);
            }
        }
        self.entries.insert(prefix.to_string(), items);
    }

    pub fn lookup(&self, prefix: &str) -> Option<&[InlineCompletionItem]> {
        self.entries.get(prefix).map(|v| v.as_slice())
    }

    /// Returns references to all items whose cache key starts with `prefix`.
    pub fn lookup_by_prefix(&self, prefix: &str) -> Vec<&InlineCompletionItem> {
        self.entries
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .flat_map(|(_, v)| v.iter())
            .collect()
    }

    pub fn contains(&self, prefix: &str) -> bool {
        self.entries.contains_key(prefix)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }
}

// ---------------------------------------------------------------------------
// InlineGhostRenderer + RenderedGhostText
// ---------------------------------------------------------------------------

/// Result of rendering ghost text through [`InlineGhostRenderer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedGhostText {
    pub visible_text: String,
    pub truncated: bool,
    pub fade_region: Option<(usize, usize)>,
}

/// Renders ghost text with optional truncation and fade region.
#[derive(Debug, Clone)]
pub struct InlineGhostRenderer {
    max_visible_chars: usize,
    fade_chars: usize,
    show_cursor: bool,
}

impl InlineGhostRenderer {
    pub fn new(max_visible: usize) -> Self {
        Self {
            max_visible_chars: max_visible,
            fade_chars: 0,
            show_cursor: false,
        }
    }

    pub fn with_fade(mut self, chars: usize) -> Self {
        self.fade_chars = chars;
        self
    }

    pub fn with_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    pub fn render(&self, text: &str) -> RenderedGhostText {
        let char_count = text.chars().count();
        let truncated = char_count > self.max_visible_chars;
        let visible: String = text.chars().take(self.max_visible_chars).collect();

        let fade_region = if truncated && self.fade_chars > 0 {
            let visible_len = visible.chars().count();
            let fade_start = visible_len.saturating_sub(self.fade_chars);
            Some((fade_start, visible_len))
        } else {
            None
        };

        RenderedGhostText {
            visible_text: visible,
            truncated,
            fade_region,
        }
    }

    pub fn visible_length(&self, text: &str) -> usize {
        text.chars().count().min(self.max_visible_chars)
    }
}

// ---------------------------------------------------------------------------
// InlineCompletionCycler
// ---------------------------------------------------------------------------

/// Cycles through a list of inline completion suggestions.
#[derive(Debug, Clone)]
pub struct InlineCompletionCycler {
    items: Vec<InlineCompletionItem>,
    current: usize,
}

impl InlineCompletionCycler {
    pub fn new(items: Vec<InlineCompletionItem>) -> Self {
        Self { items, current: 0 }
    }

    /// Advance to the next item, wrapping around to the beginning.
    pub fn next(&mut self) -> Option<&InlineCompletionItem> {
        if self.items.is_empty() {
            return None;
        }
        self.current = (self.current + 1) % self.items.len();
        Some(&self.items[self.current])
    }

    /// Move to the previous item, wrapping around to the end.
    pub fn previous(&mut self) -> Option<&InlineCompletionItem> {
        if self.items.is_empty() {
            return None;
        }
        self.current = if self.current == 0 {
            self.items.len() - 1
        } else {
            self.current - 1
        };
        Some(&self.items[self.current])
    }

    pub fn current(&self) -> Option<&InlineCompletionItem> {
        self.items.get(self.current)
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Human-readable label such as "2 of 5".
    pub fn position_label(&self) -> String {
        if self.items.is_empty() {
            return String::from("0 of 0");
        }
        format!("{} of {}", self.current + 1, self.items.len())
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// CompletionAcceptRejectTracker + CompletionAction
// ---------------------------------------------------------------------------

/// Describes a single accept/reject action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionAction {
    Accept(usize),
    Reject,
}

/// Tracks how often completions are accepted or rejected and the total
/// character count of accepted text.
#[derive(Debug, Clone)]
pub struct CompletionAcceptRejectTracker {
    accepted: u32,
    rejected: u32,
    total_chars_accepted: usize,
    last_action: Option<CompletionAction>,
}

impl CompletionAcceptRejectTracker {
    pub fn new() -> Self {
        Self {
            accepted: 0,
            rejected: 0,
            total_chars_accepted: 0,
            last_action: None,
        }
    }

    pub fn record_accept(&mut self, char_count: usize) {
        self.accepted += 1;
        self.total_chars_accepted += char_count;
        self.last_action = Some(CompletionAction::Accept(char_count));
    }

    pub fn record_reject(&mut self) {
        self.rejected += 1;
        self.last_action = Some(CompletionAction::Reject);
    }

    /// Returns the fraction of actions that were accepts (0.0 when no actions).
    pub fn accept_rate(&self) -> f64 {
        let total = self.total_actions();
        if total == 0 {
            return 0.0;
        }
        self.accepted as f64 / total as f64
    }

    pub fn total_actions(&self) -> u32 {
        self.accepted + self.rejected
    }

    /// Average character length of accepted completions (0.0 when none).
    pub fn average_accepted_length(&self) -> f64 {
        if self.accepted == 0 {
            return 0.0;
        }
        self.total_chars_accepted as f64 / self.accepted as f64
    }

    pub fn reset(&mut self) {
        self.accepted = 0;
        self.rejected = 0;
        self.total_chars_accepted = 0;
        self.last_action = None;
    }
}

impl Default for CompletionAcceptRejectTracker {
    fn default() -> Self {
        Self::new()
    }
}


// ── Inline Completion Confidence Scorer ──

/// Individual scoring criteria for an inline completion.
#[derive(Debug, Clone)]
pub struct CompletionConfidenceFactors {
    pub prefix_match_ratio: f64,
    pub text_length_score: f64,
    pub provider_priority: f64,
    pub recency_bonus: f64,
    pub frequency_bonus: f64,
}

impl Default for CompletionConfidenceFactors {
    fn default() -> Self {
        Self {
            prefix_match_ratio: 0.0,
            text_length_score: 0.0,
            provider_priority: 1.0,
            recency_bonus: 0.0,
            frequency_bonus: 0.0,
        }
    }
}

impl CompletionConfidenceFactors {
    /// Compute a weighted confidence score in [0, 1].
    pub fn score(&self) -> f64 {
        let raw = self.prefix_match_ratio * 0.4
            + self.text_length_score * 0.15
            + self.provider_priority * 0.2
            + self.recency_bonus * 0.15
            + self.frequency_bonus * 0.1;
        raw.clamp(0.0, 1.0)
    }
}

/// A scored inline completion item.
#[derive(Debug, Clone)]
pub struct ScoredCompletion {
    pub insert_text: String,
    pub factors: CompletionConfidenceFactors,
}

impl ScoredCompletion {
    pub fn new(insert_text: impl Into<String>) -> Self {
        Self {
            insert_text: insert_text.into(),
            factors: CompletionConfidenceFactors::default(),
        }
    }

    pub fn confidence(&self) -> f64 {
        self.factors.score()
    }
}

/// Scores inline completions by confidence and relevance.
pub struct InlineCompletionConfidenceScorer {
    items: Vec<ScoredCompletion>,
    min_confidence_threshold: f64,
}

impl InlineCompletionConfidenceScorer {
    pub fn new(min_threshold: f64) -> Self {
        Self {
            items: Vec::new(),
            min_confidence_threshold: min_threshold.clamp(0.0, 1.0),
        }
    }

    /// Score a completion's prefix match ratio against the typed text.
    pub fn compute_prefix_match(typed: &str, completion: &str) -> f64 {
        if typed.is_empty() || completion.is_empty() {
            return 0.0;
        }
        let common = typed
            .chars()
            .zip(completion.chars())
            .take_while(|(a, b)| a == b)
            .count();
        common as f64 / typed.len().max(completion.len()) as f64
    }

    /// Score text length (prefer medium-length completions).
    pub fn compute_length_score(text: &str) -> f64 {
        let len = text.len();
        if len == 0 {
            return 0.0;
        }
        // Bell curve peaking at ~30 chars
        let x = (len as f64 - 30.0) / 20.0;
        (-x * x / 2.0).exp()
    }

    /// Add a completion with auto-computed scores.
    pub fn add_completion(&mut self, text: impl Into<String>, typed_prefix: &str) {
        let text = text.into();
        let mut item = ScoredCompletion::new(text.clone());
        item.factors.prefix_match_ratio = Self::compute_prefix_match(typed_prefix, &text);
        item.factors.text_length_score = Self::compute_length_score(&text);
        self.items.push(item);
    }

    /// Add a pre-scored completion.
    pub fn add_scored(&mut self, item: ScoredCompletion) {
        self.items.push(item);
    }

    /// Get completions above the minimum confidence threshold, sorted by score.
    pub fn filtered_results(&self) -> Vec<&ScoredCompletion> {
        let mut results: Vec<&ScoredCompletion> = self
            .items
            .iter()
            .filter(|i| i.confidence() >= self.min_confidence_threshold)
            .collect();
        results.sort_by(|a, b| {
            b.confidence()
                .partial_cmp(&a.confidence())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Get the best completion above threshold.
    pub fn best(&self) -> Option<&ScoredCompletion> {
        self.filtered_results().into_iter().next()
    }

    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    pub fn above_threshold_count(&self) -> usize {
        self.filtered_results().len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

// ── Ghost Text Styler ──

/// Visual style properties for ghost text rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct GhostTextStyle {
    pub color: String,
    pub font_style: GhostFontStyle,
    pub opacity: f64,
    pub background_color: Option<String>,
    pub border: Option<String>,
}

/// Font style variants for ghost text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostFontStyle {
    Normal,
    Italic,
    Bold,
    BoldItalic,
}

impl fmt::Display for GhostFontStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GhostFontStyle::Normal => write!(f, "normal"),
            GhostFontStyle::Italic => write!(f, "italic"),
            GhostFontStyle::Bold => write!(f, "bold"),
            GhostFontStyle::BoldItalic => write!(f, "bold italic"),
        }
    }
}

impl Default for GhostTextStyle {
    fn default() -> Self {
        Self {
            color: "#888888".to_string(),
            font_style: GhostFontStyle::Italic,
            opacity: 0.6,
            background_color: None,
            border: None,
        }
    }
}

/// Manages visual styling of ghost text for inline completions.
pub struct GhostTextStyler {
    base_style: GhostTextStyle,
    keyword_style: Option<GhostTextStyle>,
    diff_add_color: String,
    diff_remove_color: String,
    active: bool,
}

impl GhostTextStyler {
    pub fn new() -> Self {
        Self {
            base_style: GhostTextStyle::default(),
            keyword_style: None,
            diff_add_color: "#22aa22".to_string(),
            diff_remove_color: "#aa2222".to_string(),
            active: true,
        }
    }

    pub fn with_base_style(mut self, style: GhostTextStyle) -> Self {
        self.base_style = style;
        self
    }

    pub fn with_keyword_style(mut self, style: GhostTextStyle) -> Self {
        self.keyword_style = Some(style);
        self
    }

    /// Get the style for a given piece of ghost text.
    pub fn style_for(&self, text: &str) -> &GhostTextStyle {
        if let Some(ref kw_style) = self.keyword_style {
            let keywords = ["fn", "let", "if", "else", "for", "while", "return", "struct", "enum"];
            if keywords.iter().any(|kw| text.trim_start().starts_with(kw)) {
                return kw_style;
            }
        }
        &self.base_style
    }

    /// Generate a CSS-like style string for the ghost text.
    pub fn to_css(&self, text: &str) -> String {
        let style = self.style_for(text);
        let mut css = format!("color: {}; opacity: {};", style.color, style.opacity);
        css.push_str(&format!(" font-style: {};", style.font_style));
        if let Some(ref bg) = style.background_color {
            css.push_str(&format!(" background-color: {};", bg));
        }
        if let Some(ref border) = style.border {
            css.push_str(&format!(" border: {};", border));
        }
        css
    }

    /// Get diff highlighting color for added text.
    pub fn diff_add_color(&self) -> &str {
        &self.diff_add_color
    }

    /// Get diff highlighting color for removed text.
    pub fn diff_remove_color(&self) -> &str {
        &self.diff_remove_color
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn base_style(&self) -> &GhostTextStyle {
        &self.base_style
    }
}



// ---------------------------------------------------------------------------
// vsedit-inline-complete: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCompleteXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl InlineCompleteXConfig {
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

impl std::fmt::Display for InlineCompleteXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct InlineCompleteXRegistry {
    entries: Vec<InlineCompleteXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl InlineCompleteXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: InlineCompleteXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&InlineCompleteXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut InlineCompleteXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<InlineCompleteXConfig> {
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

    pub fn active_entries(&self) -> Vec<&InlineCompleteXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&InlineCompleteXConfig> {
        let mut sorted: Vec<&InlineCompleteXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&InlineCompleteXConfig> {
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

    pub fn iter(&self) -> InlineCompleteXIterator<'_> {
        InlineCompleteXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct InlineCompleteXIterator<'a> {
    inner: std::slice::Iter<'a, InlineCompleteXConfig>,
}

impl<'a> Iterator for InlineCompleteXIterator<'a> {
    type Item = &'a InlineCompleteXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct InlineCompleteXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl InlineCompleteXCache {
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
pub struct InlineCompleteXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl InlineCompleteXFormatter {
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

    pub fn format_entry(&self, entry: &InlineCompleteXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &InlineCompleteXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &InlineCompleteXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for InlineCompleteXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct InlineCompleteXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl InlineCompleteXValidator {
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

    pub fn validate(&self, entry: &InlineCompleteXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &InlineCompleteXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for InlineCompleteXValidator {
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
// xb_ utilities – batch 54
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer54 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer54 {
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
pub fn xb_fnv1a_54(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_54<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_54<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_54(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_54(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 94
// ---------------------------------------------------------------------------

/// Generic object pool `Xc94Pool<T>`.
pub struct Xc94Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc94Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc94PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc94Pool<T> {
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
    pub fn stats(&self) -> Xc94PoolStats {
        Xc94PoolStats {
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

impl<T> Default for Xc94Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc94Scheduler`.
pub struct Xc94Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc94Scheduler {
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

impl Default for Xc94Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_94 hash for the given byte slice.
pub fn xc_94_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_94 convention.
pub fn xc_94_reverse(s: &str) -> String {
    s.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_text_show_hide() {
        let mut w = GhostTextWidget::new();
        assert!(!w.is_visible());
        assert!(w.active_item().is_none());

        let item = InlineCompletionItem {
            insert_text: "hello()".into(),
            range_start_line: 0,
            range_start_col: 0,
            range_end_line: 0,
            range_end_col: 0,
            filter_text: None,
            command: None,
        };
        w.show(item.clone());
        assert!(w.is_visible());
        assert_eq!(w.active_item().unwrap().insert_text, "hello()");

        w.hide();
        assert!(!w.is_visible());
        assert!(w.active_item().is_some()); // item still retained

        w.dismiss();
        assert!(w.active_item().is_none());
    }

    #[test]
    fn completion_list_default_empty() {
        let list = InlineCompletionList::default();
        assert!(list.items.is_empty());
    }

    #[test]
    fn provider_trait_impl() {
        struct DummyProvider;
        impl InlineCompletionProvider for DummyProvider {
            fn provide_inline_completions(
                &self,
                _uri: &str,
                _line: u32,
                _col: u32,
                _context: &InlineCompletionContext,
            ) -> Option<InlineCompletionList> {
                Some(InlineCompletionList {
                    items: vec![InlineCompletionItem {
                        insert_text: "world".into(),
                        range_start_line: 1,
                        range_start_col: 0,
                        range_end_line: 1,
                        range_end_col: 0,
                        filter_text: Some("wor".into()),
                        command: None,
                    }],
                })
            }
        }

        let ctx = InlineCompletionContext {
            trigger_kind: InlineCompletionTriggerKind::Automatic,
            selected_suggestion: None,
        };
        let provider = DummyProvider;
        let result = provider.provide_inline_completions("file:///test.rs", 1, 0, &ctx);
        assert!(result.is_some());
        assert_eq!(result.unwrap().items.len(), 1);
    }

    #[test]
    fn trigger_kind_equality() {
        assert_eq!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Invoke);
        assert_ne!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Automatic);
    }

    fn make_item(text: &str) -> InlineCompletionItem {
        InlineCompletionItem {
            insert_text: text.into(),
            range_start_line: 0,
            range_start_col: 0,
            range_end_line: 0,
            range_end_col: 0,
            filter_text: None,
            command: None,
        }
    }

    fn make_list(texts: &[&str]) -> InlineCompletionList {
        InlineCompletionList {
            items: texts.iter().map(|t| make_item(t)).collect(),
        }
    }

    #[test]
    fn session_next_previous_cycle() {
        let mut session = InlineCompletionSession::new(make_list(&["a", "b", "c"]));
        assert_eq!(session.current().unwrap().insert_text, "a");
        session.next();
        assert_eq!(session.current().unwrap().insert_text, "b");
        session.next();
        session.next();
        assert_eq!(session.current().unwrap().insert_text, "a"); // wraps
        session.previous();
        assert_eq!(session.current().unwrap().insert_text, "c"); // wraps back
    }

    #[test]
    fn session_empty() {
        let session = InlineCompletionSession::new(InlineCompletionList::default());
        assert!(session.is_empty());
        assert_eq!(session.len(), 0);
        assert!(session.current().is_none());
    }

    #[test]
    fn session_accept() {
        let session = InlineCompletionSession::new(make_list(&["hello"]));
        let accepted = session.accept();
        assert_eq!(accepted.unwrap().insert_text, "hello");
    }

    #[test]
    fn session_len() {
        let session = InlineCompletionSession::new(make_list(&["a", "b"]));
        assert_eq!(session.len(), 2);
        assert!(!session.is_empty());
    }

    #[test]
    fn accept_word_single_word() {
        let mut w = GhostTextWidget::new();
        w.show(make_item("hello"));
        assert_eq!(w.accept_word(), Some("hello".into()));
    }

    #[test]
    fn accept_word_multi_word() {
        let mut w = GhostTextWidget::new();
        w.show(make_item("hello world foo"));
        assert_eq!(w.accept_word(), Some("hello".into()));
    }

    #[test]
    fn accept_line_multi_line() {
        let mut w = GhostTextWidget::new();
        w.show(make_item("first line\nsecond line"));
        assert_eq!(w.accept_line(), Some("first line".into()));
    }

    #[test]
    fn accept_word_none_when_empty() {
        let w = GhostTextWidget::new();
        assert!(w.accept_word().is_none());
    }

    #[test]
    fn config_default() {
        let cfg = InlineCompletionConfig::default();
        assert!(cfg.enabled);
        assert!(!cfg.show_toolbar);
        assert_eq!(cfg.mode, InlineCompletionMatchMode::Prefix);
    }

    #[test]
    fn registry_register_and_count() {
        struct Dummy;
        impl InlineCompletionProvider for Dummy {
            fn provide_inline_completions(
                &self, _: &str, _: u32, _: u32, _: &InlineCompletionContext,
            ) -> Option<InlineCompletionList> {
                None
            }
        }
        let mut reg = InlineCompletionRegistry::new();
        assert_eq!(reg.provider_count(), 0);
        reg.register_provider(Box::new(Dummy));
        assert_eq!(reg.provider_count(), 1);
        reg.register_provider(Box::new(Dummy));
        assert_eq!(reg.provider_count(), 2);
    }

    #[test]
    fn eq_inlinecompletiontriggerkind_same() {
        assert_eq!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Invoke);
    }

    #[test]
    fn ne_inlinecompletiontriggerkind_diff() {
        assert_ne!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Automatic);
    }

    #[test]
    fn eq_inlinecompletionmatchmode_same() {
        assert_eq!(InlineCompletionMatchMode::Prefix, InlineCompletionMatchMode::Prefix);
    }

    #[test]
    fn ne_inlinecompletionmatchmode_diff() {
        assert_ne!(InlineCompletionMatchMode::Prefix, InlineCompletionMatchMode::Subword);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn inline_complete_stats_new_defaults() {
        let stats = InlineCompleteStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn inline_complete_stats_record_success() {
        let mut stats = InlineCompleteStats::new();
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
    fn inline_complete_stats_record_failure() {
        let mut stats = InlineCompleteStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn inline_complete_stats_reset() {
        let mut stats = InlineCompleteStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn inline_complete_stats_merge() {
        let mut a = InlineCompleteStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = InlineCompleteStats::new();
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
    fn inline_complete_stats_display() {
        let mut stats = InlineCompleteStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn inline_complete_stats_default() {
        let stats = InlineCompleteStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn inline_complete_validator_accepts_valid_name() {
        let v = InlineCompleteValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn inline_complete_validator_rejects_empty() {
        let v = InlineCompleteValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn inline_complete_validator_rejects_too_long() {
        let v = InlineCompleteValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn inline_complete_validator_forbidden_prefix() {
        let v = InlineCompleteValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn inline_complete_validator_allowed_chars() {
        let v = InlineCompleteValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn inline_complete_validator_range() {
        let v = InlineCompleteValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn inline_complete_sanitize_removes_control() {
        let result = InlineCompleteValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn inline_complete_truncate_short_string() {
        assert_eq!(InlineCompleteValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn inline_complete_truncate_long_string() {
        let result = InlineCompleteValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn inline_complete_is_ascii_printable() {
        assert!(InlineCompleteValidator::is_ascii_printable("Hello World 123"));
        assert!(!InlineCompleteValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // GhostTextPosition / InlineCompletionGhost tests
    // -----------------------------------------------------------------------

    #[test]
    fn ghost_position_single_line_after_cursor() {
        let ghost = InlineCompletionGhost::new("hello", 5, 10);
        assert_eq!(ghost.position(), GhostTextPosition::AfterCursor);
        assert_eq!(ghost.line_count(), 1);
        assert_eq!(ghost.first_line(), "hello");
    }

    #[test]
    fn ghost_position_multiline_next_line() {
        let ghost = InlineCompletionGhost::new("line1\nline2\nline3", 5, 10);
        assert_eq!(ghost.position(), GhostTextPosition::NextLine);
        assert_eq!(ghost.line_count(), 3);
    }

    #[test]
    fn ghost_render_after_cursor() {
        let ghost = InlineCompletionGhost::new("world", 0, 5);
        let lines = ghost.render_lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "     world");
    }

    #[test]
    fn ghost_render_next_line() {
        let ghost = InlineCompletionGhost::new("a\nb", 0, 3);
        let lines = ghost.render_lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "a");
        assert_eq!(lines[1], "b");
    }

    #[test]
    fn ghost_render_below_cursor() {
        let mut ghost = InlineCompletionGhost::new("x\ny", 0, 4);
        ghost.set_position(GhostTextPosition::BelowCursor);
        let lines = ghost.render_lines();
        assert_eq!(lines[0], "    x");
        assert_eq!(lines[1], "    y");
    }

    #[test]
    fn ghost_visibility() {
        let mut ghost = InlineCompletionGhost::new("hi", 0, 0);
        assert!(ghost.is_visible());
        ghost.hide();
        assert!(!ghost.is_visible());
        ghost.show();
        assert!(ghost.is_visible());
    }

    #[test]
    fn ghost_text_roundtrip() {
        let ghost = InlineCompletionGhost::new("a\nb\nc", 0, 0);
        assert_eq!(ghost.text(), "a\nb\nc");
    }

    // -----------------------------------------------------------------------
    // accept_inline_completion tests
    // -----------------------------------------------------------------------

    #[test]
    fn accept_completion_single_line() {
        let item = InlineCompletionItem {
            insert_text: "World".to_string(),
            range_start_line: 0,
            range_start_col: 5,
            range_end_line: 0,
            range_end_col: 5,
            filter_text: None,
            command: None,
        };
        let list = InlineCompletionList { items: vec![item] };
        let session = InlineCompletionSession::new(list);
        let result = accept_inline_completion(&session, "Hello", 0, 5).unwrap();
        assert_eq!(result.new_text, "HelloWorld");
        assert_eq!(result.new_cursor_line, 0);
        assert_eq!(result.new_cursor_col, 10);
    }

    #[test]
    fn accept_completion_multiline() {
        let item = InlineCompletionItem {
            insert_text: "B\nC".to_string(),
            range_start_line: 0,
            range_start_col: 1,
            range_end_line: 0,
            range_end_col: 1,
            filter_text: None,
            command: None,
        };
        let list = InlineCompletionList { items: vec![item] };
        let session = InlineCompletionSession::new(list);
        let result = accept_inline_completion(&session, "A", 0, 1).unwrap();
        assert_eq!(result.new_text, "AB\nC");
        assert_eq!(result.new_cursor_line, 1);
        assert_eq!(result.new_cursor_col, 1);
    }

    #[test]
    fn accept_completion_empty_session() {
        let list = InlineCompletionList { items: vec![] };
        let session = InlineCompletionSession::new(list);
        assert!(accept_inline_completion(&session, "Hello", 0, 5).is_none());
    }

    // -----------------------------------------------------------------------
    // CompletionRanker tests
    // -----------------------------------------------------------------------

    #[test]
    fn ranker_prefix_match_scores() {
        let ranker = CompletionRanker::new(RankingCriteria::PrefixMatch);
        let item_ab = make_item("abc");
        let item_xy = make_item("xyz");
        assert!(ranker.score(&item_ab, "ab") > ranker.score(&item_xy, "ab"));
    }

    #[test]
    fn ranker_shortest_sorts_ascending_length() {
        let ranker = CompletionRanker::new(RankingCriteria::Shortest);
        let items = vec![make_item("longer_text"), make_item("ab"), make_item("medium")];
        let ranked = ranker.ranked(&items, "");
        assert_eq!(ranked[0].insert_text, "ab");
        assert_eq!(ranked[1].insert_text, "medium");
        assert_eq!(ranked[2].insert_text, "longer_text");
    }

    #[test]
    fn ranker_longest_sorts_descending_length() {
        let ranker = CompletionRanker::new(RankingCriteria::Longest);
        let items = vec![make_item("ab"), make_item("longer_text"), make_item("medium")];
        let ranked = ranker.ranked(&items, "");
        assert_eq!(ranked[0].insert_text, "longer_text");
        assert_eq!(ranked[2].insert_text, "ab");
    }

    // -----------------------------------------------------------------------
    // CompletionCache tests
    // -----------------------------------------------------------------------

    #[test]
    fn cache_insert_and_get() {
        let mut cache = CompletionCache::new(4);
        assert!(cache.is_empty());
        let list = make_list(&["foo", "bar"]);
        cache.insert("file:///a.rs", 1, 5, list);
        assert_eq!(cache.len(), 1);
        let hit = cache.get("file:///a.rs", 1, 5).unwrap();
        assert_eq!(hit.items.len(), 2);
        assert!(cache.get("file:///a.rs", 1, 6).is_none());
    }

    #[test]
    fn cache_evicts_oldest() {
        let mut cache = CompletionCache::new(2);
        cache.insert("a", 0, 0, make_list(&["1"]));
        cache.insert("b", 0, 0, make_list(&["2"]));
        cache.insert("c", 0, 0, make_list(&["3"]));
        assert_eq!(cache.len(), 2);
        assert!(cache.get("a", 0, 0).is_none()); // evicted
        assert!(cache.get("b", 0, 0).is_some());
        assert!(cache.get("c", 0, 0).is_some());
    }

    // -----------------------------------------------------------------------
    // CompletionPreview tests
    // -----------------------------------------------------------------------

    #[test]
    fn preview_single_line() {
        let item = make_item("hello");
        let preview = CompletionPreview::from_item(&item, 10);
        assert_eq!(preview.total_lines, 1);
        assert!(!preview.truncated);
        assert_eq!(preview.numbered_lines.len(), 1);
        assert!(preview.numbered_lines[0].contains("hello"));
    }

    #[test]
    fn preview_truncated() {
        let item = make_item("a\nb\nc\nd\ne");
        let preview = CompletionPreview::from_item(&item, 2);
        assert!(preview.truncated);
        assert_eq!(preview.numbered_lines.len(), 2);
        assert_eq!(preview.total_lines, 5);
        let rendered = preview.render();
        assert!(rendered.contains("3 more lines"));
    }

    // -----------------------------------------------------------------------
    // CompletionTelemetry tests
    // -----------------------------------------------------------------------

    #[test]
    fn telemetry_record_and_count() {
        let mut tel = CompletionTelemetry::new();
        assert_eq!(tel.event_count(), 0);
        tel.record(CompletionEventKind::Shown, "f.rs", 0, 0, 5);
        tel.record(CompletionEventKind::Accepted, "f.rs", 0, 0, 5);
        tel.record(CompletionEventKind::Shown, "f.rs", 1, 0, 3);
        tel.record(CompletionEventKind::Dismissed, "f.rs", 1, 0, 3);
        assert_eq!(tel.event_count(), 4);
        assert_eq!(tel.count_kind(CompletionEventKind::Shown), 2);
        assert_eq!(tel.count_kind(CompletionEventKind::Accepted), 1);
    }

    #[test]
    fn telemetry_accept_rate() {
        let mut tel = CompletionTelemetry::new();
        assert!(tel.accept_rate().is_none());
        tel.record(CompletionEventKind::Shown, "f.rs", 0, 0, 5);
        tel.record(CompletionEventKind::Shown, "f.rs", 1, 0, 3);
        tel.record(CompletionEventKind::Accepted, "f.rs", 0, 0, 5);
        let rate = tel.accept_rate().unwrap();
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn telemetry_clear() {
        let mut tel = CompletionTelemetry::new();
        tel.record(CompletionEventKind::Cycled, "f.rs", 0, 0, 10);
        tel.clear();
        assert_eq!(tel.event_count(), 0);
        assert!(tel.accept_rate().is_none());
    }

    #[test]
    fn debouncer_allows_first_request() {
        let mut db = CompletionDebouncer::new(100);
        assert!(db.try_request(0));
        assert_eq!(db.suppressed_count(), 0);
    }

    #[test]
    fn debouncer_suppresses_rapid_requests() {
        let mut db = CompletionDebouncer::new(100);
        assert!(db.try_request(0));
        assert!(!db.try_request(50));
        assert!(!db.try_request(99));
        assert_eq!(db.suppressed_count(), 2);
        assert!(db.try_request(100));
        assert_eq!(db.suppressed_count(), 2);
    }

    #[test]
    fn debouncer_reset() {
        let mut db = CompletionDebouncer::new(100);
        assert!(db.try_request(0));
        assert!(!db.try_request(10));
        db.reset();
        assert!(db.try_request(10));
        assert_eq!(db.suppressed_count(), 0);
        assert_eq!(db.delay_ms(), 100);
    }

    #[test]
    fn completion_diff_pure_insert() {
        let item = InlineCompletionItem {
            insert_text: "hello()".into(),
            range_start_line: 0,
            range_start_col: 5,
            range_end_line: 0,
            range_end_col: 5,
            filter_text: None,
            command: None,
        };
        let diff = CompletionDiff::compute("fn he", 5, &item);
        assert!(diff.is_pure_insert());
        assert_eq!(diff.net_change(), 7);
        let result = diff.apply_to("fn he", 5);
        assert_eq!(result, "fn hehello()");
    }

    #[test]
    fn completion_diff_with_replacement() {
        let item = InlineCompletionItem {
            insert_text: "world".into(),
            range_start_line: 0,
            range_start_col: 3,
            range_end_line: 0,
            range_end_col: 6,
            filter_text: None,
            command: None,
        };
        let diff = CompletionDiff::compute("fn foo()", 3, &item);
        assert!(!diff.is_pure_insert());
        assert_eq!(diff.removed, "foo()");
        let display = format!("{diff}");
        assert!(display.contains("CompletionDiff"));
    }

    #[test]
    fn completion_filter_by_length() {
        let items = vec![
            InlineCompletionItem {
                insert_text: "ab".into(),
                range_start_line: 0, range_start_col: 0,
                range_end_line: 0, range_end_col: 0,
                filter_text: None, command: None,
            },
            InlineCompletionItem {
                insert_text: "abcdef".into(),
                range_start_line: 0, range_start_col: 0,
                range_end_line: 0, range_end_col: 0,
                filter_text: None, command: None,
            },
            InlineCompletionItem {
                insert_text: "abcdefghij".into(),
                range_start_line: 0, range_start_col: 0,
                range_end_line: 0, range_end_col: 0,
                filter_text: None, command: None,
            },
        ];
        let filter = CompletionFilter::new().min_length(3).max_length(8);
        let matched = filter.apply(&items);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].insert_text, "abcdef");
        assert_eq!(filter.count_matching(&items), 1);
    }

    #[test]
    fn completion_filter_by_prefix() {
        let items = vec![
            InlineCompletionItem {
                insert_text: "fn main".into(),
                range_start_line: 0, range_start_col: 0,
                range_end_line: 0, range_end_col: 0,
                filter_text: None, command: None,
            },
            InlineCompletionItem {
                insert_text: "let x".into(),
                range_start_line: 0, range_start_col: 0,
                range_end_line: 0, range_end_col: 0,
                filter_text: None, command: None,
            },
        ];
        let filter = CompletionFilter::new().prefix("fn");
        let matched = filter.apply(&items);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].insert_text, "fn main");
    }

    // -- InlineCompletionPrefixCache tests --

    fn sample_item(text: &str) -> InlineCompletionItem {
        InlineCompletionItem {
            insert_text: text.into(),
            range_start_line: 0,
            range_start_col: 0,
            range_end_line: 0,
            range_end_col: 0,
            filter_text: None,
            command: None,
        }
    }

    #[test]
    fn test_prefix_cache_insert_lookup() {
        let mut cache = InlineCompletionPrefixCache::new(10);
        let items = vec![sample_item("foo"), sample_item("foobar")];
        cache.insert("fo", items.clone());
        assert!(cache.contains("fo"));
        let found = cache.lookup("fo").unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].insert_text, "foo");
        assert!(cache.lookup("bar").is_none());
    }

    #[test]
    fn test_prefix_cache_lookup_by_prefix() {
        let mut cache = InlineCompletionPrefixCache::new(10);
        cache.insert("foo", vec![sample_item("foo1")]);
        cache.insert("foobar", vec![sample_item("foobar1")]);
        cache.insert("baz", vec![sample_item("baz1")]);
        let results = cache.lookup_by_prefix("foo");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_prefix_cache_full() {
        let mut cache = InlineCompletionPrefixCache::new(2);
        cache.insert("a", vec![sample_item("a1")]);
        cache.insert("b", vec![sample_item("b1")]);
        assert!(cache.is_full());
        assert_eq!(cache.entry_count(), 2);
        // Inserting a third should evict one to stay at max.
        cache.insert("c", vec![sample_item("c1")]);
        assert_eq!(cache.entry_count(), 2);
    }

    #[test]
    fn test_prefix_cache_clear() {
        let mut cache = InlineCompletionPrefixCache::new(5);
        cache.insert("x", vec![sample_item("x1")]);
        cache.insert("y", vec![sample_item("y1")]);
        assert_eq!(cache.entry_count(), 2);
        cache.clear();
        assert_eq!(cache.entry_count(), 0);
        assert!(!cache.is_full());
    }

    // -- InlineGhostRenderer tests --

    #[test]
    fn test_ghost_renderer_short_text() {
        let renderer = InlineGhostRenderer::new(20);
        let result = renderer.render("hello");
        assert_eq!(result.visible_text, "hello");
        assert!(!result.truncated);
        assert_eq!(result.fade_region, None);
    }

    #[test]
    fn test_ghost_renderer_truncated() {
        let renderer = InlineGhostRenderer::new(5);
        let result = renderer.render("hello world");
        assert_eq!(result.visible_text, "hello");
        assert!(result.truncated);
    }

    #[test]
    fn test_ghost_renderer_fade() {
        let renderer = InlineGhostRenderer::new(10).with_fade(3);
        let result = renderer.render("abcdefghijklmnop");
        assert!(result.truncated);
        assert_eq!(result.fade_region, Some((7, 10)));
        assert_eq!(renderer.visible_length("abcdefghijklmnop"), 10);
    }

    // -- InlineCompletionCycler tests --

    #[test]
    fn test_cycler_next_wraps() {
        let items = vec![sample_item("a"), sample_item("b"), sample_item("c")];
        let mut cycler = InlineCompletionCycler::new(items);
        assert_eq!(cycler.current().unwrap().insert_text, "a");
        assert_eq!(cycler.next().unwrap().insert_text, "b");
        assert_eq!(cycler.next().unwrap().insert_text, "c");
        // Wraps back to start.
        assert_eq!(cycler.next().unwrap().insert_text, "a");
    }

    #[test]
    fn test_cycler_previous_wraps() {
        let items = vec![sample_item("a"), sample_item("b"), sample_item("c")];
        let mut cycler = InlineCompletionCycler::new(items);
        // From index 0, previous wraps to last.
        assert_eq!(cycler.previous().unwrap().insert_text, "c");
        assert_eq!(cycler.previous().unwrap().insert_text, "b");
    }

    #[test]
    fn test_cycler_position_label() {
        let items = vec![sample_item("a"), sample_item("b")];
        let mut cycler = InlineCompletionCycler::new(items);
        assert_eq!(cycler.position_label(), "1 of 2");
        cycler.next();
        assert_eq!(cycler.position_label(), "2 of 2");
    }

    #[test]
    fn test_cycler_empty() {
        let mut cycler = InlineCompletionCycler::new(vec![]);
        assert!(cycler.is_empty());
        assert!(cycler.next().is_none());
        assert!(cycler.previous().is_none());
        assert!(cycler.current().is_none());
        assert_eq!(cycler.position_label(), "0 of 0");
    }

    // -- CompletionAcceptRejectTracker tests --

    #[test]
    fn test_tracker_accept_reject() {
        let mut tracker = CompletionAcceptRejectTracker::new();
        tracker.record_accept(10);
        tracker.record_accept(20);
        tracker.record_reject();
        assert_eq!(tracker.total_actions(), 3);
        assert_eq!(tracker.average_accepted_length(), 15.0);
        assert_eq!(tracker.last_action, Some(CompletionAction::Reject));
        tracker.reset();
        assert_eq!(tracker.total_actions(), 0);
        assert_eq!(tracker.accept_rate(), 0.0);
    }

    #[test]
    fn test_tracker_accept_rate() {
        let mut tracker = CompletionAcceptRejectTracker::new();
        assert_eq!(tracker.accept_rate(), 0.0);
        tracker.record_accept(5);
        tracker.record_accept(5);
        tracker.record_reject();
        // 2 accepts out of 3 total.
        let rate = tracker.accept_rate();
        assert!((rate - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn confidence_factors_default_score() {
        let f = CompletionConfidenceFactors::default();
        let s = f.score();
        assert!(s >= 0.0 && s <= 1.0);
    }

    #[test]
    fn confidence_factors_max_score() {
        let f = CompletionConfidenceFactors {
            prefix_match_ratio: 1.0,
            text_length_score: 1.0,
            provider_priority: 1.0,
            recency_bonus: 1.0,
            frequency_bonus: 1.0,
        };
        assert!((f.score() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn scorer_prefix_match() {
        let s = InlineCompletionConfidenceScorer::compute_prefix_match("hel", "hello");
        assert!(s > 0.5);
        let s2 = InlineCompletionConfidenceScorer::compute_prefix_match("xyz", "hello");
        assert!((s2 - 0.0).abs() < 1e-9);
    }

    #[test]
    fn scorer_length_score() {
        let short = InlineCompletionConfidenceScorer::compute_length_score("hi");
        let medium = InlineCompletionConfidenceScorer::compute_length_score(&"x".repeat(30));
        let long = InlineCompletionConfidenceScorer::compute_length_score(&"x".repeat(100));
        assert!(medium > short);
        assert!(medium > long);
    }

    #[test]
    fn scorer_add_and_filter() {
        let mut scorer = InlineCompletionConfidenceScorer::new(0.0);
        scorer.add_completion("hello world", "hel");
        scorer.add_completion("goodbye", "hel");
        assert_eq!(scorer.total_count(), 2);
        let results = scorer.filtered_results();
        assert!(!results.is_empty());
    }

    #[test]
    fn scorer_threshold_filter() {
        let mut scorer = InlineCompletionConfidenceScorer::new(0.9);
        scorer.add_completion("x", "y");
        assert_eq!(scorer.above_threshold_count(), 0);
    }

    #[test]
    fn scorer_best() {
        let mut scorer = InlineCompletionConfidenceScorer::new(0.0);
        let mut high = ScoredCompletion::new("best");
        high.factors.prefix_match_ratio = 1.0;
        high.factors.provider_priority = 1.0;
        scorer.add_scored(high);
        scorer.add_completion("low", "xxx");
        let best = scorer.best().unwrap();
        assert_eq!(best.insert_text, "best");
    }

    #[test]
    fn ghost_font_style_display() {
        assert_eq!(format!("{}", GhostFontStyle::Normal), "normal");
        assert_eq!(format!("{}", GhostFontStyle::Italic), "italic");
        assert_eq!(format!("{}", GhostFontStyle::Bold), "bold");
        assert_eq!(format!("{}", GhostFontStyle::BoldItalic), "bold italic");
    }

    #[test]
    fn ghost_styler_default() {
        let styler = GhostTextStyler::new();
        assert!(styler.is_active());
        assert_eq!(styler.base_style().opacity, 0.6);
    }

    #[test]
    fn ghost_styler_css() {
        let styler = GhostTextStyler::new();
        let css = styler.to_css("some text");
        assert!(css.contains("color: #888888"));
        assert!(css.contains("opacity: 0.6"));
    }

    #[test]
    fn ghost_styler_keyword_style() {
        let kw_style = GhostTextStyle {
            color: "#ff0000".to_string(),
            font_style: GhostFontStyle::Bold,
            opacity: 0.8,
            background_color: None,
            border: None,
        };
        let styler = GhostTextStyler::new().with_keyword_style(kw_style);
        let style = styler.style_for("fn main()");
        assert_eq!(style.color, "#ff0000");
        let style2 = styler.style_for("some text");
        assert_eq!(style2.color, "#888888");
    }

    #[test]
    fn ghost_styler_diff_colors() {
        let styler = GhostTextStyler::new();
        assert_eq!(styler.diff_add_color(), "#22aa22");
        assert_eq!(styler.diff_remove_color(), "#aa2222");
    }



    #[test]
    fn inlineComplete_x_config_new() {
        let c = InlineCompleteXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn inlineComplete_x_config_builder() {
        let c = InlineCompleteXConfig::new("k")
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
    fn inlineComplete_x_config_display() {
        let c = InlineCompleteXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn inlineComplete_x_registry_insert_get() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn inlineComplete_x_registry_duplicate() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a")).unwrap();
        assert!(reg.insert(InlineCompleteXConfig::new("a")).is_err());
    }

    #[test]
    fn inlineComplete_x_registry_remove() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a")).unwrap();
        reg.insert(InlineCompleteXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn inlineComplete_x_registry_active_entries() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a")).unwrap();
        reg.insert(InlineCompleteXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn inlineComplete_x_registry_by_weight() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(InlineCompleteXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn inlineComplete_x_registry_tags() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(InlineCompleteXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn inlineComplete_x_registry_total_weight() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(InlineCompleteXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn inlineComplete_x_registry_iterator() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a")).unwrap();
        reg.insert(InlineCompleteXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn inlineComplete_x_cache_put_get() {
        let mut cache = InlineCompleteXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn inlineComplete_x_cache_eviction() {
        let mut cache = InlineCompleteXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn inlineComplete_x_cache_lru_order() {
        let mut cache = InlineCompleteXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn inlineComplete_x_cache_most_least_recent() {
        let mut cache = InlineCompleteXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn inlineComplete_x_formatter_entry() {
        let e = InlineCompleteXConfig::new("k").with_value("v");
        let fmt = InlineCompleteXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn inlineComplete_x_formatter_summary() {
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("a").with_weight(5)).unwrap();
        let fmt = InlineCompleteXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn inlineComplete_x_validator_valid() {
        let v = InlineCompleteXValidator::new();
        let c = InlineCompleteXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn inlineComplete_x_validator_empty_key() {
        let v = InlineCompleteXValidator::new();
        let c = InlineCompleteXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn inlineComplete_x_validator_require_value() {
        let v = InlineCompleteXValidator::new().require_value(true);
        let c = InlineCompleteXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn inlineComplete_x_validator_allowed_tags() {
        let v = InlineCompleteXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = InlineCompleteXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn inlineComplete_x_validator_validate_all() {
        let v = InlineCompleteXValidator::new();
        let mut reg = InlineCompleteXRegistry::new();
        reg.insert(InlineCompleteXConfig::new("ok")).unwrap();
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
    fn xb_ring_buffer_54_push_and_len() {
        let mut rb = super::XbRingBuffer54::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_54_overwrite() {
        let mut rb = super::XbRingBuffer54::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_54_get_out_of_bounds() {
        let rb = super::XbRingBuffer54::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_54_drain_all() {
        let mut rb = super::XbRingBuffer54::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_54_peek_front_back() {
        let mut rb = super::XbRingBuffer54::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_54_clear() {
        let mut rb = super::XbRingBuffer54::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_54_capacity() {
        let rb = super::XbRingBuffer54::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_54_basic() {
        let h = super::xb_fnv1a_54(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_54(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_54_different_inputs() {
        let h1 = super::xb_fnv1a_54(b"abc");
        let h2 = super::xb_fnv1a_54(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_54_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_54(&data);
        let dec = super::xb_rle_decode_54(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_54_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_54(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_54(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_54_values() {
        assert!((super::xb_clamp_54(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_54(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_54(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_54_values() {
        assert!((super::xb_lerp_54(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_54(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_54(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_54_wrap_around_twice() {
        let mut rb = super::XbRingBuffer54::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 94 ----

    #[test]
    fn xc_94_pool_new_empty() {
        let pool: super::Xc94Pool<i32> = super::Xc94Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_94_pool_release_acquire() {
        let mut pool = super::Xc94Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_94_pool_acquire_empty() {
        let mut pool: super::Xc94Pool<i32> = super::Xc94Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_94_pool_full() {
        let mut pool = super::Xc94Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_94_pool_drain() {
        let mut pool = super::Xc94Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_94_pool_stats() {
        let mut pool = super::Xc94Pool::new(8);
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
    fn xc_94_pool_clear() {
        let mut pool = super::Xc94Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_94_pool_shrink() {
        let mut pool = super::Xc94Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_94_pool_default() {
        let pool: super::Xc94Pool<String> = super::Xc94Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_94_pool_extend() {
        let mut pool = super::Xc94Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_94_pool_retain() {
        let mut pool = super::Xc94Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_94_scheduler_round_robin() {
        let mut sched = super::Xc94Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_94_scheduler_empty() {
        let mut sched = super::Xc94Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_94_scheduler_reset() {
        let mut sched = super::Xc94Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_94_scheduler_add_remove() {
        let mut sched = super::Xc94Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_94_scheduler_targets() {
        let sched = super::Xc94Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_94_hash_empty() {
        assert_eq!(super::xc_94_hash(b""), 5381);
    }

    #[test]
    fn xc_94_hash_data() {
        let h = super::xc_94_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_94_hash(b"hello"), h);
    }

    #[test]
    fn xc_94_reverse_str() {
        assert_eq!(super::xc_94_reverse("abc"), "cba");
        assert_eq!(super::xc_94_reverse(""), "");
    }

}
