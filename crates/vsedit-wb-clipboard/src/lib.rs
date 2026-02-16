//! System clipboard integration.

/// The editing mode in which the copy was performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    Normal,
    Visual,
    VisualLine,
    VisualBlock,
}

/// A single entry in the clipboard history.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardEntry {
    pub text: String,
    pub timestamp: u64,
    pub source_mode: SourceMode,
}

/// Service for clipboard text storage with history (circular buffer semantics).
pub struct ClipboardService {
    history: Vec<ClipboardEntry>,
    max_history: usize,
}

impl ClipboardService {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Vec::new(),
            max_history,
        }
    }

    pub fn write_text(&mut self, text: String, timestamp: u64) {
        self.write_entry(text, timestamp, SourceMode::Normal);
    }

    /// Write a clipboard entry with source mode metadata.
    pub fn write_entry(&mut self, text: String, timestamp: u64, source_mode: SourceMode) {
        self.history.push(ClipboardEntry {
            text,
            timestamp,
            source_mode,
        });
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Returns the most recently written text.
    pub fn read_text(&self) -> Option<&str> {
        self.history.last().map(|e| e.text.as_str())
    }

    /// Returns the most recent clipboard entry with metadata.
    pub fn read_entry(&self) -> Option<&ClipboardEntry> {
        self.history.last()
    }

    pub fn get_history(&self) -> &[ClipboardEntry] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn history_count(&self) -> usize {
        self.history.len()
    }
}

/// Handle pasting to multiple cursors by splitting clipboard text on newlines.
pub struct MultiCursorClipboard;

impl MultiCursorClipboard {
    /// Split clipboard text for distribution across multiple cursor positions.
    /// If the clipboard has exactly `cursor_count` lines, each cursor gets one line.
    /// Otherwise, every cursor gets the full text.
    pub fn distribute(text: &str, cursor_count: usize) -> Vec<String> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() == cursor_count {
            lines.iter().map(|l| l.to_string()).collect()
        } else {
            vec![text.to_string(); cursor_count]
        }
    }

    /// Collect text from multiple cursor selections into a single clipboard entry.
    pub fn collect(selections: &[&str]) -> String {
        selections.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Search & filter helpers
// ---------------------------------------------------------------------------

impl ClipboardService {
    /// Case-insensitive search across clipboard history text.
    pub fn search_history(&self, query: &str) -> Vec<&ClipboardEntry> {
        let lower = query.to_lowercase();
        self.history
            .iter()
            .filter(|e| e.text.to_lowercase().contains(&lower))
            .collect()
    }

    /// Returns entries that were copied in the given `mode`.
    pub fn get_history_by_mode(&self, mode: SourceMode) -> Vec<&ClipboardEntry> {
        self.history
            .iter()
            .filter(|e| e.source_mode == mode)
            .collect()
    }

    /// Removes consecutive duplicate entries (same text).
    pub fn deduplicate(&mut self) {
        self.history.dedup_by(|a, b| a.text == b.text);
    }

    /// Total bytes of text stored across all history entries.
    pub fn total_text_size(&self) -> usize {
        self.history.iter().map(|e| e.text.len()).sum()
    }

    /// Removes and returns the most recently written entry.
    pub fn undo_last_write(&mut self) -> Option<ClipboardEntry> {
        self.history.pop()
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the clipboard subsystem.
#[derive(Debug, Clone)]
pub struct ClipboardConfig {
    pub max_history: usize,
    pub enable_paste_history: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            max_history: 20,
            enable_paste_history: true,
        }
    }
}

impl ClipboardConfig {
    pub fn new(max_history: usize, enable_paste_history: bool) -> Self {
        Self {
            max_history,
            enable_paste_history,
        }
    }
}

// ---------------------------------------------------------------------------
// MultiCursorClipboard extensions
// ---------------------------------------------------------------------------

impl MultiCursorClipboard {
    /// Join multiple selections with an arbitrary separator.
    pub fn merge_lines(selections: &[&str], separator: &str) -> String {
        selections.join(separator)
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl std::fmt::Display for SourceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceMode::Normal => write!(f, "Normal"),
            SourceMode::Visual => write!(f, "Visual"),
            SourceMode::VisualLine => write!(f, "VisualLine"),
            SourceMode::VisualBlock => write!(f, "VisualBlock"),
        }
    }
}

