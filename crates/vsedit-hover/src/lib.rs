//! Hover tooltip service.
//!
//! Equivalent to VS Code's `vs/editor/contrib/hover`.
//! Provides hover content model for displaying tooltips at cursor positions.

/// How the hover was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverTriggerKind {
    /// Explicitly invoked (e.g. via keyboard shortcut).
    Invoke,
    /// Triggered by mouse hover.
    Hover,
    /// Triggered by content interaction (e.g. clicking a link).
    ContentHover,
}

/// Configuration for hover behaviour.
#[derive(Debug, Clone)]
pub struct HoverConfig {
    /// Whether hover is enabled.
    pub enabled: bool,
    /// Delay in milliseconds before showing hover.
    pub delay_ms: u32,
    /// Whether the hover stays visible when the mouse moves away.
    pub sticky: bool,
    /// Prefer showing the hover above the line.
    pub above_line_preference: bool,
}

impl Default for HoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_ms: 300,
            sticky: true,
            above_line_preference: true,
        }
    }
}

/// Tracks the state of an active hover session.
#[derive(Debug, Clone)]
pub struct HoverSession {
    pub current_hover: Option<Hover>,
    pub line: u32,
    pub col: u32,
    pub visible: bool,
    pub pinned: bool,
}

impl HoverSession {
    pub fn new() -> Self {
        Self {
            current_hover: None,
            line: 0,
            col: 0,
            visible: false,
            pinned: false,
        }
    }

    /// Show a hover at the given position.
    pub fn show(&mut self, hover: Hover, line: u32, col: u32) {
        self.current_hover = Some(hover);
        self.line = line;
        self.col = col;
        self.visible = true;
    }

    /// Hide the current hover (unless pinned).
    pub fn hide(&mut self) {
        if !self.pinned {
            self.visible = false;
            self.current_hover = None;
        }
    }

    /// Toggle the pinned state. Pinned hovers remain visible until explicitly unpinned.
    pub fn toggle_pin(&mut self) {
        self.pinned = !self.pinned;
        if !self.pinned {
            self.visible = false;
            self.current_hover = None;
        }
    }
}

impl Default for HoverSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Content that can be displayed in a hover.
#[derive(Debug, Clone)]
pub enum HoverContent {
    /// Plain text.
    Text(String),
    /// Markdown text.
    Markdown(String),
    /// Code with optional language.
    Code {
        value: String,
        language: Option<String>,
    },
}

/// A hover result containing multiple content blocks.
#[derive(Debug, Clone)]
pub struct Hover {
    pub contents: Vec<HoverContent>,
    pub range: Option<HoverRange>,
}

/// The range a hover applies to.
#[derive(Debug, Clone, Copy)]
pub struct HoverRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl Hover {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            contents: vec![HoverContent::Text(text.into())],
            range: None,
        }
    }

    pub fn markdown(md: impl Into<String>) -> Self {
        Self {
            contents: vec![HoverContent::Markdown(md.into())],
            range: None,
        }
    }

    pub fn code(code: impl Into<String>, language: Option<&str>) -> Self {
        Self {
            contents: vec![HoverContent::Code {
                value: code.into(),
                language: language.map(|s| s.to_string()),
            }],
            range: None,
        }
    }

    /// Convenience constructor from a vec of contents.
    pub fn from_contents(contents: Vec<HoverContent>) -> Self {
        Self {
            contents,
            range: None,
        }
    }

    pub fn with_range(mut self, range: HoverRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn add_content(mut self, content: HoverContent) -> Self {
        self.contents.push(content);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// Number of content blocks.
    pub fn content_count(&self) -> usize {
        self.contents.len()
    }

    /// Returns true if any content block is a code block.
    pub fn has_code_content(&self) -> bool {
        self.contents
            .iter()
            .any(|c| matches!(c, HoverContent::Code { .. }))
    }
}

/// Provider for hover content.
pub trait HoverProvider: Send + Sync {
    fn provide_hover(&self, line: u32, column: u32) -> Option<Hover>;
}

/// Registry for hover providers.
pub struct HoverRegistry {
    providers: Vec<Box<dyn HoverProvider>>,
}

impl HoverRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn HoverProvider>) {
        self.providers.push(provider);
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Get combined hover content from all providers.
    pub fn provide_hover(&self, line: u32, column: u32) -> Option<Hover> {
        let mut contents = Vec::new();
        let mut range = None;

        for provider in &self.providers {
            if let Some(hover) = provider.provide_hover(line, column) {
                contents.extend(hover.contents);
                if range.is_none() {
                    range = hover.range;
                }
            }
        }

        if contents.is_empty() {
            None
        } else {
            Some(Hover { contents, range })
        }
    }
}

