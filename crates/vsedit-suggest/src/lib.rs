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

// ---------------------------------------------------------------------------
// Suggestion deduplication
// ---------------------------------------------------------------------------

/// Deduplicate completion items by label, keeping the first occurrence.
pub fn dedup_completions(items: &[CompletionItem]) -> Vec<CompletionItem> {
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .filter(|item| seen.insert(item.label.clone()))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Suggestion grouping by kind
// ---------------------------------------------------------------------------

/// A group of completion items sharing the same kind.
#[derive(Debug, Clone)]
pub struct CompletionGroup {
    pub kind: CompletionItemKind,
    pub items: Vec<CompletionItem>,
}

/// Group completion items by their kind, preserving insertion order of kinds.
pub fn group_by_kind(items: &[CompletionItem]) -> Vec<CompletionGroup> {
    let mut groups: Vec<CompletionGroup> = Vec::new();
    for item in items {
        if let Some(group) = groups.iter_mut().find(|g| g.kind == item.kind) {
            group.items.push(item.clone());
        } else {
            groups.push(CompletionGroup {
                kind: item.kind,
                items: vec![item.clone()],
            });
        }
    }
    groups
}

// ---------------------------------------------------------------------------
// Prefix-based filtering
// ---------------------------------------------------------------------------

/// Filter completion items to only those whose filter text starts with `prefix`
/// (case-insensitive). This is faster than full fuzzy matching for the common
/// case of typing from the start of an identifier.
pub fn filter_by_prefix(items: &[CompletionItem], prefix: &str) -> Vec<CompletionItem> {
    let p = prefix.to_lowercase();
    items
        .iter()
        .filter(|item| item.get_filter_text().to_lowercase().starts_with(&p))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Ranking improvements
// ---------------------------------------------------------------------------

/// Rank completion items by fuzzy score against `query`, returning the top `limit` results.
/// Preselected items are boosted to the top. Deprecated items are penalized.
pub fn rank_completions(items: &[CompletionItem], query: &str, limit: usize) -> Vec<ScoredCompletion> {
    let mut scored: Vec<ScoredCompletion> = items
        .iter()
        .filter_map(|item| {
            let text = item.get_filter_text();
            fuzzy_score(query, text).map(|mut s| {
                if item.preselect {
                    s += 1000;
                }
                if item.deprecated {
                    s -= 500;
                }
                ScoredCompletion::new(item.clone(), s)
            })
        })
        .collect();
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.item.label.cmp(&b.item.label)));
    scored.truncate(limit);
    scored
}

// ---------------------------------------------------------------------------
// Trigger character configuration and detection
// ---------------------------------------------------------------------------

/// Configuration for trigger characters that initiate auto-completion.
#[derive(Debug, Clone)]
pub struct TriggerConfig {
    /// Characters that trigger completion (e.g., '.', ':', '<').
    pub trigger_chars: Vec<char>,
    /// Characters that commit/accept the current completion (e.g., Tab, Enter).
    pub commit_chars: Vec<char>,
    /// Minimum word length before auto-triggering without a trigger character.
    pub min_word_length: usize,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            trigger_chars: vec!['.', ':', '<'],
            commit_chars: vec!['\t', '\n'],
            min_word_length: 3,
        }
    }
}

impl TriggerConfig {
    /// Create a config with the given trigger characters.
    pub fn with_triggers(chars: &[char]) -> Self {
        Self {
            trigger_chars: chars.to_vec(),
            ..Default::default()
        }
    }

    /// Returns `true` if `ch` is a trigger character.
    pub fn is_trigger(&self, ch: char) -> bool {
        self.trigger_chars.contains(&ch)
    }

    /// Returns `true` if `ch` is a commit character.
    pub fn is_commit(&self, ch: char) -> bool {
        self.commit_chars.contains(&ch)
    }

    /// Determine whether completion should be triggered for the given line
    /// content up to (but not including) the cursor column.
    ///
    /// Returns `true` if the character immediately before the cursor is a
    /// trigger character, or if the current word being typed has reached
    /// `min_word_length`.
    pub fn should_trigger(&self, line: &str, cursor_col: usize) -> bool {
        let text = &line[..cursor_col.min(line.len())];
        if let Some(last) = text.chars().last() {
            if self.is_trigger(last) {
                return true;
            }
        }
        // Check word length: count contiguous identifier chars at end of text.
        let word_len = text
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .count();
        word_len >= self.min_word_length
    }
}

// ---------------------------------------------------------------------------
// Snippet variable expansion
// ---------------------------------------------------------------------------

