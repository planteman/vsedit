//! System clipboard via OSC 52.
//!
//! Equivalent to VS Code's clipboard service.
//! Uses OSC 52 escape sequences for terminal clipboard access.

use std::fmt;
use std::io::Write;
use std::sync::Mutex;

/// Clipboard service trait.
pub trait IClipboardService: Send + Sync {
    /// Read text from clipboard.
    fn read_text(&self) -> Option<String>;
    /// Write text to clipboard.
    fn write_text(&self, text: &str);
    /// Check if the clipboard contains text.
    fn has_text(&self) -> bool {
        self.read_text().is_some()
    }
}

/// Terminal clipboard using OSC 52 escape sequences.
/// Falls back to an internal buffer when OSC 52 is not supported.
pub struct TerminalClipboard {
    buffer: Mutex<String>,
}

impl TerminalClipboard {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(String::new()),
        }
    }

    /// Write text to clipboard using OSC 52 escape sequence.
    fn write_osc52(&self, text: &str) {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let seq = format!("\x1b]52;c;{encoded}\x07");
        let _ = std::io::stdout().write_all(seq.as_bytes());
        let _ = std::io::stdout().flush();
    }
}

impl Default for TerminalClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl IClipboardService for TerminalClipboard {
    fn read_text(&self) -> Option<String> {
        let buf = self.buffer.lock().unwrap();
        if buf.is_empty() {
            None
        } else {
            Some(buf.clone())
        }
    }

    fn write_text(&self, text: &str) {
        {
            let mut buf = self.buffer.lock().unwrap();
            *buf = text.to_string();
        }
        self.write_osc52(text);
    }
}

/// In-memory clipboard for testing.
pub struct InMemoryClipboard {
    buffer: Mutex<String>,
}

impl InMemoryClipboard {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(String::new()),
        }
    }
}

impl Default for InMemoryClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl IClipboardService for InMemoryClipboard {
    fn read_text(&self) -> Option<String> {
        let buf = self.buffer.lock().unwrap();
        if buf.is_empty() {
            None
        } else {
            Some(buf.clone())
        }
    }

    fn write_text(&self, text: &str) {
        let mut buf = self.buffer.lock().unwrap();
        *buf = text.to_string();
    }
}

/// Errors that can occur during clipboard operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    /// The internal mutex was poisoned.
    LockPoisoned,
    /// Writing to the clipboard failed.
    WriteFailed,
    /// The requested format is not supported.
    UnsupportedFormat,
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardError::LockPoisoned => write!(f, "clipboard lock poisoned"),
            ClipboardError::WriteFailed => write!(f, "clipboard write failed"),
            ClipboardError::UnsupportedFormat => write!(f, "unsupported clipboard format"),
        }
    }
}

/// A single clipboard entry with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardItem {
    /// The text content of the clipboard entry.
    pub text: String,
    /// Unix timestamp (seconds) when the item was copied.
    pub timestamp: u64,
    /// Optional source identifier (e.g. file path or application name).
    pub source: Option<String>,
}

impl ClipboardItem {
    /// Create a new clipboard item.
    pub fn new(text: impl Into<String>, timestamp: u64, source: Option<String>) -> Self {
        Self {
            text: text.into(),
            timestamp,
            source,
        }
    }
}

impl fmt::Display for ClipboardItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.text.len() > 40 {
            write!(f, "{}...", &self.text[..40])
        } else {
            write!(f, "{}", self.text)
        }
    }
}

/// Tracks a history of clipboard entries up to a configurable maximum.
#[derive(Debug)]
pub struct ClipboardHistory {
    entries: Vec<ClipboardItem>,
    max_entries: usize,
}

impl ClipboardHistory {
    /// Create a new history with the given maximum number of entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Push a new item onto the history. If the history is full, the oldest
    /// entry is removed.
    pub fn push(&mut self, item: ClipboardItem) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(item);
    }

    /// Get the item at the given index (0 is the oldest).
    pub fn get_at(&self, index: usize) -> Option<&ClipboardItem> {
        self.entries.get(index)
    }

    /// Clear all history entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if the history contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl std::error::Error for ClipboardError {}

