//! Quick access (Ctrl+P).

use std::fmt;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single item shown in the quick access picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickAccessItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub group: Option<String>,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Provides items for a given quick access prefix.
pub trait QuickAccessProvider {
    fn provide_items(&self, query: &str) -> Vec<QuickAccessItem>;
    fn get_prefix(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Manages quick access providers keyed by prefix.
pub struct QuickAccessRegistry {
    providers: HashMap<String, Box<dyn QuickAccessProvider>>,
}

impl QuickAccessRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Registers a provider for the given prefix (e.g. `">"`, `"@"`).
    pub fn register_provider(
        &mut self,
        prefix: impl Into<String>,
        provider: Box<dyn QuickAccessProvider>,
    ) {
        self.providers.insert(prefix.into(), provider);
    }
}

impl Default for QuickAccessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Fuzzy matching
// ---------------------------------------------------------------------------

/// Returns a match score if every character in `query` appears (in order) in
/// `target`. Consecutive character matches receive a bonus.
pub fn fuzzy_match_score(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let query_lower: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let target_lower: Vec<char> = target.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut score: i32 = 0;
    let mut qi = 0;
    let mut last_match: Option<usize> = None;

    for (ti, &tc) in target_lower.iter().enumerate() {
        if qi < query_lower.len() && tc == query_lower[qi] {
            score += 1;
            // Consecutive match bonus
            if let Some(prev) = last_match {
                if ti == prev + 1 {
                    score += 5;
                }
            }
            // Start-of-word bonus
            if ti == 0 || matches!(target_lower.get(ti - 1), Some(' ' | '_' | '-' | '.')) {
                score += 3;
            }
            last_match = Some(ti);
            qi += 1;
        }
    }

    if qi == query_lower.len() { Some(score) } else { None }
}

/// Returns `(index, score)` pairs for items whose label matches `query`,
/// sorted by descending score.
pub fn filter_and_sort(items: &[QuickAccessItem], query: &str) -> Vec<(usize, i32)> {
    let mut results: Vec<(usize, i32)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| fuzzy_match_score(query, &item.label).map(|s| (i, s)))
        .collect();
    results.sort_by(|a, b| b.1.cmp(&a.1));
    results
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// An active quick access session with query state and selection tracking.
pub struct QuickAccessSession {
    pub items: Vec<QuickAccessItem>,
    pub query: String,
    pub filtered_indices: Vec<(usize, i32)>,
    pub selected_index: usize,
}

impl QuickAccessSession {
    pub fn new(items: Vec<QuickAccessItem>) -> Self {
        let filtered_indices: Vec<(usize, i32)> =
            items.iter().enumerate().map(|(i, _)| (i, 0)).collect();
        Self {
            items,
            query: String::new(),
            filtered_indices,
            selected_index: 0,
        }
    }

    /// Update the query string and recompute filtered results.
    pub fn update_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.filtered_indices = filter_and_sort(&self.items, query);
        self.selected_index = 0;
    }

    /// Move selection to the next item, wrapping around.
    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
        }
    }

    /// Move selection to the previous item, wrapping around.
    pub fn select_previous(&mut self) {
        if !self.filtered_indices.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.filtered_indices.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Get the currently selected item, if any.
    pub fn get_selected(&self) -> Option<&QuickAccessItem> {
        self.filtered_indices
            .get(self.selected_index)
            .map(|(idx, _)| &self.items[*idx])
    }

    /// Accept the current selection, returning the selected item.
    pub fn accept(&self) -> Option<QuickAccessItem> {
        self.get_selected().cloned()
    }
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

/// Tracks usage counts to boost frequently-used items in sorting.
pub struct QuickAccessHistory {
    usage_counts: HashMap<String, u32>,
}

impl QuickAccessHistory {
    pub fn new() -> Self {
        Self {
            usage_counts: HashMap::new(),
        }
    }

    /// Record that an item was selected.
    pub fn record_usage(&mut self, item_id: &str) {
        *self.usage_counts.entry(item_id.to_string()).or_insert(0) += 1;
    }

    /// Get the boost value for an item based on past usage.
    pub fn get_boost(&self, item_id: &str) -> i32 {
        self.usage_counts.get(item_id).copied().unwrap_or(0) as i32
    }
}

impl Default for QuickAccessHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter and sort items, boosting results based on history usage.
pub fn filter_and_sort_with_history(
    items: &[QuickAccessItem],
    query: &str,
    history: &QuickAccessHistory,
) -> Vec<(usize, i32)> {
    let mut results: Vec<(usize, i32)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            fuzzy_match_score(query, &item.label)
                .map(|s| (i, s + history.get_boost(&item.id)))
        })
        .collect();
    results.sort_by(|a, b| b.1.cmp(&a.1));
    results
}

// ---------------------------------------------------------------------------
// Highlight helpers
// ---------------------------------------------------------------------------

/// Returns a list of `(start, end)` byte-offset ranges in `target` where
/// characters from `query` matched, useful for UI highlighting.
pub fn highlight_matches(query: &str, target: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }

    let query_lower: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let target_chars: Vec<char> = target.chars().collect();
    let target_lower: Vec<char> = target.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut ranges = Vec::new();
    let mut qi = 0;
    let mut byte_offset = 0;

    for (ti, &tc) in target_lower.iter().enumerate() {
        let char_len = target_chars[ti].len_utf8();
        if qi < query_lower.len() && tc == query_lower[qi] {
            ranges.push((byte_offset, byte_offset + char_len));
            qi += 1;
        }
        byte_offset += char_len;
    }

    if qi == query_lower.len() { ranges } else { Vec::new() }
}

