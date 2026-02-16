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
#[derive(Debug, Clone)]
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
}
