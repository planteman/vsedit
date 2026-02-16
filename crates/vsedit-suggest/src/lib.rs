//! Autocomplete and suggestions.

use std::fmt;
/// Completion item kind matching VS Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Text, Method, Function, Constructor, Field, Variable,
    Class, Interface, Module, Property, Unit, Value, Enum,
    Keyword, Snippet, Color, File, Reference, Folder,
    EnumMember, Constant, Struct, Event, Operator, TypeParameter,
}

/// A text edit to apply alongside completion.
#[derive(Debug, Clone)]
pub struct TextEdit {
    pub range_start: (u32, u32),
    pub range_end: (u32, u32),
    pub new_text: String,
}

/// A command to execute after completion.
#[derive(Debug, Clone)]
pub struct Command {
    pub title: String,
    pub command: String,
    pub arguments: Vec<String>,
}

/// A completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub preselect: bool,
    pub deprecated: bool,
    pub additional_text_edits: Vec<TextEdit>,
    pub command: Option<Command>,
}

impl CompletionItem {
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        Self {
            label: label.into(), kind, detail: None, documentation: None,
            insert_text: None, sort_text: None, filter_text: None,
            preselect: false, deprecated: false,
            additional_text_edits: Vec::new(),
            command: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into()); self
    }

    pub fn with_insert_text(mut self, text: impl Into<String>) -> Self {
        self.insert_text = Some(text.into()); self
    }

    pub fn with_documentation(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into()); self
    }

    pub fn with_sort_text(mut self, text: impl Into<String>) -> Self {
        self.sort_text = Some(text.into()); self
    }

    pub fn with_filter_text(mut self, text: impl Into<String>) -> Self {
        self.filter_text = Some(text.into()); self
    }

    pub fn with_preselect(mut self) -> Self {
        self.preselect = true; self
    }

    pub fn with_deprecated(mut self) -> Self {
        self.deprecated = true; self
    }

    /// Get the text to use for filtering.
    pub fn get_filter_text(&self) -> &str {
        self.filter_text.as_deref()
            .or(self.insert_text.as_deref())
            .unwrap_or(&self.label)
    }

    /// Get the text to insert.
    pub fn get_insert_text(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }
}

/// A completion list.
#[derive(Debug, Clone)]
pub struct CompletionList {
    pub items: Vec<CompletionItem>,
    pub is_incomplete: bool,
}

impl CompletionList {
    pub fn new(items: Vec<CompletionItem>) -> Self {
        Self { items, is_incomplete: false }
    }

    pub fn incomplete(items: Vec<CompletionItem>) -> Self {
        Self { items, is_incomplete: true }
    }

    /// Filter items whose filter text contains `query` (case-insensitive).
    pub fn filter(&self, query: &str) -> CompletionList {
        let q = query.to_lowercase();
        let items = self.items.iter()
            .filter(|item| item.get_filter_text().to_lowercase().contains(&q))
            .cloned()
            .collect();
        CompletionList { items, is_incomplete: self.is_incomplete }
    }

    /// Sort items by sort_text (falling back to label), then by label.
    pub fn sort_by_relevance(&mut self) {
        self.items.sort_by(|a, b| {
            let sa = a.sort_text.as_deref().unwrap_or(&a.label);
            let sb = b.sort_text.as_deref().unwrap_or(&b.label);
            sa.cmp(sb).then_with(|| a.label.cmp(&b.label))
        });
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Filter items using fuzzy matching against `query`.
    pub fn fuzzy_filter(&self, query: &str) -> CompletionList {
        let items = self.items.iter()
            .filter(|item| fuzzy_match(query, item.get_filter_text()))
            .cloned()
            .collect();
        CompletionList { items, is_incomplete: self.is_incomplete }
    }
}

/// Simple fuzzy match: all characters of `query` appear in order in `target` (case-insensitive).
pub fn fuzzy_match(query: &str, target: &str) -> bool {
    let mut target_chars = target.chars().flat_map(|c| c.to_lowercase());
    for qc in query.chars().flat_map(|c| c.to_lowercase()) {
        if !target_chars.any(|tc| tc == qc) {
            return false;
        }
    }
    true
}

impl std::fmt::Display for CompletionItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Text => "Text", Self::Method => "Method", Self::Function => "Function",
            Self::Constructor => "Constructor", Self::Field => "Field",
            Self::Variable => "Variable", Self::Class => "Class",
            Self::Interface => "Interface", Self::Module => "Module",
            Self::Property => "Property", Self::Unit => "Unit", Self::Value => "Value",
            Self::Enum => "Enum", Self::Keyword => "Keyword", Self::Snippet => "Snippet",
            Self::Color => "Color", Self::File => "File", Self::Reference => "Reference",
            Self::Folder => "Folder", Self::EnumMember => "EnumMember",
            Self::Constant => "Constant", Self::Struct => "Struct", Self::Event => "Event",
            Self::Operator => "Operator", Self::TypeParameter => "TypeParameter",
        })
    }
}

impl std::fmt::Display for CompletionItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", kind_icon(self.kind), self.label)
    }
}

/// Provider for completions.
pub trait CompletionProvider: Send + Sync {
    fn trigger_characters(&self) -> Vec<char> { vec!['.'] }
    fn provide_completions(&self, uri: &str, line: u32, column: u32) -> Option<CompletionList>;
}

