//! System clipboard via OSC 52.
//!
//! Equivalent to VS Code's clipboard service.
//! Uses OSC 52 escape sequences for terminal clipboard access, with fallback
//! to external tools (`xclip`, `xsel`, `pbcopy`/`pbpaste`, `wl-copy`/`wl-paste`)
//! and an internal buffer.

use std::fmt;
use std::io::Write;
use std::process::Command;
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

// ---------------------------------------------------------------------------
// External tool clipboard backend
// ---------------------------------------------------------------------------

/// Available clipboard backends, ordered by preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardBackend {
    /// OSC 52 terminal escape sequence (write-only without terminal query).
    Osc52,
    /// `xclip` (X11).
    Xclip,
    /// `xsel` (X11).
    Xsel,
    /// `pbcopy` / `pbpaste` (macOS).
    PbCopy,
    /// `wl-copy` / `wl-paste` (Wayland).
    WlCopy,
    /// Internal in-memory buffer (always available).
    Internal,
}

impl fmt::Display for ClipboardBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardBackend::Osc52 => write!(f, "OSC 52"),
            ClipboardBackend::Xclip => write!(f, "xclip"),
            ClipboardBackend::Xsel => write!(f, "xsel"),
            ClipboardBackend::PbCopy => write!(f, "pbcopy/pbpaste"),
            ClipboardBackend::WlCopy => write!(f, "wl-copy/wl-paste"),
            ClipboardBackend::Internal => write!(f, "internal"),
        }
    }
}

/// Detects which clipboard backends are available on the current system.
pub fn detect_backends() -> Vec<ClipboardBackend> {
    let mut backends = Vec::new();

    // OSC 52 is always available for writing (reading requires terminal query)
    backends.push(ClipboardBackend::Osc52);

    if has_command("pbcopy") && has_command("pbpaste") {
        backends.push(ClipboardBackend::PbCopy);
    }
    if has_command("wl-copy") && has_command("wl-paste") {
        backends.push(ClipboardBackend::WlCopy);
    }
    if has_command("xclip") {
        backends.push(ClipboardBackend::Xclip);
    }
    if has_command("xsel") {
        backends.push(ClipboardBackend::Xsel);
    }

    // Internal is always available as final fallback
    backends.push(ClipboardBackend::Internal);
    backends
}

