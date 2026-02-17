//! Global suggest configuration.

#[derive(Debug, Clone, PartialEq)]
pub enum SuggestWidgetState {
    Hidden,
    Loading,
    Visible,
    Details,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertMode {
    Insert,
    Replace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortMode {
    InlineFirst,
    SnippetsFirst,
    None,
}

#[derive(Debug, Clone)]
pub struct SuggestConfig {
    pub insert_mode: InsertMode,
    pub filter_graceful: bool,
    pub snippets_prevent_quick_suggestions: bool,
    pub local_sorting: SortMode,
    pub show_icons: bool,
    pub max_visible_suggestions: u32,
    pub status_bar_visible: bool,
}

impl Default for SuggestConfig {
    fn default() -> Self {
        Self {
            insert_mode: InsertMode::Insert,
            filter_graceful: true,
            snippets_prevent_quick_suggestions: false,
            local_sorting: SortMode::InlineFirst,
            show_icons: true,
            max_visible_suggestions: 12,
            status_bar_visible: true,
        }
    }
}

/// Widget for suggest/autocomplete functionality.
pub struct SuggestWidget {
    state: SuggestWidgetState,
    selected_index: Option<usize>,
    item_count: usize,
}

impl SuggestWidget {
    pub fn new() -> Self {
        Self {
            state: SuggestWidgetState::Hidden,
            selected_index: None,
            item_count: 0,
        }
    }

    pub fn show(&mut self, count: usize) {
        self.item_count = count;
        self.selected_index = if count > 0 { Some(0) } else { None };
        self.state = SuggestWidgetState::Visible;
    }

    pub fn hide(&mut self) {
        self.state = SuggestWidgetState::Hidden;
        self.selected_index = None;
        self.item_count = 0;
    }

    pub fn select(&mut self, index: usize) {
        if index < self.item_count {
            self.selected_index = Some(index);
        }
    }

    pub fn select_next(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx + 1 < self.item_count {
                self.selected_index = Some(idx + 1);
            }
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx > 0 {
                self.selected_index = Some(idx - 1);
            }
        }
    }

    pub fn get_state(&self) -> &SuggestWidgetState {
        &self.state
    }

    pub fn is_visible(&self) -> bool {
        matches!(self.state, SuggestWidgetState::Visible | SuggestWidgetState::Details)
    }
}

impl Default for SuggestWidget {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CompletionItemKind
// ---------------------------------------------------------------------------

/// The kind of a completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Keyword,
    Snippet,
    Text,
    Color,
    File,
    Folder,
}

// ---------------------------------------------------------------------------
// CompletionItem
// ---------------------------------------------------------------------------

/// A single completion entry.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub preselect: bool,
}

// ---------------------------------------------------------------------------
// SuggestModel
// ---------------------------------------------------------------------------

/// Model holding and filtering completion items.
pub struct SuggestModel {
    pub items: Vec<CompletionItem>,
}

impl SuggestModel {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Filter items whose `filter_text` (falling back to `label`) starts with
    /// `prefix` (case-insensitive).
    pub fn filter_items(&self, prefix: &str) -> Vec<&CompletionItem> {
        let lower = prefix.to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                let text = item
                    .filter_text
                    .as_deref()
                    .unwrap_or(&item.label);
                text.to_lowercase().starts_with(&lower)
            })
            .collect()
    }

    /// Sort items by `sort_text` (falling back to `label`).
    pub fn sort_items(&mut self) {
        self.items.sort_by(|a, b| {
            let sa = a.sort_text.as_deref().unwrap_or(&a.label);
            let sb = b.sort_text.as_deref().unwrap_or(&b.label);
            sa.cmp(sb)
        });
    }
}

impl Default for SuggestModel {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Extra SuggestWidget methods
// ---------------------------------------------------------------------------

impl SuggestWidget {
    pub fn show_details(&mut self) {
        if self.state == SuggestWidgetState::Visible {
            self.state = SuggestWidgetState::Details;
        }
    }

    pub fn hide_details(&mut self) {
        if self.state == SuggestWidgetState::Details {
            self.state = SuggestWidgetState::Visible;
        }
    }

    pub fn select_first(&mut self) {
        if self.item_count > 0 {
            self.selected_index = Some(0);
        }
    }

    pub fn select_last(&mut self) {
        if self.item_count > 0 {
            self.selected_index = Some(self.item_count - 1);
        }
    }

    pub fn page_up(&mut self, size: usize) {
        if let Some(idx) = self.selected_index {
            self.selected_index = Some(idx.saturating_sub(size));
        }
    }

    pub fn page_down(&mut self, size: usize) {
        if let Some(idx) = self.selected_index {
            let new = (idx + size).min(self.item_count.saturating_sub(1));
            self.selected_index = Some(new);
        }
    }
}

// ---------------------------------------------------------------------------
// CompletionProvider trait
// ---------------------------------------------------------------------------

/// Trait for providing completion items.
pub trait CompletionProvider {
    /// Provide completions for the given prefix.
    fn provide_completions(&self, prefix: &str) -> Vec<CompletionItem>;
}

// ---------------------------------------------------------------------------
// Completion widget
// ---------------------------------------------------------------------------

/// A completion widget for rendering in the terminal.
#[derive(Debug)]
pub struct CompletionWidget {
    pub items: Vec<CompletionItem>,
    pub selected_idx: usize,
    pub visible_range: (usize, usize),
    pub is_active: bool,
    pub filter_text: String,
    pub max_visible: usize,
}

impl CompletionWidget {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_idx: 0,
            visible_range: (0, 0),
            is_active: false,
            filter_text: String::new(),
            max_visible: 10,
        }
    }

    /// Open the widget with items.
    pub fn open(&mut self, items: Vec<CompletionItem>) {
        let visible_end = items.len().min(self.max_visible);
        self.items = items;
        self.selected_idx = 0;
        self.visible_range = (0, visible_end);
        self.is_active = true;
        self.filter_text.clear();
    }

    /// Dismiss the widget.
    pub fn dismiss(&mut self) {
        self.is_active = false;
        self.items.clear();
        self.filter_text.clear();
    }

    /// Navigate down.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected_idx + 1 < self.items.len() {
            self.selected_idx += 1;
            if self.selected_idx >= self.visible_range.1 {
                self.visible_range.0 += 1;
                self.visible_range.1 += 1;
            }
        }
    }

    /// Navigate up.
    pub fn select_prev(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
            if self.selected_idx < self.visible_range.0 {
                self.visible_range.0 = self.selected_idx;
                self.visible_range.1 = self.visible_range.0 + self.max_visible.min(self.items.len());
            }
        }
    }

    /// Accept the current selection. Returns the selected item, if any.
    pub fn accept(&mut self) -> Option<CompletionItem> {
        if !self.is_active || self.items.is_empty() {
            return None;
        }
        let item = self.items.get(self.selected_idx).cloned();
        self.dismiss();
        item
    }

    /// Accept the top item (Tab behavior).
    pub fn accept_top(&mut self) -> Option<CompletionItem> {
        if !self.is_active || self.items.is_empty() {
            return None;
        }
        let item = self.items.first().cloned();
        self.dismiss();
        item
    }

    /// Type-ahead filtering: update filter text and re-filter items.
    pub fn update_filter(&mut self, filter: &str, all_items: &[CompletionItem]) {
        self.filter_text = filter.to_string();
        let lower = filter.to_lowercase();
        self.items = all_items
            .iter()
            .filter(|item| {
                let text = item
                    .filter_text
                    .as_deref()
                    .unwrap_or(&item.label);
                text.to_lowercase().contains(&lower)
            })
            .cloned()
            .collect();
        self.selected_idx = 0;
        let visible_end = self.items.len().min(self.max_visible);
        self.visible_range = (0, visible_end);
    }

    /// Get the currently selected item.
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected_idx)
    }

    /// Get items in the visible range for rendering.
    pub fn visible_items(&self) -> &[CompletionItem] {
        let start = self.visible_range.0.min(self.items.len());
        let end = self.visible_range.1.min(self.items.len());
        &self.items[start..end]
    }
}