/// Icon for a completion kind (for terminal display).
pub fn kind_icon(kind: CompletionItemKind) -> &'static str {
    match kind {
        CompletionItemKind::Method | CompletionItemKind::Function => "ƒ",
        CompletionItemKind::Constructor => "⊕",
        CompletionItemKind::Field | CompletionItemKind::Property => "□",
        CompletionItemKind::Variable => "𝑥",
        CompletionItemKind::Class | CompletionItemKind::Struct => "◆",
        CompletionItemKind::Interface => "◇",
        CompletionItemKind::Module => "▣",
        CompletionItemKind::Enum | CompletionItemKind::EnumMember => "∈",
        CompletionItemKind::Keyword => "⌘",
        CompletionItemKind::Snippet => "⟨⟩",
        CompletionItemKind::File => "📄",
        CompletionItemKind::Folder => "📁",
        CompletionItemKind::Constant => "π",
        _ => "·",
    }
}

/// Compute a fuzzy match score. Higher scores indicate better matches.
/// Returns `None` if the query does not match the target at all.
pub fn fuzzy_score(query: &str, target: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let query_lower: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let target_chars: Vec<char> = target.chars().collect();
    let target_lower: Vec<char> = target.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut qi = 0;
    let mut score: i64 = 0;
    let mut last_match_idx: Option<usize> = None;
    let mut first_match_idx: Option<usize> = None;

    for (ti, &tc) in target_lower.iter().enumerate() {
        if qi < query_lower.len() && tc == query_lower[qi] {
            if first_match_idx.is_none() {
                first_match_idx = Some(ti);
            }
            // Exact case match bonus
            if target_chars[ti] == query.chars().nth(qi).unwrap_or(' ') {
                score += 2;
            } else {
                score += 1;
            }
            // Consecutive match bonus
            if let Some(last) = last_match_idx {
                if ti == last + 1 {
                    score += 5;
                }
            }
            // Word boundary bonus (after _, space, or camelCase)
            if ti == 0
                || target_chars[ti - 1] == '_'
                || target_chars[ti - 1] == ' '
                || (target_chars[ti - 1].is_lowercase() && target_chars[ti].is_uppercase())
            {
                score += 10;
            }
            last_match_idx = Some(ti);
            qi += 1;
        }
    }

    if qi < query_lower.len() {
        return None; // not all query chars matched
    }

    // Penalise matches that start late in the string
    if let Some(first) = first_match_idx {
        score -= first as i64;
    }

    Some(score)
}

/// A scored completion item used for ranking.
#[derive(Debug, Clone)]
pub struct ScoredCompletion {
    pub item: CompletionItem,
    pub score: i64,
}

impl ScoredCompletion {
    pub fn new(item: CompletionItem, score: i64) -> Self {
        Self { item, score }
    }
}

impl std::fmt::Display for ScoredCompletion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (score: {})", self.item, self.score)
    }
}

impl PartialEq for ScoredCompletion {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.item.label == other.item.label
    }
}

impl CompletionList {
    /// Score and sort items by fuzzy relevance to `query`.
    /// Items that don't match are removed.
    pub fn score_and_sort(&self, query: &str) -> Vec<ScoredCompletion> {
        let mut scored: Vec<ScoredCompletion> = self
            .items
            .iter()
            .filter_map(|item| {
                let text = item.get_filter_text();
                fuzzy_score(query, text).map(|score| ScoredCompletion::new(item.clone(), score))
            })
            .collect();
        scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.item.label.cmp(&b.item.label)));
        scored
    }

    /// Return only items of the given kind.
    pub fn filter_by_kind(&self, kind: CompletionItemKind) -> CompletionList {
        let items = self
            .items
            .iter()
            .filter(|item| item.kind == kind)
            .cloned()
            .collect();
        CompletionList {
            items,
            is_incomplete: self.is_incomplete,
        }
    }

    /// Merge another completion list into this one, deduplicating by label.
    pub fn merge(&mut self, other: &CompletionList) {
        for item in &other.items {
            if !self.items.iter().any(|existing| existing.label == item.label) {
                self.items.push(item.clone());
            }
        }
        if other.is_incomplete {
            self.is_incomplete = true;
        }
    }

    /// Remove deprecated items.
    pub fn remove_deprecated(&self) -> CompletionList {
        let items = self
            .items
            .iter()
            .filter(|item| !item.deprecated)
            .cloned()
            .collect();
        CompletionList {
            items,
            is_incomplete: self.is_incomplete,
        }
    }

    /// Limit the list to at most `n` items.
    pub fn take(&self, n: usize) -> CompletionList {
        let items = self.items.iter().take(n).cloned().collect();
        CompletionList {
            items,
            is_incomplete: self.is_incomplete || self.items.len() > n,
        }
    }

    /// Return unique kinds present in the list.
    pub fn unique_kinds(&self) -> Vec<CompletionItemKind> {
        let mut kinds: Vec<CompletionItemKind> = Vec::new();
        for item in &self.items {
            if !kinds.contains(&item.kind) {
                kinds.push(item.kind);
            }
        }
        kinds
    }
}

