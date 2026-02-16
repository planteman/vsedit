//! Inline ghost text completions.

/// A single inline completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCompletionItem {
    pub insert_text: String,
    pub range_start_line: u32,
    pub range_start_col: u32,
    pub range_end_line: u32,
    pub range_end_col: u32,
    pub filter_text: Option<String>,
    pub command: Option<String>,
}

/// A list of inline completion items.
#[derive(Debug, Clone, Default)]
pub struct InlineCompletionList {
    pub items: Vec<InlineCompletionItem>,
}

/// What triggered the inline completion request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionTriggerKind {
    Invoke,
    Automatic,
}

/// Context passed to an inline completion provider.
#[derive(Debug, Clone)]
pub struct InlineCompletionContext {
    pub trigger_kind: InlineCompletionTriggerKind,
    pub selected_suggestion: Option<String>,
}

/// Trait for providing inline completions.
pub trait InlineCompletionProvider {
    fn provide_inline_completions(
        &self,
        uri: &str,
        line: u32,
        col: u32,
        context: &InlineCompletionContext,
    ) -> Option<InlineCompletionList>;
}

/// Tracks the current ghost text display state in the editor.
#[derive(Debug, Clone)]
pub struct GhostTextWidget {
    active_item: Option<InlineCompletionItem>,
    visible: bool,
}

impl GhostTextWidget {
    pub fn new() -> Self {
        Self {
            active_item: None,
            visible: false,
        }
    }

    pub fn show(&mut self, item: InlineCompletionItem) {
        self.active_item = Some(item);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn dismiss(&mut self) {
        self.active_item = None;
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn active_item(&self) -> Option<&InlineCompletionItem> {
        self.active_item.as_ref()
    }
}

impl Default for GhostTextWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl GhostTextWidget {
    /// Returns the first word of the active inline completion text.
    pub fn accept_word(&self) -> Option<String> {
        self.active_item.as_ref().map(|item| {
            let text = &item.insert_text;
            let end = text
                .find(|c: char| c.is_whitespace())
                .unwrap_or(text.len());
            text[..end].to_string()
        })
    }

    /// Returns the first line of the active inline completion text.
    pub fn accept_line(&self) -> Option<String> {
        self.active_item.as_ref().map(|item| {
            let text = &item.insert_text;
            let end = text.find('\n').unwrap_or(text.len());
            text[..end].to_string()
        })
    }
}

// ---------------------------------------------------------------------------
// Inline completion session
// ---------------------------------------------------------------------------

/// Manages cycling through a list of inline completion items.
#[derive(Debug, Clone)]
pub struct InlineCompletionSession {
    items: Vec<InlineCompletionItem>,
    current_index: usize,
}

impl InlineCompletionSession {
    /// Creates a new session from a completion list.
    pub fn new(list: InlineCompletionList) -> Self {
        Self {
            items: list.items,
            current_index: 0,
        }
    }

    /// Advances to the next completion item, wrapping around.
    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.current_index = (self.current_index + 1) % self.items.len();
        }
    }

    /// Moves to the previous completion item, wrapping around.
    pub fn previous(&mut self) {
        if !self.items.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.items.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    /// Returns a reference to the currently selected item.
    pub fn current(&self) -> Option<&InlineCompletionItem> {
        self.items.get(self.current_index)
    }

    /// Accepts the current item, returning it and consuming the session.
    pub fn accept(self) -> Option<InlineCompletionItem> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items[self.current_index].clone())
        }
    }

    /// Returns the number of items in the session.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the session has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Inline completion configuration
// ---------------------------------------------------------------------------

/// How inline completions are matched against the current prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionMatchMode {
    Prefix,
    Subword,
}

/// Configuration for inline completion behavior.
#[derive(Debug, Clone)]
pub struct InlineCompletionConfig {
    pub enabled: bool,
    pub show_toolbar: bool,
    pub mode: InlineCompletionMatchMode,
}

impl Default for InlineCompletionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_toolbar: false,
            mode: InlineCompletionMatchMode::Prefix,
        }
    }
}

// ---------------------------------------------------------------------------
// Provider registry
// ---------------------------------------------------------------------------

/// A registry holding multiple inline completion providers.
#[derive(Default)]
pub struct InlineCompletionRegistry {
    providers: Vec<Box<dyn InlineCompletionProvider>>,
}