impl std::fmt::Display for ClipboardEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}@{}] {}", self.source_mode, self.timestamp, self.text)
    }
}

// ---------------------------------------------------------------------------
// ClipboardTransform
// ---------------------------------------------------------------------------

/// Pure-function text transformations useful for clipboard contents.
pub struct ClipboardTransform;

impl ClipboardTransform {
    /// Trim leading and trailing whitespace from every line.
    pub fn trim_whitespace(text: &str) -> String {
        text.lines()
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Remove blank lines from the text.
    pub fn remove_empty_lines(text: &str) -> String {
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Sort lines alphabetically.
    pub fn sort_lines(text: &str) -> String {
        let mut lines: Vec<&str> = text.lines().collect();
        lines.sort();
        lines.join("\n")
    }

    /// Reverse the order of lines.
    pub fn reverse_lines(text: &str) -> String {
        let mut lines: Vec<&str> = text.lines().collect();
        lines.reverse();
        lines.join("\n")
    }

    /// Deduplicate lines, preserving the first occurrence order.
    pub fn unique_lines(text: &str) -> String {
        let mut seen = std::collections::HashSet::new();
        text.lines()
            .filter(|l| seen.insert(*l))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Count the number of lines.
    pub fn line_count(text: &str) -> usize {
        text.lines().count()
    }

    /// Count the number of characters.
    pub fn char_count(text: &str) -> usize {
        text.chars().count()
    }

    /// Count words (split on whitespace).
    pub fn word_count(text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// Prepend `prefix` to every line.
    pub fn indent_lines(text: &str, prefix: &str) -> String {
        text.lines()
            .map(|l| format!("{prefix}{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Prepend line numbers ("1: ", "2: ", …) to every line.
    pub fn number_lines(text: &str) -> String {
        text.lines()
            .enumerate()
            .map(|(i, l)| format!("{}: {l}", i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---------------------------------------------------------------------------
// ClipboardStats
// ---------------------------------------------------------------------------

/// Aggregate statistics derived from a `ClipboardService`.
pub struct ClipboardStats {
    pub entry_count: usize,
    pub total_bytes: usize,
    pub avg_bytes: usize,
    pub longest_entry_bytes: usize,
    pub shortest_entry_bytes: usize,
}

impl ClipboardStats {
    pub fn from_service(svc: &ClipboardService) -> Self {
        let entries = svc.get_history();
        let entry_count = entries.len();
        let total_bytes: usize = entries.iter().map(|e| e.text.len()).sum();
        let avg_bytes = if entry_count > 0 { total_bytes / entry_count } else { 0 };
        let longest_entry_bytes = entries.iter().map(|e| e.text.len()).max().unwrap_or(0);
        let shortest_entry_bytes = entries.iter().map(|e| e.text.len()).min().unwrap_or(0);
        Self {
            entry_count,
            total_bytes,
            avg_bytes,
            longest_entry_bytes,
            shortest_entry_bytes,
        }
    }
}

// ---------------------------------------------------------------------------
// Additional ClipboardService helpers
// ---------------------------------------------------------------------------

impl ClipboardService {
    /// Get the entry at a specific index in the history.
    pub fn get_entry_at(&self, index: usize) -> Option<&ClipboardEntry> {
        self.history.get(index)
    }

    /// Return entries whose timestamp is >= `timestamp`.
    pub fn entries_since(&self, timestamp: u64) -> Vec<&ClipboardEntry> {
        self.history
            .iter()
            .filter(|e| e.timestamp >= timestamp)
            .collect()
    }

    /// Return the oldest entry (first in history).
    pub fn oldest_entry(&self) -> Option<&ClipboardEntry> {
        self.history.first()
    }

    /// Return the newest entry (last in history).
    pub fn newest_entry(&self) -> Option<&ClipboardEntry> {
        self.history.last()
    }

    /// Check whether any entry contains the exact text.
    pub fn contains_text(&self, text: &str) -> bool {
        self.history.iter().any(|e| e.text == text)
    }

    /// Remove all entries whose text matches, returning the count removed.
    pub fn remove_by_text(&mut self, text: &str) -> usize {
        let before = self.history.len();
        self.history.retain(|e| e.text != text);
        before - self.history.len()
    }
}

// ---------------------------------------------------------------------------
// Copy/paste action descriptors
// ---------------------------------------------------------------------------

/// Describes a clipboard action triggered by a keybinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardAction {
    /// Ctrl+C — copy selection (or current line if no selection).
    Copy,
    /// Ctrl+X — cut selection (or current line if no selection).
    Cut,
    /// Ctrl+V — paste.
    Paste,
    /// Ctrl+Shift+V — paste plain text (strip formatting).
    PastePlain,
}

impl std::fmt::Display for ClipboardAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardAction::Copy => write!(f, "Copy"),
            ClipboardAction::Cut => write!(f, "Cut"),
            ClipboardAction::Paste => write!(f, "Paste"),
            ClipboardAction::PastePlain => write!(f, "Paste Plain"),
        }
    }
}

impl ClipboardAction {
    /// Returns the default keybinding string for the action.
    pub fn default_keybinding(&self) -> &'static str {
        match self {
            ClipboardAction::Copy => "Ctrl+C",
            ClipboardAction::Cut => "Ctrl+X",
            ClipboardAction::Paste => "Ctrl+V",
            ClipboardAction::PastePlain => "Ctrl+Shift+V",
        }
    }

    /// Returns the command ID used in the command palette.
    pub fn command_id(&self) -> &'static str {
        match self {
            ClipboardAction::Copy => "editor.action.clipboardCopyAction",
            ClipboardAction::Cut => "editor.action.clipboardCutAction",
            ClipboardAction::Paste => "editor.action.clipboardPasteAction",
            ClipboardAction::PastePlain => "editor.action.clipboardPastePlainAction",
        }
    }
}

/// Handles a copy action: if selections are provided, collect them;
/// otherwise copy the full line text.
pub fn handle_copy(
    svc: &mut ClipboardService,
    selections: &[&str],
    full_line: &str,
    timestamp: u64,
) -> String {
    let text = if selections.is_empty() || selections.iter().all(|s| s.is_empty()) {
        full_line.to_string()
    } else {
        MultiCursorClipboard::collect(selections)
    };
    svc.write_text(text.clone(), timestamp);
    text
}

/// Handles a cut action: same as copy, but returns the text that should be
/// deleted from the editor.
pub fn handle_cut(
    svc: &mut ClipboardService,
    selections: &[&str],
    full_line: &str,
    timestamp: u64,
) -> String {
    handle_copy(svc, selections, full_line, timestamp)
}

/// Handles a paste action: returns the text to insert, distributed across
/// the given number of cursors.
pub fn handle_paste(svc: &ClipboardService, cursor_count: usize) -> Vec<String> {
    match svc.read_text() {
        Some(text) => MultiCursorClipboard::distribute(text, cursor_count),
        None => vec![String::new(); cursor_count],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read() {
        let mut svc = ClipboardService::new(10);
        assert!(svc.read_text().is_none());
        svc.write_text("hello".to_string(), 1);
        assert_eq!(svc.read_text(), Some("hello"));
        svc.write_text("world".to_string(), 2);
        assert_eq!(svc.read_text(), Some("world"));
    }

    #[test]
    fn history_limit() {
        let mut svc = ClipboardService::new(2);
        svc.write_text("a".to_string(), 1);
        svc.write_text("b".to_string(), 2);
        svc.write_text("c".to_string(), 3);
        assert_eq!(svc.history_count(), 2);
        assert_eq!(svc.get_history()[0].text, "b");
        assert_eq!(svc.get_history()[1].text, "c");
    }

    #[test]
    fn clear_history() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("data".to_string(), 1);
        svc.clear_history();
        assert_eq!(svc.history_count(), 0);
        assert!(svc.read_text().is_none());
    }

    #[test]
    fn write_entry_preserves_source_mode() {
        let mut svc = ClipboardService::new(10);
        svc.write_entry("block".into(), 1, SourceMode::VisualBlock);
        let entry = svc.read_entry().unwrap();
        assert_eq!(entry.source_mode, SourceMode::VisualBlock);
        assert_eq!(entry.text, "block");
    }

    #[test]
    fn multi_cursor_distribute_matching_lines() {
        let text = "alpha\nbeta\ngamma";
        let result = MultiCursorClipboard::distribute(text, 3);
        assert_eq!(result, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn multi_cursor_distribute_mismatched_lines() {
        let text = "full text";
        let result = MultiCursorClipboard::distribute(text, 3);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|s| s == "full text"));
    }

    #[test]
    fn multi_cursor_collect() {
        let selections = vec!["aaa", "bbb", "ccc"];
        let result = MultiCursorClipboard::collect(&selections);
        assert_eq!(result, "aaa\nbbb\nccc");
    }

    #[test]
    fn write_text_defaults_to_normal_mode() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("hello".into(), 1);
        assert_eq!(svc.read_entry().unwrap().source_mode, SourceMode::Normal);
    }

    #[test]
    fn circular_buffer_preserves_newest() {
        let mut svc = ClipboardService::new(3);
        for i in 0..10u64 {
            svc.write_text(format!("item{i}"), i);
        }
        assert_eq!(svc.history_count(), 3);
        assert_eq!(svc.get_history()[0].text, "item7");
        assert_eq!(svc.get_history()[2].text, "item9");
    }

    #[test]
    fn search_history_case_insensitive() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("Hello World".into(), 1);
        svc.write_text("goodbye".into(), 2);
        svc.write_text("HELLO again".into(), 3);
        let results = svc.search_history("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_history_no_match() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("abc".into(), 1);
        assert!(svc.search_history("xyz").is_empty());
    }

    #[test]
    fn get_history_by_mode() {
        let mut svc = ClipboardService::new(10);
        svc.write_entry("a".into(), 1, SourceMode::Visual);
        svc.write_entry("b".into(), 2, SourceMode::Normal);
        svc.write_entry("c".into(), 3, SourceMode::Visual);
        assert_eq!(svc.get_history_by_mode(SourceMode::Visual).len(), 2);
        assert_eq!(svc.get_history_by_mode(SourceMode::VisualLine).len(), 0);
    }

    #[test]
    fn deduplicate_consecutive() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("dup".into(), 1);
        svc.write_text("dup".into(), 2);
        svc.write_text("other".into(), 3);
        svc.write_text("other".into(), 4);
        svc.deduplicate();
        assert_eq!(svc.history_count(), 2);
        assert_eq!(svc.get_history()[0].text, "dup");
        assert_eq!(svc.get_history()[1].text, "other");
    }

    #[test]
    fn total_text_size() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("abc".into(), 1);
        svc.write_text("de".into(), 2);
        assert_eq!(svc.total_text_size(), 5);
    }

    #[test]
    fn clipboard_config_defaults() {
        let cfg = ClipboardConfig::default();
        assert_eq!(cfg.max_history, 20);
        assert!(cfg.enable_paste_history);
    }

    #[test]
    fn clipboard_config_custom() {
        let cfg = ClipboardConfig::new(50, false);
        assert_eq!(cfg.max_history, 50);
        assert!(!cfg.enable_paste_history);
    }

    #[test]
    fn merge_lines_with_separator() {
        let result = MultiCursorClipboard::merge_lines(&["a", "b", "c"], ", ");
        assert_eq!(result, "a, b, c");
    }

    #[test]
    fn undo_last_write() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("first".into(), 1);
        svc.write_text("second".into(), 2);
        let undone = svc.undo_last_write();
        assert_eq!(undone.unwrap().text, "second");
        assert_eq!(svc.read_text(), Some("first"));
    }

    #[test]
    fn undo_last_write_empty() {
        let mut svc = ClipboardService::new(10);
        assert!(svc.undo_last_write().is_none());
    }

    #[test]
    fn test_trim_whitespace() {
        let input = "  hello  \n  world  \n  foo  ";
        assert_eq!(ClipboardTransform::trim_whitespace(input), "hello\nworld\nfoo");
    }

    #[test]
    fn test_remove_empty_lines() {
        let input = "a\n\nb\n   \nc";
        assert_eq!(ClipboardTransform::remove_empty_lines(input), "a\nb\nc");
    }

    #[test]
    fn test_sort_lines() {
        let input = "cherry\napple\nbanana";
        assert_eq!(ClipboardTransform::sort_lines(input), "apple\nbanana\ncherry");
    }

    #[test]
    fn test_reverse_lines() {
        let input = "one\ntwo\nthree";
        assert_eq!(ClipboardTransform::reverse_lines(input), "three\ntwo\none");
    }

    #[test]
    fn test_unique_lines() {
        let input = "a\nb\na\nc\nb";
        assert_eq!(ClipboardTransform::unique_lines(input), "a\nb\nc");
    }

    #[test]
    fn test_line_count() {
        assert_eq!(ClipboardTransform::line_count("a\nb\nc"), 3);
        assert_eq!(ClipboardTransform::line_count(""), 0);
    }

    #[test]
    fn test_word_count() {
        assert_eq!(ClipboardTransform::word_count("hello world foo"), 3);
        assert_eq!(ClipboardTransform::word_count("  "), 0);
    }

    #[test]
    fn test_indent_lines() {
        let input = "a\nb";
        assert_eq!(ClipboardTransform::indent_lines(input, ">> "), ">> a\n>> b");
    }

    #[test]
    fn test_number_lines() {
        let input = "foo\nbar\nbaz";
        assert_eq!(ClipboardTransform::number_lines(input), "1: foo\n2: bar\n3: baz");
    }

    #[test]
    fn test_clipboard_stats() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("ab".into(), 1);
        svc.write_text("cdef".into(), 2);
        svc.write_text("g".into(), 3);
        let stats = ClipboardStats::from_service(&svc);
        assert_eq!(stats.entry_count, 3);
        assert_eq!(stats.total_bytes, 7);
        assert_eq!(stats.avg_bytes, 2); // 7 / 3 = 2 (integer)
        assert_eq!(stats.longest_entry_bytes, 4);
        assert_eq!(stats.shortest_entry_bytes, 1);
    }

