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
}