impl Default for HoverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Check whether a position falls within a hover range (inclusive).
pub fn is_position_in_range(range: &HoverRange, line: u32, col: u32) -> bool {
    if line < range.start_line || line > range.end_line {
        return false;
    }
    if line == range.start_line && col < range.start_column {
        return false;
    }
    if line == range.end_line && col > range.end_column {
        return false;
    }
    true
}

/// Merge multiple hover results into a single hover.
///
/// Contents are concatenated. The range of the first hover that has one is used.
pub fn merge_hovers(hovers: &[Hover]) -> Hover {
    let mut contents = Vec::new();
    let mut range = None;
    for hover in hovers {
        contents.extend(hover.contents.clone());
        if range.is_none() {
            range = hover.range;
        }
    }
    Hover { contents, range }
}

/// Render hover contents to a plain-text string.
pub fn render_hover_to_string(hover: &Hover) -> String {
    let mut out = String::new();
    for (i, content) in hover.contents.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str("---");
            out.push('\n');
        }
        match content {
            HoverContent::Text(t) => out.push_str(t),
            HoverContent::Markdown(md) => out.push_str(md),
            HoverContent::Code { value, language } => {
                if let Some(lang) = language {
                    out.push('[');
                    out.push_str(lang);
                    out.push_str("] ");
                }
                out.push_str(value);
            }
        }
    }
    out
}

/// Manages hover display timing based on trigger kind and configuration.
#[derive(Debug, Clone)]
pub struct HoverDelay;

impl HoverDelay {
    /// Returns true if enough time has elapsed to show the hover.
    pub fn should_show(elapsed_ms: u32, config: &HoverConfig) -> bool {
        config.enabled && elapsed_ms >= config.delay_ms
    }

    /// Computes the appropriate delay for a given trigger kind.
    /// Explicit invocations show immediately; mouse hovers use the configured delay.
    pub fn compute_delay(trigger_kind: HoverTriggerKind, config: &HoverConfig) -> u32 {
        match trigger_kind {
            HoverTriggerKind::Invoke => 0,
            HoverTriggerKind::Hover => config.delay_ms,
            HoverTriggerKind::ContentHover => config.delay_ms / 2,
        }
    }
}

/// Tracks positions where hovers have been shown, for frequency analysis.
#[derive(Debug, Clone)]
pub struct HoverHistory {
    entries: Vec<(u32, u32)>,
}