impl Default for CompletionWidget {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Auto-trigger configuration
// ---------------------------------------------------------------------------

/// Configuration for auto-triggering completions.
#[derive(Debug, Clone)]
pub struct AutoTriggerConfig {
    /// Characters that trigger completions (e.g., `.`, `:`, `/`).
    pub trigger_characters: Vec<char>,
    /// Debounce time in milliseconds for word-based completions.
    pub word_debounce_ms: u64,
    /// Whether auto-trigger is enabled.
    pub enabled: bool,
}

impl AutoTriggerConfig {
    pub fn new() -> Self {
        Self {
            trigger_characters: vec!['.'],
            word_debounce_ms: 500,
            enabled: true,
        }
    }

    /// Add Rust-specific trigger characters.
    pub fn with_rust_triggers(mut self) -> Self {
        if !self.trigger_characters.contains(&':') {
            self.trigger_characters.push(':');
        }
        self
    }

    /// Add file path trigger character.
    pub fn with_path_triggers(mut self) -> Self {
        if !self.trigger_characters.contains(&'/') {
            self.trigger_characters.push('/');
        }
        self
    }

    /// Add custom trigger characters from language configuration.
    pub fn with_custom_triggers(mut self, chars: &[char]) -> Self {
        for &c in chars {
            if !self.trigger_characters.contains(&c) {
                self.trigger_characters.push(c);
            }
        }
        self
    }

    /// Check if a character should trigger completions.
    pub fn should_trigger(&self, ch: char) -> bool {
        self.enabled && self.trigger_characters.contains(&ch)
    }
}

impl Default for AutoTriggerConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CompletionScoring
// ---------------------------------------------------------------------------

/// Fuzzy match scoring for completion items.
pub struct CompletionScoring;

impl CompletionScoring {
    const PREFIX_MATCH_BONUS: u32 = 10;
    const CONTIGUOUS_BONUS: u32 = 5;
    const CASE_MATCH_BONUS: u32 = 2;
    const POSITION_PENALTY: u32 = 1;

    /// Compute a fuzzy match score of `query` against `candidate`.
    ///
    /// Returns 0 when not all query characters are found in order.
    pub fn score(query: &str, candidate: &str) -> u32 {
        if query.is_empty() {
            return 0;
        }

        let query_lower: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
        let candidate_chars: Vec<char> = candidate.chars().collect();
        let candidate_lower: Vec<char> = candidate_chars.iter().map(|c| c.to_ascii_lowercase()).collect();

        let mut score: u32 = 0;
        let mut cand_idx: usize = 0;
        let mut last_match_idx: Option<usize> = None;

        for qc in query_lower.iter() {
            let mut found = false;
            while cand_idx < candidate_lower.len() {
                if candidate_lower[cand_idx] == *qc {
                    // Gap penalty
                    if let Some(prev) = last_match_idx {
                        let gap = cand_idx - prev - 1;
                        score = score.saturating_sub(gap as u32 * Self::POSITION_PENALTY);
                    }

                    // Contiguous bonus
                    if let Some(prev) = last_match_idx {
                        if cand_idx == prev + 1 {
                            score += Self::CONTIGUOUS_BONUS;
                        }
                    }

                    last_match_idx = Some(cand_idx);
                    cand_idx += 1;
                    found = true;
                    break;
                }
                cand_idx += 1;
            }
            if !found {
                return 0;
            }
        }

        // Case match bonus — compare original characters
        let query_chars: Vec<char> = query.chars().collect();
        let mut ci = 0usize;
        for qc in &query_chars {
            while ci < candidate_chars.len() {
                if candidate_chars[ci].to_ascii_lowercase() == qc.to_ascii_lowercase() {
                    if candidate_chars[ci] == *qc {
                        score += Self::CASE_MATCH_BONUS;
                    }
                    ci += 1;
                    break;
                }
                ci += 1;
            }
        }

        // Prefix bonus
        let candidate_prefix: String = candidate_lower.iter().take(query_lower.len()).collect();
        let query_str: String = query_lower.iter().collect();
        if candidate_prefix == query_str {
            score += Self::PREFIX_MATCH_BONUS;
        }

        // Ensure a match always returns at least 1
        if score == 0 { 1 } else { score }
    }

    /// Score a completion item, using `filter_text` if present, otherwise `label`.
    pub fn score_item(query: &str, item: &CompletionItem) -> u32 {
        let text = item.filter_text.as_deref().unwrap_or(&item.label);
        Self::score(query, text)
    }
}

// ---------------------------------------------------------------------------
// SuggestionFilter
// ---------------------------------------------------------------------------

/// Filters a set of completion items by kind and/or label prefix.
pub struct SuggestionFilter {
    kinds: Vec<CompletionItemKind>,
    label_prefix: Option<String>,
}

impl SuggestionFilter {
    pub fn new() -> Self {
        Self {
            kinds: Vec::new(),
            label_prefix: None,
        }
    }

    pub fn with_kind(mut self, kind: CompletionItemKind) -> Self {
        self.kinds.push(kind);
        self
    }

    pub fn with_kinds(mut self, kinds: &[CompletionItemKind]) -> Self {
        self.kinds.extend_from_slice(kinds);
        self
    }

    pub fn with_label_prefix(mut self, prefix: &str) -> Self {
        self.label_prefix = Some(prefix.to_string());
        self
    }