    #[test]
    fn test_source_mode_display() {
        assert_eq!(format!("{}", SourceMode::Normal), "Normal");
        assert_eq!(format!("{}", SourceMode::Visual), "Visual");
        assert_eq!(format!("{}", SourceMode::VisualLine), "VisualLine");
        assert_eq!(format!("{}", SourceMode::VisualBlock), "VisualBlock");
    }

    #[test]
    fn test_clipboard_entry_display() {
        let entry = ClipboardEntry {
            text: "hello".into(),
            timestamp: 42,
            source_mode: SourceMode::Normal,
        };
        assert_eq!(format!("{entry}"), "[Normal@42] hello");
    }

    #[test]
    fn test_get_entry_at() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("a".into(), 1);
        svc.write_text("b".into(), 2);
        assert_eq!(svc.get_entry_at(0).unwrap().text, "a");
        assert_eq!(svc.get_entry_at(1).unwrap().text, "b");
        assert!(svc.get_entry_at(5).is_none());
    }

    #[test]
    fn test_entries_since() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("old".into(), 10);
        svc.write_text("mid".into(), 20);
        svc.write_text("new".into(), 30);
        let recent = svc.entries_since(20);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "mid");
        assert_eq!(recent[1].text, "new");
    }

    #[test]
    fn test_oldest_newest() {
        let mut svc = ClipboardService::new(10);
        assert!(svc.oldest_entry().is_none());
        assert!(svc.newest_entry().is_none());
        svc.write_text("first".into(), 1);
        svc.write_text("second".into(), 2);
        assert_eq!(svc.oldest_entry().unwrap().text, "first");
        assert_eq!(svc.newest_entry().unwrap().text, "second");
    }

    #[test]
    fn test_contains_text() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("hello".into(), 1);
        assert!(svc.contains_text("hello"));
        assert!(!svc.contains_text("world"));
    }

    #[test]
    fn test_remove_by_text() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("keep".into(), 1);
        svc.write_text("remove".into(), 2);
        svc.write_text("remove".into(), 3);
        svc.write_text("keep".into(), 4);
        let removed = svc.remove_by_text("remove");
        assert_eq!(removed, 2);
        assert_eq!(svc.history_count(), 2);
        assert!(!svc.contains_text("remove"));
    }

    // --- ClipboardAction tests ---

    #[test]
    fn clipboard_action_display() {
        assert_eq!(ClipboardAction::Copy.to_string(), "Copy");
        assert_eq!(ClipboardAction::Cut.to_string(), "Cut");
        assert_eq!(ClipboardAction::Paste.to_string(), "Paste");
        assert_eq!(ClipboardAction::PastePlain.to_string(), "Paste Plain");
    }

    #[test]
    fn clipboard_action_keybinding() {
        assert_eq!(ClipboardAction::Copy.default_keybinding(), "Ctrl+C");
        assert_eq!(ClipboardAction::Cut.default_keybinding(), "Ctrl+X");
        assert_eq!(ClipboardAction::Paste.default_keybinding(), "Ctrl+V");
        assert_eq!(ClipboardAction::PastePlain.default_keybinding(), "Ctrl+Shift+V");
    }

    #[test]
    fn clipboard_action_command_id() {
        assert!(ClipboardAction::Copy.command_id().contains("Copy"));
        assert!(ClipboardAction::Cut.command_id().contains("Cut"));
        assert!(ClipboardAction::Paste.command_id().contains("Paste"));
    }

    #[test]
    fn handle_copy_with_selections() {
        let mut svc = ClipboardService::new(10);
        let result = handle_copy(&mut svc, &["foo", "bar"], "whole line\n", 1);
        assert_eq!(result, "foo\nbar");
        assert_eq!(svc.read_text(), Some("foo\nbar"));
    }

    #[test]
    fn handle_copy_empty_selection_copies_line() {
        let mut svc = ClipboardService::new(10);
        let result = handle_copy(&mut svc, &[], "whole line\n", 1);
        assert_eq!(result, "whole line\n");
    }

    #[test]
    fn handle_copy_blank_selections_copies_line() {
        let mut svc = ClipboardService::new(10);
        let result = handle_copy(&mut svc, &["", ""], "line text\n", 1);
        assert_eq!(result, "line text\n");
    }

    #[test]
    fn handle_cut_returns_same_as_copy() {
        let mut svc = ClipboardService::new(10);
        let result = handle_cut(&mut svc, &["sel"], "line", 1);
        assert_eq!(result, "sel");
        assert_eq!(svc.read_text(), Some("sel"));
    }

    #[test]
    fn handle_paste_distributes_to_cursors() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("a\nb\nc".into(), 1);
        let result = handle_paste(&svc, 3);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn handle_paste_full_text_when_mismatch() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("hello".into(), 1);
        let result = handle_paste(&svc, 3);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|s| s == "hello"));
    }

    #[test]
    fn handle_paste_empty_clipboard() {
        let svc = ClipboardService::new(10);
        let result = handle_paste(&svc, 2);
        assert_eq!(result, vec!["", ""]);
    }
}
