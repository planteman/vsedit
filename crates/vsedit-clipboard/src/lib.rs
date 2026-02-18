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

// ---------------------------------------------------------------------------
// ClipboardRingBuffer
// ---------------------------------------------------------------------------

/// A ring-buffer clipboard history that efficiently supports cycling through
/// past clips. Unlike `ClipboardHistory`, this uses a circular buffer
/// with O(1) push and constant memory.
#[derive(Debug)]
pub struct ClipboardRingBuffer {
    buffer: Vec<Option<ClipboardItem>>,
    head: usize,
    count: usize,
}

impl ClipboardRingBuffer {
    /// Create a ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            buffer: (0..capacity).map(|_| None).collect(),
            head: 0,
            count: 0,
        }
    }

    /// Push a new item, overwriting the oldest if full.
    pub fn push(&mut self, item: ClipboardItem) {
        self.buffer[self.head] = Some(item);
        self.head = (self.head + 1) % self.buffer.len();
        if self.count < self.buffer.len() {
            self.count += 1;
        }
    }

    /// Get the Nth most recent item (0 = most recent).
    pub fn get_recent(&self, n: usize) -> Option<&ClipboardItem> {
        if n >= self.count {
            return None;
        }
        let idx = (self.head + self.buffer.len() - 1 - n) % self.buffer.len();
        self.buffer[idx].as_ref()
    }

    /// Return the most recent item.
    pub fn most_recent(&self) -> Option<&ClipboardItem> {
        self.get_recent(0)
    }

    /// Return all items from most recent to oldest.
    pub fn all_recent_first(&self) -> Vec<&ClipboardItem> {
        (0..self.count)
            .filter_map(|i| self.get_recent(i))
            .collect()
    }

    /// Return the number of items stored.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Return `true` if empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Return the capacity.
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        for slot in &mut self.buffer {
            *slot = None;
        }
        self.head = 0;
        self.count = 0;
    }
}

// ---------------------------------------------------------------------------
// Paste-as-plain-text helpers
// ---------------------------------------------------------------------------

/// Strip HTML tags from text, returning plain text content.
pub fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Paste-as-plain-text: Given a multi-format item, extract plain text.
/// If the item is HTML, strips tags. If it's rich text, returns the raw data.
/// For plain text, returns as-is.
pub fn clipboard_paste_special(item: &MultiFormatClipboardItem) -> String {
    match &item.format {
        ClipboardFormat::Html => strip_html_tags(&item.data),
        ClipboardFormat::RichText => {
            // Simple RTF strip: remove RTF control words
            item.data
                .chars()
                .filter(|c| !c.is_control())
                .collect::<String>()
                .replace("\\par", "\n")
        }
        _ => item.data.clone(),
    }
}

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

/// Detect the likely format of clipboard content by inspecting its structure.
pub fn clipboard_format_detection(content: &str) -> ClipboardFormat {
    let trimmed = content.trim();
    // HTML detection
    if trimmed.starts_with('<') && (trimmed.contains("</") || trimmed.contains("/>")) {
        if trimmed.starts_with("<!DOCTYPE html")
            || trimmed.starts_with("<html")
            || trimmed.contains("<body")
        {
            return ClipboardFormat::Html;
        }
    }
    // File path list detection (one path per line)
    if looks_like_file_paths(trimmed) {
        return ClipboardFormat::FilePaths;
    }
    // RTF detection
    if trimmed.starts_with("{\\rtf") {
        return ClipboardFormat::RichText;
    }
    ClipboardFormat::PlainText
}

fn looks_like_file_paths(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() || lines.len() > 100 {
        return false;
    }
    lines.iter().all(|line| {
        let l = line.trim();
        l.starts_with('/')
            || l.starts_with("file://")
            || (l.len() > 2 && l.as_bytes()[1] == b':')
    })
}

// ---------------------------------------------------------------------------
// Additional ClipboardHistory methods
// ---------------------------------------------------------------------------

impl ClipboardHistory {
    /// Returns the total number of characters across all entries.
    pub fn total_chars(&self) -> usize {
        self.entries.iter().map(|e| e.text.len()).sum()
    }

    /// Returns a reference to the oldest entry (first in the list).
    pub fn oldest(&self) -> Option<&ClipboardItem> {
        self.entries.first()
    }

    /// Alias for `most_recent()` — returns the newest entry.
    pub fn newest(&self) -> Option<&ClipboardItem> {
        self.most_recent()
    }
}

// ---------------------------------------------------------------------------
// Additional ClipboardItem methods
// ---------------------------------------------------------------------------

impl ClipboardItem {
    /// Returns the number of whitespace-separated words in the text.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    /// Returns `true` if the text contains at least one newline character.
    pub fn is_multiline(&self) -> bool {
        self.text.contains('\n')
    }
}

// ---------------------------------------------------------------------------
// Display for ClipboardHistory
// ---------------------------------------------------------------------------

impl fmt::Display for ClipboardHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ClipboardHistory({}/{} entries)",
            self.entries.len(),
            self.max_entries,
        )
    }
}

// ---------------------------------------------------------------------------
// Additional InMemoryClipboard methods
// ---------------------------------------------------------------------------

impl InMemoryClipboard {
    /// Returns `true` if the internal buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().is_empty()
    }
}

// ---------------------------------------------------------------------------
// Additional ClipboardWatcher methods
// ---------------------------------------------------------------------------

impl ClipboardWatcher {
    /// Returns `true` if the watcher has observed at least one change.
    pub fn has_changed(&self) -> bool {
        self.change_count > 0
    }
}

// ---------------------------------------------------------------------------
// Content type detection
// ---------------------------------------------------------------------------

/// Detected content type for clipboard text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardContentType {
    /// A URL (starts with http://, https://, or ftp://).
    Url,
    /// Looks like source code (contains braces, semicolons, or common keywords).
    Code,
    /// A file path (starts with `/`, `./`, `~`, or a Windows drive letter).
    FilePath,
    /// An email address.
    Email,
    /// Plain text.
    PlainText,
}

impl fmt::Display for ClipboardContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardContentType::Url => write!(f, "URL"),
            ClipboardContentType::Code => write!(f, "Code"),
            ClipboardContentType::FilePath => write!(f, "File Path"),
            ClipboardContentType::Email => write!(f, "Email"),
            ClipboardContentType::PlainText => write!(f, "Plain Text"),
        }
    }
}