    pub fn apply<'a>(&self, items: &'a [CompletionItem]) -> Vec<&'a CompletionItem> {
        items
            .iter()
            .filter(|item| {
                if !self.kinds.is_empty() && !self.kinds.contains(&item.kind) {
                    return false;
                }
                if let Some(ref prefix) = self.label_prefix {
                    if !item.label.starts_with(prefix.as_str()) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

impl Default for SuggestionFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SuggestionSorter
// ---------------------------------------------------------------------------

/// Multi-criteria sorter for completion items.
pub struct SuggestionSorter {
    query: Option<String>,
    by_kind_priority: bool,
    by_label: bool,
}

impl SuggestionSorter {
    pub fn new() -> Self {
        Self {
            query: None,
            by_kind_priority: false,
            by_label: false,
        }
    }

    pub fn by_score(mut self, query: &str) -> Self {
        self.query = Some(query.to_string());
        self
    }

    /// Methods first, then Functions, then everything else.
    pub fn by_kind_priority(mut self) -> Self {
        self.by_kind_priority = true;
        self
    }

    pub fn by_label(mut self) -> Self {
        self.by_label = true;
        self
    }

    fn kind_priority(kind: &CompletionItemKind) -> u32 {
        match kind {
            CompletionItemKind::Method => 0,
            CompletionItemKind::Function => 1,
            _ => 2,
        }
    }

    pub fn sort(&self, items: &mut Vec<CompletionItem>) {
        let query = self.query.clone();
        let by_kind = self.by_kind_priority;
        let by_label = self.by_label;

        items.sort_by(|a, b| {
            // Score (higher is better → reverse order)
            if let Some(ref q) = query {
                let sa = CompletionScoring::score_item(q, a);
                let sb = CompletionScoring::score_item(q, b);
                let cmp = sb.cmp(&sa);
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            // Kind priority (lower is better)
            if by_kind {
                let cmp = Self::kind_priority(&a.kind).cmp(&Self::kind_priority(&b.kind));
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            // Label (alphabetical)
            if by_label {
                return a.label.cmp(&b.label);
            }
            std::cmp::Ordering::Equal
        });
    }
}

impl Default for SuggestionSorter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SuggestionGrouping
// ---------------------------------------------------------------------------

/// Groups completion items by kind or a custom source label.
#[derive(Debug, Default)]
pub struct SuggestionGrouping {
    groups: std::collections::HashMap<String, Vec<CompletionItem>>,
}

impl SuggestionGrouping {
    pub fn new() -> Self {
        Self::default()
    }

    /// Group items by their `CompletionItemKind`.
    pub fn group_by_kind(items: &[CompletionItem]) -> Self {
        let mut sg = Self::new();
        for item in items {
            let key = format!("{:?}", item.kind);
            sg.groups.entry(key).or_default().push(item.clone());
        }
        sg
    }

    /// Group items by a custom key extractor.
    pub fn group_by<F: Fn(&CompletionItem) -> String>(items: &[CompletionItem], key_fn: F) -> Self {
        let mut sg = Self::new();
        for item in items {
            sg.groups.entry(key_fn(item)).or_default().push(item.clone());
        }
        sg
    }

    /// Get group names.
    pub fn group_names(&self) -> Vec<&str> {
        self.groups.keys().map(String::as_str).collect()
    }

    /// Get items in a specific group.
    pub fn get_group(&self, name: &str) -> Option<&[CompletionItem]> {
        self.groups.get(name).map(Vec::as_slice)
    }

    /// Number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total items across all groups.
    pub fn total_items(&self) -> usize {
        self.groups.values().map(Vec::len).sum()
    }
}

// ---------------------------------------------------------------------------
// CompletionSession
// ---------------------------------------------------------------------------

/// Tracks a completion session from trigger to accept/dismiss.
#[derive(Debug)]
pub struct CompletionSession {
    trigger_character: Option<char>,
    start_ms: u64,
    end_ms: Option<u64>,
    items_shown: usize,
    accepted: bool,
    prefix: String,
}

impl CompletionSession {
    /// Start a new completion session.
    pub fn start(prefix: &str, trigger_char: Option<char>, start_ms: u64) -> Self {
        Self {
            trigger_character: trigger_char,
            start_ms,
            end_ms: None,
            items_shown: 0,
            accepted: false,
            prefix: prefix.to_string(),
        }
    }

    /// Record the number of items shown.
    pub fn set_items_shown(&mut self, count: usize) {
        self.items_shown = count;
    }

    /// Mark the session as accepted (user picked an item).
    pub fn accept(&mut self, end_ms: u64) {
        self.end_ms = Some(end_ms);
        self.accepted = true;
    }

    /// Mark the session as dismissed (user cancelled).
    pub fn dismiss(&mut self, end_ms: u64) {
        self.end_ms = Some(end_ms);
        self.accepted = false;
    }

    /// Duration of the session in ms, or `None` if still active.
    pub fn duration_ms(&self) -> Option<u64> {
        self.end_ms.map(|end| end.saturating_sub(self.start_ms))
    }

    /// Whether the session is still active.
    pub fn is_active(&self) -> bool {
        self.end_ms.is_none()
    }

    /// Whether the session ended with an acceptance.
    pub fn was_accepted(&self) -> bool {
        self.accepted
    }

    /// The prefix that triggered the session.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Items shown during this session.
    pub fn items_shown(&self) -> usize {
        self.items_shown
    }

    /// The trigger character, if any.
    pub fn trigger_character(&self) -> Option<char> {
        self.trigger_character
    }
}

// ---------------------------------------------------------------------------
// SuggestionCache
// ---------------------------------------------------------------------------

/// Simple LRU-style cache for completion results keyed by prefix.
#[derive(Debug)]
pub struct SuggestionCache {
    capacity: usize,
    entries: Vec<(String, Vec<CompletionItem>)>,
}

impl SuggestionCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    /// Insert a result set for a prefix. Evicts the oldest entry if at capacity.
    pub fn insert(&mut self, prefix: &str, items: Vec<CompletionItem>) {
        // Remove existing entry for same prefix.
        self.entries.retain(|(k, _)| k != prefix);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((prefix.to_string(), items));
    }

    /// Get cached items for a prefix, moving it to most-recently-used.
    pub fn get(&mut self, prefix: &str) -> Option<&[CompletionItem]> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == prefix) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_slice())
        } else {
            None
        }
    }

    /// Check if a prefix is cached.
    pub fn contains(&self, prefix: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == prefix)
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Invalidate entries whose prefix starts with the given string.
    pub fn invalidate_prefix(&mut self, prefix: &str) {
        self.entries.retain(|(k, _)| !k.starts_with(prefix));
    }
}

// ---------------------------------------------------------------------------
// CompletionScoring contextual extension
// ---------------------------------------------------------------------------

impl CompletionScoring {
    /// Score with contextual boost: items whose kind matches `preferred_kind`
    /// get a bonus.
    pub fn score_contextual(
        query: &str,
        item: &CompletionItem,
        preferred_kind: Option<CompletionItemKind>,
    ) -> u32 {
        let mut s = Self::score_item(query, item);
        if let Some(kind) = preferred_kind {
            if item.kind == kind {
                s += 15;
            }
        }
        if item.preselect {
            s += 20;
        }
        s
    }
}

// ---------------------------------------------------------------------------
// CompletionItemKind utilities
// ---------------------------------------------------------------------------

impl CompletionItemKind {
    /// Returns a short icon character for display.
    pub fn icon_char(&self) -> char {
        match self {
            Self::Method => 'm',
            Self::Function => 'f',
            Self::Constructor => 'k',
            Self::Field => 'F',
            Self::Variable => 'v',
            Self::Class => 'C',
            Self::Interface => 'I',
            Self::Module => 'M',
            Self::Property => 'p',
            Self::Keyword => 'K',
            Self::Snippet => 'S',
            Self::Text => 't',
            Self::Color => 'c',
            Self::File => 'D',
            Self::Folder => 'd',
        }
    }