/// Validate a completion item for correctness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionValidationError {
    EmptyLabel,
    LabelTooLong(usize),
    InsertTextEmpty,
}

impl std::fmt::Display for CompletionValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyLabel => write!(f, "completion label is empty"),
            Self::LabelTooLong(len) => write!(f, "completion label too long: {len} chars"),
            Self::InsertTextEmpty => write!(f, "insert text is explicitly set but empty"),
        }
    }
}

/// Maximum label length for validation.
const MAX_LABEL_LENGTH: usize = 256;

impl CompletionItem {
    /// Validate this completion item.
    pub fn validate(&self) -> Result<(), CompletionValidationError> {
        if self.label.is_empty() {
            return Err(CompletionValidationError::EmptyLabel);
        }
        if self.label.len() > MAX_LABEL_LENGTH {
            return Err(CompletionValidationError::LabelTooLong(self.label.len()));
        }
        if let Some(ref text) = self.insert_text {
            if text.is_empty() {
                return Err(CompletionValidationError::InsertTextEmpty);
            }
        }
        Ok(())
    }

    /// Return the effective sort key for this item.
    pub fn sort_key(&self) -> &str {
        self.sort_text.as_deref().unwrap_or(&self.label)
    }

    /// Check if this item matches a prefix (case-insensitive).
    pub fn matches_prefix(&self, prefix: &str) -> bool {
        self.get_filter_text()
            .to_lowercase()
            .starts_with(&prefix.to_lowercase())
    }
}