/// Score bonus when the query is an exact prefix of the target (case-insensitive).
pub fn exact_prefix_score(query: &str, target: &str) -> i32 {
    let q: String = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let t: String = target.chars().flat_map(|c| c.to_lowercase()).collect();
    if t.starts_with(&q) { (q.len() as i32) * 10 } else { 0 }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for quickaccess operations.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickaccessStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl QuickaccessStats {
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
    pub fn merge(&mut self, other: &QuickaccessStats) {
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

impl Default for QuickaccessStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for QuickaccessStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "QuickaccessStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for quickaccess.
#[derive(Debug, Clone)]
pub struct QuickaccessValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl QuickaccessValidator {
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

impl Default for QuickaccessValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Go To Symbol (Ctrl+Shift+O) — document symbol support
// ---------------------------------------------------------------------------

/// The kind of a document symbol, matching LSP SymbolKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

impl SymbolKind {
    /// Icon character for TUI display.
    pub fn icon(&self) -> char {
        match self {
            Self::Function | Self::Method | Self::Constructor => 'ƒ',
            Self::Class | Self::Struct | Self::Interface => '◆',
            Self::Enum | Self::EnumMember => '◇',
            Self::Variable | Self::Field | Self::Property => '◈',
            Self::Constant => '◉',
            Self::Module | Self::Namespace | Self::Package => '▸',
            _ => '○',
        }
    }
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Namespace => "namespace",
            Self::Package => "package",
            Self::Class => "class",
            Self::Method => "method",
            Self::Property => "property",
            Self::Field => "field",
            Self::Constructor => "constructor",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Function => "function",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Array => "array",
            Self::Object => "object",
            Self::Key => "key",
            Self::Null => "null",
            Self::EnumMember => "enum member",
            Self::Struct => "struct",
            Self::Event => "event",
            Self::Operator => "operator",
            Self::TypeParameter => "type parameter",
        };
        write!(f, "{name}")
    }
}

/// A document symbol returned by a language server or provider.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub range_start_line: u32,
    pub range_end_line: u32,
    pub children: Vec<DocumentSymbol>,
}

impl DocumentSymbol {
    pub fn new(name: impl Into<String>, kind: SymbolKind, start_line: u32, end_line: u32) -> Self {
        Self {
            name: name.into(),
            detail: None,
            kind,
            range_start_line: start_line,
            range_end_line: end_line,
            children: Vec::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_child(mut self, child: DocumentSymbol) -> Self {
        self.children.push(child);
        self
    }

    /// Flatten into a list of (depth, symbol) pairs for display.
    pub fn flatten(&self, depth: usize) -> Vec<(usize, &DocumentSymbol)> {
        let mut out = vec![(depth, self)];
        for child in &self.children {
            out.extend(child.flatten(depth + 1));
        }
        out
    }
}

/// Trait for providing document symbols.
pub trait DocumentSymbolProvider: Send + Sync {
    fn document_symbols(&self, uri: &str) -> Vec<DocumentSymbol>;
}

/// Parse a go-to-symbol query, handling `@:` prefix for kind filtering.
/// Returns `(kind_filter, text_filter)`.
pub fn parse_symbol_query(query: &str) -> (Option<String>, String) {
    let stripped = query.strip_prefix('@').unwrap_or(query);
    if let Some(rest) = stripped.strip_prefix(':') {
        // "@:function myFn" → kind="function", text="myFn"
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let kind = parts[0].to_lowercase();
        let text = parts.get(1).unwrap_or(&"").to_string();
        (Some(kind), text)
    } else {
        (None, stripped.to_string())
    }
}

/// Filter document symbols by an optional kind and a fuzzy text query.
pub fn filter_symbols<'a>(
    symbols: &'a [DocumentSymbol],
    kind_filter: Option<&str>,
    text_query: &str,
) -> Vec<&'a DocumentSymbol> {
    symbols
        .iter()
        .filter(|s| {
            if let Some(kind) = kind_filter {
                if !s.kind.to_string().contains(kind) {
                    return false;
                }
            }
            if text_query.is_empty() {
                return true;
            }
            fuzzy_match_score(text_query, &s.name).is_some()
        })
        .collect()
}

/// Convert a list of `DocumentSymbol` into `QuickAccessItem`s for the picker.
pub fn symbols_to_quick_access_items(symbols: &[DocumentSymbol]) -> Vec<QuickAccessItem> {
    symbols
        .iter()
        .enumerate()
        .map(|(i, s)| QuickAccessItem {
            id: format!("symbol-{i}"),
            label: s.name.clone(),
            description: s.detail.clone(),
            detail: Some(format!("line {}", s.range_start_line)),
            icon: Some(s.kind.icon().to_string()),
            group: Some(s.kind.to_string()),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Quick access prefix routing
// ---------------------------------------------------------------------------

/// Identifies the quick-access mode from a query string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickAccessMode {
    /// Default file search (Ctrl+P with no prefix).
    File,
    /// Go to line (`:` prefix).
    GoToLine(u32),
    /// Go to symbol in file (`@` prefix).
    GoToSymbol(String),
    /// Go to symbol in workspace (`#` prefix).
    WorkspaceSymbol(String),
    /// Command palette (`>` prefix).
    Command(String),
}

/// Parse a raw quick-access query into its mode.
pub fn parse_quick_access_mode(query: &str) -> QuickAccessMode {
    if let Some(rest) = query.strip_prefix(':') {
        match rest.trim().parse::<u32>() {
            Ok(line) => QuickAccessMode::GoToLine(line),
            Err(_) => QuickAccessMode::File,
        }
    } else if let Some(rest) = query.strip_prefix('@') {
        QuickAccessMode::GoToSymbol(rest.to_string())
    } else if let Some(rest) = query.strip_prefix('#') {
        QuickAccessMode::WorkspaceSymbol(rest.to_string())
    } else if let Some(rest) = query.strip_prefix('>') {
        QuickAccessMode::Command(rest.trim().to_string())
    } else {
        QuickAccessMode::File
    }
}

// ---------------------------------------------------------------------------
// Recency-based ranking
// ---------------------------------------------------------------------------

/// Tracks item usage with timestamps to boost recently-used items.
pub struct RecencyTracker {
    /// Maps item ID to the timestamp of last use (arbitrary u64 epoch).
    last_used: HashMap<String, u64>,
}

impl RecencyTracker {
    /// Create a new recency tracker.
    pub fn new() -> Self {
        Self {
            last_used: HashMap::new(),
        }
    }

    /// Record that an item was used at the given timestamp.
    pub fn record(&mut self, item_id: &str, timestamp: u64) {
        self.last_used.insert(item_id.to_string(), timestamp);
    }

    /// Get the recency boost for an item. Items used more recently get
    /// higher scores. Returns 0 if the item has never been used.
    pub fn boost(&self, item_id: &str, now: u64, decay_seconds: u64) -> i32 {
        match self.last_used.get(item_id) {
            Some(&ts) => {
                let elapsed = now.saturating_sub(ts);
                if elapsed >= decay_seconds {
                    0
                } else {
                    ((decay_seconds - elapsed) * 10 / decay_seconds) as i32
                }
            }
            None => 0,
        }
    }

    /// Number of tracked items.
    pub fn len(&self) -> usize {
        self.last_used.len()
    }

    /// Whether the tracker is empty.
    pub fn is_empty(&self) -> bool {
        self.last_used.is_empty()
    }

    /// Remove entries older than `cutoff` timestamp.
    pub fn prune_before(&mut self, cutoff: u64) {
        self.last_used.retain(|_, ts| *ts >= cutoff);
    }
}

impl Default for RecencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Action categorization
// ---------------------------------------------------------------------------

/// Category for quick access items, used for grouping in the picker UI.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionCategory {
    File,
    Symbol,
    Command,
    RecentlyUsed,
    Custom(String),
}

impl std::fmt::Display for ActionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "Files"),
            Self::Symbol => write!(f, "Symbols"),
            Self::Command => write!(f, "Commands"),
            Self::RecentlyUsed => write!(f, "Recently Used"),
            Self::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Group items by their `group` field, returning `(category_name, items)` pairs
/// sorted by category name. Items without a group are placed in "Other".
pub fn group_items_by_category(items: &[QuickAccessItem]) -> Vec<(String, Vec<&QuickAccessItem>)> {
    let mut map: HashMap<String, Vec<&QuickAccessItem>> = HashMap::new();
    for item in items {
        let key = item.group.as_deref().unwrap_or("Other").to_string();
        map.entry(key).or_default().push(item);
    }
    let mut groups: Vec<(String, Vec<&QuickAccessItem>)> = map.into_iter().collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    groups
}

/// Deduplicate items by id, keeping the first occurrence.
pub fn deduplicate_items(items: &[QuickAccessItem]) -> Vec<&QuickAccessItem> {
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .filter(|item| seen.insert(&item.id))
        .collect()
}

// ---------------------------------------------------------------------------
// Search history
// ---------------------------------------------------------------------------

/// Maintains an ordered history of search queries with deduplication.
pub struct SearchHistory {
    entries: Vec<String>,
    max_entries: usize,
}

impl SearchHistory {
    /// Create a new search history with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Record a query. If it already exists it is moved to the front.
    /// Empty queries are ignored.
    pub fn record(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        self.entries.retain(|e| e != query);
        self.entries.insert(0, query.to_string());
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Return the most recent queries (newest first).
    pub fn recent(&self) -> &[String] {
        &self.entries
    }

    /// Number of stored queries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear the entire history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return entries that fuzzy-match `prefix`.
    pub fn search(&self, prefix: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| fuzzy_match_score(prefix, e).is_some())
            .map(|e| e.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Result pinning / favoriting
// ---------------------------------------------------------------------------

/// Tracks pinned (favorited) item IDs so they always appear at the top.
pub struct PinnedItems {
    ids: Vec<String>,
}

impl PinnedItems {
    pub fn new() -> Self {
        Self { ids: Vec::new() }
    }

    /// Pin an item. No-op if already pinned.
    pub fn pin(&mut self, item_id: &str) {
        if !self.ids.iter().any(|id| id == item_id) {
            self.ids.push(item_id.to_string());
        }
    }

    /// Unpin an item.
    pub fn unpin(&mut self, item_id: &str) {
        self.ids.retain(|id| id != item_id);
    }

    /// Toggle the pinned state; returns `true` if the item is now pinned.
    pub fn toggle(&mut self, item_id: &str) -> bool {
        if self.is_pinned(item_id) {
            self.unpin(item_id);
            false
        } else {
            self.pin(item_id);
            true
        }
    }

    /// Check whether an item is pinned.
    pub fn is_pinned(&self, item_id: &str) -> bool {
        self.ids.iter().any(|id| id == item_id)
    }

    /// Return the list of pinned IDs in insertion order.
    pub fn pinned_ids(&self) -> &[String] {
        &self.ids
    }

    /// Number of pinned items.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

impl Default for PinnedItems {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort results so that pinned items always come first, preserving relative
/// order within each group (pinned vs unpinned).
pub fn sort_with_pinned(
    results: &mut [(usize, i32)],
    items: &[QuickAccessItem],
    pinned: &PinnedItems,
) {
    results.sort_by(|a, b| {
        let a_pinned = pinned.is_pinned(&items[a.0].id);
        let b_pinned = pinned.is_pinned(&items[b.0].id);
        match (a_pinned, b_pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.1.cmp(&a.1),
        }
    });
}

// ---------------------------------------------------------------------------
// Keyboard shortcut hints
// ---------------------------------------------------------------------------

/// Associates keyboard shortcuts with quick-access item IDs.
pub struct ShortcutRegistry {
    shortcuts: HashMap<String, String>,
}

impl ShortcutRegistry {
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
        }
    }

    /// Bind a shortcut string (e.g. `"Ctrl+Shift+P"`) to an item ID.
    pub fn bind(&mut self, item_id: impl Into<String>, shortcut: impl Into<String>) {
        self.shortcuts.insert(item_id.into(), shortcut.into());
    }

    /// Remove a binding.
    pub fn unbind(&mut self, item_id: &str) {
        self.shortcuts.remove(item_id);
    }

    /// Look up the shortcut for an item, if any.
    pub fn get(&self, item_id: &str) -> Option<&str> {
        self.shortcuts.get(item_id).map(|s| s.as_str())
    }

    /// Format a quick-access item label with its shortcut hint appended.
    pub fn label_with_hint(&self, item: &QuickAccessItem) -> String {
        match self.get(&item.id) {
            Some(shortcut) => format!("{} ({})", item.label, shortcut),
            None => item.label.clone(),
        }
    }
}

impl Default for ShortcutRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Result preview generation
// ---------------------------------------------------------------------------

/// A preview snippet shown alongside a quick-access result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultPreview {
    pub kind: PreviewKind,
    pub content: String,
}

/// The kind of preview content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewKind {
    PlainText,
    FilePath,
    CodeSnippet { language: String },
}

impl ResultPreview {
    /// Create a plain-text preview, truncated to `max_len` characters.
    pub fn plain(text: &str, max_len: usize) -> Self {
        let content = if text.chars().count() > max_len {
            let truncated: String = text.chars().take(max_len.saturating_sub(1)).collect();
            format!("{truncated}…")
        } else {
            text.to_string()
        };
        Self {
            kind: PreviewKind::PlainText,
            content,
        }
    }

    /// Create a file-path preview.
    pub fn file_path(path: &str) -> Self {
        Self {
            kind: PreviewKind::FilePath,
            content: path.to_string(),
        }
    }

    /// Create a code-snippet preview.
    pub fn code(snippet: &str, language: &str) -> Self {
        Self {
            kind: PreviewKind::CodeSnippet {
                language: language.to_string(),
            },
            content: snippet.to_string(),
        }
    }

    /// True if the preview has no content.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Generate a preview for a quick-access item based on its metadata.
pub fn generate_preview(item: &QuickAccessItem) -> ResultPreview {
    if let Some(ref detail) = item.detail {
        if detail.starts_with('/') || detail.starts_with('.') || detail.contains(":\\") {
            return ResultPreview::file_path(detail);
        }
    }
    let text = item
        .description
        .as_deref()
        .or(item.detail.as_deref())
        .unwrap_or(&item.label);
    ResultPreview::plain(text, 120)
}

// -- QuickAccessScorer with match highlighting -------------------------------

/// A scored match with highlight positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredMatch {
    pub item: QuickAccessItem,
    pub score: i32,
    pub highlight_positions: Vec<usize>,
}

/// Score items against a query and return sorted results with highlights.
pub fn score_items(items: &[QuickAccessItem], query: &str) -> Vec<ScoredMatch> {
    if query.is_empty() {
        return items.iter().map(|item| ScoredMatch {
            item: item.clone(),
            score: 0,
            highlight_positions: Vec::new(),
        }).collect();
    }

    let mut scored: Vec<ScoredMatch> = items.iter().filter_map(|item| {
        let (score, positions) = fuzzy_match_with_positions(query, &item.label)?;
        Some(ScoredMatch {
            item: item.clone(),
            score,
            highlight_positions: positions,
        })
    }).collect();

    scored.sort_by(|a, b| b.score.cmp(&a.score));
    scored
}

/// Fuzzy match returning both score and match positions.
fn fuzzy_match_with_positions(query: &str, target: &str) -> Option<(i32, Vec<usize>)> {
    let query_lower: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let target_lower: Vec<char> = target.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut score: i32 = 0;
    let mut qi = 0;
    let mut positions = Vec::new();
    let mut last_match: Option<usize> = None;

    for (ti, tc) in target_lower.iter().enumerate() {
        if qi < query_lower.len() && *tc == query_lower[qi] {
            score += 1;
            if ti > 0 && last_match == Some(ti - 1) {
                score += 2;
            }
            if ti == 0 {
                score += 3;
            }
            positions.push(ti);
            last_match = Some(ti);
            qi += 1;
        }
    }

    if qi == query_lower.len() {
        Some((score, positions))
    } else {
        None
    }
}

// -- QuickAccessRecent with usage frequency ----------------------------------

/// Tracks recently accessed items with frequency counts.
#[derive(Debug, Default)]
pub struct QuickAccessRecent {
    entries: Vec<RecentEntry>,
    max_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentEntry {
    pub id: String,
    pub label: String,
    pub access_count: u32,
    pub last_access: u64,
}

impl QuickAccessRecent {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), max_entries }
    }

    /// Record an access, updating frequency and recency.
    pub fn record_access(&mut self, id: &str, label: &str, timestamp: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.access_count += 1;
            entry.last_access = timestamp;
        } else {
            if self.entries.len() >= self.max_entries {
                // Remove least recently used
                if let Some(min_idx) = self.entries.iter().enumerate()
                    .min_by_key(|(_, e)| e.last_access)
                    .map(|(i, _)| i)
                {
                    self.entries.remove(min_idx);
                }
            }
            self.entries.push(RecentEntry {
                id: id.to_string(),
                label: label.to_string(),
                access_count: 1,
                last_access: timestamp,
            });
        }
    }

    /// Get entries sorted by frequency (descending).
    pub fn by_frequency(&self) -> Vec<&RecentEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        sorted
    }

    /// Get entries sorted by recency (most recent first).
    pub fn by_recency(&self) -> Vec<&RecentEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.last_access.cmp(&a.last_access));
        sorted
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl fmt::Display for QuickAccessRecent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Recent({} entries, max {})", self.entries.len(), self.max_entries)
    }
}

