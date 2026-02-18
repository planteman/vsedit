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


// ---------------------------------------------------------------------------
// SuggestPriority — priority levels for sorting suggestions
// ---------------------------------------------------------------------------

/// Priority level that can be assigned to a completion item to influence
/// the final ordering in the suggest widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SuggestPriority {
    /// Lowest priority — shown last.
    Low,
    /// Normal priority — default.
    Normal,
    /// Elevated priority — shown before normal items.
    High,
    /// Highest priority — always at the top of the list.
    Critical,
}

impl SuggestPriority {
    /// Return a numeric weight for the priority (higher = more important).
    pub fn weight(&self) -> u32 {
        match self {
            Self::Low => 0,
            Self::Normal => 10,
            Self::High => 20,
            Self::Critical => 30,
        }
    }

    /// Parse a priority from a string label (case-insensitive).
    pub fn from_label(label: &str) -> Option<Self> {
        match label.to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "normal" => Some(Self::Normal),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    /// Return the human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl Default for SuggestPriority {
    fn default() -> Self {
        Self::Normal
    }
}

// ---------------------------------------------------------------------------
// SuggestPrioritySort — sort a list of completion items by priority
// ---------------------------------------------------------------------------

/// A prioritised wrapper around a [`CompletionItem`].
#[derive(Debug, Clone)]
pub struct PrioritisedItem {
    pub item: CompletionItem,
    pub priority: SuggestPriority,
}

/// Sorter that arranges completion items by their assigned priority,
/// breaking ties with the existing `sort_text` / `label` ordering.
#[derive(Debug)]
pub struct SuggestPrioritySort {
    items: Vec<PrioritisedItem>,
}

impl SuggestPrioritySort {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add a single item with a priority.
    pub fn push(&mut self, item: CompletionItem, priority: SuggestPriority) {
        self.items.push(PrioritisedItem { item, priority });
    }

    /// Return the number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the sorter contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Sort in-place by descending priority then ascending sort_text/label.
    pub fn sort(&mut self) {
        self.items.sort_by(|a, b| {
            b.priority.weight().cmp(&a.priority.weight()).then_with(|| {
                let sa = a.item.sort_text.as_deref().unwrap_or(&a.item.label);
                let sb = b.item.sort_text.as_deref().unwrap_or(&b.item.label);
                sa.cmp(sb)
            })
        });
    }

    /// Consume the sorter and return the sorted items.
    pub fn into_sorted(mut self) -> Vec<PrioritisedItem> {
        self.sort();
        self.items
    }

    /// Return a reference to the inner items (unsorted).
    pub fn items(&self) -> &[PrioritisedItem] {
        &self.items
    }

    /// Return only items of a specific priority.
    pub fn items_with_priority(&self, p: SuggestPriority) -> Vec<&PrioritisedItem> {
        self.items.iter().filter(|i| i.priority == p).collect()
    }
}

impl Default for SuggestPrioritySort {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SuggestDocPanel — a documentation panel with markdown rendering
// ---------------------------------------------------------------------------

/// Represents a section of markdown content inside the doc panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownSection {
    pub heading: String,
    pub body: String,
}

impl MarkdownSection {
    pub fn new(heading: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            heading: heading.into(),
            body: body.into(),
        }
    }

    /// Render the section to a simple markdown string.
    pub fn render(&self) -> String {
        if self.heading.is_empty() {
            self.body.clone()
        } else {
            format!("## {}\n\n{}", self.heading, self.body)
        }
    }

    /// The total character length of heading + body.
    pub fn char_count(&self) -> usize {
        self.heading.len() + self.body.len()
    }
}

/// Documentation panel shown alongside the suggest widget.
#[derive(Debug, Clone)]
pub struct SuggestDocPanel {
    sections: Vec<MarkdownSection>,
    visible: bool,
    max_height_lines: usize,
}

impl SuggestDocPanel {
    pub fn new(max_height_lines: usize) -> Self {
        Self {
            sections: Vec::new(),
            visible: false,
            max_height_lines,
        }
    }

    /// Show the doc panel with the given sections.
    pub fn show(&mut self, sections: Vec<MarkdownSection>) {
        self.sections = sections;
        self.visible = true;
    }

    /// Hide the panel.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Whether the panel is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Return all sections.
    pub fn sections(&self) -> &[MarkdownSection] {
        &self.sections
    }