/// Supported clipboard data formats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardFormat {
    /// Plain UTF-8 text.
    PlainText,
    /// HTML markup.
    Html,
    /// Rich text (RTF).
    RichText,
    /// Newline-separated file paths.
    FilePaths,
}

impl fmt::Display for ClipboardFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardFormat::PlainText => write!(f, "text/plain"),
            ClipboardFormat::Html => write!(f, "text/html"),
            ClipboardFormat::RichText => write!(f, "text/rtf"),
            ClipboardFormat::FilePaths => write!(f, "text/uri-list"),
        }
    }
}

/// A clipboard item that carries format information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiFormatClipboardItem {
    /// The format of the data.
    pub format: ClipboardFormat,
    /// The raw data as a string.
    pub data: String,
    /// Unix timestamp (seconds) when the item was created.
    pub timestamp: u64,
    /// Optional source identifier.
    pub source: Option<String>,
}

impl MultiFormatClipboardItem {
    /// Create a new multi-format clipboard item.
    pub fn new(format: ClipboardFormat, data: impl Into<String>, timestamp: u64) -> Self {
        Self {
            format,
            data: data.into(),
            timestamp,
            source: None,
        }
    }

    /// Set the source identifier, consuming and returning self.
    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    /// Return the byte length of the data.
    pub fn data_length(&self) -> usize {
        self.data.len()
    }

    /// Return `true` if the format is `PlainText`.
    pub fn is_plain_text(&self) -> bool {
        self.format == ClipboardFormat::PlainText
    }
}

impl ClipboardHistory {
    /// Return the most recently added item.
    pub fn most_recent(&self) -> Option<&ClipboardItem> {
        self.entries.last()
    }

    /// Search for items whose text contains the given query (case-sensitive).
    pub fn search(&self, query: &str) -> Vec<&ClipboardItem> {
        self.entries
            .iter()
            .filter(|item| item.text.contains(query))
            .collect()
    }

    /// Remove the item at the given index, returning it if the index was valid.
    pub fn remove_at(&mut self, index: usize) -> Option<ClipboardItem> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    /// Return a slice of all entries.
    pub fn entries(&self) -> &[ClipboardItem] {
        &self.entries
    }

    /// Return the maximum number of entries this history will hold.
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Return `true` if any entry has exactly the given text.
    pub fn contains_text(&self, text: &str) -> bool {
        self.entries.iter().any(|item| item.text == text)
    }

    /// Remove duplicate text entries, keeping only the latest occurrence.
    pub fn deduplicate(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for item in self.entries.iter().rev() {
            if seen.insert(item.text.clone()) {
                deduped.push(item.clone());
            }
        }
        deduped.reverse();
        self.entries = deduped;
    }
}

/// Watches for clipboard content changes.
#[derive(Debug, Clone)]
pub struct ClipboardWatcher {
    last_content: Option<String>,
    change_count: u64,
}

impl ClipboardWatcher {
    /// Create a new watcher with no prior content.
    pub fn new() -> Self {
        Self {
            last_content: None,
            change_count: 0,
        }
    }

    /// Check whether the current content differs from the last observed content.
    /// Returns `true` if the content changed, and updates internal state.
    pub fn check_change(&mut self, current: &str) -> bool {
        let changed = match &self.last_content {
            Some(prev) => prev != current,
            None => true,
        };
        if changed {
            self.last_content = Some(current.to_string());
            self.change_count += 1;
        }
        changed
    }

    /// Return the number of changes observed so far.
    pub fn change_count(&self) -> u64 {
        self.change_count
    }

    /// Return the last observed content, if any.
    pub fn last_content(&self) -> Option<&str> {
        self.last_content.as_deref()
    }

    /// Reset the watcher to its initial state.
    pub fn reset(&mut self) {
        self.last_content = None;
        self.change_count = 0;
    }
}

