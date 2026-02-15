//! System clipboard via OSC 52.
//!
//! Equivalent to VS Code's clipboard service.
//! Uses OSC 52 escape sequences for terminal clipboard access.

use std::io::Write;
use std::sync::Mutex;

/// Clipboard service trait.
pub trait IClipboardService: Send + Sync {
    /// Read text from clipboard.
    fn read_text(&self) -> Option<String>;
    /// Write text to clipboard.
    fn write_text(&self, text: &str);
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
}
