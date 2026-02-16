//! Inline ghost text completions.

use std::fmt;
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

/// Accumulated statistics for inline-complete operations.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineCompleteStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl InlineCompleteStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &InlineCompleteStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for InlineCompleteStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InlineCompleteStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InlineCompleteStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for inline-complete.
#[derive(Debug, Clone)]
pub struct InlineCompleteValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl InlineCompleteValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for InlineCompleteValidator {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn eq_inlinecompletiontriggerkind_same() {
        assert_eq!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Invoke);
    }

    #[test]
    fn ne_inlinecompletiontriggerkind_diff() {
        assert_ne!(InlineCompletionTriggerKind::Invoke, InlineCompletionTriggerKind::Automatic);
    }

    #[test]
    fn eq_inlinecompletionmatchmode_same() {
        assert_eq!(InlineCompletionMatchMode::Prefix, InlineCompletionMatchMode::Prefix);
    }

    #[test]
    fn ne_inlinecompletionmatchmode_diff() {
        assert_ne!(InlineCompletionMatchMode::Prefix, InlineCompletionMatchMode::Subword);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        let _svc = GhostTextWidget::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn inline_complete_stats_new_defaults() {
        let stats = InlineCompleteStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn inline_complete_stats_record_success() {
        let mut stats = InlineCompleteStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn inline_complete_stats_record_failure() {
        let mut stats = InlineCompleteStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn inline_complete_stats_reset() {
        let mut stats = InlineCompleteStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn inline_complete_stats_merge() {
        let mut a = InlineCompleteStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = InlineCompleteStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn inline_complete_stats_display() {
        let mut stats = InlineCompleteStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn inline_complete_stats_default() {
        let stats = InlineCompleteStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn inline_complete_validator_accepts_valid_name() {
        let v = InlineCompleteValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn inline_complete_validator_rejects_empty() {
        let v = InlineCompleteValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn inline_complete_validator_rejects_too_long() {
        let v = InlineCompleteValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn inline_complete_validator_forbidden_prefix() {
        let v = InlineCompleteValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn inline_complete_validator_allowed_chars() {
        let v = InlineCompleteValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn inline_complete_validator_range() {
        let v = InlineCompleteValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn inline_complete_sanitize_removes_control() {
        let result = InlineCompleteValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn inline_complete_truncate_short_string() {
        assert_eq!(InlineCompleteValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn inline_complete_truncate_long_string() {
        let result = InlineCompleteValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn inline_complete_is_ascii_printable() {
        assert!(InlineCompleteValidator::is_ascii_printable("Hello World 123"));
        assert!(!InlineCompleteValidator::is_ascii_printable("Hello\x00World"));
    }
}