impl Default for ClipboardWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_clipboard() {
        let clip = InMemoryClipboard::new();
        assert_eq!(clip.read_text(), None);
        clip.write_text("hello");
        assert_eq!(clip.read_text(), Some("hello".to_string()));
    }

    #[test]
    fn clipboard_overwrite() {
        let clip = InMemoryClipboard::new();
        clip.write_text("first");
        clip.write_text("second");
        assert_eq!(clip.read_text(), Some("second".to_string()));
    }

    #[test]
    fn terminal_clipboard_buffer() {
        let clip = TerminalClipboard::new();
        assert_eq!(clip.read_text(), None);
        // Don't actually write to terminal in tests, just store in buffer
        {
            let mut buf = clip.buffer.lock().unwrap();
            *buf = "test".to_string();
        }
        assert_eq!(clip.read_text(), Some("test".to_string()));
    }

    #[test]
    fn has_text_empty_clipboard() {
        let clip = InMemoryClipboard::new();
        assert!(!clip.has_text());
    }

    #[test]
    fn has_text_after_write() {
        let clip = InMemoryClipboard::new();
        clip.write_text("data");
        assert!(clip.has_text());
    }

    #[test]
    fn terminal_clipboard_has_text() {
        let clip = TerminalClipboard::new();
        assert!(!clip.has_text());
        {
            let mut buf = clip.buffer.lock().unwrap();
            *buf = "value".to_string();
        }
        assert!(clip.has_text());
    }

    #[test]
    fn clipboard_error_display() {
        assert_eq!(ClipboardError::LockPoisoned.to_string(), "clipboard lock poisoned");
        assert_eq!(ClipboardError::WriteFailed.to_string(), "clipboard write failed");
        assert_eq!(
            ClipboardError::UnsupportedFormat.to_string(),
            "unsupported clipboard format"
        );
    }

    #[test]
    fn clipboard_item_display_short() {
        let item = ClipboardItem::new("short text", 100, None);
        assert_eq!(item.to_string(), "short text");
    }

    #[test]
    fn clipboard_item_display_truncated() {
        let long = "a".repeat(50);
        let item = ClipboardItem::new(long, 200, Some("editor".into()));
        let display = item.to_string();
        assert!(display.ends_with("..."));
        assert_eq!(display.len(), 43); // 40 chars + "..."
    }

    #[test]
    fn clipboard_history_push_and_get() {
        let mut history = ClipboardHistory::new(3);
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);

        history.push(ClipboardItem::new("first", 1, None));
        history.push(ClipboardItem::new("second", 2, None));
        assert_eq!(history.len(), 2);
        assert_eq!(history.get_at(0).unwrap().text, "first");
        assert_eq!(history.get_at(1).unwrap().text, "second");
        assert!(history.get_at(2).is_none());
    }

    #[test]
    fn clipboard_history_evicts_oldest() {
        let mut history = ClipboardHistory::new(2);
        history.push(ClipboardItem::new("a", 1, None));
        history.push(ClipboardItem::new("b", 2, None));
        history.push(ClipboardItem::new("c", 3, None));
        assert_eq!(history.len(), 2);
        assert_eq!(history.get_at(0).unwrap().text, "b");
        assert_eq!(history.get_at(1).unwrap().text, "c");
    }

    #[test]
    fn clipboard_history_clear() {
        let mut history = ClipboardHistory::new(5);
        history.push(ClipboardItem::new("x", 1, None));
        history.push(ClipboardItem::new("y", 2, None));
        assert!(!history.is_empty());
        history.clear();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_clipboard_format_display() {
        assert_eq!(ClipboardFormat::PlainText.to_string(), "text/plain");
        assert_eq!(ClipboardFormat::Html.to_string(), "text/html");
        assert_eq!(ClipboardFormat::RichText.to_string(), "text/rtf");
        assert_eq!(ClipboardFormat::FilePaths.to_string(), "text/uri-list");
    }

    #[test]
    fn test_multi_format_item_new() {
        let item = MultiFormatClipboardItem::new(ClipboardFormat::Html, "<b>hi</b>", 42);
        assert_eq!(item.format, ClipboardFormat::Html);
        assert_eq!(item.data, "<b>hi</b>");
        assert_eq!(item.timestamp, 42);
        assert!(item.source.is_none());
    }

    #[test]
    fn test_multi_format_item_with_source() {
        let item = MultiFormatClipboardItem::new(ClipboardFormat::PlainText, "hello", 1)
            .with_source("editor".into());
        assert_eq!(item.source, Some("editor".to_string()));
    }

    #[test]
    fn test_multi_format_item_is_plain_text() {
        let plain = MultiFormatClipboardItem::new(ClipboardFormat::PlainText, "abc", 1);
        let html = MultiFormatClipboardItem::new(ClipboardFormat::Html, "abc", 1);
        assert!(plain.is_plain_text());
        assert!(!html.is_plain_text());
    }

    #[test]
    fn test_history_most_recent() {
        let mut history = ClipboardHistory::new(5);
        assert!(history.most_recent().is_none());
        history.push(ClipboardItem::new("a", 1, None));
        history.push(ClipboardItem::new("b", 2, None));
        assert_eq!(history.most_recent().unwrap().text, "b");
    }

    #[test]
    fn test_history_search() {
        let mut history = ClipboardHistory::new(10);
        history.push(ClipboardItem::new("hello world", 1, None));
        history.push(ClipboardItem::new("goodbye world", 2, None));
        history.push(ClipboardItem::new("hello rust", 3, None));
        let results = history.search("hello");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "hello world");
        assert_eq!(results[1].text, "hello rust");
        assert!(history.search("missing").is_empty());
    }

    #[test]
    fn test_history_remove_at() {
        let mut history = ClipboardHistory::new(5);
        history.push(ClipboardItem::new("a", 1, None));
        history.push(ClipboardItem::new("b", 2, None));
        history.push(ClipboardItem::new("c", 3, None));
        let removed = history.remove_at(1).unwrap();
        assert_eq!(removed.text, "b");
        assert_eq!(history.len(), 2);
        assert!(history.remove_at(99).is_none());
    }

    #[test]
    fn test_history_entries() {
        let mut history = ClipboardHistory::new(5);
        history.push(ClipboardItem::new("x", 1, None));
        history.push(ClipboardItem::new("y", 2, None));
        let entries = history.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "x");
        assert_eq!(entries[1].text, "y");
    }

    #[test]
    fn test_history_max_entries() {
        let history = ClipboardHistory::new(42);
        assert_eq!(history.max_entries(), 42);
    }

    #[test]
    fn test_history_contains_text() {
        let mut history = ClipboardHistory::new(5);
        history.push(ClipboardItem::new("needle", 1, None));
        assert!(history.contains_text("needle"));
        assert!(!history.contains_text("missing"));
        assert!(!history.contains_text("need")); // partial match should not count
    }

    #[test]
    fn test_history_deduplicate() {
        let mut history = ClipboardHistory::new(10);
        history.push(ClipboardItem::new("a", 1, None));
        history.push(ClipboardItem::new("b", 2, None));
        history.push(ClipboardItem::new("a", 3, None));
        history.push(ClipboardItem::new("c", 4, None));
        history.push(ClipboardItem::new("b", 5, None));
        history.deduplicate();
        assert_eq!(history.len(), 3);
        let texts: Vec<&str> = history.entries().iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, vec!["a", "c", "b"]);
        // the kept items should be the latest occurrences
        assert_eq!(history.entries()[0].timestamp, 3);
        assert_eq!(history.entries()[2].timestamp, 5);
    }

    #[test]
    fn test_watcher_new() {
        let watcher = ClipboardWatcher::new();
        assert_eq!(watcher.change_count(), 0);
        assert!(watcher.last_content().is_none());
    }

    #[test]
    fn test_watcher_check_change() {
        let mut watcher = ClipboardWatcher::new();
        assert!(watcher.check_change("first"));
        assert_eq!(watcher.change_count(), 1);
        assert!(!watcher.check_change("first")); // same content
        assert_eq!(watcher.change_count(), 1);
        assert!(watcher.check_change("second"));
        assert_eq!(watcher.change_count(), 2);
        assert_eq!(watcher.last_content(), Some("second"));
    }

    #[test]
    fn test_watcher_reset() {
        let mut watcher = ClipboardWatcher::new();
        watcher.check_change("a");
        watcher.check_change("b");
        assert_eq!(watcher.change_count(), 2);
        watcher.reset();
        assert_eq!(watcher.change_count(), 0);
        assert!(watcher.last_content().is_none());
        // After reset, any content is considered a change again
        assert!(watcher.check_change("a"));
    }

    #[test]
    fn test_clipboard_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(ClipboardError::WriteFailed);
        assert_eq!(err.to_string(), "clipboard write failed");
    }
}
