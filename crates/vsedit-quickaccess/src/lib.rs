//! Quick access (Ctrl+P).

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
}
