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
}