impl InlineCompletionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new provider.
    pub fn register_provider(&mut self, provider: Box<dyn InlineCompletionProvider>) {
        self.providers.push(provider);
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_text_show_hide() {
        let mut w = GhostTextWidget::new();
        assert!(!w.is_visible());
        assert!(w.active_item().is_none());

        let item = InlineCompletionItem {
            insert_text: "hello()".into(),
            range_start_line: 0,
            range_start_col: 0,
            range_end_line: 0,
            range_end_col: 0,
            filter_text: None,
            command: None,
        };
        w.show(item.clone());
        assert!(w.is_visible());
        assert_eq!(w.active_item().unwrap().insert_text, "hello()");

        w.hide();
        assert!(!w.is_visible());
        assert!(w.active_item().is_some()); // item still retained

        w.dismiss();
        assert!(w.active_item().is_none());
    }

    #[test]
    fn completion_list_default_empty() {
        let list = InlineCompletionList::default();
        assert!(list.items.is_empty());
    }

    #[test]
    fn provider_trait_impl() {
        struct DummyProvider;
        impl InlineCompletionProvider for DummyProvider {
            fn provide_inline_completions(
                &self,
                _uri: &str,
                _line: u32,
                _col: u32,
                _context: &InlineCompletionContext,
            ) -> Option<InlineCompletionList> {
                Some(InlineCompletionList {
                    items: vec![InlineCompletionItem {
                        insert_text: "world".into(),
                        range_start_line: 1,
                        range_start_col: 0,
                        range_end_line: 1,
                        range_end_col: 0,
                        filter_text: Some("wor".into()),
                        command: None,
                    }],
                })
            }
        }

        let ctx = InlineCompletionContext {
            trigger_kind: InlineCompletionTriggerKind::Automatic,
            selected_suggestion: None,
        };
        let provider = DummyProvider;
        let result = provider.provide_inline_completions("file:///test.rs", 1, 0, &ctx);
        assert!(result.is_some());
        assert_eq!(result.unwrap().items.len(), 1);
    }

    #[test]
    fn trigger_kind_equality() {
        assert_eq!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Invoke);
        assert_ne!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Automatic);
    }

    fn make_item(text: &str) -> InlineCompletionItem {
        InlineCompletionItem {
            insert_text: text.into(),
            range_start_line: 0,
            range_start_col: 0,
            range_end_line: 0,
            range_end_col: 0,
            filter_text: None,
            command: None,
        }
    }

    fn make_list(texts: &[&str]) -> InlineCompletionList {
        InlineCompletionList {
            items: texts.iter().map(|t| make_item(t)).collect(),
        }
    }

    #[test]
    fn session_next_previous_cycle() {
        let mut session = InlineCompletionSession::new(make_list(&["a", "b", "c"]));
        assert_eq!(session.current().unwrap().insert_text, "a");
        session.next();
        assert_eq!(session.current().unwrap().insert_text, "b");
        session.next();
        session.next();
        assert_eq!(session.current().unwrap().insert_text, "a"); // wraps
        session.previous();
        assert_eq!(session.current().unwrap().insert_text, "c"); // wraps back
    }

    #[test]
    fn session_empty() {
        let session = InlineCompletionSession::new(InlineCompletionList::default());
        assert!(session.is_empty());
        assert_eq!(session.len(), 0);
        assert!(session.current().is_none());
    }

    #[test]
    fn session_accept() {
        let session = InlineCompletionSession::new(make_list(&["hello"]));
        let accepted = session.accept();
        assert_eq!(accepted.unwrap().insert_text, "hello");
    }

    #[test]
    fn session_len() {
        let session = InlineCompletionSession::new(make_list(&["a", "b"]));
        assert_eq!(session.len(), 2);
        assert!(!session.is_empty());
    }

    #[test]
    fn accept_word_single_word() {
        let mut w = GhostTextWidget::new();
        w.show(make_item("hello"));
        assert_eq!(w.accept_word(), Some("hello".into()));
    }

    #[test]
    fn accept_word_multi_word() {
        let mut w = GhostTextWidget::new();
        w.show(make_item("hello world foo"));
        assert_eq!(w.accept_word(), Some("hello".into()));
    }

    #[test]
    fn accept_line_multi_line() {
        let mut w = GhostTextWidget::new();
        w.show(make_item("first line\nsecond line"));
        assert_eq!(w.accept_line(), Some("first line".into()));
    }

    #[test]
    fn accept_word_none_when_empty() {
        let w = GhostTextWidget::new();
        assert!(w.accept_word().is_none());
    }

    #[test]
    fn config_default() {
        let cfg = InlineCompletionConfig::default();
        assert!(cfg.enabled);
        assert!(!cfg.show_toolbar);
        assert_eq!(cfg.mode, InlineCompletionMatchMode::Prefix);
    }

    #[test]
    fn registry_register_and_count() {
        struct Dummy;
        impl InlineCompletionProvider for Dummy {
            fn provide_inline_completions(
                &self, _: &str, _: u32, _: u32, _: &InlineCompletionContext,
            ) -> Option<InlineCompletionList> {
                None
            }
        }
        let mut reg = InlineCompletionRegistry::new();
        assert_eq!(reg.provider_count(), 0);
        reg.register_provider(Box::new(Dummy));
        assert_eq!(reg.provider_count(), 1);
        reg.register_provider(Box::new(Dummy));
        assert_eq!(reg.provider_count(), 2);
    }
}