/// Detect the content type of clipboard text.
pub fn detect_content_type(text: &str) -> ClipboardContentType {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return ClipboardContentType::PlainText;
    }
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ftp://")
    {
        return ClipboardContentType::Url;
    }
    // Simple email detection: single line with exactly one @
    if !trimmed.contains('\n') && trimmed.contains('@') && trimmed.contains('.') {
        let at_count = trimmed.chars().filter(|&c| c == '@').count();
        if at_count == 1 && !trimmed.contains(' ') {
            return ClipboardContentType::Email;
        }
    }
    // File path detection
    if trimmed.starts_with('/')
        || trimmed.starts_with("./")
        || trimmed.starts_with("~/")
        || (trimmed.len() >= 3 && trimmed.as_bytes()[1] == b':' && trimmed.as_bytes()[2] == b'\\')
    {
        if !trimmed.contains('\n') {
            return ClipboardContentType::FilePath;
        }
    }
    // Code detection heuristics
    let code_indicators: &[&str] = &["{", "}", ";", "fn ", "let ", "const ", "var ", "def ", "class "];
    let indicator_count = code_indicators
        .iter()
        .filter(|&&ind| trimmed.contains(ind))
        .count();
    if indicator_count >= 2 {
        return ClipboardContentType::Code;
    }
    ClipboardContentType::PlainText
}

// ---------------------------------------------------------------------------
// Clipboard content transformation
// ---------------------------------------------------------------------------

/// Normalize whitespace in clipboard content: collapse runs of whitespace to
/// single spaces and trim leading/trailing whitespace.
pub fn normalize_clipboard_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_ws = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !prev_ws && !result.is_empty() {
                result.push(' ');
            }
            prev_ws = true;
        } else {
            prev_ws = false;
            result.push(c);
        }
    }
    result.trim_end().to_string()
}

/// Trim each line of clipboard content individually.
pub fn trim_clipboard_lines(text: &str) -> String {
    text.lines()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove duplicate consecutive lines from clipboard content.
pub fn dedup_clipboard_lines(text: &str) -> String {
    let mut result = Vec::new();
    let mut prev: Option<&str> = None;
    for line in text.lines() {
        if prev != Some(line) {
            result.push(line);
        }
        prev = Some(line);
    }
    result.join("\n")
}

// ---------------------------------------------------------------------------
// Paste format selection
// ---------------------------------------------------------------------------

/// Strategy for formatting pasted content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteStrategy {
    /// Paste as-is.
    Raw,
    /// Trim whitespace.
    Trimmed,
    /// Normalize whitespace.
    Normalized,
    /// Escape for use in a string literal.
    Escaped,
}

/// Apply a paste strategy to text.
pub fn apply_paste_strategy(text: &str, strategy: PasteStrategy) -> String {
    match strategy {
        PasteStrategy::Raw => text.to_string(),
        PasteStrategy::Trimmed => text.trim().to_string(),
        PasteStrategy::Normalized => normalize_clipboard_whitespace(text),
        PasteStrategy::Escaped => {
            let mut out = String::with_capacity(text.len());
            for c in text.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    _ => out.push(c),
                }
            }
            out
        }
    }
}

// ---------------------------------------------------------------------------
// Clipboard transformation pipeline
// ---------------------------------------------------------------------------

/// A single transformation step that can be applied to clipboard text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardTransform {
    /// Trim leading and trailing whitespace.
    Trim,
    /// Convert to lowercase.
    Lowercase,
    /// Convert to uppercase.
    Uppercase,
    /// Collapse consecutive blank lines into one.
    CollapseBlankLines,
    /// Remove all blank lines.
    RemoveBlankLines,
    /// Add line numbers (1-based).
    AddLineNumbers,
    /// Sort lines alphabetically.
    SortLines,
    /// Reverse the order of lines.
    ReverseLines,
    /// Remove trailing whitespace from each line.
    StripTrailingWhitespace,
}

/// Apply a sequence of transformations to clipboard text.
pub fn apply_transform_pipeline(text: &str, transforms: &[ClipboardTransform]) -> String {
    let mut result = text.to_string();
    for &t in transforms {
        result = apply_single_transform(&result, t);
    }
    result
}