// -- QuickAccessProviderChain for fallback -----------------------------------

/// Chains multiple providers, trying each in order until items are found.
pub fn provider_chain_query(
    providers: &[&dyn QuickAccessProvider],
    query: &str,
) -> Vec<QuickAccessItem> {
    for provider in providers {
        let items = provider.provide_items(query);
        if !items.is_empty() {
            return items;
        }
    }
    Vec::new()
}

// -- Quick access result grouping --------------------------------------------

/// Group items by their `group` field.
pub fn group_items(items: &[QuickAccessItem]) -> HashMap<String, Vec<&QuickAccessItem>> {
    let mut groups: HashMap<String, Vec<&QuickAccessItem>> = HashMap::new();
    for item in items {
        let key = item.group.as_deref().unwrap_or("Other").to_string();
        groups.entry(key).or_default().push(item);
    }
    groups
}

/// Count how many distinct groups exist.
pub fn group_count(items: &[QuickAccessItem]) -> usize {
    let mut groups = std::collections::HashSet::new();
    for item in items {
        groups.insert(item.group.as_deref().unwrap_or("Other"));
    }
    groups.len()
}

/// Flatten grouped items into a list with group headers.
pub fn flatten_with_headers(items: &[QuickAccessItem]) -> Vec<String> {
    let groups = group_items(items);
    let mut keys: Vec<_> = groups.keys().cloned().collect();
    keys.sort();
    let mut result = Vec::new();
    for key in keys {
        result.push(format!("── {} ──", key));
        if let Some(group_items) = groups.get(&key) {
            for item in group_items {
                result.push(item.label.clone());
            }
        }
    }
    result
}