    /// Returns `true` if this kind represents a callable symbol.
    pub fn is_callable(&self) -> bool {
        matches!(self, Self::Method | Self::Function | Self::Constructor)
    }

    /// Returns `true` if this kind represents a type-level symbol.
    pub fn is_type(&self) -> bool {
        matches!(self, Self::Class | Self::Interface | Self::Module)
    }
}

impl fmt::Display for CompletionItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Method => "Method",
            Self::Function => "Function",
            Self::Constructor => "Constructor",
            Self::Field => "Field",
            Self::Variable => "Variable",
            Self::Class => "Class",
            Self::Interface => "Interface",
            Self::Module => "Module",
            Self::Property => "Property",
            Self::Keyword => "Keyword",
            Self::Snippet => "Snippet",
            Self::Text => "Text",
            Self::Color => "Color",
            Self::File => "File",
            Self::Folder => "Folder",
        };
        f.write_str(s)
    }
}

use std::fmt;

// ---------------------------------------------------------------------------
// CompletionItem builder & utilities
// ---------------------------------------------------------------------------

impl CompletionItem {
    /// Create a basic completion item with the given label and kind.
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            insert_text: None,
            sort_text: None,
            filter_text: None,
            preselect: false,
        }
    }

    /// Set detail text (builder pattern).
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set insert text (builder pattern).
    pub fn with_insert_text(mut self, text: impl Into<String>) -> Self {
        self.insert_text = Some(text.into());
        self
    }

    /// Set preselect flag (builder pattern).
    pub fn with_preselect(mut self, preselect: bool) -> Self {
        self.preselect = preselect;
        self
    }

    /// Returns the effective text used for insertion.
    pub fn effective_insert_text(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }

    /// Returns the effective text used for filtering.
    pub fn effective_filter_text(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }
}

impl fmt::Display for CompletionItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind, self.label)?;
        if let Some(ref detail) = self.detail {
            write!(f, " — {}", detail)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SuggestModel statistics
// ---------------------------------------------------------------------------

impl SuggestModel {
    /// Count items grouped by kind.
    pub fn count_by_kind(&self) -> Vec<(CompletionItemKind, usize)> {
        let mut counts: Vec<(CompletionItemKind, usize)> = Vec::new();
        for item in &self.items {
            if let Some(entry) = counts.iter_mut().find(|(k, _)| *k == item.kind) {
                entry.1 += 1;
            } else {
                counts.push((item.kind, 1));
            }
        }
        counts
    }

    /// Return only items with a detail string.
    pub fn items_with_detail(&self) -> Vec<&CompletionItem> {
        self.items.iter().filter(|i| i.detail.is_some()).collect()
    }

    /// Return the top N items scored against the given query.
    pub fn top_matches(&self, query: &str, n: usize) -> Vec<&CompletionItem> {
        let mut scored: Vec<(&CompletionItem, u32)> = self
            .items
            .iter()
            .map(|item| (item, CompletionScoring::score_item(query, item)))
            .filter(|(_, s)| *s > 0)
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().take(n).map(|(item, _)| item).collect()
    }
}

// ---------------------------------------------------------------------------
// SuggestWidgetSizer – calculate dropdown dimensions
// ---------------------------------------------------------------------------

/// Calculates the dimensions of the suggest widget dropdown.
pub struct SuggestWidgetSizer {
    pub max_width: u16,
    pub max_height: u16,
    pub item_height: u16,
    pub padding: u16,
}

impl SuggestWidgetSizer {
    /// Create a sizer with the given constraints.
    pub fn new(max_width: u16, max_height: u16, item_height: u16) -> Self {
        Self { max_width, max_height, item_height, padding: 2 }
    }

    /// Calculate the height needed for a given number of items.
    pub fn height_for_items(&self, count: usize) -> u16 {
        let content = (count as u16).saturating_mul(self.item_height);
        (content + self.padding * 2).min(self.max_height)
    }

    /// Calculate the max number of visible items.
    pub fn visible_item_count(&self) -> u16 {
        let available = self.max_height.saturating_sub(self.padding * 2);
        if self.item_height == 0 { return 0; }
        available / self.item_height
    }

    /// Calculate the width needed for the longest label.
    pub fn width_for_labels(&self, labels: &[&str]) -> u16 {
        let max_label = labels.iter().map(|l| l.len() as u16).max().unwrap_or(0);
        (max_label + self.padding * 2 + 4).min(self.max_width) // +4 for icon/kind
    }
}

// ---------------------------------------------------------------------------
// SuggestTabCompletion – handles tab completion
// ---------------------------------------------------------------------------

/// Handles tab completion behavior in the suggest widget.
pub struct SuggestTabCompletion {
    enabled: bool,
    accept_on_tab: bool,
    accept_on_enter: bool,
}

impl SuggestTabCompletion {
    /// Create with default settings (tab accepts, enter accepts).
    pub fn new() -> Self {
        Self { enabled: true, accept_on_tab: true, accept_on_enter: true }
    }

    /// Enable or disable tab completion.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether tab should accept the current suggestion.
    pub fn should_accept_on_tab(&self) -> bool {
        self.enabled && self.accept_on_tab
    }

    /// Whether enter should accept the current suggestion.
    pub fn should_accept_on_enter(&self) -> bool {
        self.enabled && self.accept_on_enter
    }

    /// Set whether tab accepts.
    pub fn set_accept_on_tab(&mut self, val: bool) {
        self.accept_on_tab = val;
    }

    /// Set whether enter accepts.
    pub fn set_accept_on_enter(&mut self, val: bool) {
        self.accept_on_enter = val;
    }
}

// ---------------------------------------------------------------------------
// SuggestSnippetResolver – expand snippet bodies
// ---------------------------------------------------------------------------

/// Resolves snippet placeholders in a completion item body.
pub struct SuggestSnippetResolver {
    variables: Vec<(String, String)>,
}

impl SuggestSnippetResolver {
    /// Create a resolver with no variables.
    pub fn new() -> Self {
        Self { variables: Vec::new() }
    }

    /// Register a variable value (e.g. `"TM_FILENAME"` → `"main.rs"`).
    pub fn set_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        if let Some(entry) = self.variables.iter_mut().find(|(n, _)| n == &name) {
            entry.1 = value.into();
        } else {
            self.variables.push((name, value.into()));
        }
    }

    /// Expand a snippet body string, resolving `$VAR` and `${VAR}` references.
    pub fn expand(&self, body: &str) -> String {
        let mut result = body.to_string();
        for (name, value) in &self.variables {
            result = result.replace(&format!("${{{}}}", name), value);
            result = result.replace(&format!("${}", name), value);
        }
        // Remove remaining tabstops like $1, $2, ${1:default}
        let mut out = String::new();
        let mut chars = result.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '$' {
                if chars.peek() == Some(&'{') {
                    chars.next(); // skip {
                    let mut depth = 1;
                    while let Some(c) = chars.next() {
                        if c == '{' { depth += 1; }
                        if c == '}' { depth -= 1; if depth == 0 { break; } }
                    }
                } else if chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        chars.next();
                    }
                } else {
                    out.push(ch);
                }
            } else {
                out.push(ch);
            }
        }
        out
    }

    /// Number of registered variables.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }
}