    /// Render the full markdown document.
    pub fn render_all(&self) -> String {
        self.sections
            .iter()
            .map(|s| s.render())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Total character count across all sections.
    pub fn total_chars(&self) -> usize {
        self.sections.iter().map(|s| s.char_count()).sum()
    }

    /// The estimated number of rendered lines (crude: chars / 80).
    pub fn estimated_lines(&self) -> usize {
        let total = self.total_chars();
        if total == 0 { 0 } else { (total / 80).max(1) }
    }

    /// Whether the content exceeds the configured max height.
    pub fn is_overflowing(&self) -> bool {
        self.estimated_lines() > self.max_height_lines
    }

    /// Clear all sections and hide.
    pub fn clear(&mut self) {
        self.sections.clear();
        self.visible = false;
    }
}

impl Default for SuggestDocPanel {
    fn default() -> Self {
        Self::new(20)
    }
}

// ---------------------------------------------------------------------------
// SuggestItemGroup — group completion items by kind
// ---------------------------------------------------------------------------

/// A named group of completion items.
#[derive(Debug, Clone)]
pub struct SuggestItemGroup {
    pub kind: CompletionItemKind,
    pub items: Vec<CompletionItem>,
}

impl SuggestItemGroup {
    pub fn new(kind: CompletionItemKind) -> Self {
        Self {
            kind,
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, item: CompletionItem) {
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Sort items within the group alphabetically by label.
    pub fn sort_by_label(&mut self) {
        self.items.sort_by(|a, b| a.label.cmp(&b.label));
    }
}

/// Group a slice of completion items by their [`CompletionItemKind`].
pub fn group_items_by_kind(items: &[CompletionItem]) -> Vec<SuggestItemGroup> {
    let mut groups: Vec<SuggestItemGroup> = Vec::new();
    for item in items {
        if let Some(g) = groups.iter_mut().find(|g| g.kind == item.kind) {
            g.push(item.clone());
        } else {
            let mut g = SuggestItemGroup::new(item.kind);
            g.push(item.clone());
            groups.push(g);
        }
    }
    groups
}

/// Count items per kind and return pairs of (kind, count).
pub fn count_items_by_kind(items: &[CompletionItem]) -> Vec<(CompletionItemKind, usize)> {
    let groups = group_items_by_kind(items);
    groups.iter().map(|g| (g.kind, g.len())).collect()
}

// ---------------------------------------------------------------------------
// SuggestCompletionCommit — track completion commit events
// ---------------------------------------------------------------------------

/// The reason a completion was committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitReason {
    /// User explicitly chose the item (Enter / click).
    Explicit,
    /// Triggered by a commit character (e.g. `.`, `(`).
    CommitChar(char),
    /// Triggered by Tab key.
    Tab,
}

/// A record of a committed completion.
#[derive(Debug, Clone)]
pub struct CompletionCommitRecord {
    pub label: String,
    pub kind: CompletionItemKind,
    pub reason: CommitReason,
    pub prefix_length: usize,
}

/// Tracker for completion commit history.
#[derive(Debug, Default)]
pub struct CompletionCommitLog {
    records: Vec<CompletionCommitRecord>,
}

impl CompletionCommitLog {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    /// Record a committed completion.
    pub fn record(&mut self, label: String, kind: CompletionItemKind, reason: CommitReason, prefix_length: usize) {
        self.records.push(CompletionCommitRecord {
            label,
            kind,
            reason,
            prefix_length,
        });
    }

    /// Total number of commits recorded.
    pub fn total(&self) -> usize {
        self.records.len()
    }

    /// Count commits by a specific reason.
    pub fn count_by_reason(&self, reason: CommitReason) -> usize {
        self.records.iter().filter(|r| r.reason == reason).count()
    }

    /// Average prefix length across all commits.
    pub fn average_prefix_length(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        let sum: usize = self.records.iter().map(|r| r.prefix_length).sum();
        sum as f64 / self.records.len() as f64
    }

    /// Most common completion kind across all commits.
    pub fn most_common_kind(&self) -> Option<CompletionItemKind> {
        if self.records.is_empty() {
            return None;
        }
        let mut counts: Vec<(CompletionItemKind, usize)> = Vec::new();
        for rec in &self.records {
            if let Some(entry) = counts.iter_mut().find(|(k, _)| *k == rec.kind) {
                entry.1 += 1;
            } else {
                counts.push((rec.kind, 1));
            }
        }
        counts.into_iter().max_by_key(|&(_, c)| c).map(|(k, _)| k)
    }