fn has_command(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn read_via_command(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn write_via_command(cmd: &str, args: &[&str], text: &str) -> bool {
    let mut child = match Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

/// Read text from the system clipboard using an external tool.
pub fn read_external(backend: ClipboardBackend) -> Option<String> {
    match backend {
        ClipboardBackend::PbCopy => read_via_command("pbpaste", &[]),
        ClipboardBackend::WlCopy => read_via_command("wl-paste", &["--no-newline"]),
        ClipboardBackend::Xclip => {
            read_via_command("xclip", &["-selection", "clipboard", "-o"])
        }
        ClipboardBackend::Xsel => read_via_command("xsel", &["--clipboard", "--output"]),
        _ => None,
    }
}

/// Write text to the system clipboard using an external tool.
pub fn write_external(backend: ClipboardBackend, text: &str) -> bool {
    match backend {
        ClipboardBackend::PbCopy => write_via_command("pbcopy", &[], text),
        ClipboardBackend::WlCopy => write_via_command("wl-copy", &[], text),
        ClipboardBackend::Xclip => {
            write_via_command("xclip", &["-selection", "clipboard"], text)
        }
        ClipboardBackend::Xsel => write_via_command("xsel", &["--clipboard", "--input"], text),
        _ => false,
    }
}

/// Read HTML from the system clipboard using an external tool (if supported).
pub fn read_html_external(backend: ClipboardBackend) -> Option<String> {
    match backend {
        ClipboardBackend::Xclip => {
            read_via_command("xclip", &["-selection", "clipboard", "-t", "text/html", "-o"])
        }
        ClipboardBackend::WlCopy => {
            read_via_command("wl-paste", &["--no-newline", "--type", "text/html"])
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ClipboardService — unified clipboard with auto-detection
// ---------------------------------------------------------------------------

/// Unified clipboard service that auto-detects the best available backend
/// and provides read/write with fallback chain.
pub struct ClipboardService {
    backends: Vec<ClipboardBackend>,
    buffer: Mutex<String>,
    html_buffer: Mutex<String>,
}

impl ClipboardService {
    /// Create a new service, auto-detecting available backends.
    pub fn new() -> Self {
        Self {
            backends: detect_backends(),
            buffer: Mutex::new(String::new()),
            html_buffer: Mutex::new(String::new()),
        }
    }

    /// Create a service with explicit backends (useful for testing).
    pub fn with_backends(backends: Vec<ClipboardBackend>) -> Self {
        Self {
            backends,
            buffer: Mutex::new(String::new()),
            html_buffer: Mutex::new(String::new()),
        }
    }

    /// Returns the detected backends in priority order.
    pub fn backends(&self) -> &[ClipboardBackend] {
        &self.backends
    }

    /// Returns the primary (preferred) backend.
    pub fn primary_backend(&self) -> ClipboardBackend {
        self.backends.first().copied().unwrap_or(ClipboardBackend::Internal)
    }

    /// Read text from the clipboard, trying external tools then falling back
    /// to the internal buffer.
    pub fn read_text(&self) -> Result<String, ClipboardError> {
        // Try external tools first
        for &backend in &self.backends {
            if backend == ClipboardBackend::Osc52 || backend == ClipboardBackend::Internal {
                continue;
            }
            if let Some(text) = read_external(backend) {
                return Ok(text);
            }
        }
        // Fall back to internal buffer
        let buf = self.buffer.lock().map_err(|_| ClipboardError::LockPoisoned)?;
        if buf.is_empty() {
            Ok(String::new())
        } else {
            Ok(buf.clone())
        }
    }

    /// Write text to the clipboard, writing to OSC 52, the best external tool,
    /// and the internal buffer.
    pub fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        // Always update internal buffer
        {
            let mut buf = self.buffer.lock().map_err(|_| ClipboardError::LockPoisoned)?;
            *buf = text.to_string();
        }

        // Write via OSC 52 if available
        if self.backends.contains(&ClipboardBackend::Osc52) {
            write_osc52(text);
        }

        // Write via best external tool
        for &backend in &self.backends {
            if backend == ClipboardBackend::Osc52 || backend == ClipboardBackend::Internal {
                continue;
            }
            if write_external(backend, text) {
                break;
            }
        }

        Ok(())
    }

    /// Read HTML from clipboard (if supported by the active backend).
    pub fn read_html(&self) -> Result<String, ClipboardError> {
        for &backend in &self.backends {
            if let Some(html) = read_html_external(backend) {
                return Ok(html);
            }
        }
        // Fall back to internal html buffer
        let buf = self.html_buffer.lock().map_err(|_| ClipboardError::LockPoisoned)?;
        Ok(buf.clone())
    }

    /// Write HTML to the internal HTML buffer.
    pub fn write_html(&self, html: &str) -> Result<(), ClipboardError> {
        let mut buf = self.html_buffer.lock().map_err(|_| ClipboardError::LockPoisoned)?;
        *buf = html.to_string();
        Ok(())
    }
}

impl Default for ClipboardService {
    fn default() -> Self {
        Self::new()
    }
}

impl IClipboardService for ClipboardService {
    fn read_text(&self) -> Option<String> {
        ClipboardService::read_text(self).ok().filter(|s| !s.is_empty())
    }

    fn write_text(&self, text: &str) {
        let _ = ClipboardService::write_text(self, text);
    }
}

/// Write text via OSC 52 escape sequence.
fn write_osc52(text: &str) {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let seq = format!("\x1b]52;c;{encoded}\x07");
    let _ = std::io::stdout().write_all(seq.as_bytes());
    let _ = std::io::stdout().flush();
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

    // --- ClipboardService tests ---

    #[test]
    fn clipboard_service_internal_only() {
        let svc = ClipboardService::with_backends(vec![ClipboardBackend::Internal]);
        assert!(svc.write_text("hello").is_ok());
        let text = svc.read_text().unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn clipboard_service_read_empty() {
        let svc = ClipboardService::with_backends(vec![ClipboardBackend::Internal]);
        let text = svc.read_text().unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn clipboard_service_overwrite() {
        let svc = ClipboardService::with_backends(vec![ClipboardBackend::Internal]);
        svc.write_text("first").unwrap();
        svc.write_text("second").unwrap();
        assert_eq!(svc.read_text().unwrap(), "second");
    }

    #[test]
    fn clipboard_service_html_roundtrip() {
        let svc = ClipboardService::with_backends(vec![ClipboardBackend::Internal]);
        svc.write_html("<b>bold</b>").unwrap();
        assert_eq!(svc.read_html().unwrap(), "<b>bold</b>");
    }

    #[test]
    fn clipboard_service_html_empty() {
        let svc = ClipboardService::with_backends(vec![ClipboardBackend::Internal]);
        assert!(svc.read_html().unwrap().is_empty());
    }

    #[test]
    fn clipboard_service_implements_trait() {
        let svc = ClipboardService::with_backends(vec![ClipboardBackend::Internal]);
        let trait_ref: &dyn IClipboardService = &svc;
        assert!(trait_ref.read_text().is_none());
        trait_ref.write_text("via trait");
        assert_eq!(trait_ref.read_text(), Some("via trait".to_string()));
    }

    #[test]
    fn clipboard_service_primary_backend() {
        let svc = ClipboardService::with_backends(vec![
            ClipboardBackend::Osc52,
            ClipboardBackend::Internal,
        ]);
        assert_eq!(svc.primary_backend(), ClipboardBackend::Osc52);
    }

    #[test]
    fn clipboard_service_default_has_backends() {
        let svc = ClipboardService::default();
        assert!(!svc.backends().is_empty());
        // Internal is always the last fallback
        assert_eq!(*svc.backends().last().unwrap(), ClipboardBackend::Internal);
    }

    #[test]
    fn detect_backends_always_has_internal() {
        let backends = detect_backends();
        assert!(backends.contains(&ClipboardBackend::Internal));
        assert!(backends.contains(&ClipboardBackend::Osc52));
    }

    #[test]
    fn clipboard_backend_display() {
        assert_eq!(ClipboardBackend::Osc52.to_string(), "OSC 52");
        assert_eq!(ClipboardBackend::Xclip.to_string(), "xclip");
        assert_eq!(ClipboardBackend::Xsel.to_string(), "xsel");
        assert_eq!(ClipboardBackend::PbCopy.to_string(), "pbcopy/pbpaste");
        assert_eq!(ClipboardBackend::WlCopy.to_string(), "wl-copy/wl-paste");
        assert_eq!(ClipboardBackend::Internal.to_string(), "internal");
    }
}