/// Expand simple snippet tab-stop and variable placeholders in `body`.
///
/// Supported syntax:
///   - `$0`                 → empty (final cursor position marker, removed)
///   - `${N:default}`       → `default`
///   - `$N`                 → empty string (bare tab-stop without default)
///   - `${TM_FILENAME}`     → looked up in `vars`; empty if absent
///   - `${VAR:fallback}`    → value from `vars`, or `fallback` if absent
pub fn expand_snippet(body: &str, vars: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(body.len());
    let chars: Vec<char> = body.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '$' && i + 1 < len {
            if chars[i + 1] == '{' {
                // Find matching '}'
                if let Some(close) = chars[i + 2..].iter().position(|&c| c == '}') {
                    let inner: String = chars[i + 2..i + 2 + close].iter().collect();
                    let expanded = expand_snippet_placeholder(&inner, vars);
                    result.push_str(&expanded);
                    i = i + 2 + close + 1;
                    continue;
                }
            } else if chars[i + 1].is_ascii_digit() {
                // Bare $N — skip the tab-stop number
                i += 1;
                while i < len && chars[i].is_ascii_digit() {
                    i += 1;
                }
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Expand a single `${...}` placeholder interior (the text between braces).
fn expand_snippet_placeholder(inner: &str, vars: &HashMap<String, String>) -> String {
    if let Some(colon_pos) = inner.find(':') {
        let key = &inner[..colon_pos];
        let fallback = &inner[colon_pos + 1..];
        // If key is purely numeric it's a tab-stop with default text.
        if key.chars().all(|c| c.is_ascii_digit()) {
            return fallback.to_string();
        }
        // Otherwise it's a variable with fallback.
        vars.get(key).cloned().unwrap_or_else(|| fallback.to_string())
    } else {
        // No colon — could be a bare number or a variable name.
        if inner.chars().all(|c| c.is_ascii_digit()) {
            String::new()
        } else {
            vars.get(inner).cloned().unwrap_or_default()
        }
    }
}

// ---------------------------------------------------------------------------
// Completion item sorting by relevance + recency
// ---------------------------------------------------------------------------

/// Tracks recently-selected completion labels so they can be boosted in future
/// ranking.
#[derive(Debug, Clone)]
pub struct RecencyTracker {
    /// Most-recently-used labels, newest first.
    history: Vec<String>,
    /// Maximum number of entries to retain.
    capacity: usize,
}

impl RecencyTracker {
    pub fn new(capacity: usize) -> Self {
        Self {
            history: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Record that `label` was selected by the user.
    pub fn record(&mut self, label: &str) {
        // Remove existing entry if present so it moves to front.
        self.history.retain(|l| l != label);
        self.history.insert(0, label.to_string());
        self.history.truncate(self.capacity);
    }

    /// Return a recency boost for `label`.  Items used most recently get the
    /// highest boost (capacity points for position 0, capacity-1 for position
    /// 1, etc.).  Returns 0 if the label has no history.
    pub fn boost(&self, label: &str) -> i64 {
        self.history
            .iter()
            .position(|l| l == label)
            .map(|pos| (self.capacity - pos) as i64)
            .unwrap_or(0)
    }

    /// The number of labels currently tracked.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

/// Rank completions considering both fuzzy score and recency.
pub fn rank_with_recency(
    items: &[CompletionItem],
    query: &str,
    recency: &RecencyTracker,
    limit: usize,
) -> Vec<ScoredCompletion> {
    let mut scored: Vec<ScoredCompletion> = items
        .iter()
        .filter_map(|item| {
            let text = item.get_filter_text();
            fuzzy_score(query, text).map(|mut s| {
                if item.preselect {
                    s += 1000;
                }
                if item.deprecated {
                    s -= 500;
                }
                s += recency.boost(&item.label) * 50;
                ScoredCompletion::new(item.clone(), s)
            })
        })
        .collect();
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.item.label.cmp(&b.item.label)));
    scored.truncate(limit);
    scored
}

// ---------------------------------------------------------------------------
// Suggestion pre-filtering by multiple kinds
// ---------------------------------------------------------------------------

impl CompletionList {
    /// Return only items whose kind is in the given set.
    pub fn filter_by_kinds(&self, kinds: &[CompletionItemKind]) -> CompletionList {
        let items = self
            .items
            .iter()
            .filter(|item| kinds.contains(&item.kind))
            .cloned()
            .collect();
        CompletionList {
            items,
            is_incomplete: self.is_incomplete,
        }
    }

    /// Exclude items whose kind is in the given set.
    pub fn exclude_kinds(&self, kinds: &[CompletionItemKind]) -> CompletionList {
        let items = self
            .items
            .iter()
            .filter(|item| !kinds.contains(&item.kind))
            .cloned()
            .collect();
        CompletionList {
            items,
            is_incomplete: self.is_incomplete,
        }
    }
}

// ---------------------------------------------------------------------------
// SuggestionRanker – multi-criteria scoring
// ---------------------------------------------------------------------------

/// Ranks suggestions using recency, frequency, and relevance scores.
#[derive(Debug, Clone)]
pub struct SuggestionRanker {
    /// Weight for recency (how recently the item was used).
    pub recency_weight: f64,
    /// Weight for frequency (how often the item was used).
    pub frequency_weight: f64,
    /// Weight for textual relevance.
    pub relevance_weight: f64,
    /// Usage counts keyed by label.
    usage_counts: std::collections::HashMap<String, u64>,
    /// Last-used timestamps keyed by label (epoch millis).
    last_used: std::collections::HashMap<String, u64>,
}

impl Default for SuggestionRanker {
    fn default() -> Self {
        Self {
            recency_weight: 1.0,
            frequency_weight: 1.0,
            relevance_weight: 2.0,
            usage_counts: std::collections::HashMap::new(),
            last_used: std::collections::HashMap::new(),
        }
    }
}

impl SuggestionRanker {
    /// Create a ranker with custom weights.
    pub fn new(recency: f64, frequency: f64, relevance: f64) -> Self {
        Self {
            recency_weight: recency,
            frequency_weight: frequency,
            relevance_weight: relevance,
            ..Default::default()
        }
    }

    /// Record that a suggestion was selected.
    pub fn record_usage(&mut self, label: &str, timestamp: u64) {
        *self.usage_counts.entry(label.to_string()).or_insert(0) += 1;
        self.last_used.insert(label.to_string(), timestamp);
    }

    /// Score a single completion item.
    ///
    /// The relevance score is based on fuzzy match quality against the query.
    pub fn score(&self, item: &CompletionItem, query: &str, now: u64) -> f64 {
        let relevance = fuzzy_score(query, item.get_filter_text())
            .map(|s| s as f64)
            .unwrap_or(0.0);

        let frequency = self
            .usage_counts
            .get(&item.label)
            .copied()
            .unwrap_or(0) as f64;

        let recency = self
            .last_used
            .get(&item.label)
            .map(|&ts| {
                let age = now.saturating_sub(ts);
                1.0 / (1.0 + age as f64 / 1000.0)
            })
            .unwrap_or(0.0);

        relevance * self.relevance_weight
            + frequency * self.frequency_weight
            + recency * self.recency_weight
    }

    /// Rank a completion list by multi-criteria score.
    pub fn rank(&self, list: &CompletionList, query: &str, now: u64) -> Vec<ScoredCompletion> {
        let mut scored: Vec<ScoredCompletion> = list
            .items
            .iter()
            .map(|item| {
                let s = self.score(item, query, now) as i64;
                ScoredCompletion::new(item.clone(), s)
            })
            .collect();
        scored.sort_by(|a, b| b.score.cmp(&a.score));
        scored
    }
}

// ---------------------------------------------------------------------------
// SuggestionFilter – type-based filtering
// ---------------------------------------------------------------------------

/// Filters suggestions by completion item kind.
#[derive(Debug, Clone)]
pub struct SuggestionFilter {
    /// Kinds to include. If empty, all kinds are included.
    include_kinds: Vec<CompletionItemKind>,
    /// Kinds to exclude.
    exclude_kinds: Vec<CompletionItemKind>,
    /// Whether to exclude deprecated items.
    pub hide_deprecated: bool,
}

impl Default for SuggestionFilter {
    fn default() -> Self {
        Self {
            include_kinds: Vec::new(),
            exclude_kinds: Vec::new(),
            hide_deprecated: false,
        }
    }
}

impl SuggestionFilter {
    /// Create a filter that includes only the specified kinds.
    pub fn include(kinds: Vec<CompletionItemKind>) -> Self {
        Self {
            include_kinds: kinds,
            ..Default::default()
        }
    }

    /// Create a filter that excludes the specified kinds.
    pub fn exclude(kinds: Vec<CompletionItemKind>) -> Self {
        Self {
            exclude_kinds: kinds,
            ..Default::default()
        }
    }

    /// Apply the filter to a completion list.
    pub fn apply(&self, list: &CompletionList) -> CompletionList {
        let items: Vec<CompletionItem> = list
            .items
            .iter()
            .filter(|item| {
                if self.hide_deprecated && item.deprecated {
                    return false;
                }
                if !self.include_kinds.is_empty() && !self.include_kinds.contains(&item.kind) {
                    return false;
                }
                if self.exclude_kinds.contains(&item.kind) {
                    return false;
                }
                true
            })
            .cloned()
            .collect();
        CompletionList {
            items,
            is_incomplete: list.is_incomplete,
        }
    }
}

// ---------------------------------------------------------------------------
// Snippet preview in suggestions
// ---------------------------------------------------------------------------

/// Generates a preview of a snippet by stripping placeholder markers.
///
/// Converts `"println!(\"$1\")"` to `"println!(\"\")"`.
pub fn snippet_preview(snippet: &str) -> String {
    let mut result = String::with_capacity(snippet.len());
    let mut chars = snippet.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            // Skip $N or ${N:default}
            if let Some(&next) = chars.peek() {
                if next == '{' {
                    chars.next(); // skip '{'
                    // Find the colon for default, or the closing brace
                    let mut depth = 1;
                    let mut in_default = false;
                    while let Some(bc) = chars.next() {
                        if bc == '{' {
                            depth += 1;
                        } else if bc == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        } else if bc == ':' && depth == 1 && !in_default {
                            in_default = true;
                        } else if in_default {
                            result.push(bc);
                        }
                    }
                } else if next.is_ascii_digit() {
                    // Skip $N
                    while let Some(&d) = chars.peek() {
                        if d.is_ascii_digit() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                } else {
                    result.push(c);
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Suggestion detail resolving
// ---------------------------------------------------------------------------

/// Resolved details for a completion item.
#[derive(Debug, Clone)]
pub struct ResolvedSuggestionDetail {
    /// The original item label.
    pub label: String,
    /// Full documentation text.
    pub documentation: Option<String>,
    /// Snippet preview text.
    pub preview: Option<String>,
    /// Parameter hints.
    pub parameters: Vec<String>,
}

/// Resolves additional details for a completion item.
pub fn resolve_suggestion_detail(item: &CompletionItem) -> ResolvedSuggestionDetail {
    let preview = item
        .insert_text
        .as_ref()
        .map(|s| snippet_preview(s));

    // Extract parameters from the insert text (simple heuristic: look for placeholders)
    let parameters = item
        .insert_text
        .as_ref()
        .map(|s| {
            let mut params = Vec::new();
            let mut chars = s.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '$' && chars.peek() == Some(&'{') {
                    chars.next(); // skip '{'
                    let mut placeholder = String::new();
                    for bc in chars.by_ref() {
                        if bc == '}' {
                            break;
                        }
                        placeholder.push(bc);
                    }
                    if let Some((_num, name)) = placeholder.split_once(':') {
                        params.push(name.to_string());
                    }
                }
            }
            params
        })
        .unwrap_or_default();

    ResolvedSuggestionDetail {
        label: item.label.clone(),
        documentation: item.documentation.clone(),
        preview,
        parameters,
    }
}


// ---------------------------------------------------------------------------
// SuggestCommitCharHandler – decides whether a typed char commits a suggestion
// ---------------------------------------------------------------------------

/// Determines whether a given character should commit (accept) the current
/// suggestion.  Different completion kinds may have different commit-character
/// sets.
#[derive(Debug, Clone)]
pub struct SuggestCommitCharHandler {
    /// Default characters that always commit.
    default_chars: Vec<char>,
    /// Per-kind overrides.
    kind_overrides: Vec<(CompletionItemKind, Vec<char>)>,
}

impl SuggestCommitCharHandler {
    /// Create with sensible defaults: `.`, `;`, `(` commit for most kinds.
    pub fn new() -> Self {
        Self {
            default_chars: vec!['.', ';', '('],
            kind_overrides: Vec::new(),
        }
    }

    /// Replace the default commit characters.
    pub fn set_default_chars(&mut self, chars: Vec<char>) {
        self.default_chars = chars;
    }

    /// Add commit characters for a specific completion kind.
    pub fn add_override(&mut self, kind: CompletionItemKind, chars: Vec<char>) {
        // Remove existing override for same kind
        self.kind_overrides.retain(|(k, _)| *k != kind);
        self.kind_overrides.push((kind, chars));
    }

    /// Returns `true` if `ch` should commit the suggestion of the given `kind`.
    pub fn should_commit(&self, ch: char, kind: CompletionItemKind) -> bool {
        for (k, chars) in &self.kind_overrides {
            if *k == kind {
                return chars.contains(&ch);
            }
        }
        self.default_chars.contains(&ch)
    }

    /// Return the effective commit characters for a given kind.
    pub fn commit_chars_for(&self, kind: CompletionItemKind) -> &[char] {
        for (k, chars) in &self.kind_overrides {
            if *k == kind {
                return chars;
            }
        }
        &self.default_chars
    }

    /// Number of default commit characters.
    pub fn default_count(&self) -> usize {
        self.default_chars.len()
    }

    /// Number of kind-specific overrides registered.
    pub fn override_count(&self) -> usize {
        self.kind_overrides.len()
    }

    /// Clear all overrides.
    pub fn clear_overrides(&mut self) {
        self.kind_overrides.clear();
    }
}

impl Default for SuggestCommitCharHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SuggestDetailPanel – layout for showing documentation / detail
// ---------------------------------------------------------------------------

/// Layout information for the suggestion detail panel that appears beside
/// the suggestion list.
#[derive(Debug, Clone)]
pub struct SuggestDetailPanel {
    /// Whether the detail panel is visible.
    pub visible: bool,
    /// Width in characters.
    pub width: u32,
    /// Maximum height in lines.
    pub max_height: u32,
    /// Currently displayed documentation text (rendered).
    pub rendered_doc: Option<String>,
    /// Label of the currently selected item.
    pub current_label: Option<String>,
}

impl SuggestDetailPanel {
    /// Create a hidden panel with default dimensions.
    pub fn new() -> Self {
        Self {
            visible: false,
            width: 60,
            max_height: 20,
            rendered_doc: None,
            current_label: None,
        }
    }

    /// Show the panel with a given item's detail.
    pub fn show(&mut self, item: &CompletionItem) {
        self.visible = true;
        self.current_label = Some(item.label.clone());
        self.rendered_doc = item.documentation.clone()
            .or_else(|| item.detail.clone());
    }

    /// Hide the panel and clear its content.
    pub fn hide(&mut self) {
        self.visible = false;
        self.rendered_doc = None;
        self.current_label = None;
    }

    /// Whether the panel has content to display.
    pub fn has_content(&self) -> bool {
        self.rendered_doc.is_some()
    }

    /// Count of lines in the rendered documentation.
    pub fn doc_line_count(&self) -> usize {
        self.rendered_doc
            .as_ref()
            .map(|d| d.lines().count())
            .unwrap_or(0)
    }

    /// Truncate the rendered doc to fit `max_height` lines, appending "..." if needed.
    pub fn truncated_doc(&self) -> Option<String> {
        let doc = self.rendered_doc.as_ref()?;
        let lines: Vec<&str> = doc.lines().collect();
        if lines.len() <= self.max_height as usize {
            return Some(doc.clone());
        }
        let mut result: Vec<&str> = lines[..self.max_height as usize].to_vec();
        result.push("...");
        Some(result.join("\n"))
    }

    /// Set the panel width.
    pub fn set_width(&mut self, width: u32) {
        self.width = width;
    }

    /// Set the panel max height.
    pub fn set_max_height(&mut self, height: u32) {
        self.max_height = height;
    }
}

impl Default for SuggestDetailPanel {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SuggestTypingIndicator – tracks typing state for suggestion triggering
// ---------------------------------------------------------------------------

/// Tracks the user's typing cadence to help decide when to trigger or
/// dismiss the suggestion widget.
#[derive(Debug, Clone)]
pub struct SuggestTypingIndicator {
    /// Characters typed since the last trigger point.
    buffer: String,
    /// Number of consecutive non-whitespace chars typed.
    consecutive_chars: usize,
    /// Minimum chars before auto-triggering.
    min_trigger_length: usize,
    /// Whether the user is actively typing (set to false on pause).
    active: bool,
}

impl SuggestTypingIndicator {
    /// Create a new indicator that triggers after `min_trigger_length` characters.
    pub fn new(min_trigger_length: usize) -> Self {
        Self {
            buffer: String::new(),
            consecutive_chars: 0,
            min_trigger_length,
            active: false,
        }
    }

    /// Record a typed character.
    pub fn type_char(&mut self, ch: char) {
        self.active = true;
        if ch.is_whitespace() {
            self.consecutive_chars = 0;
        } else {
            self.consecutive_chars += 1;
            self.buffer.push(ch);
        }
    }

    /// Whether enough characters have been typed to trigger suggestions.
    pub fn should_trigger(&self) -> bool {
        self.active && self.consecutive_chars >= self.min_trigger_length
    }

    /// The current typed word/prefix.
    pub fn current_prefix(&self) -> &str {
        // Return the last `consecutive_chars` characters from buffer
        let start = self.buffer.len().saturating_sub(self.consecutive_chars);
        &self.buffer[start..]
    }

    /// Reset the indicator (e.g., when suggestion is accepted or dismissed).
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.consecutive_chars = 0;
        self.active = false;
    }

    /// Mark typing as paused.
    pub fn pause(&mut self) {
        self.active = false;
    }

    /// Whether the indicator is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Number of consecutive non-whitespace characters.
    pub fn consecutive_count(&self) -> usize {
        self.consecutive_chars
    }

    /// Simulate a backspace: remove the last character if any.
    pub fn backspace(&mut self) {
        if self.consecutive_chars > 0 {
            self.consecutive_chars -= 1;
            self.buffer.pop();
        }
    }

    /// Total characters in the buffer.
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}

// ---------------------------------------------------------------------------
// SuggestPreselectionStrategy – decides which item to pre-select
// ---------------------------------------------------------------------------

/// Strategy for selecting which completion item should be pre-selected
/// when the suggestion list appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreselectionSource {
    /// Always select the first item.
    First,
    /// Select the item with the highest fuzzy score.
    BestScore,
    /// Select the item that was most recently accepted in the same context.
    RecentlyUsed,
    /// Select an item marked with `preselect: true`.
    Preselect,
}

/// Applies a preselection strategy to a completion list and returns the
/// 0-based index of the item that should be selected.
pub struct SuggestPreselectionStrategy {
    pub source: PreselectionSource,
}

impl SuggestPreselectionStrategy {
    pub fn new(source: PreselectionSource) -> Self {
        Self { source }
    }

    /// Given a list of items and an optional query, return the 0-based index
    /// of the item to pre-select.  Returns 0 if the list is empty.
    pub fn select_index(&self, items: &[CompletionItem], query: &str) -> usize {
        if items.is_empty() {
            return 0;
        }
        match self.source {
            PreselectionSource::First => 0,
            PreselectionSource::BestScore => {
                let mut best_idx = 0usize;
                let mut best_score: Option<i64> = None;
                for (i, item) in items.iter().enumerate() {
                    if let Some(score) = fuzzy_score(query, item.get_filter_text()) {
                        if best_score.map_or(true, |bs| score > bs) {
                            best_score = Some(score);
                            best_idx = i;
                        }
                    }
                }
                best_idx
            }
            PreselectionSource::RecentlyUsed => {
                // Fallback: we don't have history here, so pick the first.
                0
            }
            PreselectionSource::Preselect => {
                items.iter()
                    .position(|item| item.preselect)
                    .unwrap_or(0)
            }
        }
    }

    /// Shorthand: is this the "first" strategy?
    pub fn is_first(&self) -> bool {
        self.source == PreselectionSource::First
    }

    /// Shorthand: is this the "best score" strategy?
    pub fn is_best_score(&self) -> bool {
        self.source == PreselectionSource::BestScore
    }
}

impl fmt::Display for PreselectionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreselectionSource::First => write!(f, "first"),
            PreselectionSource::BestScore => write!(f, "bestScore"),
            PreselectionSource::RecentlyUsed => write!(f, "recentlyUsed"),
            PreselectionSource::Preselect => write!(f, "preselect"),
        }
    }
}


/// Autocomplete suggestion configuration manager.
#[derive(Debug, Clone)]
pub struct SuggestConfig {
    entries: Vec<SuggestEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single autocomplete suggestion entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl SuggestEntry {
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

impl SuggestConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: SuggestEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&SuggestEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut SuggestEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&SuggestEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&SuggestEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&SuggestEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<SuggestEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Autocomplete suggestion engine — extended utilities (yq)
// ---------------------------------------------------------------------------

/// Metric accumulator for suggest operations.
#[derive(Debug, Clone)]
pub struct YqMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YqMetrics {
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

/// Sliding-window rate counter for suggest.
#[derive(Debug, Clone)]
pub struct YqRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YqRateWindow {
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

/// A small LRU-style cache for suggest lookups.
#[derive(Debug, Clone)]
pub struct YqLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YqLruCache {
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
// xa_ extended helpers for suggest
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaSuggestRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaSuggestRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaSuggestCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaSuggestCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaSuggestCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 170
// ---------------------------------------------------------------------------

/// Generic object pool `Xc170Pool<T>`.
pub struct Xc170Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc170Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc170PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc170Pool<T> {
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
    pub fn stats(&self) -> Xc170PoolStats {
        Xc170PoolStats {
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

impl<T> Default for Xc170Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc170Scheduler`.
pub struct Xc170Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc170Scheduler {
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

impl Default for Xc170Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_170 hash for the given byte slice.
pub fn xc_170_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_170 convention.
pub fn xc_170_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_101 deepening: state machine + event bus ---

/// States for the Xd101 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd101State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd101State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd101Transition {
    pub from: Xd101State,
    pub to: Xd101State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd101StateMachine {
    current: Xd101State,
    history: Vec<Xd101Transition>,
    step_counter: usize,
}

impl Xd101StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd101State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd101State {
        self.current
    }

    pub fn history(&self) -> &[Xd101Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd101State) -> Result<Xd101State, String> {
        let allowed = match (self.current, target) {
            (Xd101State::Idle, Xd101State::Running) => true,
            (Xd101State::Running, Xd101State::Paused) => true,
            (Xd101State::Running, Xd101State::Done) => true,
            (Xd101State::Paused, Xd101State::Running) => true,
            (Xd101State::Paused, Xd101State::Done) => true,
            (Xd101State::Done, Xd101State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_101: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd101Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd101SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd101State> {
        let prefix = "Xd101SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd101State::Idle),
            "Running" => Some(Xd101State::Running),
            "Paused" => Some(Xd101State::Paused),
            "Done" => Some(Xd101State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd101State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd101 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd101Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd101Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd101HandlerFn = Box<dyn Fn(&Xd101Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd101EventBus {
    handlers: Vec<(usize, Option<String>, Xd101HandlerFn)>,
    next_id: usize,
    published: Vec<Xd101Event>,
}

impl Xd101EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd101Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd101Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd101Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd101Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_25: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg25Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg25Graph {
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

impl Default for Xg25Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_25: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg25Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg25Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg25Heap<T>) {
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

impl<T: Ord> Default for Xg25Heap<T> {
    fn default() -> Self { Self::new() }
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

    #[test]
    fn dedup_completions_removes_label_duplicates() {
        let items = vec![
            CompletionItem::new("foo", CompletionItemKind::Function),
            CompletionItem::new("bar", CompletionItemKind::Variable),
            CompletionItem::new("foo", CompletionItemKind::Method),
        ];
        let deduped = dedup_completions(&items);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].label, "foo");
        assert_eq!(deduped[0].kind, CompletionItemKind::Function);
        assert_eq!(deduped[1].label, "bar");
    }

    #[test]
    fn group_by_kind_groups_correctly() {
        let items = vec![
            CompletionItem::new("a", CompletionItemKind::Function),
            CompletionItem::new("b", CompletionItemKind::Variable),
            CompletionItem::new("c", CompletionItemKind::Function),
        ];
        let groups = group_by_kind(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].kind, CompletionItemKind::Function);
        assert_eq!(groups[0].items.len(), 2);
        assert_eq!(groups[1].kind, CompletionItemKind::Variable);
        assert_eq!(groups[1].items.len(), 1);
    }

    #[test]
    fn filter_by_prefix_case_insensitive() {
        let items = vec![
            CompletionItem::new("getString", CompletionItemKind::Method),
            CompletionItem::new("getValue", CompletionItemKind::Method),
            CompletionItem::new("setValue", CompletionItemKind::Method),
        ];
        let filtered = filter_by_prefix(&items, "get");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].label, "getString");
        assert_eq!(filtered[1].label, "getValue");
    }

    #[test]
    fn rank_completions_boosts_preselect() {
        let items = vec![
            CompletionItem::new("apple", CompletionItemKind::Variable),
            CompletionItem::new("apply", CompletionItemKind::Function).with_preselect(),
        ];
        let ranked = rank_completions(&items, "app", 10);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].item.label, "apply"); // preselect boosted
    }

    #[test]
    fn rank_completions_penalizes_deprecated() {
        let items = vec![
            CompletionItem::new("oldFunc", CompletionItemKind::Function).with_deprecated(),
            CompletionItem::new("newFunc", CompletionItemKind::Function),
        ];
        let ranked = rank_completions(&items, "Func", 10);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].item.label, "newFunc");
    }

    #[test]
    fn rank_completions_respects_limit() {
        let items = vec![
            CompletionItem::new("a", CompletionItemKind::Variable),
            CompletionItem::new("ab", CompletionItemKind::Variable),
            CompletionItem::new("abc", CompletionItemKind::Variable),
        ];
        let ranked = rank_completions(&items, "a", 2);
        assert_eq!(ranked.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Trigger character configuration tests
    // -----------------------------------------------------------------------

    #[test]
    fn trigger_config_default_triggers() {
        let cfg = TriggerConfig::default();
        assert!(cfg.is_trigger('.'));
        assert!(cfg.is_trigger(':'));
        assert!(!cfg.is_trigger(' '));
        assert!(cfg.is_commit('\t'));
        assert!(!cfg.is_commit('.'));
    }

    #[test]
    fn trigger_config_should_trigger_on_dot() {
        let cfg = TriggerConfig::default();
        assert!(cfg.should_trigger("foo.", 4));
        assert!(!cfg.should_trigger("fo", 2)); // only 2 chars, min is 3
        assert!(cfg.should_trigger("foo", 3)); // 3 chars == min_word_length
    }

    // -----------------------------------------------------------------------
    // Snippet variable expansion tests
    // -----------------------------------------------------------------------

    #[test]
    fn expand_snippet_tab_stops_and_defaults() {
        let vars = HashMap::new();
        let body = "for ${1:i} in ${2:collection} { $0 }";
        let expanded = expand_snippet(body, &vars);
        assert_eq!(expanded, "for i in collection {  }");
    }

    #[test]
    fn expand_snippet_variables() {
        let mut vars = HashMap::new();
        vars.insert("TM_FILENAME".to_string(), "main.rs".to_string());
        let body = "// File: ${TM_FILENAME}\n// Author: ${AUTHOR:unknown}";
        let expanded = expand_snippet(body, &vars);
        assert_eq!(expanded, "// File: main.rs\n// Author: unknown");
    }

    // -----------------------------------------------------------------------
    // Recency tracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn recency_tracker_boost_ordering() {
        let mut tracker = RecencyTracker::new(5);
        tracker.record("alpha");
        tracker.record("beta");
        tracker.record("gamma");
        // gamma is most recent (position 0), alpha is oldest (position 2)
        assert!(tracker.boost("gamma") > tracker.boost("beta"));
        assert!(tracker.boost("beta") > tracker.boost("alpha"));
        assert_eq!(tracker.boost("unknown"), 0);
        assert_eq!(tracker.len(), 3);
    }

    #[test]
    fn recency_tracker_capacity_eviction() {
        let mut tracker = RecencyTracker::new(2);
        tracker.record("a");
        tracker.record("b");
        tracker.record("c");
        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker.boost("a"), 0); // evicted
        assert!(tracker.boost("c") > 0);
    }

    #[test]
    fn rank_with_recency_boosts_recent() {
        let items = vec![
            CompletionItem::new("apple", CompletionItemKind::Variable),
            CompletionItem::new("apply", CompletionItemKind::Function),
        ];
        let mut tracker = RecencyTracker::new(10);
        tracker.record("apple");
        let ranked = rank_with_recency(&items, "app", &tracker, 10);
        assert_eq!(ranked[0].item.label, "apple");
    }

    // -----------------------------------------------------------------------
    // Multi-kind filtering tests
    // -----------------------------------------------------------------------

    #[test]
    fn filter_by_kinds_multiple() {
        let list = CompletionList::new(vec![
            CompletionItem::new("foo", CompletionItemKind::Function),
            CompletionItem::new("bar", CompletionItemKind::Variable),
            CompletionItem::new("baz", CompletionItemKind::Keyword),
            CompletionItem::new("qux", CompletionItemKind::Class),
        ]);
        let filtered = list.filter_by_kinds(&[
            CompletionItemKind::Function,
            CompletionItemKind::Variable,
        ]);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.items.iter().all(|i| {
            i.kind == CompletionItemKind::Function || i.kind == CompletionItemKind::Variable
        }));
    }

    #[test]
    fn exclude_kinds_removes_snippets_and_keywords() {
        let list = CompletionList::new(vec![
            CompletionItem::new("fn", CompletionItemKind::Keyword),
            CompletionItem::new("for_loop", CompletionItemKind::Snippet),
            CompletionItem::new("process", CompletionItemKind::Function),
        ]);
        let filtered = list.exclude_kinds(&[
            CompletionItemKind::Keyword,
            CompletionItemKind::Snippet,
        ]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.items[0].label, "process");
    }

    // -- SuggestionRanker tests --

    #[test]
    fn ranker_default_weights() {
        let r = SuggestionRanker::default();
        assert!((r.relevance_weight - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ranker_score_with_usage() {
        let mut r = SuggestionRanker::default();
        r.record_usage("println", 1000);
        r.record_usage("println", 2000);
        let item = CompletionItem::new("println", CompletionItemKind::Function);
        let score = r.score(&item, "print", 3000);
        assert!(score > 0.0);
    }

    #[test]
    fn ranker_rank_list() {
        let r = SuggestionRanker::default();
        let list = CompletionList::new(vec![
            CompletionItem::new("aaa", CompletionItemKind::Variable),
            CompletionItem::new("abc", CompletionItemKind::Function),
        ]);
        let ranked = r.rank(&list, "a", 0);
        assert_eq!(ranked.len(), 2);
    }

    // -- SuggestionFilter tests --

    #[test]
    fn filter_include_kinds() {
        let list = CompletionList::new(vec![
            CompletionItem::new("f", CompletionItemKind::Function),
            CompletionItem::new("v", CompletionItemKind::Variable),
        ]);
        let f = SuggestionFilter::include(vec![CompletionItemKind::Function]);
        let result = f.apply(&list);
        assert_eq!(result.len(), 1);
        assert_eq!(result.items[0].label, "f");
    }

    #[test]
    fn filter_exclude_kinds() {
        let list = CompletionList::new(vec![
            CompletionItem::new("f", CompletionItemKind::Function),
            CompletionItem::new("v", CompletionItemKind::Variable),
        ]);
        let f = SuggestionFilter::exclude(vec![CompletionItemKind::Variable]);
        let result = f.apply(&list);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_hide_deprecated() {
        let list = CompletionList::new(vec![
            CompletionItem::new("old", CompletionItemKind::Function).with_deprecated(),
            CompletionItem::new("new", CompletionItemKind::Function),
        ]);
        let mut f = SuggestionFilter::default();
        f.hide_deprecated = true;
        let result = f.apply(&list);
        assert_eq!(result.len(), 1);
        assert_eq!(result.items[0].label, "new");
    }

    // -- snippet_preview tests --

    #[test]
    fn snippet_preview_simple() {
        assert_eq!(snippet_preview("hello $1 world"), "hello  world");
    }

    #[test]
    fn snippet_preview_with_default() {
        assert_eq!(snippet_preview("fn ${1:name}()"), "fn name()");
    }

    #[test]
    fn snippet_preview_no_placeholders() {
        assert_eq!(snippet_preview("plain text"), "plain text");
    }

    // -- resolve_suggestion_detail tests --

    #[test]
    fn resolve_detail_basic() {
        let item = CompletionItem::new("println", CompletionItemKind::Function)
            .with_insert_text("println!(\"${1:msg}\")")
            .with_documentation("Print to stdout");
        let detail = resolve_suggestion_detail(&item);
        assert_eq!(detail.label, "println");
        assert_eq!(detail.documentation.as_deref(), Some("Print to stdout"));
        assert_eq!(detail.parameters, vec!["msg"]);
        assert!(detail.preview.is_some());
    }

    #[test]
    fn resolve_detail_no_snippet() {
        let item = CompletionItem::new("x", CompletionItemKind::Variable);
        let detail = resolve_suggestion_detail(&item);
        assert!(detail.preview.is_none());
        assert!(detail.parameters.is_empty());
    }

    // -- SuggestCommitCharHandler tests --

    #[test]
    fn commit_char_defaults() {
        let handler = SuggestCommitCharHandler::new();
        assert!(handler.should_commit('.', CompletionItemKind::Method));
        assert!(handler.should_commit(';', CompletionItemKind::Variable));
        assert!(!handler.should_commit('x', CompletionItemKind::Function));
        assert_eq!(handler.default_count(), 3);
    }

    #[test]
    fn commit_char_override() {
        let mut handler = SuggestCommitCharHandler::new();
        handler.add_override(CompletionItemKind::Snippet, vec!['|', '\t']);
        assert!(handler.should_commit('|', CompletionItemKind::Snippet));
        assert!(!handler.should_commit('.', CompletionItemKind::Snippet));
        // Default still works for other kinds
        assert!(handler.should_commit('.', CompletionItemKind::Method));
        assert_eq!(handler.override_count(), 1);
    }

    #[test]
    fn commit_char_clear_overrides() {
        let mut handler = SuggestCommitCharHandler::new();
        handler.add_override(CompletionItemKind::Snippet, vec!['|']);
        handler.clear_overrides();
        assert_eq!(handler.override_count(), 0);
        // Falls back to default
        assert!(handler.should_commit('.', CompletionItemKind::Snippet));
    }

    #[test]
    fn commit_char_for_kind() {
        let mut handler = SuggestCommitCharHandler::new();
        handler.add_override(CompletionItemKind::Keyword, vec!['(', ' ']);
        let chars = handler.commit_chars_for(CompletionItemKind::Keyword);
        assert_eq!(chars, &['(', ' ']);
        // Non-overridden kind returns defaults
        let def = handler.commit_chars_for(CompletionItemKind::Variable);
        assert_eq!(def, &['.', ';', '(']);
    }

    // -- SuggestDetailPanel tests --

    #[test]
    fn detail_panel_hidden_by_default() {
        let panel = SuggestDetailPanel::new();
        assert!(!panel.visible);
        assert!(!panel.has_content());
        assert_eq!(panel.doc_line_count(), 0);
    }

    #[test]
    fn detail_panel_show_item() {
        let mut panel = SuggestDetailPanel::new();
        let item = CompletionItem::new("myFunc", CompletionItemKind::Function)
            .with_documentation("Does something\nuseful");
        panel.show(&item);
        assert!(panel.visible);
        assert!(panel.has_content());
        assert_eq!(panel.current_label.as_deref(), Some("myFunc"));
        assert_eq!(panel.doc_line_count(), 2);
    }

    #[test]
    fn detail_panel_hide() {
        let mut panel = SuggestDetailPanel::new();
        let item = CompletionItem::new("x", CompletionItemKind::Variable)
            .with_documentation("info");
        panel.show(&item);
        panel.hide();
        assert!(!panel.visible);
        assert!(!panel.has_content());
    }

    #[test]
    fn detail_panel_truncated_doc() {
        let mut panel = SuggestDetailPanel::new();
        panel.set_max_height(3);
        let long_doc = "line1\nline2\nline3\nline4\nline5";
        let item = CompletionItem::new("f", CompletionItemKind::Function)
            .with_documentation(long_doc);
        panel.show(&item);
        let trunc = panel.truncated_doc().unwrap();
        let lines: Vec<&str> = trunc.lines().collect();
        assert_eq!(lines.len(), 4); // 3 lines + "..."
        assert_eq!(lines[3], "...");
    }

    #[test]
    fn detail_panel_no_truncation_needed() {
        let mut panel = SuggestDetailPanel::new();
        panel.set_max_height(10);
        let item = CompletionItem::new("g", CompletionItemKind::Variable)
            .with_documentation("short");
        panel.show(&item);
        let trunc = panel.truncated_doc().unwrap();
        assert_eq!(trunc, "short");
    }

    // -- SuggestTypingIndicator tests --

    #[test]
    fn typing_indicator_initial() {
        let ind = SuggestTypingIndicator::new(3);
        assert!(!ind.is_active());
        assert!(!ind.should_trigger());
        assert_eq!(ind.current_prefix(), "");
        assert_eq!(ind.buffer_len(), 0);
    }

    #[test]
    fn typing_indicator_trigger() {
        let mut ind = SuggestTypingIndicator::new(3);
        ind.type_char('f');
        assert!(!ind.should_trigger());
        ind.type_char('o');
        assert!(!ind.should_trigger());
        ind.type_char('o');
        assert!(ind.should_trigger());
        assert_eq!(ind.current_prefix(), "foo");
        assert_eq!(ind.consecutive_count(), 3);
    }

    #[test]
    fn typing_indicator_whitespace_resets_consecutive() {
        let mut ind = SuggestTypingIndicator::new(2);
        ind.type_char('a');
        ind.type_char('b');
        assert!(ind.should_trigger());
        ind.type_char(' ');
        assert!(!ind.should_trigger());
        assert_eq!(ind.consecutive_count(), 0);
    }

    #[test]
    fn typing_indicator_reset() {
        let mut ind = SuggestTypingIndicator::new(1);
        ind.type_char('x');
        assert!(ind.should_trigger());
        ind.reset();
        assert!(!ind.is_active());
        assert_eq!(ind.buffer_len(), 0);
    }

    #[test]
    fn typing_indicator_backspace() {
        let mut ind = SuggestTypingIndicator::new(2);
        ind.type_char('a');
        ind.type_char('b');
        ind.backspace();
        assert_eq!(ind.consecutive_count(), 1);
        assert!(!ind.should_trigger());
    }

    #[test]
    fn typing_indicator_pause() {
        let mut ind = SuggestTypingIndicator::new(1);
        ind.type_char('z');
        assert!(ind.is_active());
        ind.pause();
        assert!(!ind.is_active());
        assert!(!ind.should_trigger());
    }

    // -- SuggestPreselectionStrategy tests --

    #[test]
    fn preselect_first() {
        let strat = SuggestPreselectionStrategy::new(PreselectionSource::First);
        let items = vec![
            CompletionItem::new("alpha", CompletionItemKind::Variable),
            CompletionItem::new("beta", CompletionItemKind::Variable),
        ];
        assert_eq!(strat.select_index(&items, ""), 0);
        assert!(strat.is_first());
    }

    #[test]
    fn preselect_best_score() {
        let strat = SuggestPreselectionStrategy::new(PreselectionSource::BestScore);
        let items = vec![
            CompletionItem::new("something_else", CompletionItemKind::Variable),
            CompletionItem::new("toString", CompletionItemKind::Method),
        ];
        // "toStr" should match "toString" better
        let idx = strat.select_index(&items, "toStr");
        assert_eq!(idx, 1);
        assert!(strat.is_best_score());
    }

    #[test]
    fn preselect_preselect_flag() {
        let strat = SuggestPreselectionStrategy::new(PreselectionSource::Preselect);
        let items = vec![
            CompletionItem::new("a", CompletionItemKind::Variable),
            CompletionItem::new("b", CompletionItemKind::Variable).with_preselect(),
            CompletionItem::new("c", CompletionItemKind::Variable),
        ];
        assert_eq!(strat.select_index(&items, ""), 1);
    }

    #[test]
    fn preselect_empty_list() {
        let strat = SuggestPreselectionStrategy::new(PreselectionSource::BestScore);
        assert_eq!(strat.select_index(&[], "query"), 0);
    }

    #[test]
    fn preselection_source_display() {
        assert_eq!(format!("{}", PreselectionSource::First), "first");
        assert_eq!(format!("{}", PreselectionSource::BestScore), "bestScore");
        assert_eq!(format!("{}", PreselectionSource::RecentlyUsed), "recentlyUsed");
        assert_eq!(format!("{}", PreselectionSource::Preselect), "preselect");
    }


    #[test]
    fn suggest_entry_creation() {
        let e = SuggestEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn suggest_entry_with_priority() {
        let e = SuggestEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn suggest_entry_metadata() {
        let e = SuggestEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn suggest_entry_remove_meta() {
        let mut e = SuggestEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn suggest_entry_activate_deactivate() {
        let mut e = SuggestEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn suggest_config_add_sorted() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("lo", "Lo").with_priority(1));
        c.add(SuggestEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn suggest_config_capacity() {
        let mut c = SuggestConfig::new(1);
        assert!(c.add(SuggestEntry::new("a", "A")));
        assert!(!c.add(SuggestEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn suggest_config_remove() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn suggest_config_get() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn suggest_config_active_entries() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "A"));
        c.add(SuggestEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn suggest_config_enable_disable() {
        let mut c = SuggestConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn suggest_config_clear() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn suggest_config_find_by_label() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn suggest_config_top_n() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "A").with_priority(1));
        c.add(SuggestEntry::new("b", "B").with_priority(2));
        c.add(SuggestEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn suggest_config_deactivate_activate_all() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "A"));
        c.add(SuggestEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn suggest_config_highest_priority() {
        let mut c = SuggestConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(SuggestEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn suggest_config_contains() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn suggest_config_labels() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "Alpha"));
        c.add(SuggestEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn suggest_config_drain_inactive() {
        let mut c = SuggestConfig::new(10);
        c.add(SuggestEntry::new("a", "A"));
        c.add(SuggestEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn yq_metrics_empty() {
        let m = YqMetrics::new("suggest");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yq_metrics_record_and_mean() {
        let mut m = YqMetrics::new("suggest");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yq_metrics_min_max() {
        let mut m = YqMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yq_metrics_variance_and_std() {
        let mut m = YqMetrics::new("v");
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
    fn yq_metrics_percentile() {
        let mut m = YqMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yq_metrics_merge() {
        let mut a = YqMetrics::new("a");
        a.record(1.0);
        let mut b = YqMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yq_metrics_reset() {
        let mut m = YqMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yq_rate_window_empty() {
        let rw = YqRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yq_rate_window_tick_and_rate() {
        let mut rw = YqRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yq_lru_cache_basic() {
        let mut c = YqLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yq_lru_cache_contains_and_keys() {
        let mut c = YqLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yq_lru_cache_remove() {
        let mut c = YqLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yq_metrics_sum() {
        let mut m = YqMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yq_metrics_label() {
        let m = YqMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yq_lru_cache_clear() {
        let mut c = YqLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for suggest
    #[test]
    fn xa_suggest_ring_new() {
        let rb = super::XaSuggestRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_suggest_ring_push_len() {
        let mut rb = super::XaSuggestRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_suggest_ring_wrap() {
        let mut rb = super::XaSuggestRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_suggest_ring_mean_empty() {
        let rb = super::XaSuggestRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_suggest_ring_mean_values() {
        let mut rb = super::XaSuggestRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_suggest_ring_min_max() {
        let mut rb = super::XaSuggestRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_suggest_ring_iter() {
        let mut rb = super::XaSuggestRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_suggest_counter_new() {
        let c = super::XaSuggestCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_suggest_counter_inc() {
        let mut c = super::XaSuggestCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_suggest_counter_inc_by() {
        let mut c = super::XaSuggestCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_suggest_counter_reset() {
        let mut c = super::XaSuggestCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_suggest_counter_clear() {
        let mut c = super::XaSuggestCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_suggest_counter_default() {
        let c = super::XaSuggestCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 170 ----

    #[test]
    fn xc_170_pool_new_empty() {
        let pool: super::Xc170Pool<i32> = super::Xc170Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_170_pool_release_acquire() {
        let mut pool = super::Xc170Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_170_pool_acquire_empty() {
        let mut pool: super::Xc170Pool<i32> = super::Xc170Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_170_pool_full() {
        let mut pool = super::Xc170Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_170_pool_drain() {
        let mut pool = super::Xc170Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_170_pool_stats() {
        let mut pool = super::Xc170Pool::new(8);
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
    fn xc_170_pool_clear() {
        let mut pool = super::Xc170Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_170_pool_shrink() {
        let mut pool = super::Xc170Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_170_pool_default() {
        let pool: super::Xc170Pool<String> = super::Xc170Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_170_pool_extend() {
        let mut pool = super::Xc170Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_170_pool_retain() {
        let mut pool = super::Xc170Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_170_scheduler_round_robin() {
        let mut sched = super::Xc170Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_170_scheduler_empty() {
        let mut sched = super::Xc170Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_170_scheduler_reset() {
        let mut sched = super::Xc170Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_170_scheduler_add_remove() {
        let mut sched = super::Xc170Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_170_scheduler_targets() {
        let sched = super::Xc170Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_170_hash_empty() {
        assert_eq!(super::xc_170_hash(b""), 5381);
    }

    #[test]
    fn xc_170_hash_data() {
        let h = super::xc_170_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_170_hash(b"hello"), h);
    }

    #[test]
    fn xc_170_reverse_str() {
        assert_eq!(super::xc_170_reverse("abc"), "cba");
        assert_eq!(super::xc_170_reverse(""), "");
    }


    // --- xd_101 deepening tests ---

    #[test]
    fn xd_101_sm_initial_state() {
        let sm = Xd101StateMachine::new();
        assert_eq!(sm.current_state(), Xd101State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_101_sm_valid_idle_to_running() {
        let mut sm = Xd101StateMachine::new();
        assert!(sm.transition(Xd101State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd101State::Running);
    }

    #[test]
    fn xd_101_sm_valid_running_to_paused() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        assert!(sm.transition(Xd101State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd101State::Paused);
    }

    #[test]
    fn xd_101_sm_valid_running_to_done() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        assert!(sm.transition(Xd101State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd101State::Done);
    }

    #[test]
    fn xd_101_sm_valid_paused_to_running() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        sm.transition(Xd101State::Paused).unwrap();
        assert!(sm.transition(Xd101State::Running).is_ok());
    }

    #[test]
    fn xd_101_sm_valid_done_to_idle() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        sm.transition(Xd101State::Done).unwrap();
        assert!(sm.transition(Xd101State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd101State::Idle);
    }

    #[test]
    fn xd_101_sm_invalid_idle_to_done() {
        let mut sm = Xd101StateMachine::new();
        assert!(sm.transition(Xd101State::Done).is_err());
    }

    #[test]
    fn xd_101_sm_invalid_idle_to_paused() {
        let mut sm = Xd101StateMachine::new();
        assert!(sm.transition(Xd101State::Paused).is_err());
    }

    #[test]
    fn xd_101_sm_history_tracking() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        sm.transition(Xd101State::Paused).unwrap();
        sm.transition(Xd101State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd101State::Idle);
        assert_eq!(sm.history()[0].to, Xd101State::Running);
        assert_eq!(sm.history()[1].from, Xd101State::Running);
        assert_eq!(sm.history()[2].to, Xd101State::Done);
    }

    #[test]
    fn xd_101_sm_serialize_deserialize() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd101StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd101State::Running));
    }

    #[test]
    fn xd_101_sm_deserialize_invalid() {
        assert_eq!(Xd101StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_101_sm_reset() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd101State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_101_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd101EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd101Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_101_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd101EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd101Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd101Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_101_bus_unsubscribe() {
        let mut bus = Xd101EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_101_event_kind_and_payload() {
        let e = Xd101Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd101Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_101_bus_clear_history() {
        let mut bus = Xd101EventBus::new();
        bus.publish(Xd101Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_101_sm_step_counter_increments() {
        let mut sm = Xd101StateMachine::new();
        sm.transition(Xd101State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd101State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_25 graph tests ------------------------------------------------

    #[test]
    fn xg_25_graph_empty() {
        let g = super::Xg25Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_25_graph_add_node() {
        let mut g = super::Xg25Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_25_graph_add_edge() {
        let mut g = super::Xg25Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_25_graph_neighbors() {
        let mut g = super::Xg25Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_25_graph_has_path() {
        let mut g = super::Xg25Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_25_graph_self_path() {
        let g = super::Xg25Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_25_graph_topo_sort() {
        let mut g = super::Xg25Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_25_graph_cycle_detect_false() {
        let mut g = super::Xg25Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_25_graph_cycle_detect_true() {
        let mut g = super::Xg25Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_25 heap tests -------------------------------------------------

    #[test]
    fn xg_25_heap_empty() {
        let h: super::Xg25Heap<i32> = super::Xg25Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_25_heap_push_pop() {
        let mut h = super::Xg25Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_25_heap_peek() {
        let mut h = super::Xg25Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_25_heap_drain_sorted() {
        let mut h = super::Xg25Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_25_heap_merge() {
        let mut a = super::Xg25Heap::new();
        let mut b = super::Xg25Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_25_heap_default() {
        let h: super::Xg25Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_25_graph_default() {
        let g: super::Xg25Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }

}