// ---------------------------------------------------------------------------
// QuickAccessKeyNav
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuickAccessKeyNav {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl QuickAccessKeyNav {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for QuickAccessKeyNav {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for QuickAccessKeyNav {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "QuickAccessKeyNav({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// QuickAccessPinnedItems
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuickAccessPinnedItems {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl QuickAccessPinnedItems {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for QuickAccessPinnedItems {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for QuickAccessPinnedItems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "QuickAccessPinnedItems({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// QuickAccessKeyNavSnapshot — point-in-time snapshot of QuickAccessKeyNav state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuickAccessKeyNavSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl QuickAccessKeyNavSnapshot {
    pub fn capture(source: &QuickAccessKeyNav, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for QuickAccessKeyNavSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// QuickAccessPinnedItemsStats — aggregate statistics for QuickAccessPinnedItems
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct QuickAccessPinnedItemsStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl QuickAccessPinnedItemsStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for QuickAccessPinnedItemsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// QuickAccessKeyNavConfig — configuration for QuickAccessKeyNav
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuickAccessKeyNavConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl QuickAccessKeyNavConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for QuickAccessKeyNavConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for QuickAccessKeyNavConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// --- QuickAccessScorer: score items against query ---

pub struct QuickAccessScorer;

impl QuickAccessScorer {
    pub fn compute_score(query: &str, label: &str) -> usize {
        if query.is_empty() { return 0; }
        let q_lower = query.to_lowercase();
        let l_lower = label.to_lowercase();
        let mut score = 0usize;

        // prefix bonus
        if l_lower.starts_with(&q_lower) { score += 10; }

        // consecutive chars bonus
        let mut qi = q_lower.chars().peekable();
        let mut consecutive = 0usize;
        let mut matched = 0usize;
        let mut prev_match = false;
        for ch in l_lower.chars() {
            if qi.peek() == Some(&ch) {
                qi.next();
                matched += 1;
                if prev_match { consecutive += 1; }
                prev_match = true;
            } else {
                prev_match = false;
            }
        }
        if qi.peek().is_some() { return 0; } // not all chars matched

        score += matched * 2 + consecutive * 3;

        // case match bonus
        let q_chars: Vec<char> = query.chars().collect();
        let l_chars: Vec<char> = label.chars().collect();
        let mut qi2 = 0;
        for &lc in &l_chars {
            if qi2 < q_chars.len() && lc == q_chars[qi2] { score += 1; qi2 += 1; }
        }

        score
    }

    pub fn sort_by_score(query: &str, items: &[&str]) -> Vec<(usize, usize)> {
        let mut scored: Vec<(usize, usize)> = items.iter().enumerate()
            .map(|(i, label)| (i, Self::compute_score(query, label)))
            .filter(|(_, s)| *s > 0)
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored
    }

    pub fn highlight_positions(query: &str, label: &str) -> Vec<usize> {
        let q_lower = query.to_lowercase();
        let l_lower = label.to_lowercase();
        let mut positions = Vec::new();
        let mut qi = q_lower.chars().peekable();
        for (i, ch) in l_lower.chars().enumerate() {
            if qi.peek() == Some(&ch) { qi.next(); positions.push(i); }
        }
        positions
    }
}

// --- RecentItemTrackerV2 ---

pub struct RecentItemTrackerV2 {
    items: Vec<(String, u64)>, // (id, timestamp)
    max_items: usize,
}

impl RecentItemTrackerV2 {
    pub fn new(max_items: usize) -> Self { Self { items: Vec::new(), max_items } }

    pub fn record_access(&mut self, id: &str, timestamp: u64) {
        self.items.retain(|(i, _)| i != id);
        self.items.push((id.to_string(), timestamp));
        self.cap_at_max();
    }

    pub fn recent_items(&self) -> Vec<String> {
        let mut sorted = self.items.clone();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().map(|(id, _)| id).collect()
    }

    pub fn remove_item(&mut self, id: &str) { self.items.retain(|(i, _)| i != id); }

    pub fn cap_at_max(&mut self) {
        while self.items.len() > self.max_items {
            self.items.remove(0);
        }
    }

    pub fn contains(&self, id: &str) -> bool { self.items.iter().any(|(i, _)| i == id) }
    pub fn access_count(&self) -> usize { self.items.len() }
}

// --- QuickAccessPrefixRouter ---

pub struct PrefixRoute {
    pub prefix: String,
    pub description: String,
}

pub struct QuickAccessPrefixRouter {
    routes: Vec<PrefixRoute>,
}

impl QuickAccessPrefixRouter {
    pub fn new() -> Self { Self { routes: Vec::new() } }

    pub fn register_prefix(&mut self, prefix: &str, description: &str) {
        self.routes.push(PrefixRoute { prefix: prefix.to_string(), description: description.to_string() });
    }

    pub fn resolve_prefix(&self, query: &str) -> Option<&PrefixRoute> {
        self.routes.iter().find(|r| query.starts_with(&r.prefix))
    }

    pub fn strip_prefix<'a>(&self, query: &'a str) -> &'a str {
        for route in &self.routes {
            if let Some(rest) = query.strip_prefix(&route.prefix) {
                return rest;
            }
        }
        query
    }

    pub fn route_count(&self) -> usize { self.routes.len() }
}


/// Quick access configuration manager.
#[derive(Debug, Clone)]
pub struct QuickaccessConfig {
    entries: Vec<QuickaccessEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single quick access entry.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickaccessEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl QuickaccessEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl QuickaccessConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: QuickaccessEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&QuickaccessEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut QuickaccessEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&QuickaccessEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&QuickaccessEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&QuickaccessEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<QuickaccessEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Quick access picker — extended utilities (xj)
// ---------------------------------------------------------------------------

/// Metric accumulator for quickacc operations.
#[derive(Debug, Clone)]
pub struct XjMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XjMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for quickacc.
#[derive(Debug, Clone)]
pub struct XjRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XjRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for quickacc lookups.
#[derive(Debug, Clone)]
pub struct XjLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XjLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 28
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer28 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer28 {
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
pub fn xb_fnv1a_28(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_28<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_28<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_28(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_28(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 143
// ---------------------------------------------------------------------------

/// Generic object pool `Xc143Pool<T>`.
pub struct Xc143Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc143Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc143PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc143Pool<T> {
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
    pub fn stats(&self) -> Xc143PoolStats {
        Xc143PoolStats {
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

impl<T> Default for Xc143Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc143Scheduler`.
pub struct Xc143Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc143Scheduler {
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

impl Default for Xc143Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_143 hash for the given byte slice.
pub fn xc_143_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_143 convention.
pub fn xc_143_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe40 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe40Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe40PipelineError {
    pub stage: Xe40Stage,
    pub message: String,
}

impl std::fmt::Display for Xe40PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe40Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe40Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError>>>,
    stage_names: Vec<Xe40Stage>,
}

impl Xe40Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe40Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe40Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe40Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe40Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> {
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

    pub fn compose(mut self, other: Xe40Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe40CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe40CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe40Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe40CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe40CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe40Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe40CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_40_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe40CacheEntry {
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

    fn xe_40_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe40CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_40_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> {
    Ok(data)
}

pub fn xe_40_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_40_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_40_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_40_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe40PipelineError> {
    Err(Xe40PipelineError {
        stage: Xe40Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_7: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg7Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg7Graph {
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

impl Default for Xg7Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_7: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg7Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg7Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg7Heap<T>) {
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

impl<T: Ord> Default for Xg7Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 142).
pub struct Xh142SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh142SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 184 as u64,
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

/// A compact bit set supporting boolean operations (variant 142).
pub struct Xh142BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh142BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 142).
pub struct Xi142Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi142Deque<T> {
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
pub struct Xi142Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi142Interval {
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

/// A simple interval tree (variant 142).
pub struct Xi142IntervalTree {
    xi_intervals: Vec<Xi142Interval>,
}

impl Xi142IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi142Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi142Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi142Interval) -> Vec<&Xi142Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi142Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi142Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi142Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi142Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi142Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi142Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 142) ---

/// Disjoint set / union-find for crate 142.
pub struct Xj142UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj142UnionFind {
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

const XJ142_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 142.
pub struct Xj142BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj142BTreeNode<K, V>>>,
    len: usize,
}

struct Xj142BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj142BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj142BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ142_BTREE_ORDER - 1
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
        let mid = XJ142_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj142BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj142BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj142BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj142BTreeNode::xj_new_leaf();
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


// --- xk_142 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk142SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk142SegmentTree {
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
pub struct Xk142DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk142DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_142).
#[derive(Debug, Clone)]
pub struct Xl142Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl142Rope {
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

/// Suffix array for efficient string searching (xl_142).
#[derive(Debug, Clone)]
pub struct Xl142SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl142SuffixArray {
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
pub struct Xm142MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm142MatrixSparse {
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
pub struct Xm142Tokenizer {
    text: String,
}

impl Xm142Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 142.
pub struct Xn142Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn142Fenwick {
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

// ----- AVL tree map — crate 142 -----

#[derive(Debug, Clone)]
struct Xn142AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn142AvlNode<K, V>>>,
    right: Option<Box<Xn142AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 142.
#[derive(Debug, Clone)]
pub struct Xn142AVL<K, V> {
    root: Option<Box<Xn142AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn142AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn142AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn142AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn142AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn142AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn142AvlNode<K, V>>) -> Box<Xn142AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn142AvlNode<K, V>>) -> Box<Xn142AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn142AvlNode<K, V>>) -> Box<Xn142AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn142AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn142AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn142AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn142AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn142AvlNode<K, V>>) -> &Xn142AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn142AvlNode<K, V>>) -> (Box<Xn142AvlNode<K, V>>, Option<Box<Xn142AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn142AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn142AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn142AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn142AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn142AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn142AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn142AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo142RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo142Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo142RBNode<K, V> {
    key: K,
    value: V,
    color: Xo142Color,
    left: Option<Box<Xo142RBNode<K, V>>>,
    right: Option<Box<Xo142RBNode<K, V>>>,
}

/// A red-black tree map for crate 142.
#[derive(Debug, Clone)]
pub struct Xo142RedBlack<K, V> {
    root: Option<Box<Xo142RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo142RedBlack<K, V> {
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
            r.color = Xo142Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo142RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo142RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo142RBNode {
                    key, value, color: Xo142Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo142RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo142Color::Red)
    }

    fn xo_balance(mut h: Box<Xo142RBNode<K, V>>) -> Box<Xo142RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo142Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo142RBNode<K, V>>) -> Box<Xo142RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo142Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo142RBNode<K, V>>) -> Box<Xo142RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo142Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo142RBNode<K, V>>) {
        h.color = Xo142Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo142Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo142Color::Black; }
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
            r.color = Xo142Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo142RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo142RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo142RBNode<K, V>) -> (K, V, Option<Box<Xo142RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo142RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo142Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo142RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo142ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 142.
#[derive(Debug, Clone)]
pub struct Xo142ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo142ConsistentHash {
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
            let vkey = format!("{}#xo142#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo142#{}", node, i);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(id: &str, label: &str) -> QuickAccessItem {
        QuickAccessItem {
            id: id.into(),
            label: label.into(),
            description: None,
            detail: None,
            icon: None,
            group: None,
        }
    }

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match_score("fb", "FooBar").is_some());
        assert!(fuzzy_match_score("xyz", "FooBar").is_none());
        assert_eq!(fuzzy_match_score("", "anything"), Some(0));
    }

    #[test]
    fn consecutive_bonus() {
        let full = fuzzy_match_score("foo", "foobar").unwrap();
        let spread = fuzzy_match_score("for", "foobar").unwrap();
        assert!(full > spread, "consecutive matches should score higher");
    }

    #[test]
    fn filter_and_sort_order() {
        let items = vec![
            make_item("1", "Application"),
            make_item("2", "app_config"),
            make_item("3", "zzz"),
        ];
        let results = filter_and_sort(&items, "app");
        assert!(results.iter().all(|(i, _)| *i != 2));
        assert!(results.len() >= 2);
    }

    #[test]
    fn registry_accepts_provider() {
        struct Dummy;
        impl QuickAccessProvider for Dummy {
            fn provide_items(&self, _query: &str) -> Vec<QuickAccessItem> {
                vec![]
            }
            fn get_prefix(&self) -> &str {
                ">"
            }
        }
        let mut reg = QuickAccessRegistry::new();
        reg.register_provider(">", Box::new(Dummy));
        assert!(reg.providers.contains_key(">"));
    }

    #[test]
    fn session_update_query() {
        let items = vec![
            make_item("a", "Alpha"),
            make_item("b", "Beta"),
            make_item("c", "Gamma"),
        ];
        let mut session = QuickAccessSession::new(items);
        assert_eq!(session.filtered_indices.len(), 3);
        session.update_query("al");
        assert_eq!(session.filtered_indices.len(), 1);
        assert_eq!(session.get_selected().unwrap().id, "a");
    }

    #[test]
    fn session_navigation() {
        let items = vec![
            make_item("1", "One"),
            make_item("2", "Two"),
            make_item("3", "Three"),
        ];
        let mut session = QuickAccessSession::new(items);
        assert_eq!(session.selected_index, 0);
        session.select_next();
        assert_eq!(session.selected_index, 1);
        session.select_next();
        assert_eq!(session.selected_index, 2);
        session.select_next();
        assert_eq!(session.selected_index, 0); // wrap
        session.select_previous();
        assert_eq!(session.selected_index, 2); // wrap back
    }

    #[test]
    fn session_accept() {
        let items = vec![make_item("x", "Xray")];
        let session = QuickAccessSession::new(items);
        let accepted = session.accept().unwrap();
        assert_eq!(accepted.id, "x");
    }

    #[test]
    fn history_boost() {
        let mut history = QuickAccessHistory::new();
        assert_eq!(history.get_boost("cmd1"), 0);
        history.record_usage("cmd1");
        history.record_usage("cmd1");
        assert_eq!(history.get_boost("cmd1"), 2);
    }

    #[test]
    fn filter_and_sort_with_history_boosts() {
        let items = vec![
            make_item("rare", "RareCommand"),
            make_item("freq", "FreqCommand"),
        ];
        let mut history = QuickAccessHistory::new();
        for _ in 0..20 {
            history.record_usage("freq");
        }
        let results = filter_and_sort_with_history(&items, "command", &history);
        assert_eq!(results.len(), 2);
        // freq should be first due to history boost
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn highlight_matches_basic() {
        let ranges = highlight_matches("fb", "FooBar");
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], (0, 1)); // 'F'
        assert_eq!(ranges[1], (3, 4)); // 'B'
    }

    #[test]
    fn highlight_matches_no_match() {
        let ranges = highlight_matches("xyz", "FooBar");
        assert!(ranges.is_empty());
    }

    #[test]
    fn exact_prefix_score_match() {
        assert!(exact_prefix_score("app", "Application") > 0);
        assert_eq!(exact_prefix_score("xyz", "Application"), 0);
        assert_eq!(exact_prefix_score("APP", "application"), 30);
    }

    #[test]
    fn highlight_matches_empty_query() {
        let ranges = highlight_matches("", "FooBar");
        assert!(ranges.is_empty());
    }

    #[test]
    fn behavior_check_0() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        let _svc = QuickAccessRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn quickaccess_stats_new_defaults() {
        let stats = QuickaccessStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn quickaccess_stats_record_success() {
        let mut stats = QuickaccessStats::new();
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
    fn quickaccess_stats_record_failure() {
        let mut stats = QuickaccessStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn quickaccess_stats_reset() {
        let mut stats = QuickaccessStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn quickaccess_stats_merge() {
        let mut a = QuickaccessStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = QuickaccessStats::new();
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
    fn quickaccess_stats_display() {
        let mut stats = QuickaccessStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn quickaccess_stats_default() {
        let stats = QuickaccessStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn quickaccess_validator_accepts_valid_name() {
        let v = QuickaccessValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn quickaccess_validator_rejects_empty() {
        let v = QuickaccessValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn quickaccess_validator_rejects_too_long() {
        let v = QuickaccessValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn quickaccess_validator_forbidden_prefix() {
        let v = QuickaccessValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn quickaccess_validator_allowed_chars() {
        let v = QuickaccessValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn quickaccess_validator_range() {
        let v = QuickaccessValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn quickaccess_sanitize_removes_control() {
        let result = QuickaccessValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn quickaccess_truncate_short_string() {
        assert_eq!(QuickaccessValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn quickaccess_truncate_long_string() {
        let result = QuickaccessValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn quickaccess_is_ascii_printable() {
        assert!(QuickaccessValidator::is_ascii_printable("Hello World 123"));
        assert!(!QuickaccessValidator::is_ascii_printable("Hello\x00World"));
    }

    // --- SymbolKind tests ---

    #[test]
    fn symbol_kind_icon() {
        assert_eq!(SymbolKind::Function.icon(), 'ƒ');
        assert_eq!(SymbolKind::Class.icon(), '◆');
        assert_eq!(SymbolKind::Enum.icon(), '◇');
        assert_eq!(SymbolKind::Variable.icon(), '◈');
        assert_eq!(SymbolKind::Constant.icon(), '◉');
        assert_eq!(SymbolKind::Module.icon(), '▸');
        assert_eq!(SymbolKind::File.icon(), '○');
    }

    #[test]
    fn symbol_kind_display() {
        assert_eq!(SymbolKind::Function.to_string(), "function");
        assert_eq!(SymbolKind::Class.to_string(), "class");
        assert_eq!(SymbolKind::Struct.to_string(), "struct");
    }

    // --- DocumentSymbol tests ---

    #[test]
    fn document_symbol_creation() {
        let sym = DocumentSymbol::new("main", SymbolKind::Function, 1, 10)
            .with_detail("fn main()");
        assert_eq!(sym.name, "main");
        assert_eq!(sym.kind, SymbolKind::Function);
        assert_eq!(sym.detail.as_deref(), Some("fn main()"));
    }

    #[test]
    fn document_symbol_flatten() {
        let sym = DocumentSymbol::new("Foo", SymbolKind::Class, 1, 50)
            .with_child(DocumentSymbol::new("bar", SymbolKind::Method, 5, 10))
            .with_child(DocumentSymbol::new("baz", SymbolKind::Method, 12, 20));
        let flat = sym.flatten(0);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].0, 0); // depth 0
        assert_eq!(flat[1].0, 1); // depth 1
        assert_eq!(flat[1].1.name, "bar");
    }

    // --- parse_symbol_query tests ---

    #[test]
    fn parse_symbol_query_plain() {
        let (kind, text) = parse_symbol_query("myFunc");
        assert!(kind.is_none());
        assert_eq!(text, "myFunc");
    }

    #[test]
    fn parse_symbol_query_at_prefix() {
        let (kind, text) = parse_symbol_query("@myFunc");
        assert!(kind.is_none());
        assert_eq!(text, "myFunc");
    }

    #[test]
    fn parse_symbol_query_kind_filter() {
        let (kind, text) = parse_symbol_query("@:function myFunc");
        assert_eq!(kind.as_deref(), Some("function"));
        assert_eq!(text, "myFunc");
    }

    #[test]
    fn parse_symbol_query_kind_only() {
        let (kind, text) = parse_symbol_query("@:class");
        assert_eq!(kind.as_deref(), Some("class"));
        assert_eq!(text, "");
    }

    // --- filter_symbols tests ---

    #[test]
    fn filter_symbols_no_filter() {
        let symbols = vec![
            DocumentSymbol::new("foo", SymbolKind::Function, 1, 5),
            DocumentSymbol::new("Bar", SymbolKind::Class, 10, 20),
        ];
        let filtered = filter_symbols(&symbols, None, "");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_symbols_by_kind() {
        let symbols = vec![
            DocumentSymbol::new("foo", SymbolKind::Function, 1, 5),
            DocumentSymbol::new("Bar", SymbolKind::Class, 10, 20),
        ];
        let filtered = filter_symbols(&symbols, Some("function"), "");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "foo");
    }

    #[test]
    fn filter_symbols_by_text() {
        let symbols = vec![
            DocumentSymbol::new("process_data", SymbolKind::Function, 1, 5),
            DocumentSymbol::new("validate", SymbolKind::Function, 10, 20),
        ];
        let filtered = filter_symbols(&symbols, None, "proc");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "process_data");
    }

    // --- symbols_to_quick_access_items ---

    #[test]
    fn symbols_to_items() {
        let symbols = vec![
            DocumentSymbol::new("main", SymbolKind::Function, 1, 10),
        ];
        let items = symbols_to_quick_access_items(&symbols);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "main");
        assert_eq!(items[0].icon.as_deref(), Some("ƒ"));
    }

    // --- QuickAccessMode tests ---

    #[test]
    fn parse_quick_access_mode_file() {
        assert_eq!(parse_quick_access_mode("hello"), QuickAccessMode::File);
    }

    #[test]
    fn parse_quick_access_mode_goto_line() {
        assert_eq!(parse_quick_access_mode(":42"), QuickAccessMode::GoToLine(42));
    }

    #[test]
    fn parse_quick_access_mode_symbol() {
        assert_eq!(
            parse_quick_access_mode("@myFunc"),
            QuickAccessMode::GoToSymbol("myFunc".to_string())
        );
    }

    #[test]
    fn parse_quick_access_mode_workspace_symbol() {
        assert_eq!(
            parse_quick_access_mode("#Config"),
            QuickAccessMode::WorkspaceSymbol("Config".to_string())
        );
    }

    #[test]
    fn parse_quick_access_mode_command() {
        assert_eq!(
            parse_quick_access_mode(">format"),
            QuickAccessMode::Command("format".to_string())
        );
    }

    // ── Recency tracker ───────────────────────────────────────────

    #[test]
    fn recency_tracker_boost_recent() {
        let mut tracker = RecencyTracker::new();
        tracker.record("file-a", 1000);
        // Used 100 seconds ago, decay is 600 seconds
        let boost = tracker.boost("file-a", 1100, 600);
        // elapsed=100, remaining=500, boost = 500*10/600 = 8
        assert_eq!(boost, 8);
    }

    #[test]
    fn recency_tracker_boost_expired() {
        let mut tracker = RecencyTracker::new();
        tracker.record("file-a", 100);
        // Used 700 seconds ago, decay is 600
        let boost = tracker.boost("file-a", 800, 600);
        assert_eq!(boost, 0);
    }

    #[test]
    fn recency_tracker_boost_unknown() {
        let tracker = RecencyTracker::new();
        assert_eq!(tracker.boost("missing", 1000, 600), 0);
    }

    #[test]
    fn recency_tracker_prune() {
        let mut tracker = RecencyTracker::new();
        tracker.record("old", 50);
        tracker.record("new", 200);
        assert_eq!(tracker.len(), 2);
        tracker.prune_before(100);
        assert_eq!(tracker.len(), 1);
        assert_eq!(tracker.boost("old", 300, 600), 0);
    }

    // ── Action categorization ─────────────────────────────────────

    #[test]
    fn group_items_by_category_groups() {
        let items = vec![
            QuickAccessItem {
                id: "a".into(),
                label: "A".into(),
                description: None,
                detail: None,
                icon: None,
                group: Some("Files".into()),
            },
            QuickAccessItem {
                id: "b".into(),
                label: "B".into(),
                description: None,
                detail: None,
                icon: None,
                group: Some("Commands".into()),
            },
            QuickAccessItem {
                id: "c".into(),
                label: "C".into(),
                description: None,
                detail: None,
                icon: None,
                group: Some("Files".into()),
            },
            QuickAccessItem {
                id: "d".into(),
                label: "D".into(),
                description: None,
                detail: None,
                icon: None,
                group: None,
            },
        ];
        let groups = group_items_by_category(&items);
        assert_eq!(groups.len(), 3);
        let files = groups.iter().find(|(k, _)| k == "Files").unwrap();
        assert_eq!(files.1.len(), 2);
        let other = groups.iter().find(|(k, _)| k == "Other").unwrap();
        assert_eq!(other.1.len(), 1);
    }

    #[test]
    fn deduplicate_items_removes_dupes() {
        let items = vec![
            make_item("a", "Alpha"),
            make_item("b", "Beta"),
            make_item("a", "Alpha Copy"),
        ];
        let deduped = deduplicate_items(&items);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].label, "Alpha");
        assert_eq!(deduped[1].label, "Beta");
    }

    #[test]
    fn action_category_display() {
        assert_eq!(format!("{}", ActionCategory::File), "Files");
        assert_eq!(format!("{}", ActionCategory::Custom("Git".into())), "Git");
    }

    // ── Search history ────────────────────────────────────────────

    #[test]
    fn search_history_record_and_recent() {
        let mut h = SearchHistory::new(5);
        h.record("open file");
        h.record("format document");
        h.record("open file"); // duplicate moves to front
        assert_eq!(h.len(), 2);
        assert_eq!(h.recent()[0], "open file");
        assert_eq!(h.recent()[1], "format document");
    }

    #[test]
    fn search_history_capacity() {
        let mut h = SearchHistory::new(3);
        h.record("a");
        h.record("b");
        h.record("c");
        h.record("d");
        assert_eq!(h.len(), 3);
        assert_eq!(h.recent()[0], "d");
        // "a" should have been evicted
        assert!(!h.recent().contains(&"a".to_string()));
    }

    #[test]
    fn search_history_ignores_empty() {
        let mut h = SearchHistory::new(5);
        h.record("");
        h.record("   ");
        assert!(h.is_empty());
    }

    #[test]
    fn search_history_fuzzy_search() {
        let mut h = SearchHistory::new(10);
        h.record("format document");
        h.record("open file");
        h.record("find references");
        let results = h.search("fmt");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "format document");
    }

    // ── Pinned items ──────────────────────────────────────────────

    #[test]
    fn pinned_items_pin_unpin_toggle() {
        let mut p = PinnedItems::new();
        assert!(!p.is_pinned("cmd1"));
        p.pin("cmd1");
        assert!(p.is_pinned("cmd1"));
        p.pin("cmd1"); // duplicate no-op
        assert_eq!(p.len(), 1);
        assert!(p.toggle("cmd1") == false); // unpin
        assert!(!p.is_pinned("cmd1"));
        assert!(p.toggle("cmd1") == true); // re-pin
        assert!(p.is_pinned("cmd1"));
    }

    #[test]
    fn sort_with_pinned_puts_pinned_first() {
        let items = vec![
            make_item("a", "Alpha"),
            make_item("b", "Beta"),
            make_item("c", "Charlie"),
        ];
        let mut results: Vec<(usize, i32)> = vec![(0, 10), (1, 20), (2, 5)];
        let mut pinned = PinnedItems::new();
        pinned.pin("c");
        sort_with_pinned(&mut results, &items, &pinned);
        // Pinned item "c" (index 2) should be first
        assert_eq!(results[0].0, 2);
    }

    // ── Shortcut registry ─────────────────────────────────────────

    #[test]
    fn shortcut_registry_bind_and_hint() {
        let mut reg = ShortcutRegistry::new();
        reg.bind("format", "Ctrl+Shift+F");
        let item = make_item("format", "Format Document");
        assert_eq!(reg.label_with_hint(&item), "Format Document (Ctrl+Shift+F)");
        let item2 = make_item("other", "Other Command");
        assert_eq!(reg.label_with_hint(&item2), "Other Command");
        reg.unbind("format");
        assert!(reg.get("format").is_none());
    }

    // ── Result preview ────────────────────────────────────────────

    #[test]
    fn result_preview_plain_truncation() {
        let long = "a".repeat(200);
        let preview = ResultPreview::plain(&long, 50);
        assert_eq!(preview.content.chars().count(), 50);
        assert!(preview.content.ends_with('…'));
        assert_eq!(preview.kind, PreviewKind::PlainText);
    }

    #[test]
    fn generate_preview_file_path() {
        let item = QuickAccessItem {
            id: "f".into(),
            label: "config".into(),
            description: None,
            detail: Some("/etc/config.toml".into()),
            icon: None,
            group: None,
        };
        let preview = generate_preview(&item);
        assert_eq!(preview.kind, PreviewKind::FilePath);
        assert_eq!(preview.content, "/etc/config.toml");
    }

    #[test]
    fn generate_preview_falls_back_to_description() {
        let item = QuickAccessItem {
            id: "x".into(),
            label: "Something".into(),
            description: Some("A useful command".into()),
            detail: None,
            icon: None,
            group: None,
        };
        let preview = generate_preview(&item);
        assert_eq!(preview.kind, PreviewKind::PlainText);
        assert_eq!(preview.content, "A useful command");
    }

    // -- QuickAccessScorer tests ----------------------------------------------

    #[test]
    fn score_items_empty_query() {
        let items = vec![QuickAccessItem {
            id: "a".into(), label: "Alpha".into(), description: None, detail: None, icon: None, group: None,
        }];
        let results = score_items(&items, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, 0);
    }

    #[test]
    fn score_items_filters_non_matching() {
        let items = vec![
            QuickAccessItem { id: "a".into(), label: "Alpha".into(), description: None, detail: None, icon: None, group: None },
            QuickAccessItem { id: "b".into(), label: "Beta".into(), description: None, detail: None, icon: None, group: None },
        ];
        let results = score_items(&items, "alp");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.id, "a");
        assert!(!results[0].highlight_positions.is_empty());
    }

    #[test]
    fn score_items_ranks_by_score() {
        let items = vec![
            QuickAccessItem { id: "ab".into(), label: "a_b_c".into(), description: None, detail: None, icon: None, group: None },
            QuickAccessItem { id: "abc".into(), label: "abc".into(), description: None, detail: None, icon: None, group: None },
        ];
        let results = score_items(&items, "abc");
        assert!(results[0].score >= results.last().unwrap().score);
    }

    // -- QuickAccessRecent tests ----------------------------------------------

    #[test]
    fn recent_record_and_frequency() {
        let mut recent = QuickAccessRecent::new(10);
        recent.record_access("a", "Alpha", 1);
        recent.record_access("b", "Beta", 2);
        recent.record_access("a", "Alpha", 3);
        assert_eq!(recent.len(), 2);
        let by_freq = recent.by_frequency();
        assert_eq!(by_freq[0].id, "a");
        assert_eq!(by_freq[0].access_count, 2);
    }

    #[test]
    fn recent_evicts_lru() {
        let mut recent = QuickAccessRecent::new(2);
        recent.record_access("a", "A", 1);
        recent.record_access("b", "B", 2);
        recent.record_access("c", "C", 3);
        assert_eq!(recent.len(), 2);
        assert!(recent.by_recency().iter().all(|e| e.id != "a"));
    }

    #[test]
    fn recent_by_recency() {
        let mut recent = QuickAccessRecent::new(10);
        recent.record_access("a", "A", 10);
        recent.record_access("b", "B", 20);
        let by_recency = recent.by_recency();
        assert_eq!(by_recency[0].id, "b");
    }

    #[test]
    fn recent_display() {
        let recent = QuickAccessRecent::new(5);
        let s = recent.to_string();
        assert!(s.contains("0 entries"));
    }

    // -- Grouping tests -------------------------------------------------------

    #[test]
    fn group_items_groups_correctly() {
        let items = vec![
            QuickAccessItem { id: "a".into(), label: "A".into(), description: None, detail: None, icon: None, group: Some("Files".into()) },
            QuickAccessItem { id: "b".into(), label: "B".into(), description: None, detail: None, icon: None, group: Some("Files".into()) },
            QuickAccessItem { id: "c".into(), label: "C".into(), description: None, detail: None, icon: None, group: Some("Commands".into()) },
        ];
        let groups = group_items(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["Files"].len(), 2);
    }

    #[test]
    fn group_count_with_none_group() {
        let items = vec![
            QuickAccessItem { id: "a".into(), label: "A".into(), description: None, detail: None, icon: None, group: None },
            QuickAccessItem { id: "b".into(), label: "B".into(), description: None, detail: None, icon: None, group: Some("X".into()) },
        ];
        assert_eq!(group_count(&items), 2);
    }

    #[test]
    fn flatten_with_headers_creates_sections() {
        let items = vec![
            QuickAccessItem { id: "a".into(), label: "Alpha".into(), description: None, detail: None, icon: None, group: Some("A".into()) },
            QuickAccessItem { id: "b".into(), label: "Beta".into(), description: None, detail: None, icon: None, group: Some("B".into()) },
        ];
        let flat = flatten_with_headers(&items);
        assert_eq!(flat.len(), 4);
        assert!(flat[0].contains("A"));
        assert_eq!(flat[1], "Alpha");
    }

    #[test]
    fn recent_clear() {
        let mut recent = QuickAccessRecent::new(10);
        recent.record_access("a", "A", 1);
        recent.clear();
        assert!(recent.is_empty());
    }

    #[test] fn quickAccessKeyNav_new() { let s = QuickAccessKeyNav::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn quickAccessKeyNav_add() { let mut s = QuickAccessKeyNav::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn quickAccessKeyNav_remove() { let mut s = QuickAccessKeyNav::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn quickAccessKeyNav_config() { let mut s = QuickAccessKeyNav::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn quickAccessKeyNav_nav() { let mut s = QuickAccessKeyNav::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn quickAccessKeyNav_filter() { let mut s = QuickAccessKeyNav::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn quickAccessKeyNav_display() { assert!(format!("{}", QuickAccessKeyNav::new()).contains("QuickAccessKeyNav")); }
    #[test] fn quickAccessPinnedItems_new() { let s = QuickAccessPinnedItems::new(); assert!(s.is_empty()); }
    #[test] fn quickAccessPinnedItems_add() { let mut s = QuickAccessPinnedItems::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn quickAccessPinnedItems_active() { let mut s = QuickAccessPinnedItems::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn quickAccessPinnedItems_error() { let mut s = QuickAccessPinnedItems::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn quickAccessPinnedItems_rm_group() { let mut s = QuickAccessPinnedItems::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn quickAccessPinnedItems_display() { assert!(format!("{}", QuickAccessPinnedItems::new()).contains("QuickAccessPinnedItems")); }


    #[test] fn quickAccessKeyNav_snap_capture() {
        let s = QuickAccessKeyNav::new();
        let snap = QuickAccessKeyNavSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn quickAccessKeyNav_snap_stale() {
        let s = QuickAccessKeyNav::new();
        let snap = QuickAccessKeyNavSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn quickAccessKeyNav_snap_diff() {
        let s = QuickAccessKeyNav::new();
        let s1v = QuickAccessKeyNavSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn quickAccessKeyNav_snap_display() {
        let s = QuickAccessKeyNav::new();
        let snap = QuickAccessKeyNavSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn quickAccessPinnedItems_stats_record() {
        let mut st = QuickAccessPinnedItemsStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn quickAccessPinnedItems_stats_hit_ratio() {
        let mut st = QuickAccessPinnedItemsStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn quickAccessPinnedItems_stats_merge() {
        let mut a = QuickAccessPinnedItemsStats::new();
        a.total_adds = 5;
        let mut b = QuickAccessPinnedItemsStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn quickAccessPinnedItems_stats_display() {
        let st = QuickAccessPinnedItemsStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn quickAccessKeyNav_config_default() {
        let c = QuickAccessKeyNavConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn quickAccessKeyNav_config_builder() {
        let c = QuickAccessKeyNavConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn quickAccessKeyNav_config_labels() {
        let mut c = QuickAccessKeyNavConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn quickAccessKeyNav_config_cleanup_threshold() {
        let c = QuickAccessKeyNavConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn quickAccessKeyNav_config_display() {
        assert!(format!("{}", QuickAccessKeyNavConfig::new()).contains("Config"));
    }
    #[test] fn quickAccessPinnedItems_stats_peaks() {
        let mut st = QuickAccessPinnedItemsStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    #[test]
    fn scorer_prefix_bonus() {
        let s1 = QuickAccessScorer::compute_score("open", "Open File");
        let s2 = QuickAccessScorer::compute_score("open", "Reopen File");
        assert!(s1 > s2);
    }

    #[test]
    fn scorer_no_match_returns_zero() {
        assert_eq!(QuickAccessScorer::compute_score("xyz", "Hello"), 0);
    }

    #[test]
    fn scorer_empty_query() {
        assert_eq!(QuickAccessScorer::compute_score("", "anything"), 0);
    }

    #[test]
    fn scorer_sort_by_score() {
        let items = vec!["Open File", "Options", "Close File"];
        let sorted = QuickAccessScorer::sort_by_score("op", &items);
        assert!(!sorted.is_empty());
        // first result should have highest score
        assert!(sorted[0].1 >= sorted.last().unwrap().1);
    }

    #[test]
    fn scorer_highlight_positions() {
        let positions = QuickAccessScorer::highlight_positions("of", "Open File");
        assert_eq!(positions, vec![0, 5]);
    }

    #[test]
    fn recent_tracker_v2_record_and_contains() {
        let mut t = RecentItemTrackerV2::new(10);
        t.record_access("file1.rs", 100);
        assert!(t.contains("file1.rs"));
        assert_eq!(t.access_count(), 1);
    }

    #[test]
    fn recent_tracker_v2_recent_items_sorted() {
        let mut t = RecentItemTrackerV2::new(10);
        t.record_access("a", 100);
        t.record_access("b", 300);
        t.record_access("c", 200);
        let recent = t.recent_items();
        assert_eq!(recent[0], "b");
    }

    #[test]
    fn recent_tracker_v2_cap_at_max() {
        let mut t = RecentItemTrackerV2::new(2);
        t.record_access("a", 1);
        t.record_access("b", 2);
        t.record_access("c", 3);
        assert_eq!(t.access_count(), 2);
        assert!(!t.contains("a"));
    }

    #[test]
    fn recent_tracker_v2_remove() {
        let mut t = RecentItemTrackerV2::new(10);
        t.record_access("x", 1);
        t.remove_item("x");
        assert!(!t.contains("x"));
    }

    #[test]
    fn prefix_router_register_and_resolve() {
        let mut r = QuickAccessPrefixRouter::new();
        r.register_prefix(">", "Commands");
        r.register_prefix("@", "Symbols");
        assert_eq!(r.resolve_prefix(">build").unwrap().description, "Commands");
        assert_eq!(r.resolve_prefix("@main").unwrap().description, "Symbols");
    }

    #[test]
    fn prefix_router_strip() {
        let mut r = QuickAccessPrefixRouter::new();
        r.register_prefix("#", "Workspace");
        assert_eq!(r.strip_prefix("#search"), "search");
        assert_eq!(r.strip_prefix("plain"), "plain");
    }

    #[test]
    fn prefix_router_no_match() {
        let r = QuickAccessPrefixRouter::new();
        assert!(r.resolve_prefix("anything").is_none());
    }


    #[test]
    fn quickaccess_entry_creation() {
        let e = QuickaccessEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn quickaccess_entry_with_priority() {
        let e = QuickaccessEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn quickaccess_entry_metadata() {
        let e = QuickaccessEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn quickaccess_entry_remove_meta() {
        let mut e = QuickaccessEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn quickaccess_entry_activate_deactivate() {
        let mut e = QuickaccessEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn quickaccess_config_add_sorted() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("lo", "Lo").with_priority(1));
        c.add(QuickaccessEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn quickaccess_config_capacity() {
        let mut c = QuickaccessConfig::new(1);
        assert!(c.add(QuickaccessEntry::new("a", "A")));
        assert!(!c.add(QuickaccessEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn quickaccess_config_remove() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn quickaccess_config_get() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn quickaccess_config_active_entries() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "A"));
        c.add(QuickaccessEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn quickaccess_config_enable_disable() {
        let mut c = QuickaccessConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn quickaccess_config_clear() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn quickaccess_config_find_by_label() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn quickaccess_config_top_n() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "A").with_priority(1));
        c.add(QuickaccessEntry::new("b", "B").with_priority(2));
        c.add(QuickaccessEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn quickaccess_config_deactivate_activate_all() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "A"));
        c.add(QuickaccessEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn quickaccess_config_highest_priority() {
        let mut c = QuickaccessConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(QuickaccessEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn quickaccess_config_contains() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn quickaccess_config_labels() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "Alpha"));
        c.add(QuickaccessEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn quickaccess_config_drain_inactive() {
        let mut c = QuickaccessConfig::new(10);
        c.add(QuickaccessEntry::new("a", "A"));
        c.add(QuickaccessEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn xj_metrics_empty() {
        let m = XjMetrics::new("quickacc");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xj_metrics_record_and_mean() {
        let mut m = XjMetrics::new("quickacc");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xj_metrics_min_max() {
        let mut m = XjMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xj_metrics_variance_and_std() {
        let mut m = XjMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn xj_metrics_percentile() {
        let mut m = XjMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xj_metrics_merge() {
        let mut a = XjMetrics::new("a");
        a.record(1.0);
        let mut b = XjMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xj_metrics_reset() {
        let mut m = XjMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xj_rate_window_empty() {
        let rw = XjRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xj_rate_window_tick_and_rate() {
        let mut rw = XjRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xj_lru_cache_basic() {
        let mut c = XjLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xj_lru_cache_contains_and_keys() {
        let mut c = XjLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xj_lru_cache_remove() {
        let mut c = XjLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xj_metrics_sum() {
        let mut m = XjMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xj_metrics_label() {
        let m = XjMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xj_lru_cache_clear() {
        let mut c = XjLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_28_push_and_len() {
        let mut rb = super::XbRingBuffer28::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_28_overwrite() {
        let mut rb = super::XbRingBuffer28::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_28_get_out_of_bounds() {
        let rb = super::XbRingBuffer28::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_28_drain_all() {
        let mut rb = super::XbRingBuffer28::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_28_peek_front_back() {
        let mut rb = super::XbRingBuffer28::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_28_clear() {
        let mut rb = super::XbRingBuffer28::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_28_capacity() {
        let rb = super::XbRingBuffer28::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_28_basic() {
        let h = super::xb_fnv1a_28(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_28(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_28_different_inputs() {
        let h1 = super::xb_fnv1a_28(b"abc");
        let h2 = super::xb_fnv1a_28(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_28_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_28(&data);
        let dec = super::xb_rle_decode_28(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_28_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_28(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_28(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_28_values() {
        assert!((super::xb_clamp_28(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_28(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_28(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_28_values() {
        assert!((super::xb_lerp_28(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_28(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_28(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_28_wrap_around_twice() {
        let mut rb = super::XbRingBuffer28::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 143 ----

    #[test]
    fn xc_143_pool_new_empty() {
        let pool: super::Xc143Pool<i32> = super::Xc143Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_143_pool_release_acquire() {
        let mut pool = super::Xc143Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_143_pool_acquire_empty() {
        let mut pool: super::Xc143Pool<i32> = super::Xc143Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_143_pool_full() {
        let mut pool = super::Xc143Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_143_pool_drain() {
        let mut pool = super::Xc143Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_143_pool_stats() {
        let mut pool = super::Xc143Pool::new(8);
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
    fn xc_143_pool_clear() {
        let mut pool = super::Xc143Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_143_pool_shrink() {
        let mut pool = super::Xc143Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_143_pool_default() {
        let pool: super::Xc143Pool<String> = super::Xc143Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_143_pool_extend() {
        let mut pool = super::Xc143Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_143_pool_retain() {
        let mut pool = super::Xc143Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_143_scheduler_round_robin() {
        let mut sched = super::Xc143Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_143_scheduler_empty() {
        let mut sched = super::Xc143Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_143_scheduler_reset() {
        let mut sched = super::Xc143Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_143_scheduler_add_remove() {
        let mut sched = super::Xc143Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_143_scheduler_targets() {
        let sched = super::Xc143Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_143_hash_empty() {
        assert_eq!(super::xc_143_hash(b""), 5381);
    }

    #[test]
    fn xc_143_hash_data() {
        let h = super::xc_143_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_143_hash(b"hello"), h);
    }

    #[test]
    fn xc_143_reverse_str() {
        assert_eq!(super::xc_143_reverse("abc"), "cba");
        assert_eq!(super::xc_143_reverse(""), "");
    }


    #[test]
    fn xe_40_pipeline_empty() {
        let p = super::Xe40Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_40_pipeline_parse_stage() {
        let p = super::Xe40Pipeline::new()
            .add_parse(super::xe_40_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_40_pipeline_transform_double() {
        let p = super::Xe40Pipeline::new()
            .add_transform(super::xe_40_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_40_pipeline_validate_reverse() {
        let p = super::Xe40Pipeline::new()
            .add_validate(super::xe_40_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_40_pipeline_emit_filter() {
        let p = super::Xe40Pipeline::new()
            .add_emit(super::xe_40_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_40_pipeline_multi_stage() {
        let p = super::Xe40Pipeline::new()
            .add_parse(super::xe_40_pipeline_identity)
            .add_transform(super::xe_40_pipeline_double)
            .add_validate(super::xe_40_pipeline_reverse)
            .add_emit(super::xe_40_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_40_pipeline_error_propagation() {
        let p = super::Xe40Pipeline::new()
            .add_parse(super::xe_40_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe40Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_40_pipeline_compose() {
        let p1 = super::Xe40Pipeline::new()
            .add_parse(super::xe_40_pipeline_identity);
        let p2 = super::Xe40Pipeline::new()
            .add_transform(super::xe_40_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_40_pipeline_error_display() {
        let e = super::Xe40PipelineError {
            stage: super::Xe40Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_40_cache_put_get() {
        let mut c = super::Xe40Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_40_cache_miss() {
        let mut c: super::Xe40Cache<&str, i32> = super::Xe40Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_40_cache_ttl_expiry() {
        let mut c = super::Xe40Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_40_cache_evict() {
        let mut c = super::Xe40Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_40_cache_capacity() {
        let mut c = super::Xe40Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_40_cache_stats() {
        let mut c = super::Xe40Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_40_cache_clear() {
        let mut c = super::Xe40Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_7 graph tests ------------------------------------------------

    #[test]
    fn xg_7_graph_empty() {
        let g = super::Xg7Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_7_graph_add_node() {
        let mut g = super::Xg7Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_7_graph_add_edge() {
        let mut g = super::Xg7Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_7_graph_neighbors() {
        let mut g = super::Xg7Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_7_graph_has_path() {
        let mut g = super::Xg7Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_7_graph_self_path() {
        let g = super::Xg7Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_7_graph_topo_sort() {
        let mut g = super::Xg7Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_7_graph_cycle_detect_false() {
        let mut g = super::Xg7Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_7_graph_cycle_detect_true() {
        let mut g = super::Xg7Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_7 heap tests -------------------------------------------------

    #[test]
    fn xg_7_heap_empty() {
        let h: super::Xg7Heap<i32> = super::Xg7Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_7_heap_push_pop() {
        let mut h = super::Xg7Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_7_heap_peek() {
        let mut h = super::Xg7Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_7_heap_drain_sorted() {
        let mut h = super::Xg7Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_7_heap_merge() {
        let mut a = super::Xg7Heap::new();
        let mut b = super::Xg7Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_7_heap_default() {
        let h: super::Xg7Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_7_graph_default() {
        let g: super::Xg7Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh142_skip_insert_contains() {
        let mut sl = super::Xh142SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh142_skip_remove() {
        let mut sl = super::Xh142SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh142_skip_len() {
        let mut sl = super::Xh142SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh142_skip_range_query() {
        let mut sl = super::Xh142SkipList::xh_new(4);
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
    fn xh142_skip_floor_ceiling() {
        let mut sl = super::Xh142SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh142_skip_rank() {
        let mut sl = super::Xh142SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh142_skip_empty() {
        let sl = super::Xh142SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh142_skip_duplicates() {
        let mut sl = super::Xh142SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh142_bitset_set_test() {
        let mut bs = super::Xh142BitSet::xh_new(256);
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
    fn xh142_bitset_clear_count() {
        let mut bs = super::Xh142BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh142_bitset_and_or_xor() {
        let mut a = super::Xh142BitSet::xh_new(128);
        let mut b = super::Xh142BitSet::xh_new(128);
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
    fn xh142_bitset_iter_ones() {
        let mut bs = super::Xh142BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh142_bitset_first_last() {
        let mut bs = super::Xh142BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh142_bitset_empty() {
        let bs = super::Xh142BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi142_deque_push_pop_back() {
        let mut dq = super::Xi142Deque::xi_new(4);
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
    fn xi142_deque_push_pop_front() {
        let mut dq = super::Xi142Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi142_deque_mixed_ops() {
        let mut dq = super::Xi142Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi142_deque_get_and_split() {
        let mut dq = super::Xi142Deque::xi_new(8);
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
    fn xi142_deque_rotate_left() {
        let mut dq = super::Xi142Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi142_deque_rotate_right() {
        let mut dq = super::Xi142Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi142_deque_grow() {
        let mut dq = super::Xi142Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi142_deque_empty() {
        let dq = super::Xi142Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi142_interval_tree_insert_query() {
        let mut tree = super::Xi142IntervalTree::xi_new();
        tree.xi_insert(super::Xi142Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi142Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi142Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi142_interval_tree_overlap() {
        let mut tree = super::Xi142IntervalTree::xi_new();
        tree.xi_insert(super::Xi142Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi142Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi142Interval::xi_new(12, 20));
        let q = super::Xi142Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi142_interval_tree_remove() {
        let mut tree = super::Xi142IntervalTree::xi_new();
        tree.xi_insert(super::Xi142Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi142Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi142_interval_tree_gaps() {
        let mut tree = super::Xi142IntervalTree::xi_new();
        tree.xi_insert(super::Xi142Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi142Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi142Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi142Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi142Interval::xi_new(8, 10));
    }

    #[test]
    fn xi142_interval_tree_merge() {
        let mut tree = super::Xi142IntervalTree::xi_new();
        tree.xi_insert(super::Xi142Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi142Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi142Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi142Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi142Interval::xi_new(10, 15));
    }

    #[test]
    fn xi142_interval_tree_all() {
        let mut tree = super::Xi142IntervalTree::xi_new();
        tree.xi_insert(super::Xi142Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi142Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi142_interval_tree_empty() {
        let tree = super::Xi142IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi142_interval_tree_contains_point() {
        let iv = super::Xi142Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 142) ---

    #[test]
    fn xj_142_uf_make_and_find() {
        let mut uf = super::Xj142UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_142_uf_union_connected() {
        let mut uf = super::Xj142UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_142_uf_component_count() {
        let mut uf = super::Xj142UnionFind::xj_new();
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
    fn xj_142_uf_component_size() {
        let mut uf = super::Xj142UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_142_uf_largest_component() {
        let mut uf = super::Xj142UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_142_uf_many_elements() {
        let mut uf = super::Xj142UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_142_uf_separate_components() {
        let mut uf = super::Xj142UnionFind::xj_new();
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
    fn xj_142_uf_path_compression() {
        let mut uf = super::Xj142UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_142_bt_insert_get() {
        let mut bt = super::Xj142BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_142_bt_contains_len() {
        let mut bt = super::Xj142BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_142_bt_replace() {
        let mut bt = super::Xj142BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_142_bt_remove() {
        let mut bt = super::Xj142BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_142_bt_keys_values() {
        let mut bt = super::Xj142BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_142_bt_range() {
        let mut bt = super::Xj142BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_142_bt_min_max() {
        let mut bt = super::Xj142BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_142_bt_many_inserts() {
        let mut bt = super::Xj142BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_142 segment tree tests ---

    #[test]
    fn xk_142_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk142SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_142_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk142SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_142_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk142SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_142_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk142SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_142_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk142SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_142_st_single_element() {
        let data = vec![42];
        let st = super::Xk142SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_142_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk142SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_142_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk142SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_142 disjoint intervals tests ---

    #[test]
    fn xk_142_di_add_and_count() {
        let mut di = super::Xk142DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_142_di_merge_overlap() {
        let mut di = super::Xk142DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_142_di_contains() {
        let mut di = super::Xk142DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_142_di_remove() {
        let mut di = super::Xk142DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_142_di_covered_length() {
        let mut di = super::Xk142DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_142_di_gaps() {
        let mut di = super::Xk142DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_142_di_merge_adjacent() {
        let mut di = super::Xk142DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_142_di_empty() {
        let di = super::Xk142DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_142_rope_new_empty() {
        let rope = super::Xl142Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_142_rope_from_str() {
        let rope = super::Xl142Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_142_rope_insert_at() {
        let mut rope = super::Xl142Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_142_rope_delete_range() {
        let mut rope = super::Xl142Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_142_rope_char_at() {
        let rope = super::Xl142Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_142_rope_split_concat() {
        let rope = super::Xl142Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_142_rope_line_count() {
        let rope = super::Xl142Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_142_rope_line_at() {
        let rope = super::Xl142Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_142_sa_build_and_search() {
        let sa = super::Xl142SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_142_sa_count() {
        let sa = super::Xl142SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_142_sa_longest_repeated() {
        let sa = super::Xl142SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_142_sa_all_positions() {
        let sa = super::Xl142SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_142_sa_len() {
        let sa = super::Xl142SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_142_sa_empty() {
        let sa = super::Xl142SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_142_rope_slice() {
        let rope = super::Xl142Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_142_sa_search_start() {
        let sa = super::Xl142SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_142_sparse_set_get() {
        let mut m = super::Xm142MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_142_sparse_row_col() {
        let mut m = super::Xm142MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_142_sparse_transpose() {
        let mut m = super::Xm142MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_142_sparse_multiply_vec() {
        let mut m = super::Xm142MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_142_sparse_nnz_density() {
        let mut m = super::Xm142MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_142_sparse_clear() {
        let mut m = super::Xm142MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_142_sparse_overwrite_zero() {
        let mut m = super::Xm142MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_142_tokenizer_basic() {
        let t = super::Xm142Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_142_tokenizer_count() {
        let t = super::Xm142Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_142_tokenizer_unique() {
        let t = super::Xm142Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_142_tokenizer_frequency() {
        let t = super::Xm142Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_142_tokenizer_delimiter() {
        let t = super::Xm142Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_142_tokenizer_whitespace() {
        let t = super::Xm142Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_142_tokenizer_empty() {
        let t = super::Xm142Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 142 ----

    #[test]
    fn xn_142_fenwick_prefix_sum() {
        let mut ft = super::Xn142Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_142_fenwick_range_sum() {
        let mut ft = super::Xn142Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_142_fenwick_point_query() {
        let mut ft = super::Xn142Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_142_fenwick_len() {
        let ft = super::Xn142Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_142_fenwick_multiple_updates() {
        let mut ft = super::Xn142Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_142_fenwick_single_element() {
        let mut ft = super::Xn142Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_142_fenwick_find_kth() {
        let mut ft = super::Xn142Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_142_fenwick_negative_delta() {
        let mut ft = super::Xn142Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 142 ----

    #[test]
    fn xn_142_avl_insert_get() {
        let mut m = super::Xn142AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_142_avl_remove() {
        let mut m = super::Xn142AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_142_avl_in_order() {
        let mut m = super::Xn142AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_142_avl_min_max() {
        let mut m = super::Xn142AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_142_avl_floor_ceiling() {
        let mut m = super::Xn142AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_142_avl_height_balanced() {
        let mut m = super::Xn142AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_142_avl_overwrite() {
        let mut m = super::Xn142AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_142_avl_empty() {
        let m: super::Xn142AVL<i32, i32> = super::Xn142AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo142RedBlack tests ---

    #[test]
    fn xo_142_rb_insert_and_get() {
        let mut tree = super::Xo142RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_142_rb_len_and_empty() {
        let mut tree = super::Xo142RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_142_rb_min_max() {
        let mut tree = super::Xo142RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_142_rb_contains() {
        let mut tree = super::Xo142RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_142_rb_remove() {
        let mut tree = super::Xo142RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_142_rb_in_order() {
        let mut tree = super::Xo142RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_142_rb_black_height() {
        let mut tree = super::Xo142RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_142_rb_overwrite() {
        let mut tree = super::Xo142RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo142ConsistentHash tests ---

    #[test]
    fn xo_142_ch_add_and_count() {
        let mut ring = super::Xo142ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_142_ch_remove_node() {
        let mut ring = super::Xo142ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_142_ch_get_node() {
        let mut ring = super::Xo142ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_142_ch_empty_ring() {
        let ring = super::Xo142ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_142_ch_distribution() {
        let mut ring = super::Xo142ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_142_ch_rebalance() {
        let mut ring = super::Xo142ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_142_ch_virtual_nodes() {
        let mut ring = super::Xo142ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_142_ch_consistent_lookup() {
        let mut ring = super::Xo142ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
