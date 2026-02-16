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
}
