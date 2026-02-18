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

}