impl HoverHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record that a hover was shown at the given position.
    pub fn record(&mut self, line: u32, col: u32) {
        self.entries.push((line, col));
    }

    /// Return the top-N most frequently hovered positions as `(line, col, count)`.
    pub fn get_frequent_positions(&self, top_n: usize) -> Vec<(u32, u32, usize)> {
        use std::collections::HashMap;
        let mut counts: HashMap<(u32, u32), usize> = HashMap::new();
        for &pos in &self.entries {
            *counts.entry(pos).or_insert(0) += 1;
        }
        let mut sorted: Vec<(u32, u32, usize)> =
            counts.into_iter().map(|((l, c), n)| (l, c, n)).collect();
        sorted.sort_by(|a, b| b.2.cmp(&a.2));
        sorted.truncate(top_n);
        sorted
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for HoverHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Fluent builder for constructing [`Hover`] instances.
#[derive(Debug, Clone)]
pub struct HoverContentBuilder {
    contents: Vec<HoverContent>,
    range: Option<HoverRange>,
}

impl HoverContentBuilder {
    pub fn new() -> Self {
        Self {
            contents: Vec::new(),
            range: None,
        }
    }

    pub fn add_text(mut self, text: impl Into<String>) -> Self {
        self.contents.push(HoverContent::Text(text.into()));
        self
    }

    pub fn add_markdown(mut self, md: impl Into<String>) -> Self {
        self.contents.push(HoverContent::Markdown(md.into()));
        self
    }

    pub fn add_code(mut self, code: impl Into<String>, language: Option<&str>) -> Self {
        self.contents.push(HoverContent::Code {
            value: code.into(),
            language: language.map(|s| s.to_string()),
        });
        self
    }

    pub fn add_separator(mut self) -> Self {
        self.contents
            .push(HoverContent::Text("---".to_string()));
        self
    }

    pub fn set_range(mut self, range: HoverRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn build(self) -> Hover {
        Hover {
            contents: self.contents,
            range: self.range,
        }
    }
}

impl Default for HoverContentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns a new hover with each content block truncated to `max_chars`.
pub fn truncate_hover_content(hover: &Hover, max_chars: usize) -> Hover {
    let contents = hover
        .contents
        .iter()
        .map(|c| match c {
            HoverContent::Text(t) => {
                HoverContent::Text(t.chars().take(max_chars).collect())
            }
            HoverContent::Markdown(md) => {
                HoverContent::Markdown(md.chars().take(max_chars).collect())
            }
            HoverContent::Code { value, language } => HoverContent::Code {
                value: value.chars().take(max_chars).collect(),
                language: language.clone(),
            },
        })
        .collect();
    Hover {
        contents,
        range: hover.range,
    }
}

/// Returns the total character length across all content blocks in the hover.
pub fn hover_content_length(hover: &Hover) -> usize {
    hover
        .contents
        .iter()
        .map(|c| match c {
            HoverContent::Text(t) => t.len(),
            HoverContent::Markdown(md) => md.len(),
            HoverContent::Code { value, .. } => value.len(),
        })
        .sum()
}

/// Conditionally filters hover results by language and position constraints.
#[derive(Debug, Clone)]
pub struct HoverFilter {
    /// If set, only hovers that contain code blocks with this language are accepted.
    pub language: Option<String>,
    /// If set, only hovers whose range contains this position are accepted.
    pub position: Option<(u32, u32)>,
}

impl HoverFilter {
    pub fn new() -> Self {
        Self {
            language: None,
            position: None,
        }
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    pub fn with_position(mut self, line: u32, col: u32) -> Self {
        self.position = Some((line, col));
        self
    }

    /// Returns `true` if the hover passes all configured filters.
    pub fn accepts(&self, hover: &Hover) -> bool {
        if let Some(ref lang) = self.language {
            let has_lang = hover.contents.iter().any(|c| match c {
                HoverContent::Code { language, .. } => {
                    language.as_deref() == Some(lang.as_str())
                }
                _ => false,
            });
            if !has_lang {
                return false;
            }
        }
        if let Some((line, col)) = self.position {
            if let Some(ref range) = hover.range {
                if !is_position_in_range(range, line, col) {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for HoverFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider;
    impl HoverProvider for TestProvider {
        fn provide_hover(&self, line: u32, _column: u32) -> Option<Hover> {
            if line == 1 {
                Some(Hover::markdown("**bold** text"))
            } else {
                None
            }
        }
    }

    #[test]
    fn hover_text() {
        let h = Hover::text("hello");
        assert!(!h.is_empty());
        assert!(matches!(&h.contents[0], HoverContent::Text(t) if t == "hello"));
    }

    #[test]
    fn hover_code() {
        let h = Hover::code("fn main() {}", Some("rust"));
        if let HoverContent::Code { value, language } = &h.contents[0] {
            assert_eq!(value, "fn main() {}");
            assert_eq!(language.as_deref(), Some("rust"));
        } else {
            panic!("Expected code");
        }
    }

    #[test]
    fn hover_registry() {
        let mut reg = HoverRegistry::new();
        reg.register(Box::new(TestProvider));

        assert!(reg.provide_hover(1, 1).is_some());
        assert!(reg.provide_hover(2, 1).is_none());
    }

    #[test]
    fn hover_with_range() {
        let h = Hover::text("info").with_range(HoverRange {
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 5,
        });
        assert!(h.range.is_some());
    }

    #[test]
    fn multi_content_hover() {
        let h = Hover::text("type: string")
            .add_content(HoverContent::Code {
                value: "let x: String".into(),
                language: Some("rust".into()),
            });
        assert_eq!(h.contents.len(), 2);
    }

    #[test]
    fn hover_config_defaults() {
        let cfg = HoverConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.delay_ms, 300);
        assert!(cfg.sticky);
        assert!(cfg.above_line_preference);
    }

    #[test]
    fn hover_session_show_and_hide() {
        let mut session = HoverSession::new();
        assert!(!session.visible);

        session.show(Hover::text("hello"), 5, 10);
        assert!(session.visible);
        assert_eq!(session.line, 5);
        assert_eq!(session.col, 10);
        assert!(session.current_hover.is_some());

        session.hide();
        assert!(!session.visible);
        assert!(session.current_hover.is_none());
    }

    #[test]
    fn hover_session_pin_prevents_hide() {
        let mut session = HoverSession::new();
        session.show(Hover::text("pinned"), 1, 1);
        session.toggle_pin();
        assert!(session.pinned);

        session.hide();
        assert!(session.visible);
        assert!(session.current_hover.is_some());
    }

    #[test]
    fn hover_session_unpin_hides() {
        let mut session = HoverSession::new();
        session.show(Hover::text("pinned"), 1, 1);
        session.toggle_pin();
        assert!(session.pinned);

        session.toggle_pin();
        assert!(!session.pinned);
        assert!(!session.visible);
        assert!(session.current_hover.is_none());
    }

    #[test]
    fn position_in_range_basic() {
        let range = HoverRange {
            start_line: 5,
            start_column: 3,
            end_line: 5,
            end_column: 10,
        };
        assert!(is_position_in_range(&range, 5, 3));
        assert!(is_position_in_range(&range, 5, 10));
        assert!(is_position_in_range(&range, 5, 7));
        assert!(!is_position_in_range(&range, 5, 2));
        assert!(!is_position_in_range(&range, 5, 11));
        assert!(!is_position_in_range(&range, 4, 5));
        assert!(!is_position_in_range(&range, 6, 5));
    }

    #[test]
    fn position_in_multiline_range() {
        let range = HoverRange {
            start_line: 2,
            start_column: 5,
            end_line: 4,
            end_column: 8,
        };
        assert!(!is_position_in_range(&range, 2, 4));
        assert!(is_position_in_range(&range, 2, 5));
        assert!(is_position_in_range(&range, 3, 0));
        assert!(is_position_in_range(&range, 3, 100));
        assert!(is_position_in_range(&range, 4, 8));
        assert!(!is_position_in_range(&range, 4, 9));
    }

    #[test]
    fn merge_hovers_combines_contents() {
        let h1 = Hover::text("first");
        let h2 = Hover::markdown("**second**");
        let merged = merge_hovers(&[h1, h2]);
        assert_eq!(merged.contents.len(), 2);
        assert!(merged.range.is_none());
    }

    #[test]
    fn merge_hovers_keeps_first_range() {
        let range = HoverRange {
            start_line: 1,
            start_column: 0,
            end_line: 1,
            end_column: 5,
        };
        let h1 = Hover::text("a");
        let h2 = Hover::text("b").with_range(range);
        let h3 = Hover::text("c").with_range(HoverRange {
            start_line: 9,
            start_column: 0,
            end_line: 9,
            end_column: 1,
        });
        let merged = merge_hovers(&[h1, h2, h3]);
        let r = merged.range.unwrap();
        assert_eq!(r.start_line, 1);
        assert_eq!(r.end_column, 5);
    }

    #[test]
    fn render_hover_plain_text() {
        let h = Hover::text("hello world");
        assert_eq!(render_hover_to_string(&h), "hello world");
    }

    #[test]
    fn render_hover_mixed() {
        let h = Hover::text("description")
            .add_content(HoverContent::Code {
                value: "fn foo()".into(),
                language: Some("rust".into()),
            });
        let rendered = render_hover_to_string(&h);
        assert!(rendered.contains("description"));
        assert!(rendered.contains("[rust] fn foo()"));
        assert!(rendered.contains("---"));
    }

    #[test]
    fn hover_from_contents() {
        let h = Hover::from_contents(vec![
            HoverContent::Text("one".into()),
            HoverContent::Markdown("**two**".into()),
        ]);
        assert_eq!(h.content_count(), 2);
        assert!(h.range.is_none());
    }

    #[test]
    fn hover_has_code_content() {
        let h1 = Hover::text("no code");
        assert!(!h1.has_code_content());

        let h2 = Hover::code("x = 1", Some("python"));
        assert!(h2.has_code_content());
    }

    #[test]
    fn hover_registry_provider_count() {
        let mut reg = HoverRegistry::new();
        assert_eq!(reg.provider_count(), 0);

        reg.register(Box::new(TestProvider));
        assert_eq!(reg.provider_count(), 1);
    }

    #[test]
    fn hover_trigger_kind_equality() {
        assert_eq!(HoverTriggerKind::Invoke, HoverTriggerKind::Invoke);
        assert_ne!(HoverTriggerKind::Hover, HoverTriggerKind::ContentHover);
    }

    #[test]
    fn hover_delay_should_show() {
        let cfg = HoverConfig::default();
        assert!(!HoverDelay::should_show(100, &cfg));
        assert!(HoverDelay::should_show(300, &cfg));
        assert!(HoverDelay::should_show(500, &cfg));

        let disabled = HoverConfig { enabled: false, ..HoverConfig::default() };
        assert!(!HoverDelay::should_show(1000, &disabled));
    }

    #[test]
    fn hover_delay_compute_delay() {
        let cfg = HoverConfig::default();
        assert_eq!(HoverDelay::compute_delay(HoverTriggerKind::Invoke, &cfg), 0);
        assert_eq!(HoverDelay::compute_delay(HoverTriggerKind::Hover, &cfg), 300);
        assert_eq!(HoverDelay::compute_delay(HoverTriggerKind::ContentHover, &cfg), 150);
    }

    #[test]
    fn hover_history_record_and_frequent() {
        let mut history = HoverHistory::new();
        assert!(history.is_empty());

        history.record(1, 5);
        history.record(1, 5);
        history.record(2, 3);
        history.record(1, 5);
        history.record(2, 3);
        assert_eq!(history.len(), 5);

        let top = history.get_frequent_positions(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], (1, 5, 3));
        assert_eq!(top[1], (2, 3, 2));
    }

    #[test]
    fn hover_history_clear() {
        let mut history = HoverHistory::new();
        history.record(0, 0);
        history.record(1, 1);
        assert_eq!(history.len(), 2);
        history.clear();
        assert_eq!(history.len(), 0);
        assert!(history.is_empty());
    }

    #[test]
    fn hover_content_builder_fluent() {
        let hover = HoverContentBuilder::new()
            .add_text("Type info")
            .add_markdown("**bold**")
            .add_code("let x = 1;", Some("rust"))
            .add_separator()
            .set_range(HoverRange {
                start_line: 1,
                start_column: 0,
                end_line: 1,
                end_column: 10,
            })
            .build();
        assert_eq!(hover.content_count(), 4);
        assert!(hover.range.is_some());
        assert!(hover.has_code_content());
    }

    #[test]
    fn truncate_hover_content_test() {
        let hover = Hover::text("hello world");
        let truncated = truncate_hover_content(&hover, 5);
        assert_eq!(render_hover_to_string(&truncated), "hello");
    }

    #[test]
    fn hover_content_length_test() {
        let hover = Hover::text("abc")
            .add_content(HoverContent::Markdown("de".into()))
            .add_content(HoverContent::Code {
                value: "fgh".into(),
                language: None,
            });
        assert_eq!(hover_content_length(&hover), 8);
    }

    #[test]
    fn hover_filter_by_language() {
        let rust_hover = Hover::code("fn main() {}", Some("rust"));
        let py_hover = Hover::code("def main():", Some("python"));
        let text_hover = Hover::text("plain");

        let filter = HoverFilter::new().with_language("rust");
        assert!(filter.accepts(&rust_hover));
        assert!(!filter.accepts(&py_hover));
        assert!(!filter.accepts(&text_hover));
    }

    #[test]
    fn hover_filter_by_position() {
        let range = HoverRange {
            start_line: 5,
            start_column: 0,
            end_line: 5,
            end_column: 10,
        };
        let hover = Hover::text("info").with_range(range);

        let inside = HoverFilter::new().with_position(5, 5);
        assert!(inside.accepts(&hover));

        let outside = HoverFilter::new().with_position(6, 0);
        assert!(!outside.accepts(&hover));
    }

    #[test]
    fn hover_filter_no_constraints_accepts_all() {
        let filter = HoverFilter::new();
        assert!(filter.accepts(&Hover::text("anything")));
        assert!(filter.accepts(&Hover::code("x", Some("go"))));
    }
}