fn apply_single_transform(text: &str, transform: ClipboardTransform) -> String {
    match transform {
        ClipboardTransform::Trim => text.trim().to_string(),
        ClipboardTransform::Lowercase => text.to_lowercase(),
        ClipboardTransform::Uppercase => text.to_uppercase(),
        ClipboardTransform::CollapseBlankLines => {
            let mut result = Vec::new();
            let mut prev_blank = false;
            for line in text.lines() {
                let blank = line.trim().is_empty();
                if blank && prev_blank {
                    continue;
                }
                result.push(line);
                prev_blank = blank;
            }
            result.join("\n")
        }
        ClipboardTransform::RemoveBlankLines => {
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
        ClipboardTransform::AddLineNumbers => {
            text.lines()
                .enumerate()
                .map(|(i, l)| format!("{:>4} {l}", i + 1))
                .collect::<Vec<_>>()
                .join("\n")
        }
        ClipboardTransform::SortLines => {
            let mut lines: Vec<&str> = text.lines().collect();
            lines.sort();
            lines.join("\n")
        }
        ClipboardTransform::ReverseLines => {
            let mut lines: Vec<&str> = text.lines().collect();
            lines.reverse();
            lines.join("\n")
        }
        ClipboardTransform::StripTrailingWhitespace => {
            text.lines()
                .map(|l| l.trim_end())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

// ---------------------------------------------------------------------------
// Clipboard size-limited history
// ---------------------------------------------------------------------------

/// A clipboard history that enforces a maximum total byte size across all
/// entries, in addition to an entry count limit.
#[derive(Debug)]
pub struct SizeLimitedHistory {
    entries: Vec<ClipboardItem>,
    max_entries: usize,
    max_bytes: usize,
    current_bytes: usize,
}

impl SizeLimitedHistory {
    /// Create a history with limits on both entry count and total byte size.
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            max_bytes,
            current_bytes: 0,
        }
    }

    /// Push an item, evicting oldest entries as needed to stay within limits.
    pub fn push(&mut self, item: ClipboardItem) {
        let item_bytes = item.text.len();
        // Evict until we have room for the new item
        while self.entries.len() >= self.max_entries
            || (self.current_bytes + item_bytes > self.max_bytes && !self.entries.is_empty())
        {
            let removed = self.entries.remove(0);
            self.current_bytes -= removed.text.len();
        }
        self.current_bytes += item_bytes;
        self.entries.push(item);
    }

    /// Current total bytes across all entries.
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the most recent entry.
    pub fn most_recent(&self) -> Option<&ClipboardItem> {
        self.entries.last()
    }

    /// Return a slice of all entries.
    pub fn entries(&self) -> &[ClipboardItem] {
        &self.entries
    }

    /// Remaining byte capacity.
    pub fn remaining_bytes(&self) -> usize {
        self.max_bytes.saturating_sub(self.current_bytes)
    }
}

// ---------------------------------------------------------------------------
// Multi-cursor paste distribution
// ---------------------------------------------------------------------------

/// Distribute clipboard text across multiple cursors.
///
/// If the clipboard contains the same number of lines as there are cursors,
/// each cursor gets its own line. Otherwise every cursor gets the full text.
pub fn distribute_paste(text: &str, cursor_count: usize) -> Vec<String> {
    if cursor_count == 0 {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() == cursor_count {
        lines.iter().map(|l| l.to_string()).collect()
    } else {
        vec![text.to_string(); cursor_count]
    }
}

// ---------------------------------------------------------------------------
// Case-insensitive history search
// ---------------------------------------------------------------------------

impl ClipboardHistory {
    /// Search for items whose text contains the query (case-insensitive).
    pub fn search_case_insensitive(&self, query: &str) -> Vec<&ClipboardItem> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|item| item.text.to_lowercase().contains(&q))
            .collect()
    }

    /// Return aggregate statistics about the history.
    pub fn statistics(&self) -> ClipboardHistoryStats {
        let total_bytes: usize = self.entries.iter().map(|e| e.text.len()).sum();
        let total_lines: usize = self.entries.iter().map(|e| e.text.lines().count()).sum();
        let avg_bytes = if self.entries.is_empty() {
            0
        } else {
            total_bytes / self.entries.len()
        };
        let multiline_count = self.entries.iter().filter(|e| e.text.contains('\n')).count();
        ClipboardHistoryStats {
            entry_count: self.entries.len(),
            total_bytes,
            average_bytes: avg_bytes,
            total_lines,
            multiline_entries: multiline_count,
        }
    }
}

/// Aggregate statistics about clipboard history entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardHistoryStats {
    /// Total number of entries.
    pub entry_count: usize,
    /// Total bytes across all entries.
    pub total_bytes: usize,
    /// Average bytes per entry.
    pub average_bytes: usize,
    /// Total number of lines across all entries.
    pub total_lines: usize,
    /// Number of entries that span multiple lines.
    pub multiline_entries: usize,
}

// ---------------------------------------------------------------------------
// ClipboardContentKind – refined content classification
// ---------------------------------------------------------------------------

/// Fine-grained classification of clipboard text content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContentKind {
    PlainText,
    Json,
    Url,
    Email,
    Code,
    FilePath,
    NumberList,
}

// ---------------------------------------------------------------------------
// ClipboardContentDetector
// ---------------------------------------------------------------------------

/// Heuristic detector for classifying clipboard text.
pub struct ClipboardContentDetector;

impl ClipboardContentDetector {
    /// Return `true` if `text` looks like valid JSON.
    pub fn is_json(text: &str) -> bool {
        let t = text.trim();
        (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']'))
    }

    /// Return `true` if `text` looks like a URL.
    pub fn is_url(text: &str) -> bool {
        let t = text.trim();
        t.starts_with("http://") || t.starts_with("https://") || t.starts_with("ftp://")
    }

    /// Return `true` if `text` looks like an email address.
    pub fn is_email(text: &str) -> bool {
        let t = text.trim();
        let parts: Vec<&str> = t.splitn(2, '@').collect();
        parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.')
    }

    /// Count the number of lines in `text`.
    pub fn count_lines(text: &str) -> usize {
        if text.is_empty() {
            0
        } else {
            text.lines().count()
        }
    }

    /// Classify `text` into a [`ClipboardContentKind`].
    pub fn detect(text: &str) -> ClipboardContentKind {
        if Self::is_json(text) {
            ClipboardContentKind::Json
        } else if Self::is_url(text) {
            ClipboardContentKind::Url
        } else if Self::is_email(text) {
            ClipboardContentKind::Email
        } else if text.trim().starts_with('/') || text.trim().starts_with("C:\\") {
            ClipboardContentKind::FilePath
        } else if text.lines().all(|l| l.trim().parse::<f64>().is_ok()) && !text.trim().is_empty()
        {
            ClipboardContentKind::NumberList
        } else if text.contains("fn ") || text.contains("let ") || text.contains("pub ") {
            ClipboardContentKind::Code
        } else {
            ClipboardContentKind::PlainText
        }
    }
}

// ---------------------------------------------------------------------------
// ClipboardHistoryManager
// ---------------------------------------------------------------------------

/// Wraps [`ClipboardHistory`] with pinning support.
pub struct ClipboardHistoryManager {
    history: ClipboardHistory,
    pinned: Vec<String>,
}

impl ClipboardHistoryManager {
    pub fn new(max_entries: usize) -> Self {
        Self {
            history: ClipboardHistory::new(max_entries),
            pinned: Vec::new(),
        }
    }

    pub fn add(&mut self, text: &str, timestamp: u64) {
        self.history
            .push(ClipboardItem::new(text, timestamp, None));
    }

    pub fn pin(&mut self, text: &str) {
        if !self.pinned.contains(&text.to_string()) {
            self.pinned.push(text.to_string());
        }
    }

    pub fn unpin(&mut self, text: &str) {
        self.pinned.retain(|p| p != text);
    }

    pub fn is_pinned(&self, text: &str) -> bool {
        self.pinned.contains(&text.to_string())
    }

    pub fn pinned_count(&self) -> usize {
        self.pinned.len()
    }

    pub fn all_entries(&self) -> &[ClipboardItem] {
        self.history.entries()
    }

    /// Clears history entries but preserves the pinned list.
    pub fn clear_unpinned(&mut self) {
        self.history.clear();
    }
}

// ---------------------------------------------------------------------------
// ClipboardFormatConverter
// ---------------------------------------------------------------------------

/// Stateless helpers for text format conversions.
pub struct ClipboardFormatConverter;

impl ClipboardFormatConverter {
    pub fn new() -> Self {
        Self
    }

    pub fn to_uppercase(text: &str) -> String {
        text.to_uppercase()
    }