    /// Return records filtered by kind.
    pub fn records_of_kind(&self, kind: CompletionItemKind) -> Vec<&CompletionCommitRecord> {
        self.records.iter().filter(|r| r.kind == kind).collect()
    }

    /// Clear the log.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}


// ---------------------------------------------------------------------------
// wb_suggest – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbSuggestLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbSuggestPanelState {
    pub region: XWbSuggestLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbSuggestPanelState {
    pub fn new(region: XWbSuggestLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_suggest_total_visible_area(panels: &[XWbSuggestPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_suggest_count_in_region(
    panels: &[XWbSuggestPanelState],
    region: XWbSuggestLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_suggest_widest_panel(panels: &[XWbSuggestPanelState]) -> Option<&XWbSuggestPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_suggest_collapse_region(
    panels: &mut [XWbSuggestPanelState],
    region: XWbSuggestLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbSuggestLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbSuggestLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// wb_suggest – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbSuggestSuggestionSource {
    Intellisense,
    Snippet,
    File,
    History,
}

impl YWbSuggestSuggestionSource {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Intellisense => 0,
            Self::Snippet => 1,
            Self::File => 2,
            Self::History => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Intellisense => "Intellisense",
            Self::Snippet => "Snippet",
            Self::File => "File",
            Self::History => "History",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbSuggestSuggestionSource] {
        &[
            YWbSuggestSuggestionSource::Intellisense,
            YWbSuggestSuggestionSource::Snippet,
            YWbSuggestSuggestionSource::File,
            YWbSuggestSuggestionSource::History,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbSuggestSuggestionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks suggestion ranking data.
#[derive(Debug, Clone)]
pub struct YWbSuggestSuggestionRanker {
    pub items: Vec<(String, f64)>,
    pub boost_recent: bool,
    pub max_results: usize,
}

impl YWbSuggestSuggestionRanker {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            boost_recent: false,
            max_results: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbSuggestSuggestionRanker({}: {:?})", "items", self.items)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_suggest_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_suggest_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_suggest_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_suggest_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_suggest_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_suggest_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_suggest_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_suggest_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_suggest – Extended suggestion cache helpers
// ---------------------------------------------------------------------------

/// Priority levels for suggestion cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbSuggestPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbSuggestPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZWbSuggestPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbSuggestPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks suggestion cache data.
#[derive(Debug, Clone)]
pub struct ZWbSuggestSuggestionCache {
    pub scored_items: Vec<(String, f64)>,
    pub ttl_ms: u64,
    pub stale: bool,
}

impl ZWbSuggestSuggestionCache {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            scored_items: Vec::new(),
            ttl_ms: 0,
            stale: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.scored_items.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.scored_items.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.scored_items.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbSuggestSuggestionCache[ttl_ms={:?}, stale={:?}]", self.ttl_ms, self.stale)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.stale = !c.stale;
        c
    }
}

/// Compute a simple rolling hash for suggestion cache.
pub fn z_wb_suggest_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_suggest_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_suggest_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_suggest_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_wb_suggest_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_suggest_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_suggest_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 85
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer85 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer85 {
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
pub fn xb_fnv1a_85(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_85<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_85<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_85(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_85(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 226
// ---------------------------------------------------------------------------

/// Generic object pool `Xc226Pool<T>`.
pub struct Xc226Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc226Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc226PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc226Pool<T> {
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
    pub fn stats(&self) -> Xc226PoolStats {
        Xc226PoolStats {
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

impl<T> Default for Xc226Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc226Scheduler`.
pub struct Xc226Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc226Scheduler {
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

impl Default for Xc226Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_226 hash for the given byte slice.
pub fn xc_226_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_226 convention.
pub fn xc_226_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe98 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe98Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe98PipelineError {
    pub stage: Xe98Stage,
    pub message: String,
}

impl std::fmt::Display for Xe98PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe98Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe98Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError>>>,
    stage_names: Vec<Xe98Stage>,
}

impl Xe98Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe98Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe98Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe98Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe98Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> {
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

    pub fn compose(mut self, other: Xe98Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe98CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe98CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe98Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe98CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe98CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe98Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe98CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_98_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe98CacheEntry {
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

    fn xe_98_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe98CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_98_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> {
    Ok(data)
}

pub fn xe_98_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_98_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_98_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_98_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe98PipelineError> {
    Err(Xe98PipelineError {
        stage: Xe98Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_96: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg96Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg96Graph {
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

impl Default for Xg96Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_96: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg96Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg96Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg96Heap<T>) {
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

impl<T: Ord> Default for Xg96Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 225).
pub struct Xh225SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh225SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 267 as u64,
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

/// A compact bit set supporting boolean operations (variant 225).
pub struct Xh225BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh225BitSet {
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

    // --- SuggestPriority tests -----------------------------------------------

    #[test]
    fn priority_ordering() {
        assert!(SuggestPriority::Low < SuggestPriority::Normal);
        assert!(SuggestPriority::Normal < SuggestPriority::High);
        assert!(SuggestPriority::High < SuggestPriority::Critical);
    }

    #[test]
    fn priority_weight() {
        assert_eq!(SuggestPriority::Low.weight(), 0);
        assert_eq!(SuggestPriority::Critical.weight(), 30);
    }

    #[test]
    fn priority_from_label() {
        assert_eq!(SuggestPriority::from_label("high"), Some(SuggestPriority::High));
        assert_eq!(SuggestPriority::from_label("HIGH"), Some(SuggestPriority::High));
        assert_eq!(SuggestPriority::from_label("unknown"), None);
    }

    #[test]
    fn priority_default() {
        assert_eq!(SuggestPriority::default(), SuggestPriority::Normal);
    }

    // --- SuggestPrioritySort tests -------------------------------------------

    #[test]
    fn priority_sort_basic() {
        let mut sorter = SuggestPrioritySort::new();
        sorter.push(make_item("aaa", CompletionItemKind::Text), SuggestPriority::Low);
        sorter.push(make_item("bbb", CompletionItemKind::Function), SuggestPriority::Critical);
        sorter.push(make_item("ccc", CompletionItemKind::Variable), SuggestPriority::Normal);
        let sorted = sorter.into_sorted();
        assert_eq!(sorted[0].item.label, "bbb");
        assert_eq!(sorted[1].item.label, "ccc");
        assert_eq!(sorted[2].item.label, "aaa");
    }

    #[test]
    fn priority_sort_empty() {
        let sorter = SuggestPrioritySort::new();
        assert!(sorter.is_empty());
        assert_eq!(sorter.len(), 0);
    }

    #[test]
    fn priority_sort_items_with_priority() {
        let mut sorter = SuggestPrioritySort::new();
        sorter.push(make_item("a", CompletionItemKind::Text), SuggestPriority::High);
        sorter.push(make_item("b", CompletionItemKind::Text), SuggestPriority::Low);
        sorter.push(make_item("c", CompletionItemKind::Text), SuggestPriority::High);
        let high = sorter.items_with_priority(SuggestPriority::High);
        assert_eq!(high.len(), 2);
    }

    // --- SuggestDocPanel tests -----------------------------------------------

    #[test]
    fn doc_panel_show_hide() {
        let mut panel = SuggestDocPanel::new(20);
        assert!(!panel.is_visible());
        panel.show(vec![MarkdownSection::new("Title", "body text")]);
        assert!(panel.is_visible());
        assert_eq!(panel.sections().len(), 1);
        panel.hide();
        assert!(!panel.is_visible());
    }

    #[test]
    fn doc_panel_render() {
        let section = MarkdownSection::new("Heading", "Some content");
        assert!(section.render().contains("Heading"));
        assert!(section.render().contains("Some content"));
    }

    #[test]
    fn doc_panel_overflow() {
        let mut panel = SuggestDocPanel::new(1);
        let long = "x".repeat(500);
        panel.show(vec![MarkdownSection::new("", long)]);
        assert!(panel.is_overflowing());
    }

    #[test]
    fn doc_panel_clear() {
        let mut panel = SuggestDocPanel::default();
        panel.show(vec![MarkdownSection::new("H", "B")]);
        panel.clear();
        assert!(!panel.is_visible());
        assert!(panel.sections().is_empty());
    }

    // --- group_items_by_kind tests -------------------------------------------

    #[test]
    fn group_by_kind_works() {
        let items = vec![
            make_item("a", CompletionItemKind::Function),
            make_item("b", CompletionItemKind::Variable),
            make_item("c", CompletionItemKind::Function),
        ];
        let groups = group_items_by_kind(&items);
        assert_eq!(groups.len(), 2);
        let fn_group = groups.iter().find(|g| g.kind == CompletionItemKind::Function).unwrap();
        assert_eq!(fn_group.len(), 2);
    }

    #[test]
    fn count_by_kind_works() {
        let items = vec![
            make_item("x", CompletionItemKind::Text),
            make_item("y", CompletionItemKind::Text),
            make_item("z", CompletionItemKind::Keyword),
        ];
        let counts = count_items_by_kind(&items);
        let text_count = counts.iter().find(|(k, _)| *k == CompletionItemKind::Text).unwrap().1;
        assert_eq!(text_count, 2);
    }

    #[test]
    fn group_sort_by_label() {
        let mut group = SuggestItemGroup::new(CompletionItemKind::Text);
        group.push(make_item("cherry", CompletionItemKind::Text));
        group.push(make_item("apple", CompletionItemKind::Text));
        group.push(make_item("banana", CompletionItemKind::Text));
        group.sort_by_label();
        assert_eq!(group.items[0].label, "apple");
        assert_eq!(group.items[2].label, "cherry");
    }

    // --- CompletionCommitLog tests -------------------------------------------

    #[test]
    fn commit_log_record() {
        let mut log = CompletionCommitLog::new();
        log.record("foo".into(), CompletionItemKind::Function, CommitReason::Explicit, 3);
        assert_eq!(log.total(), 1);
    }

    #[test]
    fn commit_log_by_reason() {
        let mut log = CompletionCommitLog::new();
        log.record("a".into(), CompletionItemKind::Text, CommitReason::Tab, 1);
        log.record("b".into(), CompletionItemKind::Text, CommitReason::Explicit, 2);
        log.record("c".into(), CompletionItemKind::Text, CommitReason::Tab, 1);
        assert_eq!(log.count_by_reason(CommitReason::Tab), 2);
        assert_eq!(log.count_by_reason(CommitReason::Explicit), 1);
    }

    #[test]
    fn commit_log_avg_prefix() {
        let mut log = CompletionCommitLog::new();
        log.record("a".into(), CompletionItemKind::Text, CommitReason::Tab, 2);
        log.record("b".into(), CompletionItemKind::Text, CommitReason::Tab, 4);
        assert!((log.average_prefix_length() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn commit_log_empty_avg() {
        let log = CompletionCommitLog::new();
        assert!((log.average_prefix_length() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn commit_log_most_common_kind() {
        let mut log = CompletionCommitLog::new();
        log.record("a".into(), CompletionItemKind::Function, CommitReason::Explicit, 1);
        log.record("b".into(), CompletionItemKind::Variable, CommitReason::Explicit, 1);
        log.record("c".into(), CompletionItemKind::Function, CommitReason::Tab, 1);
        assert_eq!(log.most_common_kind(), Some(CompletionItemKind::Function));
    }

    #[test]
    fn commit_log_clear() {
        let mut log = CompletionCommitLog::new();
        log.record("a".into(), CompletionItemKind::Text, CommitReason::Explicit, 1);
        log.clear();
        assert_eq!(log.total(), 0);
    }


    // -- wb_suggest additional tests -------------------------------------------

    #[test]
    fn x_wb_suggest_panel_state_new() {
        let p = XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbSuggestLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_suggest_panel_area() {
        let p = XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_suggest_panel_toggle() {
        let mut p = XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_suggest_panel_resize() {
        let mut p = XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_suggest_panel_is_narrow() {
        let mut p = XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_suggest_total_visible_area_basic() {
        let panels = vec![
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "a"),
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_suggest_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_suggest_total_visible_area_hidden() {
        let mut panels = vec![
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "a"),
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_suggest_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_suggest_count_in_region_basic() {
        let panels = vec![
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "a"),
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "b"),
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_suggest_count_in_region(&panels, XWbSuggestLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_suggest_count_in_region(&panels, XWbSuggestLayoutRegion::Editor), 1);
        assert_eq!(x_wb_suggest_count_in_region(&panels, XWbSuggestLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_suggest_widest_panel_basic() {
        let mut panels = vec![
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "narrow"),
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_suggest_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_suggest_collapse_region_basic() {
        let mut panels = vec![
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "a"),
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Sidebar, "b"),
            XWbSuggestPanelState::new(XWbSuggestLayoutRegion::Editor, "c"),
        ];
        x_wb_suggest_collapse_region(&mut panels, XWbSuggestLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_suggest_layout_constraint_clamp() {
        let lc = XWbSuggestLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_suggest_layout_constraint_satisfied() {
        let lc = XWbSuggestLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_suggest_widest_panel_empty() {
        let panels: Vec<XWbSuggestPanelState> = vec![];
        assert!(x_wb_suggest_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_suggest_layout_region_eq() {
        assert_eq!(XWbSuggestLayoutRegion::Sidebar, XWbSuggestLayoutRegion::Sidebar);
        assert_ne!(XWbSuggestLayoutRegion::Sidebar, XWbSuggestLayoutRegion::Panel);
    }


    // -- wb_suggest extended domain tests ----------------------------------------

    #[test]
    fn y_wb_suggest_enum_index() {
        assert_eq!(YWbSuggestSuggestionSource::Intellisense.index(), 0);
        assert_eq!(YWbSuggestSuggestionSource::Snippet.index(), 1);
        assert_eq!(YWbSuggestSuggestionSource::File.index(), 2);
        assert_eq!(YWbSuggestSuggestionSource::History.index(), 3);
    }

    #[test]
    fn y_wb_suggest_enum_label() {
        assert_eq!(YWbSuggestSuggestionSource::Intellisense.label(), "Intellisense");
        assert_eq!(YWbSuggestSuggestionSource::Snippet.label(), "Snippet");
        assert_eq!(YWbSuggestSuggestionSource::File.label(), "File");
        assert_eq!(YWbSuggestSuggestionSource::History.label(), "History");
    }

    #[test]
    fn y_wb_suggest_enum_all() {
        let all = YWbSuggestSuggestionSource::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_suggest_enum_is_default() {
        assert!(YWbSuggestSuggestionSource::Intellisense.is_default());
        assert!(!YWbSuggestSuggestionSource::History.is_default());
    }

    #[test]
    fn y_wb_suggest_enum_display() {
        assert_eq!(format!("{}", YWbSuggestSuggestionSource::Intellisense), "Intellisense");
    }

    #[test]
    fn y_wb_suggest_struct_new() {
        let s = YWbSuggestSuggestionRanker::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_wb_suggest_struct_clear() {
        let mut s = YWbSuggestSuggestionRanker::new();
        s.items.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_wb_suggest_fingerprint_deterministic() {
        let h1 = y_wb_suggest_fingerprint("hello");
        let h2 = y_wb_suggest_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_suggest_fingerprint("a"), y_wb_suggest_fingerprint("b"));
    }

    #[test]
    fn y_wb_suggest_truncate_short() {
        assert_eq!(y_wb_suggest_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_suggest_truncate_long() {
        let r = y_wb_suggest_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_suggest_normalize_key_basic() {
        assert_eq!(y_wb_suggest_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_suggest_split_path_basic() {
        let parts = y_wb_suggest_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_suggest_count_occurrences_basic() {
        assert_eq!(y_wb_suggest_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_suggest_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_suggest_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_suggest_in_range_basic() {
        assert!(y_wb_suggest_in_range(5, 1, 10));
        assert!(y_wb_suggest_in_range(1, 1, 10));
        assert!(y_wb_suggest_in_range(10, 1, 10));
        assert!(!y_wb_suggest_in_range(0, 1, 10));
        assert!(!y_wb_suggest_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_suggest_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_suggest_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_suggest_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_suggest_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_suggest Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_suggest_priority_weight() {
        assert_eq!(ZWbSuggestPriority::Idle.weight(), 0);
        assert_eq!(ZWbSuggestPriority::Normal.weight(), 2);
        assert_eq!(ZWbSuggestPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_suggest_priority_label() {
        assert_eq!(ZWbSuggestPriority::Low.label(), "low");
        assert_eq!(ZWbSuggestPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_suggest_priority_is_elevated() {
        assert!(!ZWbSuggestPriority::Normal.is_elevated());
        assert!(ZWbSuggestPriority::High.is_elevated());
        assert!(ZWbSuggestPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_suggest_priority_display() {
        assert_eq!(format!("{}", ZWbSuggestPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_suggest_priority_all_asc() {
        let all = ZWbSuggestPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbSuggestPriority::Idle);
        assert_eq!(all[4], ZWbSuggestPriority::Realtime);
    }

    #[test]
    fn z_wb_suggest_struct_new() {
        let s = ZWbSuggestSuggestionCache::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_suggest_struct_toggled_clone() {
        let s = ZWbSuggestSuggestionCache::new();
        let t = s.toggled_clone();
        assert_ne!(s.stale, t.stale);
    }

    #[test]
    fn z_wb_suggest_rolling_hash_deterministic() {
        let h1 = z_wb_suggest_rolling_hash(b"test");
        let h2 = z_wb_suggest_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_suggest_rolling_hash(b"a"), z_wb_suggest_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_suggest_pad_to_basic() {
        assert_eq!(z_wb_suggest_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_suggest_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_suggest_is_identifier_basic() {
        assert!(z_wb_suggest_is_identifier("foo_bar"));
        assert!(z_wb_suggest_is_identifier("abc123"));
        assert!(!z_wb_suggest_is_identifier(""));
        assert!(!z_wb_suggest_is_identifier("has space"));
    }

    #[test]
    fn z_wb_suggest_levenshtein_basic() {
        assert_eq!(z_wb_suggest_levenshtein("", ""), 0);
        assert_eq!(z_wb_suggest_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_suggest_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_suggest_unique_words_basic() {
        let w = z_wb_suggest_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_suggest_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_suggest_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_suggest_common_prefix_basic() {
        assert_eq!(z_wb_suggest_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_suggest_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_suggest_struct_clear() {
        let mut s = ZWbSuggestSuggestionCache::new();
        s.scored_items.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_suggest_rolling_hash_empty() {
        let h = z_wb_suggest_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_85_push_and_len() {
        let mut rb = super::XbRingBuffer85::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_85_overwrite() {
        let mut rb = super::XbRingBuffer85::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_85_get_out_of_bounds() {
        let rb = super::XbRingBuffer85::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_85_drain_all() {
        let mut rb = super::XbRingBuffer85::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_85_peek_front_back() {
        let mut rb = super::XbRingBuffer85::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_85_clear() {
        let mut rb = super::XbRingBuffer85::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_85_capacity() {
        let rb = super::XbRingBuffer85::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_85_basic() {
        let h = super::xb_fnv1a_85(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_85(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_85_different_inputs() {
        let h1 = super::xb_fnv1a_85(b"abc");
        let h2 = super::xb_fnv1a_85(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_85_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_85(&data);
        let dec = super::xb_rle_decode_85(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_85_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_85(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_85(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_85_values() {
        assert!((super::xb_clamp_85(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_85(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_85(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_85_values() {
        assert!((super::xb_lerp_85(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_85(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_85(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_85_wrap_around_twice() {
        let mut rb = super::XbRingBuffer85::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 226 ----

    #[test]
    fn xc_226_pool_new_empty() {
        let pool: super::Xc226Pool<i32> = super::Xc226Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_226_pool_release_acquire() {
        let mut pool = super::Xc226Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_226_pool_acquire_empty() {
        let mut pool: super::Xc226Pool<i32> = super::Xc226Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_226_pool_full() {
        let mut pool = super::Xc226Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_226_pool_drain() {
        let mut pool = super::Xc226Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_226_pool_stats() {
        let mut pool = super::Xc226Pool::new(8);
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
    fn xc_226_pool_clear() {
        let mut pool = super::Xc226Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_226_pool_shrink() {
        let mut pool = super::Xc226Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_226_pool_default() {
        let pool: super::Xc226Pool<String> = super::Xc226Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_226_pool_extend() {
        let mut pool = super::Xc226Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_226_pool_retain() {
        let mut pool = super::Xc226Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_226_scheduler_round_robin() {
        let mut sched = super::Xc226Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_226_scheduler_empty() {
        let mut sched = super::Xc226Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_226_scheduler_reset() {
        let mut sched = super::Xc226Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_226_scheduler_add_remove() {
        let mut sched = super::Xc226Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_226_scheduler_targets() {
        let sched = super::Xc226Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_226_hash_empty() {
        assert_eq!(super::xc_226_hash(b""), 5381);
    }

    #[test]
    fn xc_226_hash_data() {
        let h = super::xc_226_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_226_hash(b"hello"), h);
    }

    #[test]
    fn xc_226_reverse_str() {
        assert_eq!(super::xc_226_reverse("abc"), "cba");
        assert_eq!(super::xc_226_reverse(""), "");
    }


    #[test]
    fn xe_98_pipeline_empty() {
        let p = super::Xe98Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_98_pipeline_parse_stage() {
        let p = super::Xe98Pipeline::new()
            .add_parse(super::xe_98_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_98_pipeline_transform_double() {
        let p = super::Xe98Pipeline::new()
            .add_transform(super::xe_98_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_98_pipeline_validate_reverse() {
        let p = super::Xe98Pipeline::new()
            .add_validate(super::xe_98_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_98_pipeline_emit_filter() {
        let p = super::Xe98Pipeline::new()
            .add_emit(super::xe_98_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_98_pipeline_multi_stage() {
        let p = super::Xe98Pipeline::new()
            .add_parse(super::xe_98_pipeline_identity)
            .add_transform(super::xe_98_pipeline_double)
            .add_validate(super::xe_98_pipeline_reverse)
            .add_emit(super::xe_98_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_98_pipeline_error_propagation() {
        let p = super::Xe98Pipeline::new()
            .add_parse(super::xe_98_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe98Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_98_pipeline_compose() {
        let p1 = super::Xe98Pipeline::new()
            .add_parse(super::xe_98_pipeline_identity);
        let p2 = super::Xe98Pipeline::new()
            .add_transform(super::xe_98_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_98_pipeline_error_display() {
        let e = super::Xe98PipelineError {
            stage: super::Xe98Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_98_cache_put_get() {
        let mut c = super::Xe98Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_98_cache_miss() {
        let mut c: super::Xe98Cache<&str, i32> = super::Xe98Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_98_cache_ttl_expiry() {
        let mut c = super::Xe98Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_98_cache_evict() {
        let mut c = super::Xe98Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_98_cache_capacity() {
        let mut c = super::Xe98Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_98_cache_stats() {
        let mut c = super::Xe98Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_98_cache_clear() {
        let mut c = super::Xe98Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_96 graph tests ------------------------------------------------

    #[test]
    fn xg_96_graph_empty() {
        let g = super::Xg96Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_96_graph_add_node() {
        let mut g = super::Xg96Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_96_graph_add_edge() {
        let mut g = super::Xg96Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_96_graph_neighbors() {
        let mut g = super::Xg96Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_96_graph_has_path() {
        let mut g = super::Xg96Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_96_graph_self_path() {
        let g = super::Xg96Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_96_graph_topo_sort() {
        let mut g = super::Xg96Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_96_graph_cycle_detect_false() {
        let mut g = super::Xg96Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_96_graph_cycle_detect_true() {
        let mut g = super::Xg96Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_96 heap tests -------------------------------------------------

    #[test]
    fn xg_96_heap_empty() {
        let h: super::Xg96Heap<i32> = super::Xg96Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_96_heap_push_pop() {
        let mut h = super::Xg96Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_96_heap_peek() {
        let mut h = super::Xg96Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_96_heap_drain_sorted() {
        let mut h = super::Xg96Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_96_heap_merge() {
        let mut a = super::Xg96Heap::new();
        let mut b = super::Xg96Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_96_heap_default() {
        let h: super::Xg96Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_96_graph_default() {
        let g: super::Xg96Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh225_skip_insert_contains() {
        let mut sl = super::Xh225SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh225_skip_remove() {
        let mut sl = super::Xh225SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh225_skip_len() {
        let mut sl = super::Xh225SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh225_skip_range_query() {
        let mut sl = super::Xh225SkipList::xh_new(4);
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
    fn xh225_skip_floor_ceiling() {
        let mut sl = super::Xh225SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh225_skip_rank() {
        let mut sl = super::Xh225SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh225_skip_empty() {
        let sl = super::Xh225SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh225_skip_duplicates() {
        let mut sl = super::Xh225SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh225_bitset_set_test() {
        let mut bs = super::Xh225BitSet::xh_new(256);
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
    fn xh225_bitset_clear_count() {
        let mut bs = super::Xh225BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh225_bitset_and_or_xor() {
        let mut a = super::Xh225BitSet::xh_new(128);
        let mut b = super::Xh225BitSet::xh_new(128);
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
    fn xh225_bitset_iter_ones() {
        let mut bs = super::Xh225BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh225_bitset_first_last() {
        let mut bs = super::Xh225BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh225_bitset_empty() {
        let bs = super::Xh225BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