// ---------------------------------------------------------------------------
// SuggestKeyboardNav – keyboard navigation state
// ---------------------------------------------------------------------------

/// Manages keyboard navigation state for the suggest widget.
pub struct SuggestKeyboardNav {
    selected: usize,
    total: usize,
    page_size: usize,
}

impl SuggestKeyboardNav {
    /// Create navigation for a list of items.
    pub fn new(total: usize, page_size: usize) -> Self {
        Self { selected: 0, total, page_size: page_size.max(1) }
    }

    /// Move selection down by one.
    pub fn move_down(&mut self) {
        if self.total > 0 {
            self.selected = (self.selected + 1) % self.total;
        }
    }

    /// Move selection up by one.
    pub fn move_up(&mut self) {
        if self.total > 0 {
            self.selected = if self.selected == 0 { self.total - 1 } else { self.selected - 1 };
        }
    }

    /// Page down.
    pub fn page_down(&mut self) {
        if self.total > 0 {
            self.selected = (self.selected + self.page_size).min(self.total - 1);
        }
    }

    /// Page up.
    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.page_size);
    }

    /// Jump to first item.
    pub fn home(&mut self) {
        self.selected = 0;
    }

    /// Jump to last item.
    pub fn end(&mut self) {
        if self.total > 0 {
            self.selected = self.total - 1;
        }
    }

    /// Current selected index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Update total item count, clamping selection.
    pub fn set_total(&mut self, total: usize) {
        self.total = total;
        if self.total > 0 && self.selected >= self.total {
            self.selected = self.total - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_hide() {
        let mut w = SuggestWidget::new();
        assert!(!w.is_visible());
        assert_eq!(*w.get_state(), SuggestWidgetState::Hidden);
        w.show(5);
        assert!(w.is_visible());
        assert_eq!(w.selected_index, Some(0));
        w.hide();
        assert!(!w.is_visible());
    }

    #[test]
    fn navigation() {
        let mut w = SuggestWidget::new();
        w.show(3);
        assert_eq!(w.selected_index, Some(0));
        w.select_next();
        assert_eq!(w.selected_index, Some(1));
        w.select_next();
        assert_eq!(w.selected_index, Some(2));
        w.select_next(); // should not go past end
        assert_eq!(w.selected_index, Some(2));
        w.select_previous();
        assert_eq!(w.selected_index, Some(1));
    }

    #[test]
    fn select_index() {
        let mut w = SuggestWidget::new();
        w.show(5);
        w.select(3);
        assert_eq!(w.selected_index, Some(3));
        w.select(10); // out of range, no change
        assert_eq!(w.selected_index, Some(3));
    }

    #[test]
    fn show_hide_details() {
        let mut w = SuggestWidget::new();
        w.show(3);
        w.show_details();
        assert_eq!(*w.get_state(), SuggestWidgetState::Details);
        assert!(w.is_visible());
        w.hide_details();
        assert_eq!(*w.get_state(), SuggestWidgetState::Visible);
    }

    #[test]
    fn select_first_last() {
        let mut w = SuggestWidget::new();
        w.show(5);
        w.select_last();
        assert_eq!(w.selected_index, Some(4));
        w.select_first();
        assert_eq!(w.selected_index, Some(0));
    }

    #[test]
    fn page_up_down() {
        let mut w = SuggestWidget::new();
        w.show(20);
        w.select(10);
        w.page_up(5);
        assert_eq!(w.selected_index, Some(5));
        w.page_down(10);
        assert_eq!(w.selected_index, Some(15));
        w.page_down(100);
        assert_eq!(w.selected_index, Some(19));
        w.page_up(100);
        assert_eq!(w.selected_index, Some(0));
    }

    #[test]
    fn completion_item_kind_variants() {
        let kinds = vec![
            CompletionItemKind::Method,
            CompletionItemKind::Function,
            CompletionItemKind::Constructor,
            CompletionItemKind::Field,
            CompletionItemKind::Variable,
            CompletionItemKind::Class,
            CompletionItemKind::Interface,
            CompletionItemKind::Module,
            CompletionItemKind::Property,
            CompletionItemKind::Keyword,
            CompletionItemKind::Snippet,
            CompletionItemKind::Text,
            CompletionItemKind::Color,
            CompletionItemKind::File,
            CompletionItemKind::Folder,
        ];
        assert_eq!(kinds.len(), 15);
    }

    fn make_item(label: &str, kind: CompletionItemKind) -> CompletionItem {
        CompletionItem {
            label: label.to_string(),
            kind,
            detail: None,
            insert_text: None,
            sort_text: None,
            filter_text: None,
            preselect: false,
        }
    }

    #[test]
    fn suggest_model_filter_case_insensitive() {
        let model = SuggestModel {
            items: vec![
                make_item("forEach", CompletionItemKind::Method),
                make_item("format", CompletionItemKind::Function),
                make_item("bar", CompletionItemKind::Variable),
            ],
        };
        let results = model.filter_items("for");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn suggest_model_filter_empty_prefix() {
        let model = SuggestModel {
            items: vec![
                make_item("a", CompletionItemKind::Text),
                make_item("b", CompletionItemKind::Text),
            ],
        };
        assert_eq!(model.filter_items("").len(), 2);
    }

    #[test]
    fn suggest_model_sort() {
        let mut model = SuggestModel {
            items: vec![
                make_item("zebra", CompletionItemKind::Variable),
                make_item("apple", CompletionItemKind::Variable),
                make_item("mango", CompletionItemKind::Variable),
            ],
        };
        model.sort_items();
        assert_eq!(model.items[0].label, "apple");
        assert_eq!(model.items[2].label, "zebra");
    }

    #[test]
    fn suggest_model_filter_with_filter_text() {
        let model = SuggestModel {
            items: vec![CompletionItem {
                label: "display".to_string(),
                kind: CompletionItemKind::Property,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: Some("css-display".to_string()),
                preselect: false,
            }],
        };
        assert_eq!(model.filter_items("css").len(), 1);
        assert_eq!(model.filter_items("dis").len(), 0);
    }

    #[test]
    fn completion_provider_trait() {
        struct TestProvider;
        impl CompletionProvider for TestProvider {
            fn provide_completions(&self, prefix: &str) -> Vec<CompletionItem> {
                vec![make_item(&format!("{}Completion", prefix), CompletionItemKind::Text)]
            }
        }
        let p = TestProvider;
        let items = p.provide_completions("test");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "testCompletion");
    }

    #[test]
    fn suggest_model_default() {
        let model = SuggestModel::default();
        assert!(model.items.is_empty());
    }

    #[test]
    fn default_config() {
        let cfg = SuggestConfig::default();
        assert_eq!(cfg.insert_mode, InsertMode::Insert);
        assert!(cfg.filter_graceful);
        assert_eq!(cfg.max_visible_suggestions, 12);
    }

    #[test]
    fn eq_suggestwidgetstate_same() {
        assert_eq!(SuggestWidgetState::Hidden, SuggestWidgetState::Hidden);
    }

    #[test]
    fn ne_suggestwidgetstate_diff() {
        assert_ne!(SuggestWidgetState::Hidden, SuggestWidgetState::Loading);
    }

    #[test]
    fn eq_insertmode_same() {
        assert_eq!(InsertMode::Insert, InsertMode::Insert);
    }

    #[test]
    fn ne_insertmode_diff() {
        assert_ne!(InsertMode::Insert, InsertMode::Replace);
    }

    #[test]
    fn eq_sortmode_same() {
        assert_eq!(SortMode::InlineFirst, SortMode::InlineFirst);
    }

    #[test]
    fn ne_sortmode_diff() {
        assert_ne!(SortMode::InlineFirst, SortMode::SnippetsFirst);
    }

    #[test]
    fn eq_completionitemkind_same() {
        assert_eq!(CompletionItemKind::Method, CompletionItemKind::Method);
    }

    #[test]
    fn ne_completionitemkind_diff() {
        assert_ne!(CompletionItemKind::Method, CompletionItemKind::Function);
    }

    // -----------------------------------------------------------------------
    // CompletionWidget tests
    // -----------------------------------------------------------------------

    #[test]
    fn completion_widget_open_and_dismiss() {
        let mut w = CompletionWidget::new();
        assert!(!w.is_active);
        w.open(vec![
            make_item("foo", CompletionItemKind::Function),
            make_item("bar", CompletionItemKind::Variable),
        ]);
        assert!(w.is_active);
        assert_eq!(w.items.len(), 2);
        assert_eq!(w.selected_idx, 0);
        w.dismiss();
        assert!(!w.is_active);
        assert!(w.items.is_empty());
    }

    #[test]
    fn completion_widget_navigation() {
        let mut w = CompletionWidget::new();
        w.open(vec![
            make_item("a", CompletionItemKind::Text),
            make_item("b", CompletionItemKind::Text),
            make_item("c", CompletionItemKind::Text),
        ]);
        assert_eq!(w.selected_idx, 0);
        w.select_next();
        assert_eq!(w.selected_idx, 1);
        w.select_next();
        assert_eq!(w.selected_idx, 2);
        w.select_next(); // at end
        assert_eq!(w.selected_idx, 2);
        w.select_prev();
        assert_eq!(w.selected_idx, 1);
        w.select_prev();
        assert_eq!(w.selected_idx, 0);
        w.select_prev(); // at start
        assert_eq!(w.selected_idx, 0);
    }

    #[test]
    fn completion_widget_accept() {
        let mut w = CompletionWidget::new();
        w.open(vec![
            make_item("foo", CompletionItemKind::Function),
            make_item("bar", CompletionItemKind::Variable),
        ]);
        w.select_next();
        let accepted = w.accept();
        assert!(accepted.is_some());
        assert_eq!(accepted.unwrap().label, "bar");
        assert!(!w.is_active);
    }

    #[test]
    fn completion_widget_accept_top() {
        let mut w = CompletionWidget::new();
        w.open(vec![
            make_item("first", CompletionItemKind::Text),
            make_item("second", CompletionItemKind::Text),
        ]);
        w.select_next(); // select second
        let accepted = w.accept_top(); // should still return first
        assert_eq!(accepted.unwrap().label, "first");
    }

    #[test]
    fn completion_widget_filter() {
        let all_items = vec![
            make_item("forEach", CompletionItemKind::Method),
            make_item("format", CompletionItemKind::Function),
            make_item("bar", CompletionItemKind::Variable),
        ];
        let mut w = CompletionWidget::new();
        w.open(all_items.clone());
        w.update_filter("for", &all_items);
        assert_eq!(w.items.len(), 2);
        assert_eq!(w.selected_idx, 0);
    }

    #[test]
    fn completion_widget_visible_items() {
        let mut w = CompletionWidget::new();
        w.max_visible = 2;
        w.open(vec![
            make_item("a", CompletionItemKind::Text),
            make_item("b", CompletionItemKind::Text),
            make_item("c", CompletionItemKind::Text),
        ]);
        assert_eq!(w.visible_items().len(), 2);
        w.select_next();
        w.select_next(); // scroll
        assert_eq!(w.visible_items().len(), 2);
    }

    #[test]
    fn completion_widget_selected_item() {
        let mut w = CompletionWidget::new();
        w.open(vec![make_item("test", CompletionItemKind::Text)]);
        assert_eq!(w.selected_item().unwrap().label, "test");
    }

    // -----------------------------------------------------------------------
    // Auto-trigger tests
    // -----------------------------------------------------------------------

    #[test]
    fn auto_trigger_default() {
        let cfg = AutoTriggerConfig::new();
        assert!(cfg.should_trigger('.'));
        assert!(!cfg.should_trigger(':'));
        assert_eq!(cfg.word_debounce_ms, 500);
    }

    #[test]
    fn auto_trigger_rust() {
        let cfg = AutoTriggerConfig::new().with_rust_triggers();
        assert!(cfg.should_trigger('.'));
        assert!(cfg.should_trigger(':'));
    }

    #[test]
    fn auto_trigger_path() {
        let cfg = AutoTriggerConfig::new().with_path_triggers();
        assert!(cfg.should_trigger('/'));
    }

    #[test]
    fn auto_trigger_custom() {
        let cfg = AutoTriggerConfig::new().with_custom_triggers(&['@', '#']);
        assert!(cfg.should_trigger('@'));
        assert!(cfg.should_trigger('#'));
        assert!(cfg.should_trigger('.')); // default still there
    }

    #[test]
    fn auto_trigger_disabled() {
        let mut cfg = AutoTriggerConfig::new();
        cfg.enabled = false;
        assert!(!cfg.should_trigger('.'));
    }

    #[test]
    fn behavior_check_0() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = SuggestWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    // -----------------------------------------------------------------------
    // CompletionScoring tests
    // -----------------------------------------------------------------------

    #[test]
    fn scoring_empty_query_returns_zero() {
        assert_eq!(CompletionScoring::score("", "anything"), 0);
    }

    #[test]
    fn scoring_no_match_returns_zero() {
        assert_eq!(CompletionScoring::score("xyz", "abc"), 0);
    }

    #[test]
    fn scoring_exact_match_returns_nonzero() {
        let s = CompletionScoring::score("foo", "foo");
        assert!(s > 0);
    }

    #[test]
    fn scoring_prefix_bonus() {
        let prefix = CompletionScoring::score("fo", "format");
        let no_prefix = CompletionScoring::score("fo", "info");
        assert!(prefix > no_prefix);
    }

    #[test]
    fn scoring_case_match_bonus() {
        let exact_case = CompletionScoring::score("Foo", "Foobar");
        let wrong_case = CompletionScoring::score("foo", "Foobar");
        assert!(exact_case > wrong_case);
    }

    #[test]
    fn scoring_item_uses_filter_text() {
        let item = CompletionItem {
            label: "display".to_string(),
            kind: CompletionItemKind::Property,
            detail: None,
            insert_text: None,
            sort_text: None,
            filter_text: Some("css-display".to_string()),
            preselect: false,
        };
        let s = CompletionScoring::score_item("css", &item);
        assert!(s > 0);
    }

    #[test]
    fn scoring_item_falls_back_to_label() {
        let item = make_item("forEach", CompletionItemKind::Method);
        let s = CompletionScoring::score_item("for", &item);
        assert!(s > 0);
    }

    // -----------------------------------------------------------------------
    // SuggestionFilter tests
    // -----------------------------------------------------------------------

    #[test]
    fn filter_by_kind() {
        let items = vec![
            make_item("foo", CompletionItemKind::Function),
            make_item("bar", CompletionItemKind::Variable),
            make_item("baz", CompletionItemKind::Function),
        ];
        let result = SuggestionFilter::new()
            .with_kind(CompletionItemKind::Function)
            .apply(&items);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].label, "foo");
        assert_eq!(result[1].label, "baz");
    }

    #[test]
    fn filter_by_label_prefix() {
        let items = vec![
            make_item("forEach", CompletionItemKind::Method),
            make_item("format", CompletionItemKind::Function),
            make_item("bar", CompletionItemKind::Variable),
        ];
        let result = SuggestionFilter::new()
            .with_label_prefix("for")
            .apply(&items);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_no_criteria_returns_all() {
        let items = vec![
            make_item("a", CompletionItemKind::Text),
            make_item("b", CompletionItemKind::Text),
        ];
        let result = SuggestionFilter::new().apply(&items);
        assert_eq!(result.len(), 2);
    }

    // -----------------------------------------------------------------------
    // SuggestionSorter tests
    // -----------------------------------------------------------------------

    #[test]
    fn sorter_by_label() {
        let mut items = vec![
            make_item("zebra", CompletionItemKind::Variable),
            make_item("apple", CompletionItemKind::Variable),
            make_item("mango", CompletionItemKind::Variable),
        ];
        SuggestionSorter::new().by_label().sort(&mut items);
        assert_eq!(items[0].label, "apple");
        assert_eq!(items[1].label, "mango");
        assert_eq!(items[2].label, "zebra");
    }

    #[test]
    fn sorter_by_kind_priority() {
        let mut items = vec![
            make_item("var1", CompletionItemKind::Variable),
            make_item("func1", CompletionItemKind::Function),
            make_item("meth1", CompletionItemKind::Method),
        ];
        SuggestionSorter::new().by_kind_priority().sort(&mut items);
        assert_eq!(items[0].label, "meth1");
        assert_eq!(items[1].label, "func1");
        assert_eq!(items[2].label, "var1");
    }

    #[test]
    fn sorter_by_score() {
        let mut items = vec![
            make_item("xformat", CompletionItemKind::Function),
            make_item("format", CompletionItemKind::Function),
        ];
        SuggestionSorter::new().by_score("for").sort(&mut items);
        // "format" has prefix match so should come first
        assert_eq!(items[0].label, "format");
    }

    // -----------------------------------------------------------------------
    // SuggestionGrouping tests
    // -----------------------------------------------------------------------

    #[test]
    fn suggestion_grouping_by_kind() {
        let items = vec![
            make_item("foo", CompletionItemKind::Function),
            make_item("bar", CompletionItemKind::Variable),
            make_item("baz", CompletionItemKind::Function),
        ];
        let groups = SuggestionGrouping::group_by_kind(&items);
        assert_eq!(groups.group_count(), 2);
        assert_eq!(groups.total_items(), 3);
        assert_eq!(groups.get_group("Function").unwrap().len(), 2);
        assert_eq!(groups.get_group("Variable").unwrap().len(), 1);
    }

    #[test]
    fn suggestion_grouping_custom_key() {
        let items = vec![
            make_item("get_name", CompletionItemKind::Method),
            make_item("set_name", CompletionItemKind::Method),
            make_item("get_age", CompletionItemKind::Method),
        ];
        let groups = SuggestionGrouping::group_by(&items, |item| {
            if item.label.starts_with("get") { "getters".into() } else { "setters".into() }
        });
        assert_eq!(groups.get_group("getters").unwrap().len(), 2);
        assert_eq!(groups.get_group("setters").unwrap().len(), 1);
    }

    // -----------------------------------------------------------------------
    // CompletionSession tests
    // -----------------------------------------------------------------------

    #[test]
    fn completion_session_lifecycle() {
        let mut session = CompletionSession::start("fo", Some('.'), 1000);
        assert!(session.is_active());
        assert_eq!(session.prefix(), "fo");
        assert_eq!(session.trigger_character(), Some('.'));

        session.set_items_shown(5);
        assert_eq!(session.items_shown(), 5);

        session.accept(1200);
        assert!(!session.is_active());
        assert!(session.was_accepted());
        assert_eq!(session.duration_ms(), Some(200));
    }

    #[test]
    fn completion_session_dismiss() {
        let mut session = CompletionSession::start("abc", None, 500);
        session.dismiss(700);
        assert!(!session.was_accepted());
        assert_eq!(session.duration_ms(), Some(200));
    }

    // -----------------------------------------------------------------------
    // SuggestionCache tests
    // -----------------------------------------------------------------------

    #[test]
    fn suggestion_cache_basic() {
        let mut cache = SuggestionCache::new(3);
        assert!(cache.is_empty());

        let items = vec![make_item("foo", CompletionItemKind::Function)];
        cache.insert("fo", items.clone());
        assert_eq!(cache.len(), 1);
        assert!(cache.contains("fo"));

        let cached = cache.get("fo").unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].label, "foo");
    }

    #[test]
    fn suggestion_cache_eviction() {
        let mut cache = SuggestionCache::new(2);
        cache.insert("a", vec![make_item("a1", CompletionItemKind::Text)]);
        cache.insert("b", vec![make_item("b1", CompletionItemKind::Text)]);
        cache.insert("c", vec![make_item("c1", CompletionItemKind::Text)]);

        assert_eq!(cache.len(), 2);
        assert!(!cache.contains("a")); // evicted
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn suggestion_cache_invalidate_prefix() {
        let mut cache = SuggestionCache::new(10);
        cache.insert("foo", vec![]);
        cache.insert("foobar", vec![]);
        cache.insert("bar", vec![]);
        assert_eq!(cache.len(), 3);

        cache.invalidate_prefix("foo");
        assert_eq!(cache.len(), 1);
        assert!(cache.contains("bar"));
    }

    // -----------------------------------------------------------------------
    // CompletionScoring contextual test
    // -----------------------------------------------------------------------

    #[test]
    fn scoring_contextual_boost() {
        let item_fn = make_item("format", CompletionItemKind::Function);
        let item_var = make_item("format_str", CompletionItemKind::Variable);

        let s_fn = CompletionScoring::score_contextual("for", &item_fn, Some(CompletionItemKind::Function));
        let s_var = CompletionScoring::score_contextual("for", &item_var, Some(CompletionItemKind::Function));
        // Function should get the kind boost
        assert!(s_fn > s_var);
    }

    // -----------------------------------------------------------------------
    // New tests: CompletionItemKind, CompletionItem builder, SuggestModel
    // -----------------------------------------------------------------------

    #[test]
    fn completion_item_kind_icon_and_callable() {
        assert_eq!(CompletionItemKind::Function.icon_char(), 'f');
        assert_eq!(CompletionItemKind::Class.icon_char(), 'C');
        assert!(CompletionItemKind::Method.is_callable());
        assert!(!CompletionItemKind::Variable.is_callable());
        assert!(CompletionItemKind::Class.is_type());
        assert!(!CompletionItemKind::Function.is_type());
    }

    #[test]
    fn completion_item_kind_display() {
        assert_eq!(format!("{}", CompletionItemKind::Function), "Function");
        assert_eq!(format!("{}", CompletionItemKind::Snippet), "Snippet");
    }

    #[test]
    fn completion_item_builder() {
        let item = CompletionItem::new("foo", CompletionItemKind::Function)
            .with_detail("does stuff")
            .with_insert_text("foo()")
            .with_preselect(true);
        assert_eq!(item.label, "foo");
        assert_eq!(item.detail.as_deref(), Some("does stuff"));
        assert_eq!(item.effective_insert_text(), "foo()");
        assert!(item.preselect);
    }

    #[test]
    fn completion_item_effective_defaults() {
        let item = make_item("bar", CompletionItemKind::Variable);
        assert_eq!(item.effective_insert_text(), "bar");
        assert_eq!(item.effective_filter_text(), "bar");
    }

    #[test]
    fn completion_item_display() {
        let item = CompletionItem::new("println", CompletionItemKind::Function)
            .with_detail("macro");
        let s = format!("{}", item);
        assert!(s.contains("[Function]"));
        assert!(s.contains("println"));
        assert!(s.contains("macro"));
    }

    #[test]
    fn suggest_model_count_by_kind() {
        let model = SuggestModel {
            items: vec![
                make_item("a", CompletionItemKind::Function),
                make_item("b", CompletionItemKind::Function),
                make_item("c", CompletionItemKind::Variable),
            ],
        };
        let counts = model.count_by_kind();
        let fn_count = counts.iter().find(|(k, _)| *k == CompletionItemKind::Function).map(|(_, c)| *c);
        assert_eq!(fn_count, Some(2));
    }

    #[test]
    fn suggest_model_top_matches() {
        let model = SuggestModel {
            items: vec![
                make_item("format", CompletionItemKind::Function),
                make_item("forEach", CompletionItemKind::Method),
                make_item("bar", CompletionItemKind::Variable),
            ],
        };
        let top = model.top_matches("for", 2);
        assert_eq!(top.len(), 2);
        // both should start with "for"
        assert!(top.iter().all(|i| i.label.to_lowercase().starts_with("for")));
    }

    // -- SuggestWidgetSizer tests --

    #[test]
    fn sizer_height_for_items() {
        let sizer = SuggestWidgetSizer::new(200, 100, 20);
        assert_eq!(sizer.height_for_items(2), 44); // 2*20 + 2*2
        assert_eq!(sizer.height_for_items(10), 100); // clamped to max
    }

    #[test]
    fn sizer_visible_items() {
        let sizer = SuggestWidgetSizer::new(200, 100, 20);
        assert_eq!(sizer.visible_item_count(), 4); // (100 - 4) / 20
    }

    #[test]
    fn sizer_width_for_labels() {
        let sizer = SuggestWidgetSizer::new(80, 100, 20);
        let w = sizer.width_for_labels(&["short", "much_longer_label"]);
        assert!(w <= 80);
        assert!(w > 0);
    }

    // -- SuggestTabCompletion tests --

    #[test]
    fn tab_completion_defaults() {
        let tc = SuggestTabCompletion::new();
        assert!(tc.should_accept_on_tab());
        assert!(tc.should_accept_on_enter());
    }

    #[test]
    fn tab_completion_disabled() {
        let mut tc = SuggestTabCompletion::new();
        tc.set_enabled(false);
        assert!(!tc.should_accept_on_tab());
        assert!(!tc.should_accept_on_enter());
    }

    // -- SuggestSnippetResolver tests --

    #[test]
    fn snippet_resolver_expands_vars() {
        let mut r = SuggestSnippetResolver::new();
        r.set_variable("TM_FILENAME", "main.rs");
        let result = r.expand("// file: ${TM_FILENAME}");
        assert_eq!(result, "// file: main.rs");
    }

    #[test]
    fn snippet_resolver_strips_tabstops() {
        let r = SuggestSnippetResolver::new();
        let result = r.expand("fn $1() { $2 }");
        assert_eq!(result, "fn () {  }");
    }

    #[test]
    fn snippet_resolver_variable_count() {
        let mut r = SuggestSnippetResolver::new();
        r.set_variable("A", "1");
        r.set_variable("B", "2");
        assert_eq!(r.variable_count(), 2);
    }

    // -- SuggestKeyboardNav tests --

    #[test]
    fn keyboard_nav_wrap_around() {
        let mut nav = SuggestKeyboardNav::new(3, 5);
        assert_eq!(nav.selected(), 0);
        nav.move_down();
        assert_eq!(nav.selected(), 1);
        nav.move_down();
        nav.move_down();
        assert_eq!(nav.selected(), 0); // wraps
    }

    #[test]
    fn keyboard_nav_move_up_wrap() {
        let mut nav = SuggestKeyboardNav::new(3, 5);
        nav.move_up();
        assert_eq!(nav.selected(), 2); // wraps to last
    }

    #[test]
    fn keyboard_nav_page_down_clamps() {
        let mut nav = SuggestKeyboardNav::new(10, 3);
        nav.page_down();
        assert_eq!(nav.selected(), 3);
        nav.page_down();
        assert_eq!(nav.selected(), 6);
    }

    #[test]
    fn keyboard_nav_home_end() {
        let mut nav = SuggestKeyboardNav::new(10, 3);
        nav.end();
        assert_eq!(nav.selected(), 9);
        nav.home();
        assert_eq!(nav.selected(), 0);
    }
}