    pub fn to_lowercase(text: &str) -> String {
        text.to_lowercase()
    }

    pub fn to_title_case(text: &str) -> String {
        text.split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => {
                        let upper: String = c.to_uppercase().collect();
                        let rest: String = chars.as_str().to_lowercase();
                        format!("{upper}{rest}")
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Convert newline-separated text into a single CSV row.
    pub fn lines_to_csv(text: &str) -> String {
        text.lines().collect::<Vec<_>>().join(",")
    }

    /// Convert a single CSV row into newline-separated lines.
    pub fn csv_to_lines(csv: &str) -> String {
        csv.split(',')
            .map(|s| s.trim())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn to_snake_case(text: &str) -> String {
        let mut result = String::with_capacity(text.len() + 4);
        for (i, ch) in text.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                for lc in ch.to_lowercase() {
                    result.push(lc);
                }
            } else if ch == ' ' || ch == '-' {
                result.push('_');
            } else {
                result.push(ch);
            }
        }
        result
    }

    pub fn to_camel_case(text: &str) -> String {
        let mut capitalize_next = false;
        let mut result = String::with_capacity(text.len());
        for ch in text.chars() {
            if ch == '_' || ch == ' ' || ch == '-' {
                capitalize_next = true;
            } else if capitalize_next {
                for uc in ch.to_uppercase() {
                    result.push(uc);
                }
                capitalize_next = false;
            } else {
                result.push(ch);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// ClipboardSyncState
// ---------------------------------------------------------------------------

/// Tracks synchronisation state for cross-session clipboard sharing.
pub struct ClipboardSyncState {
    pub session_id: String,
    pub sequence: u64,
    pub last_content: Option<String>,
    pub dirty: bool,
}

impl ClipboardSyncState {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            sequence: 0,
            last_content: None,
            dirty: false,
        }
    }

    /// Update the content, increment the sequence counter, and mark dirty.
    pub fn update(&mut self, content: &str) {
        self.last_content = Some(content.to_string());
        self.sequence += 1;
        self.dirty = true;
    }

    /// Mark the state as synced (no longer dirty).
    pub fn mark_synced(&mut self) {
        self.dirty = false;
    }

    pub fn needs_sync(&self) -> bool {
        self.dirty
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence
    }
}

// ---------------------------------------------------------------------------
// ClipboardFormatNegotiator - clipboard format negotiator
// ---------------------------------------------------------------------------

/// Severity level for clipboard format negotiator issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClipboardFormatNegotiatorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ClipboardFormatNegotiatorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ClipboardFormatNegotiator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardFormatNegotiatorEntry {
    pub id: String,
    pub label: String,
    pub severity: ClipboardFormatNegotiatorSeverity,
    pub detail: Option<String>,
    pub format_count: usize,
    enabled: bool,
}

impl ClipboardFormatNegotiatorEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ClipboardFormatNegotiatorSeverity::Low,
            detail: None,
            format_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ClipboardFormatNegotiatorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_format_count(mut self, val: usize) -> Self {
        self.format_count = val;
        self
    }

    pub fn has_text(&self) -> bool {
        self.enabled && self.severity >= ClipboardFormatNegotiatorSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.format_count, det)
    }
}

impl fmt::Display for ClipboardFormatNegotiatorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ClipboardFormatNegotiatorEntry] items.
#[derive(Debug, Clone)]
pub struct ClipboardFormatNegotiator {
    entries: Vec<ClipboardFormatNegotiatorEntry>,
    name: String,
    capacity: usize,
}

impl ClipboardFormatNegotiator {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ClipboardFormatNegotiatorEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ClipboardFormatNegotiatorEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ClipboardFormatNegotiatorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn format_count(&self) -> usize { self.entries.len() }

    pub fn has_text(&self) -> bool {
        self.entries.iter().any(|e| e.has_text())
    }

    pub fn entries_by_severity(&self, severity: ClipboardFormatNegotiatorSeverity) -> Vec<&ClipboardFormatNegotiatorEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ClipboardFormatNegotiatorSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ClipboardFormatNegotiatorEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ClipboardFormatNegotiatorEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ClipboardPasteTransform - clipboard paste transform
// ---------------------------------------------------------------------------

/// Configuration for [ClipboardPasteTransform].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardPasteTransformConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub transform_count: usize,
}

impl ClipboardPasteTransformConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, transform_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_transform_count(mut self, val: usize) -> Self { self.transform_count = val; self }
}