impl CompletionList {
    /// Partition items into (matching, non-matching) based on a prefix.
    pub fn partition_by_prefix(&self, prefix: &str) -> (CompletionList, CompletionList) {
        let (matching, rest): (Vec<_>, Vec<_>) = self
            .items
            .iter()
            .cloned()
            .partition(|item| item.matches_prefix(prefix));
        (
            CompletionList {
                items: matching,
                is_incomplete: self.is_incomplete,
            },
            CompletionList {
                items: rest,
                is_incomplete: self.is_incomplete,
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Word-based completion provider
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// Extracts words from document text for fallback completions.
pub struct WordCompletionProvider;

impl WordCompletionProvider {
    /// Extract completion items from document text.
    ///
    /// Words are scored by frequency and proximity to `cursor_offset`.
    /// The `current_word` being typed is excluded from results.
    pub fn provide(text: &str, cursor_offset: usize, current_word: &str) -> CompletionList {
        let mut word_freq: HashMap<&str, (usize, usize)> = HashMap::new();
        let mut last_end = 0;
        for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if word.len() < 2 || word == current_word {
                last_end += word.len() + 1;
                continue;
            }
            let distance = cursor_offset.abs_diff(last_end);
            let entry = word_freq.entry(word).or_insert((0, usize::MAX));
            entry.0 += 1;
            entry.1 = entry.1.min(distance);
            last_end += word.len() + 1;
        }

        let mut scored: Vec<(&str, i64)> = word_freq
            .iter()
            .map(|(word, (freq, dist))| {
                let score = (*freq as i64) * 10 - (*dist as i64 / 100);
                (*word, score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

        let items = scored
            .into_iter()
            .take(50)
            .map(|(word, _)| CompletionItem::new(word, CompletionItemKind::Text))
            .collect();
        CompletionList::new(items)
    }
}

// ---------------------------------------------------------------------------
// Snippet completion provider
// ---------------------------------------------------------------------------

/// A snippet entry for completion integration.
#[derive(Debug, Clone)]
pub struct SnippetEntry {
    pub name: String,
    pub prefix: String,
    pub body: String,
    pub description: Option<String>,
}

/// Converts snippet entries into completion items.
pub struct SnippetCompletionProvider;

impl SnippetCompletionProvider {
    /// Convert snippets into completion items for display in the completion widget.
    pub fn provide(snippets: &[SnippetEntry]) -> CompletionList {
        let items = snippets
            .iter()
            .map(|s| {
                let mut item = CompletionItem::new(&s.prefix, CompletionItemKind::Snippet)
                    .with_insert_text(&s.body)
                    .with_detail(format!("Snippet: {}", s.name));
                if let Some(ref desc) = s.description {
                    item = item.with_documentation(desc);
                }
                item = item.with_sort_text(format!("zz_{}", s.prefix));
                item
            })
            .collect();
        CompletionList::new(items)
    }

    /// Merge snippet completions into an existing completion list.
    pub fn merge_into(list: &mut CompletionList, snippets: &[SnippetEntry]) {
        let snippet_list = Self::provide(snippets);
        list.merge(&snippet_list);
    }
}

// ---------------------------------------------------------------------------
// LSP completion provider
// ---------------------------------------------------------------------------

/// Converts an LSP completion item kind number to our `CompletionItemKind`.
fn lsp_kind_to_completion_kind(kind: u64) -> CompletionItemKind {
    match kind {
        1 => CompletionItemKind::Text,
        2 => CompletionItemKind::Method,
        3 => CompletionItemKind::Function,
        4 => CompletionItemKind::Constructor,
        5 => CompletionItemKind::Field,
        6 => CompletionItemKind::Variable,
        7 => CompletionItemKind::Class,
        8 => CompletionItemKind::Interface,
        9 => CompletionItemKind::Module,
        10 => CompletionItemKind::Property,
        11 => CompletionItemKind::Unit,
        12 => CompletionItemKind::Value,
        13 => CompletionItemKind::Enum,
        14 => CompletionItemKind::Keyword,
        15 => CompletionItemKind::Snippet,
        16 => CompletionItemKind::Color,
        17 => CompletionItemKind::File,
        18 => CompletionItemKind::Reference,
        19 => CompletionItemKind::Folder,
        20 => CompletionItemKind::EnumMember,
        21 => CompletionItemKind::Constant,
        22 => CompletionItemKind::Struct,
        23 => CompletionItemKind::Event,
        24 => CompletionItemKind::Operator,
        25 => CompletionItemKind::TypeParameter,
        _ => CompletionItemKind::Text,
    }
}

/// Converts an LSP completion item (as JSON) to our `CompletionItem` format.
pub fn from_lsp_completion(item: &serde_json::Value) -> Option<CompletionItem> {
    let label = item.get("label")?.as_str()?;
    let kind = item
        .get("kind")
        .and_then(|k| k.as_u64())
        .map(lsp_kind_to_completion_kind)
        .unwrap_or(CompletionItemKind::Text);
    let detail = item.get("detail").and_then(|d| d.as_str()).map(String::from);
    let insert_text = item
        .get("insertText")
        .and_then(|t| t.as_str())
        .map(String::from);

    let mut ci = CompletionItem::new(label, kind);
    ci.detail = detail;
    ci.insert_text = insert_text;
    Some(ci)
}

/// Parse an LSP completion response into a list of `CompletionItem`s.
///
/// Handles both `CompletionList` (`{ items: [...] }`) and bare `CompletionItem[]`
/// response shapes.
pub fn parse_lsp_completions(response: &serde_json::Value) -> Vec<CompletionItem> {
    if let Some(items) = response.get("items").and_then(|i| i.as_array()) {
        items.iter().filter_map(from_lsp_completion).collect()
    } else if let Some(items) = response.as_array() {
        items.iter().filter_map(from_lsp_completion).collect()
    } else {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// suggest_details_panel — show documentation alongside completions
// ---------------------------------------------------------------------------

/// Content for the details panel shown alongside a completion.
#[derive(Debug, Clone)]
pub struct DetailsPanel {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub deprecated: bool,
}

impl DetailsPanel {
    /// Render the details panel as a multi-line string.
    pub fn render(&self) -> String {
        let mut out = format!("{} ({})", self.label, self.kind);
        if self.deprecated {
            out.push_str(" [deprecated]");
        }
        out.push('\n');
        if let Some(ref detail) = self.detail {
            out.push_str(detail);
            out.push('\n');
        }
        if let Some(ref doc) = self.documentation {
            out.push_str("---\n");
            out.push_str(doc);
            out.push('\n');
        }
        out
    }

    /// Returns true if there is any content to display beyond the label.
    pub fn has_content(&self) -> bool {
        self.detail.is_some() || self.documentation.is_some()
    }
}

/// Build a details panel from a completion item.
pub fn suggest_details_panel(item: &CompletionItem) -> DetailsPanel {
    DetailsPanel {
        label: item.label.clone(),
        kind: item.kind,
        detail: item.detail.clone(),
        documentation: item.documentation.clone(),
        deprecated: item.deprecated,
    }
}

/// Build details panels for all items in a completion list.
pub fn suggest_details_panels(list: &CompletionList) -> Vec<DetailsPanel> {
    list.items.iter().map(suggest_details_panel).collect()
}

/// Find the details panel for the currently selected (first) item.
pub fn suggest_active_details(list: &CompletionList) -> Option<DetailsPanel> {
    list.items.first().map(suggest_details_panel)
}

// ---------------------------------------------------------------------------
// Additional helpers
// ---------------------------------------------------------------------------

impl CompletionList {
    /// Returns a list of labels from all items.
    pub fn labels(&self) -> Vec<&str> {
        self.items.iter().map(|i| i.label.as_str()).collect()
    }

    /// Returns the first item, if any.
    pub fn first(&self) -> Option<&CompletionItem> {
        self.items.first()
    }

    /// Returns the last item, if any.
    pub fn last(&self) -> Option<&CompletionItem> {
        self.items.last()
    }

    /// Counts items matching the given kind.
    pub fn count_by_kind(&self, kind: CompletionItemKind) -> usize {
        self.items.iter().filter(|i| i.kind == kind).count()
    }
}

impl fmt::Display for CompletionList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_incomplete { "incomplete" } else { "complete" };
        write!(f, "CompletionList({} items, {status})", self.items.len())
    }
}

impl CompletionItem {
    /// Returns the length of the label string.
    pub fn label_length(&self) -> usize {
        self.label.len()
    }

    /// Returns `true` if documentation is set.
    pub fn has_documentation(&self) -> bool {
        self.documentation.is_some()
    }

    /// Returns `true` if detail is set.
    pub fn has_detail(&self) -> bool {
        self.detail.is_some()
    }
}

impl CompletionItemKind {
    /// Returns `true` for text-like kinds: Text, Keyword, Snippet.
    pub fn is_text_like(&self) -> bool {
        matches!(self, Self::Text | Self::Keyword | Self::Snippet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_item_creation() {
        let item = CompletionItem::new("println", CompletionItemKind::Function)
            .with_detail("macro")
            .with_insert_text("println!(\"$1\")");
        assert_eq!(item.label, "println");
        assert_eq!(item.get_insert_text(), "println!(\"$1\")");
        assert_eq!(item.get_filter_text(), "println!(\"$1\")");
    }

    #[test]
    fn completion_list() {
        let list = CompletionList::new(vec![
            CompletionItem::new("foo", CompletionItemKind::Function),
            CompletionItem::new("bar", CompletionItemKind::Variable),
        ]);
        assert_eq!(list.items.len(), 2);
        assert!(!list.is_incomplete);
    }

    #[test]
    fn kind_icons() {
        assert_eq!(kind_icon(CompletionItemKind::Function), "ƒ");
        assert_eq!(kind_icon(CompletionItemKind::Class), "◆");
        assert_eq!(kind_icon(CompletionItemKind::Keyword), "⌘");
    }

    #[test]
    fn builder_documentation_and_sort_text() {
        let item = CompletionItem::new("foo", CompletionItemKind::Function)
            .with_documentation("Does foo things")
            .with_sort_text("00_foo");
        assert_eq!(item.documentation.as_deref(), Some("Does foo things"));
        assert_eq!(item.sort_text.as_deref(), Some("00_foo"));
    }

    #[test]
    fn builder_filter_preselect_deprecated() {
        let item = CompletionItem::new("old_fn", CompletionItemKind::Function)
            .with_filter_text("oldfn")
            .with_preselect()
            .with_deprecated();
        assert_eq!(item.get_filter_text(), "oldfn");
        assert!(item.preselect);
        assert!(item.deprecated);
    }

    #[test]
    fn filter_completions() {
        let list = CompletionList::new(vec![
            CompletionItem::new("to_string", CompletionItemKind::Method),
            CompletionItem::new("to_uppercase", CompletionItemKind::Method),
            CompletionItem::new("len", CompletionItemKind::Method),
        ]);
        let filtered = list.filter("to_");
        assert_eq!(filtered.len(), 2);
        assert!(!filtered.is_empty());
    }

    #[test]
    fn sort_by_relevance_ordering() {
        let mut list = CompletionList::new(vec![
            CompletionItem::new("banana", CompletionItemKind::Variable)
                .with_sort_text("2"),
            CompletionItem::new("apple", CompletionItemKind::Variable)
                .with_sort_text("1"),
            CompletionItem::new("cherry", CompletionItemKind::Variable),
        ]);
        list.sort_by_relevance();
        assert_eq!(list.items[0].label, "apple");
        assert_eq!(list.items[1].label, "banana");
        assert_eq!(list.items[2].label, "cherry");
    }

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match("fn", "function"));
        assert!(fuzzy_match("FN", "function"));
        assert!(fuzzy_match("abc", "a_big_cat"));
        assert!(!fuzzy_match("xyz", "function"));
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn fuzzy_filter_completions() {
        let list = CompletionList::new(vec![
            CompletionItem::new("get_value", CompletionItemKind::Method),
            CompletionItem::new("set_value", CompletionItemKind::Method),
            CompletionItem::new("clear", CompletionItemKind::Method),
        ]);
        let filtered = list.fuzzy_filter("gv");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.items[0].label, "get_value");
    }

    #[test]
    fn display_completion_kind() {
        assert_eq!(format!("{}", CompletionItemKind::Function), "Function");
        assert_eq!(format!("{}", CompletionItemKind::Struct), "Struct");
        assert_eq!(format!("{}", CompletionItemKind::TypeParameter), "TypeParameter");
    }

    #[test]
    fn display_completion_item() {
        let item = CompletionItem::new("main", CompletionItemKind::Function);
        assert_eq!(format!("{}", item), "ƒ main");
    }

    #[test]
    fn empty_completion_list() {
        let list = CompletionList::new(vec![]);
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn fuzzy_score_basic_match() {
        let score = fuzzy_score("fn", "function");
        assert!(score.is_some());
        assert!(score.unwrap() > 0);
    }

    #[test]
    fn fuzzy_score_no_match() {
        assert!(fuzzy_score("xyz", "function").is_none());
    }

    #[test]
    fn fuzzy_score_empty_query() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn fuzzy_score_consecutive_bonus() {
        // "get" matches the start of "get_value" consecutively
        let consecutive = fuzzy_score("get", "get_value").unwrap();
        assert!(consecutive > 0);
        // "gxv" should not match at all (no 'x' in "get_value")
        let no_match = fuzzy_score("gxv", "get_value");
        assert!(no_match.is_none());
    }

    #[test]
    fn fuzzy_score_case_bonus() {
        let exact = fuzzy_score("Get", "GetValue").unwrap();
        let lower = fuzzy_score("get", "GetValue").unwrap();
        assert!(exact > lower);
    }

    #[test]
    fn score_and_sort_ordering() {
        let list = CompletionList::new(vec![
            CompletionItem::new("xyz_fn", CompletionItemKind::Function),
            CompletionItem::new("fn_call", CompletionItemKind::Function),
            CompletionItem::new("my_func", CompletionItemKind::Function),
        ]);
        let scored = list.score_and_sort("fn");
        assert!(!scored.is_empty());
        // Best match should be first
        for i in 0..scored.len() - 1 {
            assert!(scored[i].score >= scored[i + 1].score);
        }
    }

    #[test]
    fn filter_by_kind() {
        let list = CompletionList::new(vec![
            CompletionItem::new("foo", CompletionItemKind::Function),
            CompletionItem::new("bar", CompletionItemKind::Variable),
            CompletionItem::new("baz", CompletionItemKind::Function),
        ]);
        let fns = list.filter_by_kind(CompletionItemKind::Function);
        assert_eq!(fns.len(), 2);
        assert!(fns.items.iter().all(|i| i.kind == CompletionItemKind::Function));
    }

    #[test]
    fn merge_lists_dedup() {
        let mut list1 = CompletionList::new(vec![
            CompletionItem::new("foo", CompletionItemKind::Function),
            CompletionItem::new("bar", CompletionItemKind::Variable),
        ]);
        let list2 = CompletionList::new(vec![
            CompletionItem::new("bar", CompletionItemKind::Variable),
            CompletionItem::new("baz", CompletionItemKind::Method),
        ]);
        list1.merge(&list2);
        assert_eq!(list1.len(), 3);
    }

    #[test]
    fn remove_deprecated_items() {
        let list = CompletionList::new(vec![
            CompletionItem::new("old_fn", CompletionItemKind::Function).with_deprecated(),
            CompletionItem::new("new_fn", CompletionItemKind::Function),
        ]);
        let clean = list.remove_deprecated();
        assert_eq!(clean.len(), 1);
        assert_eq!(clean.items[0].label, "new_fn");
    }

    #[test]
    fn take_limits_items() {
        let list = CompletionList::new(vec![
            CompletionItem::new("a", CompletionItemKind::Text),
            CompletionItem::new("b", CompletionItemKind::Text),
            CompletionItem::new("c", CompletionItemKind::Text),
        ]);
        let taken = list.take(2);
        assert_eq!(taken.len(), 2);
        assert!(taken.is_incomplete);
    }

    #[test]
    fn unique_kinds_list() {
        let list = CompletionList::new(vec![
            CompletionItem::new("a", CompletionItemKind::Function),
            CompletionItem::new("b", CompletionItemKind::Variable),
            CompletionItem::new("c", CompletionItemKind::Function),
        ]);
        let kinds = list.unique_kinds();
        assert_eq!(kinds.len(), 2);
    }

    #[test]
    fn validate_empty_label() {
        let item = CompletionItem::new("", CompletionItemKind::Text);
        assert_eq!(item.validate(), Err(CompletionValidationError::EmptyLabel));
    }

    #[test]
    fn validate_long_label() {
        let long = "x".repeat(300);
        let item = CompletionItem::new(long, CompletionItemKind::Text);
        assert!(matches!(item.validate(), Err(CompletionValidationError::LabelTooLong(_))));
    }

    #[test]
    fn validate_empty_insert_text() {
        let item = CompletionItem::new("foo", CompletionItemKind::Text)
            .with_insert_text("");
        assert_eq!(item.validate(), Err(CompletionValidationError::InsertTextEmpty));
    }

    #[test]
    fn validate_ok() {
        let item = CompletionItem::new("foo", CompletionItemKind::Text);
        assert!(item.validate().is_ok());
    }

    #[test]
    fn matches_prefix_case_insensitive() {
        let item = CompletionItem::new("getValue", CompletionItemKind::Method);
        assert!(item.matches_prefix("get"));
        assert!(item.matches_prefix("GET"));
        assert!(!item.matches_prefix("set"));
    }

    #[test]
    fn partition_by_prefix_splits() {
        let list = CompletionList::new(vec![
            CompletionItem::new("get_value", CompletionItemKind::Method),
            CompletionItem::new("set_value", CompletionItemKind::Method),
            CompletionItem::new("get_name", CompletionItemKind::Method),
        ]);
        let (matching, rest) = list.partition_by_prefix("get");
        assert_eq!(matching.len(), 2);
        assert_eq!(rest.len(), 1);
    }

    #[test]
    fn scored_completion_display() {
        let item = CompletionItem::new("test", CompletionItemKind::Function);
        let scored = ScoredCompletion::new(item, 42);
        assert!(format!("{scored}").contains("score: 42"));
    }

    #[test]
    fn sort_key_fallback() {
        let item = CompletionItem::new("foo", CompletionItemKind::Text);
        assert_eq!(item.sort_key(), "foo");
        let item2 = CompletionItem::new("foo", CompletionItemKind::Text)
            .with_sort_text("aaa");
        assert_eq!(item2.sort_key(), "aaa");
    }

    #[test]
    fn validation_error_display() {
        assert_eq!(
            CompletionValidationError::EmptyLabel.to_string(),
            "completion label is empty"
        );
        assert!(CompletionValidationError::LabelTooLong(300).to_string().contains("300"));
        assert_eq!(
            CompletionValidationError::InsertTextEmpty.to_string(),
            "insert text is explicitly set but empty"
        );
    }

    #[test]
    fn merge_sets_incomplete_flag() {
        let mut list1 = CompletionList::new(vec![
            CompletionItem::new("a", CompletionItemKind::Text),
        ]);
        assert!(!list1.is_incomplete);
        let list2 = CompletionList::incomplete(vec![
            CompletionItem::new("b", CompletionItemKind::Text),
        ]);
        list1.merge(&list2);
        assert!(list1.is_incomplete);
    }

    // -----------------------------------------------------------------------
    // Word completion tests
    // -----------------------------------------------------------------------

    #[test]
    fn word_completion_extracts_words() {
        let text = "fn main() { let value = 42; println!(value); }";
        let list = WordCompletionProvider::provide(text, 20, "");
        assert!(!list.is_empty());
        let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"value"));
        assert!(labels.contains(&"main"));
        assert!(labels.contains(&"println"));
    }

    #[test]
    fn word_completion_excludes_current_word() {
        let text = "hello world hello";
        let list = WordCompletionProvider::provide(text, 5, "hello");
        let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert!(!labels.contains(&"hello"));
        assert!(labels.contains(&"world"));
    }

    #[test]
    fn word_completion_skips_short_words() {
        let text = "a b cd efg";
        let list = WordCompletionProvider::provide(text, 0, "");
        let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert!(!labels.contains(&"a"));
        assert!(!labels.contains(&"b"));
        assert!(labels.contains(&"cd"));
        assert!(labels.contains(&"efg"));
    }

    #[test]
    fn word_completion_frequency_scoring() {
        let text = "foo bar foo foo baz bar";
        let list = WordCompletionProvider::provide(text, 0, "");
        // foo appears 3 times, should be first
        assert_eq!(list.items[0].label, "foo");
    }

    // -----------------------------------------------------------------------
    // Snippet completion tests
    // -----------------------------------------------------------------------

    #[test]
    fn snippet_completion_items() {
        let snippets = vec![
            SnippetEntry {
                name: "For Loop".to_string(),
                prefix: "for".to_string(),
                body: "for ${1:i} in ${2:iter} { $0 }".to_string(),
                description: Some("A for loop".to_string()),
            },
        ];
        let list = SnippetCompletionProvider::provide(&snippets);
        assert_eq!(list.len(), 1);
        assert_eq!(list.items[0].kind, CompletionItemKind::Snippet);
        assert_eq!(list.items[0].label, "for");
        assert!(list.items[0].documentation.is_some());
    }

    #[test]
    fn snippet_completion_merge() {
        let mut word_list = CompletionList::new(vec![
            CompletionItem::new("format", CompletionItemKind::Function),
        ]);
        let snippets = vec![
            SnippetEntry {
                name: "fn".to_string(),
                prefix: "fn".to_string(),
                body: "fn $1() {}".to_string(),
                description: None,
            },
        ];
        SnippetCompletionProvider::merge_into(&mut word_list, &snippets);
        assert_eq!(word_list.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Additional text edits and command tests
    // -----------------------------------------------------------------------

    #[test]
    fn completion_item_additional_edits() {
        let item = CompletionItem::new("import_foo", CompletionItemKind::Module);
        assert!(item.additional_text_edits.is_empty());
        assert!(item.command.is_none());
    }

    #[test]
    fn text_edit_struct() {
        let edit = TextEdit {
            range_start: (0, 0),
            range_end: (0, 5),
            new_text: "hello".to_string(),
        };
        assert_eq!(edit.new_text, "hello");
    }

    #[test]
    fn command_struct() {
        let cmd = Command {
            title: "Trigger suggest".to_string(),
            command: "editor.action.triggerSuggest".to_string(),
            arguments: vec![],
        };
        assert_eq!(cmd.command, "editor.action.triggerSuggest");
    }

    // -----------------------------------------------------------------------
    // LSP completion parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_lsp_completion_list() {
        let json = serde_json::json!({
            "isIncomplete": false,
            "items": [
                { "label": "println", "kind": 3, "detail": "macro" },
                { "label": "format", "kind": 3, "insertText": "format!(\"$1\")" },
            ]
        });
        let items = parse_lsp_completions(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "println");
        assert_eq!(items[0].kind, CompletionItemKind::Function);
        assert_eq!(items[0].detail.as_deref(), Some("macro"));
        assert_eq!(items[1].insert_text.as_deref(), Some("format!(\"$1\")"));
    }

    #[test]
    fn parse_lsp_completion_array() {
        let json = serde_json::json!([
            { "label": "foo", "kind": 6 },
            { "label": "bar" },
        ]);
        let items = parse_lsp_completions(&json);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, CompletionItemKind::Variable);
        assert_eq!(items[1].kind, CompletionItemKind::Text);
    }

    #[test]
    fn parse_lsp_completion_empty() {
        assert!(parse_lsp_completions(&serde_json::json!(null)).is_empty());
        assert!(parse_lsp_completions(&serde_json::json!({})).is_empty());
        assert!(parse_lsp_completions(&serde_json::json!([])).is_empty());
    }

    #[test]
    fn parse_lsp_completion_malformed_items() {
        let json = serde_json::json!([
            { "kind": 3 },           // missing label
            { "label": 42 },         // label not a string
            { "label": "ok", "kind": 999 },
        ]);
        let items = parse_lsp_completions(&json);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "ok");
        assert_eq!(items[0].kind, CompletionItemKind::Text); // unknown kind
    }

    #[test]
    fn from_lsp_completion_all_kinds() {
        for kind_num in 1u64..=25 {
            let json = serde_json::json!({ "label": "x", "kind": kind_num });
            let item = from_lsp_completion(&json).unwrap();
            assert_ne!(item.label, "");
            // All valid kinds should map to something other than crashing
        }
    }

    // -- suggest_details_panel tests ----------------------------------------

    #[test]
    fn details_panel_from_item() {
        let item = CompletionItem::new("println", CompletionItemKind::Function)
            .with_detail("macro".to_string())
            .with_documentation("Prints to stdout".to_string());
        let panel = suggest_details_panel(&item);
        assert_eq!(panel.label, "println");
        assert_eq!(panel.kind, CompletionItemKind::Function);
        assert_eq!(panel.detail.as_deref(), Some("macro"));
        assert!(panel.has_content());
    }

    #[test]
    fn details_panel_render_full() {
        let panel = DetailsPanel {
            label: "format".into(),
            kind: CompletionItemKind::Function,
            detail: Some("macro".into()),
            documentation: Some("Creates a String.".into()),
            deprecated: false,
        };
        let rendered = panel.render();
        assert!(rendered.contains("format (Function)"));
        assert!(rendered.contains("macro"));
        assert!(rendered.contains("Creates a String."));
    }

    #[test]
    fn details_panel_render_deprecated() {
        let panel = DetailsPanel {
            label: "old_fn".into(),
            kind: CompletionItemKind::Function,
            detail: None,
            documentation: None,
            deprecated: true,
        };
        let rendered = panel.render();
        assert!(rendered.contains("[deprecated]"));
    }

    #[test]
    fn details_panel_no_content() {
        let item = CompletionItem::new("x", CompletionItemKind::Variable);
        let panel = suggest_details_panel(&item);
        assert!(!panel.has_content());
    }

    #[test]
    fn details_panels_list() {
        let list = CompletionList::new(vec![
            CompletionItem::new("a", CompletionItemKind::Variable),
            CompletionItem::new("b", CompletionItemKind::Function),
        ]);
        let panels = suggest_details_panels(&list);
        assert_eq!(panels.len(), 2);
    }

    #[test]
    fn active_details_returns_first() {
        let list = CompletionList::new(vec![
            CompletionItem::new("first", CompletionItemKind::Keyword),
            CompletionItem::new("second", CompletionItemKind::Variable),
        ]);
        let panel = suggest_active_details(&list).unwrap();
        assert_eq!(panel.label, "first");
    }

    #[test]
    fn active_details_empty_list() {
        let list = CompletionList::new(vec![]);
        assert!(suggest_active_details(&list).is_none());
    }

    #[test]
    fn completion_list_labels() {
        let list = CompletionList::new(vec![
            CompletionItem::new("foo", CompletionItemKind::Function),
            CompletionItem::new("bar", CompletionItemKind::Variable),
        ]);
        let labels = list.labels();
        assert_eq!(labels, vec!["foo", "bar"]);
    }

    #[test]
    fn completion_list_first_and_last() {
        let list = CompletionList::new(vec![
            CompletionItem::new("first", CompletionItemKind::Function),
            CompletionItem::new("last", CompletionItemKind::Variable),
        ]);
        assert_eq!(list.first().unwrap().label, "first");
        assert_eq!(list.last().unwrap().label, "last");

        let empty = CompletionList::new(vec![]);
        assert!(empty.first().is_none());
        assert!(empty.last().is_none());
    }

    #[test]
    fn completion_list_count_by_kind() {
        let list = CompletionList::new(vec![
            CompletionItem::new("a", CompletionItemKind::Function),
            CompletionItem::new("b", CompletionItemKind::Function),
            CompletionItem::new("c", CompletionItemKind::Variable),
        ]);
        assert_eq!(list.count_by_kind(CompletionItemKind::Function), 2);
        assert_eq!(list.count_by_kind(CompletionItemKind::Variable), 1);
        assert_eq!(list.count_by_kind(CompletionItemKind::Class), 0);
    }

    #[test]
    fn completion_list_display() {
        let list = CompletionList::new(vec![
            CompletionItem::new("x", CompletionItemKind::Text),
        ]);
        let s = format!("{list}");
        assert!(s.contains("1 items"));
        assert!(s.contains("complete"));

        let inc = CompletionList::incomplete(vec![]);
        let s2 = format!("{inc}");
        assert!(s2.contains("incomplete"));
    }

    #[test]
    fn completion_item_label_length_and_has_fields() {
        let item = CompletionItem::new("hello", CompletionItemKind::Function)
            .with_documentation("docs");
        assert_eq!(item.label_length(), 5);
        assert!(item.has_documentation());
        assert!(!item.has_detail());

        let item2 = CompletionItem::new("x", CompletionItemKind::Variable)
            .with_detail("detail");
        assert!(item2.has_detail());
        assert!(!item2.has_documentation());
    }

    #[test]
    fn completion_item_kind_is_text_like() {
        assert!(CompletionItemKind::Text.is_text_like());
        assert!(CompletionItemKind::Keyword.is_text_like());
        assert!(CompletionItemKind::Snippet.is_text_like());
        assert!(!CompletionItemKind::Function.is_text_like());
        assert!(!CompletionItemKind::Class.is_text_like());
    }
}
