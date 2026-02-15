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
}