impl Default for ClipboardPasteTransformConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ClipboardPasteTransform].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardPasteTransformItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ClipboardPasteTransformItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn needs_transform(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ClipboardPasteTransformItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ClipboardPasteTransformItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ClipboardPasteTransform {
    config: ClipboardPasteTransformConfig,
    items: Vec<ClipboardPasteTransformItem>,
}

impl ClipboardPasteTransform {
    pub fn new(config: ClipboardPasteTransformConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ClipboardPasteTransformItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ClipboardPasteTransformItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ClipboardPasteTransformItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn transform_count(&self) -> usize { self.items.len() }

    pub fn needs_transform(&self) -> bool {
        self.items.iter().any(|i| i.needs_transform())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ClipboardPasteTransformItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ClipboardPasteTransformItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ClipboardPasteTransformConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// clipboard – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XClipboardCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XClipboardCapabilities {
    pub fn new() -> Self {
        Self { flags: std::collections::HashSet::new() }
    }

    pub fn register(&mut self, cap: impl Into<String>) {
        self.flags.insert(cap.into());
    }

    pub fn has(&self, cap: &str) -> bool {
        self.flags.contains(cap)
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Return the intersection with another capability set.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.intersection(&other.flags).cloned().collect(),
        }
    }

    /// Return capabilities present here but not in `other`.
    pub fn diff(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.difference(&other.flags).cloned().collect(),
        }
    }

    pub fn all(&self) -> Vec<&str> {
        self.flags.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for XClipboardCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XClipboardServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XClipboardServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. Returns the previous value if the key was already present.
    pub fn register(&mut self, name: impl Into<String>, descriptor: impl Into<String>) -> Option<String> {
        self.services.insert(name.into(), descriptor.into())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.services.get(name).map(|s| s.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.services.remove(name)
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

/// Sanitize a path-like string by collapsing repeated separators and removing trailing ones.
pub fn x_clipboard_sanitize_path(p: &str) -> String {
    let mut result = String::with_capacity(p.len());
    let mut last_was_sep = false;
    for ch in p.chars() {
        if ch == '/' || ch == '\\' {
            if !last_was_sep {
                result.push('/');
            }
            last_was_sep = true;
        } else {
            result.push(ch);
            last_was_sep = false;
        }
    }
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}



// ---------------------------------------------------------------------------
// clipboard – Extended clipboard history helpers
// ---------------------------------------------------------------------------

/// Priority levels for clipboard history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZClipboardPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZClipboardPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZClipboardPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZClipboardPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks clipboard history data.
#[derive(Debug, Clone)]
pub struct ZClipboardClipboardHistory {
    pub snapshots: Vec<(u64, String)>,
    pub max_snapshots: usize,
    pub total_copies: u64,
}

impl ZClipboardClipboardHistory {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots: 0,
            total_copies: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZClipboardClipboardHistory[max_snapshots={:?}, total_copies={:?}]", self.max_snapshots, self.total_copies)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for clipboard history.
pub fn z_clipboard_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_clipboard_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_clipboard_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_clipboard_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_clipboard_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_clipboard_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_clipboard_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
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

    #[test]
    fn ring_buffer_push_and_recent() {
        let mut rb = ClipboardRingBuffer::new(3);
        rb.push(ClipboardItem::new("first", 1, None));
        rb.push(ClipboardItem::new("second", 2, None));
        rb.push(ClipboardItem::new("third", 3, None));
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.most_recent().unwrap().text, "third");
        assert_eq!(rb.get_recent(1).unwrap().text, "second");
        assert_eq!(rb.get_recent(2).unwrap().text, "first");
    }

    #[test]
    fn ring_buffer_overflow() {
        let mut rb = ClipboardRingBuffer::new(2);
        rb.push(ClipboardItem::new("a", 1, None));
        rb.push(ClipboardItem::new("b", 2, None));
        rb.push(ClipboardItem::new("c", 3, None)); // overwrites "a"
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.most_recent().unwrap().text, "c");
        assert_eq!(rb.get_recent(1).unwrap().text, "b");
        assert!(rb.get_recent(2).is_none()); // "a" was evicted
    }

    #[test]
    fn ring_buffer_all_recent_first() {
        let mut rb = ClipboardRingBuffer::new(5);
        rb.push(ClipboardItem::new("a", 1, None));
        rb.push(ClipboardItem::new("b", 2, None));
        let all = rb.all_recent_first();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].text, "b");
        assert_eq!(all[1].text, "a");
    }

    #[test]
    fn ring_buffer_clear() {
        let mut rb = ClipboardRingBuffer::new(5);
        rb.push(ClipboardItem::new("x", 1, None));
        rb.clear();
        assert!(rb.is_empty());
        assert!(rb.most_recent().is_none());
    }

    #[test]
    fn strip_html_tags_basic() {
        assert_eq!(strip_html_tags("<b>hello</b> <i>world</i>"), "hello world");
        assert_eq!(strip_html_tags("no tags here"), "no tags here");
        assert_eq!(strip_html_tags("<p>paragraph</p>"), "paragraph");
    }

    #[test]
    fn clipboard_paste_special_html() {
        let item = MultiFormatClipboardItem::new(ClipboardFormat::Html, "<b>bold</b> text", 1);
        assert_eq!(clipboard_paste_special(&item), "bold text");
    }

    #[test]
    fn clipboard_paste_special_plain() {
        let item =
            MultiFormatClipboardItem::new(ClipboardFormat::PlainText, "hello world", 1);
        assert_eq!(clipboard_paste_special(&item), "hello world");
    }

    #[test]
    fn format_detection_html() {
        assert_eq!(
            clipboard_format_detection("<!DOCTYPE html><html></html>"),
            ClipboardFormat::Html
        );
        assert_eq!(
            clipboard_format_detection("just plain text"),
            ClipboardFormat::PlainText
        );
        assert_eq!(
            clipboard_format_detection("{\\rtf1 content}"),
            ClipboardFormat::RichText
        );
    }

    #[test]
    fn clipboard_history_total_chars() {
        let mut h = ClipboardHistory::new(10);
        h.push(ClipboardItem::new("hello", 1, None));
        h.push(ClipboardItem::new("world!", 2, None));
        assert_eq!(h.total_chars(), 11); // 5 + 6
    }

    #[test]
    fn clipboard_history_oldest_newest() {
        let mut h = ClipboardHistory::new(10);
        h.push(ClipboardItem::new("first", 1, None));
        h.push(ClipboardItem::new("second", 2, None));
        h.push(ClipboardItem::new("third", 3, None));
        assert_eq!(h.oldest().unwrap().text, "first");
        assert_eq!(h.newest().unwrap().text, "third");
    }

    #[test]
    fn clipboard_item_word_count() {
        let item = ClipboardItem::new("hello world foo", 1, None);
        assert_eq!(item.word_count(), 3);
        let empty = ClipboardItem::new("", 1, None);
        assert_eq!(empty.word_count(), 0);
    }

    #[test]
    fn clipboard_item_is_multiline() {
        let single = ClipboardItem::new("one line", 1, None);
        assert!(!single.is_multiline());
        let multi = ClipboardItem::new("line1\nline2", 1, None);
        assert!(multi.is_multiline());
    }

    #[test]
    fn clipboard_history_display() {
        let mut h = ClipboardHistory::new(5);
        h.push(ClipboardItem::new("a", 1, None));
        h.push(ClipboardItem::new("b", 2, None));
        let s = format!("{h}");
        assert!(s.contains("2/5"));
    }

    #[test]
    fn in_memory_clipboard_is_empty() {
        let clip = InMemoryClipboard::new();
        assert!(clip.is_empty());
        clip.write_text("data");
        assert!(!clip.is_empty());
    }

    #[test]
    fn clipboard_watcher_has_changed() {
        let mut w = ClipboardWatcher::new();
        assert!(!w.has_changed());
        w.check_change("hello");
        assert!(w.has_changed());
    }

    #[test]
    fn clipboard_history_oldest_empty() {
        let h = ClipboardHistory::new(10);
        assert!(h.oldest().is_none());
        assert!(h.newest().is_none());
    }

    // -- new tests --

    #[test]
    fn detect_content_type_url() {
        assert_eq!(detect_content_type("https://example.com"), ClipboardContentType::Url);
        assert_eq!(detect_content_type("http://foo.bar/baz"), ClipboardContentType::Url);
        assert_eq!(detect_content_type("ftp://files.example.com"), ClipboardContentType::Url);
    }

    #[test]
    fn detect_content_type_email() {
        assert_eq!(detect_content_type("user@example.com"), ClipboardContentType::Email);
    }

    #[test]
    fn detect_content_type_file_path() {
        assert_eq!(detect_content_type("/usr/bin/bash"), ClipboardContentType::FilePath);
        assert_eq!(detect_content_type("./src/main.rs"), ClipboardContentType::FilePath);
        assert_eq!(detect_content_type("~/Documents"), ClipboardContentType::FilePath);
    }

    #[test]
    fn detect_content_type_code() {
        assert_eq!(
            detect_content_type("fn main() { let x = 42; }"),
            ClipboardContentType::Code,
        );
    }

    #[test]
    fn detect_content_type_plain_text() {
        assert_eq!(detect_content_type("hello world"), ClipboardContentType::PlainText);
        assert_eq!(detect_content_type(""), ClipboardContentType::PlainText);
    }

    #[test]
    fn normalize_clipboard_whitespace_collapses() {
        assert_eq!(normalize_clipboard_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_clipboard_whitespace("a\n\n\nb"), "a b");
    }

    #[test]
    fn trim_clipboard_lines_trims_each_line() {
        assert_eq!(trim_clipboard_lines("  a  \n  b  \n  c  "), "a\nb\nc");
    }

    #[test]
    fn dedup_clipboard_lines_removes_consecutive() {
        assert_eq!(dedup_clipboard_lines("a\na\nb\nb\na"), "a\nb\na");
    }

    #[test]
    fn paste_strategy_raw_returns_unchanged() {
        assert_eq!(apply_paste_strategy("  hi  ", PasteStrategy::Raw), "  hi  ");
    }

    #[test]
    fn paste_strategy_trimmed() {
        assert_eq!(apply_paste_strategy("  hi  ", PasteStrategy::Trimmed), "hi");
    }

    #[test]
    fn paste_strategy_escaped() {
        let result = apply_paste_strategy("line1\nline2\t\"quoted\"", PasteStrategy::Escaped);
        assert!(result.contains("\\n"));
        assert!(result.contains("\\t"));
        assert!(result.contains("\\\""));
    }

    #[test]
    fn clipboard_content_type_display() {
        assert_eq!(ClipboardContentType::Url.to_string(), "URL");
        assert_eq!(ClipboardContentType::Code.to_string(), "Code");
        assert_eq!(ClipboardContentType::PlainText.to_string(), "Plain Text");
    }

    // --- transform pipeline tests ---

    #[test]
    fn transform_pipeline_single_trim() {
        let result = apply_transform_pipeline("  hello  ", &[ClipboardTransform::Trim]);
        assert_eq!(result, "hello");
    }

    #[test]
    fn transform_pipeline_chained() {
        let result = apply_transform_pipeline(
            "  Hello World  ",
            &[ClipboardTransform::Trim, ClipboardTransform::Lowercase],
        );
        assert_eq!(result, "hello world");
    }

    #[test]
    fn transform_collapse_blank_lines() {
        let input = "a\n\n\n\nb\n\nc";
        let result = apply_transform_pipeline(input, &[ClipboardTransform::CollapseBlankLines]);
        assert_eq!(result, "a\n\nb\n\nc");
    }

    #[test]
    fn transform_remove_blank_lines() {
        let input = "a\n\nb\n\nc";
        let result = apply_transform_pipeline(input, &[ClipboardTransform::RemoveBlankLines]);
        assert_eq!(result, "a\nb\nc");
    }

    #[test]
    fn transform_add_line_numbers() {
        let input = "alpha\nbeta\ngamma";
        let result = apply_transform_pipeline(input, &[ClipboardTransform::AddLineNumbers]);
        assert!(result.starts_with("   1 alpha"));
        assert!(result.contains("   2 beta"));
        assert!(result.contains("   3 gamma"));
    }

    #[test]
    fn transform_sort_and_reverse() {
        let input = "banana\napple\ncherry";
        let sorted = apply_transform_pipeline(input, &[ClipboardTransform::SortLines]);
        assert_eq!(sorted, "apple\nbanana\ncherry");
        let reversed = apply_transform_pipeline(input, &[ClipboardTransform::ReverseLines]);
        assert_eq!(reversed, "cherry\napple\nbanana");
    }

    #[test]
    fn transform_strip_trailing_whitespace() {
        let input = "hello   \nworld  \n  ok  ";
        let result =
            apply_transform_pipeline(input, &[ClipboardTransform::StripTrailingWhitespace]);
        assert_eq!(result, "hello\nworld\n  ok");
    }

    // --- size-limited history tests ---

    #[test]
    fn size_limited_history_byte_limit() {
        let mut h = SizeLimitedHistory::new(100, 10);
        h.push(ClipboardItem::new("abcde", 1, None)); // 5 bytes
        h.push(ClipboardItem::new("fghij", 2, None)); // 5 bytes, total = 10
        assert_eq!(h.len(), 2);
        assert_eq!(h.current_bytes(), 10);
        // Pushing 3 more bytes forces eviction of first entry
        h.push(ClipboardItem::new("xyz", 3, None));
        assert_eq!(h.len(), 2);
        assert_eq!(h.current_bytes(), 8); // 5 + 3
        assert_eq!(h.most_recent().unwrap().text, "xyz");
    }

    #[test]
    fn size_limited_history_remaining_bytes() {
        let mut h = SizeLimitedHistory::new(10, 20);
        h.push(ClipboardItem::new("hello", 1, None));
        assert_eq!(h.remaining_bytes(), 15);
    }

    // --- multi-cursor paste tests ---

    #[test]
    fn distribute_paste_matching_lines() {
        let text = "line1\nline2\nline3";
        let result = distribute_paste(text, 3);
        assert_eq!(result, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn distribute_paste_mismatch_broadcasts() {
        let text = "line1\nline2";
        let result = distribute_paste(text, 3);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|s| s == "line1\nline2"));
    }

    #[test]
    fn distribute_paste_zero_cursors() {
        assert!(distribute_paste("anything", 0).is_empty());
    }

    // --- case-insensitive search test ---

    #[test]
    fn history_search_case_insensitive() {
        let mut h = ClipboardHistory::new(10);
        h.push(ClipboardItem::new("Hello World", 1, None));
        h.push(ClipboardItem::new("goodbye WORLD", 2, None));
        h.push(ClipboardItem::new("no match", 3, None));
        let results = h.search_case_insensitive("world");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "Hello World");
        assert_eq!(results[1].text, "goodbye WORLD");
    }

    // --- history statistics test ---

    #[test]
    fn history_statistics() {
        let mut h = ClipboardHistory::new(10);
        h.push(ClipboardItem::new("hello", 1, None));
        h.push(ClipboardItem::new("line1\nline2", 2, None));
        let stats = h.statistics();
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.total_bytes, 16); // 5 + 11
        assert_eq!(stats.average_bytes, 8);
        assert_eq!(stats.total_lines, 3); // 1 + 2
        assert_eq!(stats.multiline_entries, 1);
    }

    // -- ClipboardHistoryManager tests --

    #[test]
    fn test_history_manager_add_and_count() {
        let mut mgr = ClipboardHistoryManager::new(5);
        mgr.add("alpha", 1);
        mgr.add("beta", 2);
        assert_eq!(mgr.all_entries().len(), 2);
    }

    #[test]
    fn test_history_manager_pin_unpin() {
        let mut mgr = ClipboardHistoryManager::new(5);
        mgr.pin("sticky");
        assert!(mgr.is_pinned("sticky"));
        assert_eq!(mgr.pinned_count(), 1);
        mgr.unpin("sticky");
        assert!(!mgr.is_pinned("sticky"));
        assert_eq!(mgr.pinned_count(), 0);
    }

    #[test]
    fn test_history_manager_clear_unpinned() {
        let mut mgr = ClipboardHistoryManager::new(10);
        mgr.add("one", 1);
        mgr.add("two", 2);
        mgr.pin("one");
        mgr.clear_unpinned();
        assert_eq!(mgr.all_entries().len(), 0);
        assert!(mgr.is_pinned("one"));
    }

    // -- ClipboardFormatConverter tests --

    #[test]
    fn test_format_converter_title_case() {
        assert_eq!(
            ClipboardFormatConverter::to_title_case("hello world foo"),
            "Hello World Foo"
        );
    }

    #[test]
    fn test_format_converter_lines_to_csv() {
        assert_eq!(
            ClipboardFormatConverter::lines_to_csv("a\nb\nc"),
            "a,b,c"
        );
    }

    #[test]
    fn test_format_converter_csv_to_lines() {
        assert_eq!(
            ClipboardFormatConverter::csv_to_lines("x, y, z"),
            "x\ny\nz"
        );
    }

    #[test]
    fn test_format_converter_snake_case() {
        assert_eq!(
            ClipboardFormatConverter::to_snake_case("clipboardHistory"),
            "clipboard_history"
        );
    }

    #[test]
    fn test_format_converter_camel_case() {
        assert_eq!(
            ClipboardFormatConverter::to_camel_case("clipboard_history"),
            "clipboardHistory"
        );
    }

    // -- ClipboardSyncState tests --

    #[test]
    fn test_sync_state_update() {
        let mut state = ClipboardSyncState::new("sess-1");
        assert_eq!(state.sequence_number(), 0);
        assert!(!state.needs_sync());
        state.update("hello");
        assert_eq!(state.sequence_number(), 1);
        assert!(state.needs_sync());
        assert_eq!(state.last_content.as_deref(), Some("hello"));
    }

    #[test]
    fn test_sync_state_mark_synced() {
        let mut state = ClipboardSyncState::new("sess-2");
        state.update("data");
        assert!(state.needs_sync());
        state.mark_synced();
        assert!(!state.needs_sync());
    }

    // -- ClipboardContentDetector tests --

    #[test]
    fn test_content_detector_json() {
        assert!(ClipboardContentDetector::is_json(r#"{"key":"val"}"#));
        assert!(ClipboardContentDetector::is_json("[1,2,3]"));
        assert!(!ClipboardContentDetector::is_json("just text"));
        assert_eq!(
            ClipboardContentDetector::detect(r#"{"a":1}"#),
            ClipboardContentKind::Json
        );
    }

    #[test]
    fn test_content_detector_url() {
        assert!(ClipboardContentDetector::is_url("https://example.com"));
        assert!(!ClipboardContentDetector::is_url("not a url"));
        assert_eq!(
            ClipboardContentDetector::detect("https://example.com"),
            ClipboardContentKind::Url
        );
    }

    #[test]
    fn test_content_detector_email() {
        assert!(ClipboardContentDetector::is_email("user@example.com"));
        assert!(!ClipboardContentDetector::is_email("noatsign"));
        assert_eq!(
            ClipboardContentDetector::detect("user@example.com"),
            ClipboardContentKind::Email
        );
    }

#[test]
    fn clipboardformatnegotiator_severity_ordering() {
        assert!(ClipboardFormatNegotiatorSeverity::Critical > ClipboardFormatNegotiatorSeverity::High);
        assert!(ClipboardFormatNegotiatorSeverity::High > ClipboardFormatNegotiatorSeverity::Medium);
        assert!(ClipboardFormatNegotiatorSeverity::Medium > ClipboardFormatNegotiatorSeverity::Low);
    }

    #[test]
    fn clipboardformatnegotiator_severity_display() {
        assert_eq!(ClipboardFormatNegotiatorSeverity::Low.to_string(), "low");
        assert_eq!(ClipboardFormatNegotiatorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn clipboardformatnegotiator_entry_creation() {
        let e = ClipboardFormatNegotiatorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ClipboardFormatNegotiatorSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn clipboardformatnegotiator_entry_builder() {
        let e = ClipboardFormatNegotiatorEntry::new("e2", "Entry 2")
            .with_severity(ClipboardFormatNegotiatorSeverity::High)
            .with_detail("some detail")
            .with_format_count(42);
        assert_eq!(e.severity, ClipboardFormatNegotiatorSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.format_count, 42);
    }

    #[test]
    fn clipboardformatnegotiator_entry_enable_disable() {
        let mut e = ClipboardFormatNegotiatorEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn clipboardformatnegotiator_add_and_count() {
        let mut mgr = ClipboardFormatNegotiator::new("test");
        mgr.add(ClipboardFormatNegotiatorEntry::new("a", "A"));
        mgr.add(ClipboardFormatNegotiatorEntry::new("b", "B").with_severity(ClipboardFormatNegotiatorSeverity::High));
        assert_eq!(mgr.format_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn clipboardformatnegotiator_remove() {
        let mut mgr = ClipboardFormatNegotiator::new("test");
        mgr.add(ClipboardFormatNegotiatorEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn clipboardformatnegotiator_capacity() {
        let mut mgr = ClipboardFormatNegotiator::new("test").with_capacity(1);
        assert!(mgr.add(ClipboardFormatNegotiatorEntry::new("a", "A")));
        assert!(!mgr.add(ClipboardFormatNegotiatorEntry::new("b", "B")));
    }

    #[test]
    fn clipboardformatnegotiator_sorted_by_severity() {
        let mut mgr = ClipboardFormatNegotiator::new("test");
        mgr.add(ClipboardFormatNegotiatorEntry::new("lo", "Low"));
        mgr.add(ClipboardFormatNegotiatorEntry::new("hi", "High").with_severity(ClipboardFormatNegotiatorSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ClipboardFormatNegotiatorSeverity::Critical);
    }

    #[test]
    fn clipboardformatnegotiator_summary() {
        let mgr = ClipboardFormatNegotiator::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn clipboardpastetransform_config_defaults() {
        let cfg = ClipboardPasteTransformConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn clipboardpastetransform_item_creation() {
        let item = ClipboardPasteTransformItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn clipboardpastetransform_add_and_get() {
        let mut mgr = ClipboardPasteTransform::new(ClipboardPasteTransformConfig::new("test"));
        mgr.add(ClipboardPasteTransformItem::new("k1", "v1"));
        assert_eq!(mgr.transform_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn clipboardpastetransform_remove_item() {
        let mut mgr = ClipboardPasteTransform::new(ClipboardPasteTransformConfig::new("test"));
        mgr.add(ClipboardPasteTransformItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn clipboardpastetransform_sorted_by_priority() {
        let mut mgr = ClipboardPasteTransform::new(ClipboardPasteTransformConfig::new("test"));
        mgr.add(ClipboardPasteTransformItem::new("lo", "low").with_priority(1));
        mgr.add(ClipboardPasteTransformItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn clipboardpastetransform_items_with_tag() {
        let mut mgr = ClipboardPasteTransform::new(ClipboardPasteTransformConfig::new("test"));
        mgr.add(ClipboardPasteTransformItem::new("a", "1").with_tag("x"));
        mgr.add(ClipboardPasteTransformItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn clipboardpastetransform_report() {
        let mgr = ClipboardPasteTransform::new(ClipboardPasteTransformConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    // -- clipboard additional tests -------------------------------------------

    #[test]
    fn x_clipboard_capabilities_register_and_has() {
        let mut caps = XClipboardCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_clipboard_capabilities_len() {
        let mut caps = XClipboardCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_clipboard_capabilities_intersect() {
        let mut a = XClipboardCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XClipboardCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_clipboard_capabilities_diff() {
        let mut a = XClipboardCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XClipboardCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_clipboard_service_registry_basic() {
        let mut reg = XClipboardServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_clipboard_service_registry_replace() {
        let mut reg = XClipboardServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_clipboard_service_registry_remove() {
        let mut reg = XClipboardServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_clipboard_service_registry_names() {
        let mut reg = XClipboardServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_clipboard_sanitize_path_basic() {
        assert_eq!(x_clipboard_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_clipboard_sanitize_path_backslash() {
        assert_eq!(x_clipboard_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_clipboard_sanitize_path_single() {
        assert_eq!(x_clipboard_sanitize_path("/"), "/");
    }

    #[test]
    fn x_clipboard_capabilities_default() {
        let caps = XClipboardCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_clipboard_capabilities_all() {
        let mut caps = XClipboardCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    // -- clipboard Z-extended tests -----------------------------------------------

    #[test]
    fn z_clipboard_priority_weight() {
        assert_eq!(ZClipboardPriority::Idle.weight(), 0);
        assert_eq!(ZClipboardPriority::Normal.weight(), 2);
        assert_eq!(ZClipboardPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_clipboard_priority_label() {
        assert_eq!(ZClipboardPriority::Low.label(), "low");
        assert_eq!(ZClipboardPriority::High.label(), "high");
    }

    #[test]
    fn z_clipboard_priority_is_elevated() {
        assert!(!ZClipboardPriority::Normal.is_elevated());
        assert!(ZClipboardPriority::High.is_elevated());
        assert!(ZClipboardPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_clipboard_priority_display() {
        assert_eq!(format!("{}", ZClipboardPriority::Idle), "idle");
    }

    #[test]
    fn z_clipboard_priority_all_asc() {
        let all = ZClipboardPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZClipboardPriority::Idle);
        assert_eq!(all[4], ZClipboardPriority::Realtime);
    }

    #[test]
    fn z_clipboard_struct_new() {
        let s = ZClipboardClipboardHistory::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_clipboard_struct_toggled_clone() {
        let s = ZClipboardClipboardHistory::new();
        let t = s.toggled_clone();
        let _ = t.total_copies;
    }

    #[test]
    fn z_clipboard_rolling_hash_deterministic() {
        let h1 = z_clipboard_rolling_hash(b"test");
        let h2 = z_clipboard_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_clipboard_rolling_hash(b"a"), z_clipboard_rolling_hash(b"b"));
    }

    #[test]
    fn z_clipboard_pad_to_basic() {
        assert_eq!(z_clipboard_pad_to("hi", 5), "hi   ");
        assert_eq!(z_clipboard_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_clipboard_is_identifier_basic() {
        assert!(z_clipboard_is_identifier("foo_bar"));
        assert!(z_clipboard_is_identifier("abc123"));
        assert!(!z_clipboard_is_identifier(""));
        assert!(!z_clipboard_is_identifier("has space"));
    }

    #[test]
    fn z_clipboard_levenshtein_basic() {
        assert_eq!(z_clipboard_levenshtein("", ""), 0);
        assert_eq!(z_clipboard_levenshtein("abc", "abc"), 0);
        assert_eq!(z_clipboard_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_clipboard_unique_words_basic() {
        let w = z_clipboard_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_clipboard_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_clipboard_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_clipboard_common_prefix_basic() {
        assert_eq!(z_clipboard_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_clipboard_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_clipboard_struct_clear() {
        let mut s = ZClipboardClipboardHistory::new();
        s.snapshots.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_clipboard_rolling_hash_empty() {
        let h = z_clipboard_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }

}
