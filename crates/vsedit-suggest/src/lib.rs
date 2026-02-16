//! Autocomplete and suggestions.

/// Completion item kind matching VS Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Text, Method, Function, Constructor, Field, Variable,
    Class, Interface, Module, Property, Unit, Value, Enum,
    Keyword, Snippet, Color, File, Reference, Folder,
    EnumMember, Constant, Struct, Event, Operator, TypeParameter,
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
}

impl CompletionItem {
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        Self {
            label: label.into(), kind, detail: None, documentation: None,
            insert_text: None, sort_text: None, filter_text: None,
            preselect: false, deprecated: false,
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
}
