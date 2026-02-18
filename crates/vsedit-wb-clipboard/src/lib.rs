//! System clipboard integration.

use std::collections::HashMap;
use std::fmt;

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

/// A ring buffer for cycling through clipboard history.
pub struct ClipboardRing {
    entries: Vec<ClipboardEntry>,
    cursor: Option<usize>,
    max_size: usize,
}

impl ClipboardRing {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            cursor: None,
            max_size,
        }
    }

    /// Push a new entry to the ring. Resets the cursor.
    pub fn push(&mut self, entry: ClipboardEntry) {
        self.entries.push(entry);
        if self.entries.len() > self.max_size {
            self.entries.remove(0);
        }
        self.cursor = None;
    }

    /// Move to the next (more recent) entry and return it.
    pub fn next(&mut self) -> Option<&ClipboardEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.cursor {
            Some(c) => {
                if c + 1 < self.entries.len() {
                    c + 1
                } else {
                    0
                }
            }
            None => self.entries.len() - 1,
        };
        self.cursor = Some(idx);
        Some(&self.entries[idx])
    }

    /// Move to the previous (older) entry and return it.
    pub fn prev(&mut self) -> Option<&ClipboardEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.cursor {
            Some(0) => self.entries.len() - 1,
            Some(c) => c - 1,
            None => self.entries.len() - 1,
        };
        self.cursor = Some(idx);
        Some(&self.entries[idx])
    }

    /// Get the currently selected entry without advancing.
    pub fn current(&self) -> Option<&ClipboardEntry> {
        self.cursor.and_then(|c| self.entries.get(c))
    }

    /// Number of entries in the ring.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Reset the cursor position.
    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }
}

/// Target format for clipboard text formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardFormat {
    Plain,
    Markdown,
    HtmlEscaped,
}

/// Formats clipboard text for different targets.
pub struct ClipboardFormatter;

impl ClipboardFormatter {
    /// Format text according to the target format.
    pub fn format(text: &str, target: ClipboardFormat) -> String {
        match target {
            ClipboardFormat::Plain => text.to_string(),
            ClipboardFormat::Markdown => Self::to_markdown(text),
            ClipboardFormat::HtmlEscaped => Self::to_html_escaped(text),
        }
    }

    fn to_markdown(text: &str) -> String {
        let mut result = String::with_capacity(text.len() + 10);
        result.push_str("```\n");
        result.push_str(text);
        if !text.ends_with('\n') {
            result.push('\n');
        }
        result.push_str("```");
        result
    }

    fn to_html_escaped(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for c in text.chars() {
            match c {
                '&' => result.push_str("&amp;"),
                '<' => result.push_str("&lt;"),
                '>' => result.push_str("&gt;"),
                '"' => result.push_str("&quot;"),
                '\'' => result.push_str("&#39;"),
                _ => result.push(c),
            }
        }
        result
    }
}

/// Computes a simple diff between two clipboard entries.
pub fn clipboard_diff(a: &ClipboardEntry, b: &ClipboardEntry) -> ClipboardDiffResult {
    let a_lines: Vec<&str> = a.text.lines().collect();
    let b_lines: Vec<&str> = b.text.lines().collect();
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut unchanged = 0usize;

    let max_len = a_lines.len().max(b_lines.len());
    for i in 0..max_len {
        match (a_lines.get(i), b_lines.get(i)) {
            (Some(al), Some(bl)) => {
                if al == bl {
                    unchanged += 1;
                } else {
                    removed += 1;
                    added += 1;
                }
            }
            (Some(_), None) => removed += 1,
            (None, Some(_)) => added += 1,
            (None, None) => {}
        }
    }

    ClipboardDiffResult {
        lines_added: added,
        lines_removed: removed,
        lines_unchanged: unchanged,
        text_equal: a.text == b.text,
    }
}

/// Result of comparing two clipboard entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardDiffResult {
    pub lines_added: usize,
    pub lines_removed: usize,
    pub lines_unchanged: usize,
    pub text_equal: bool,
}

impl ClipboardDiffResult {
    /// Whether there are any changes between the two entries.
    pub fn has_changes(&self) -> bool {
        !self.text_equal
    }

    /// Total lines involved in the comparison.
    pub fn total_lines(&self) -> usize {
        self.lines_added + self.lines_removed + self.lines_unchanged
    }
}

impl fmt::Display for ClipboardDiffResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Diff(+{}, -{}, ={})",
            self.lines_added, self.lines_removed, self.lines_unchanged,
        )
    }
}

// ---------------------------------------------------------------------------
// clipboard_monitor — track clipboard changes
// ---------------------------------------------------------------------------

/// A record of a clipboard change event.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardChange {
    pub old_text: Option<String>,
    pub new_text: String,
    pub timestamp: u64,
    pub source_mode: SourceMode,
}

/// Monitor that tracks clipboard changes over time.
pub struct ClipboardMonitor {
    changes: Vec<ClipboardChange>,
    last_text: Option<String>,
    max_changes: usize,
}

impl ClipboardMonitor {
    pub fn new(max_changes: usize) -> Self {
        Self {
            changes: Vec::new(),
            last_text: None,
            max_changes,
        }
    }

    /// Record a new clipboard value. Only records if text actually changed.
    pub fn record(&mut self, text: String, timestamp: u64, source_mode: SourceMode) -> bool {
        if self.last_text.as_deref() == Some(&text) {
            return false;
        }
        let change = ClipboardChange {
            old_text: self.last_text.take(),
            new_text: text.clone(),
            timestamp,
            source_mode,
        };
        self.last_text = Some(text);
        self.changes.push(change);
        if self.changes.len() > self.max_changes {
            self.changes.remove(0);
        }
        true
    }

    /// Number of recorded changes.
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// Get the most recent change.
    pub fn last_change(&self) -> Option<&ClipboardChange> {
        self.changes.last()
    }

    /// Get all recorded changes.
    pub fn changes(&self) -> &[ClipboardChange] {
        &self.changes
    }

    /// Clear all recorded changes.
    pub fn clear(&mut self) {
        self.changes.clear();
        self.last_text = None;
    }

    /// Get changes within a timestamp range (inclusive).
    pub fn changes_in_range(&self, from: u64, to: u64) -> Vec<&ClipboardChange> {
        self.changes
            .iter()
            .filter(|c| c.timestamp >= from && c.timestamp <= to)
            .collect()
    }

    /// Return the frequency of changes (changes per second) over the recorded history.
    pub fn change_frequency(&self) -> f64 {
        if self.changes.len() < 2 {
            return 0.0;
        }
        let first = self.changes.first().unwrap().timestamp;
        let last = self.changes.last().unwrap().timestamp;
        let span = last.saturating_sub(first);
        if span == 0 {
            return 0.0;
        }
        self.changes.len() as f64 / span as f64
    }
}


// ---------------------------------------------------------------------------
// SourceMode helpers
// ---------------------------------------------------------------------------

impl SourceMode {
    /// Returns all source mode variants.
    pub fn all() -> &'static [SourceMode] {
        &[SourceMode::Normal, SourceMode::Visual, SourceMode::VisualLine, SourceMode::VisualBlock]
    }

    /// Parse from a string name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "normal" | "n" => Some(Self::Normal),
            "visual" | "v" => Some(Self::Visual),
            "visual-line" | "vl" | "visualline" => Some(Self::VisualLine),
            "visual-block" | "vb" | "visualblock" => Some(Self::VisualBlock),
            _ => None,
        }
    }

    /// Returns true if this is a visual mode.
    pub fn is_visual(&self) -> bool {
        !matches!(self, SourceMode::Normal)
    }

    /// Returns the mode character (like Vim status line).
    pub fn mode_char(&self) -> char {
        match self {
            SourceMode::Normal => 'N',
            SourceMode::Visual => 'V',
            SourceMode::VisualLine => 'L',
            SourceMode::VisualBlock => 'B',
        }
    }
}

impl Default for SourceMode {
    fn default() -> Self {
        SourceMode::Normal
    }
}

// ---------------------------------------------------------------------------
// ClipboardEntry helpers
// ---------------------------------------------------------------------------

impl ClipboardEntry {
    /// Create a new entry with the current timestamp placeholder (0).
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            timestamp: 0,
            source_mode: SourceMode::Normal,
        }
    }

    /// Create with a specific source mode.
    pub fn with_mode(text: impl Into<String>, mode: SourceMode) -> Self {
        Self {
            text: text.into(),
            timestamp: 0,
            source_mode: mode,
        }
    }

    /// Returns the number of lines in the text.
    pub fn line_count(&self) -> usize {
        self.text.lines().count().max(1)
    }

    /// Returns the character count.
    pub fn char_count(&self) -> usize {
        self.text.len()
    }

    /// Returns true if the text contains multiple lines.
    pub fn is_multiline(&self) -> bool {
        self.text.contains('\n')
    }

    /// Truncate the text to a maximum length, adding "..." if truncated.
    pub fn preview(&self, max_len: usize) -> String {
        if self.text.len() <= max_len {
            self.text.clone()
        } else {
            format!("{}...", &self.text[..max_len.saturating_sub(3)])
        }
    }
}

// ---------------------------------------------------------------------------
// ClipboardSearch — structured history queries
// ---------------------------------------------------------------------------

/// Structured search over clipboard history with substring, regex, and mode filters.
pub struct ClipboardSearch<'a> {
    entries: &'a [ClipboardEntry],
}

impl<'a> ClipboardSearch<'a> {
    /// Create a searcher over a slice of clipboard entries.
    pub fn new(entries: &'a [ClipboardEntry]) -> Self {
        Self { entries }
    }

    /// Build from a `ClipboardService`.
    pub fn from_service(svc: &'a ClipboardService) -> Self {
        Self::new(svc.get_history())
    }

    /// Return entries whose text contains `needle` (case-sensitive).
    pub fn by_substring(&self, needle: &str) -> Vec<&'a ClipboardEntry> {
        self.entries
            .iter()
            .filter(|e| e.text.contains(needle))
            .collect()
    }

    /// Return entries whose text matches a simple glob-like pattern.
    /// Supports `*` (any chars) and `?` (single char).
    pub fn by_glob(&self, pattern: &str) -> Vec<&'a ClipboardEntry> {
        self.entries
            .iter()
            .filter(|e| glob_match(pattern, &e.text))
            .collect()
    }

    /// Return entries that were copied in the given source mode.
    pub fn by_mode(&self, mode: SourceMode) -> Vec<&'a ClipboardEntry> {
        self.entries
            .iter()
            .filter(|e| e.source_mode == mode)
            .collect()
    }

    /// Return entries whose timestamp falls within `[from, to]` inclusive.
    pub fn by_time_range(&self, from: u64, to: u64) -> Vec<&'a ClipboardEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= from && e.timestamp <= to)
            .collect()
    }

    /// Combined filter: optional substring, optional mode, optional time range.
    pub fn query(
        &self,
        substring: Option<&str>,
        mode: Option<SourceMode>,
        from_ts: Option<u64>,
        to_ts: Option<u64>,
    ) -> Vec<&'a ClipboardEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(s) = substring {
                    if !e.text.contains(s) {
                        return false;
                    }
                }
                if let Some(m) = mode {
                    if e.source_mode != m {
                        return false;
                    }
                }
                if let Some(f) = from_ts {
                    if e.timestamp < f {
                        return false;
                    }
                }
                if let Some(t) = to_ts {
                    if e.timestamp > t {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

/// Simple glob matching supporting `*` and `?`.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, &t)
}

fn glob_match_inner(p: &[char], t: &[char]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // '*' matches zero or more characters
            glob_match_inner(&p[1..], t) || (!t.is_empty() && glob_match_inner(p, &t[1..]))
        }
        (Some('?'), Some(_)) => glob_match_inner(&p[1..], &t[1..]),
        (Some(pc), Some(tc)) if pc == tc => glob_match_inner(&p[1..], &t[1..]),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// ClipboardDiffResult — detailed line-based diff hunks
// ---------------------------------------------------------------------------

/// A single line in a diff output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Line present only in the first entry (removed).
    Removed(String),
    /// Line present only in the second entry (added).
    Added(String),
    /// Line present in both entries (context).
    Context(String),
}

impl fmt::Display for DiffLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffLine::Removed(l) => write!(f, "- {l}"),
            DiffLine::Added(l) => write!(f, "+ {l}"),
            DiffLine::Context(l) => write!(f, "  {l}"),
        }
    }
}

/// Compute a detailed line-based diff between two texts, returning each line
/// annotated as added, removed, or context.
pub fn detailed_diff(a: &str, b: &str) -> Vec<DiffLine> {
    let a_lines: Vec<&str> = a.lines().collect();
    let b_lines: Vec<&str> = b.lines().collect();
    let max_len = a_lines.len().max(b_lines.len());
    let mut result = Vec::with_capacity(max_len);
    for i in 0..max_len {
        match (a_lines.get(i), b_lines.get(i)) {
            (Some(al), Some(bl)) if al == bl => {
                result.push(DiffLine::Context(al.to_string()));
            }
            (Some(al), Some(bl)) => {
                result.push(DiffLine::Removed(al.to_string()));
                result.push(DiffLine::Added(bl.to_string()));
            }
            (Some(al), None) => {
                result.push(DiffLine::Removed(al.to_string()));
            }
            (None, Some(bl)) => {
                result.push(DiffLine::Added(bl.to_string()));
            }
            (None, None) => {}
        }
    }
    result
}

/// Format a detailed diff as a unified-style string.
pub fn format_diff(lines: &[DiffLine]) -> String {
    lines
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Enhanced ClipboardStats — entries per mode, most recent timestamp
// ---------------------------------------------------------------------------

impl ClipboardStats {
    /// Compute per-mode entry counts from a service.
    pub fn entries_per_mode(svc: &ClipboardService) -> HashMap<SourceMode, usize> {
        let mut counts = HashMap::new();
        for entry in svc.get_history() {
            *counts.entry(entry.source_mode).or_insert(0) += 1;
        }
        counts
    }

    /// Return the most recent timestamp across all entries, or `None` if empty.
    pub fn most_recent_timestamp(svc: &ClipboardService) -> Option<u64> {
        svc.get_history().iter().map(|e| e.timestamp).max()
    }

    /// Return the average entry length as a floating-point value.
    pub fn avg_bytes_f64(svc: &ClipboardService) -> f64 {
        let entries = svc.get_history();
        if entries.is_empty() {
            return 0.0;
        }
        let total: usize = entries.iter().map(|e| e.text.len()).sum();
        total as f64 / entries.len() as f64
    }
}

impl fmt::Display for ClipboardStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ClipboardStats(entries={}, bytes={}, avg={}, longest={}, shortest={})",
            self.entry_count,
            self.total_bytes,
            self.avg_bytes,
            self.longest_entry_bytes,
            self.shortest_entry_bytes,
        )
    }
}

// ---------------------------------------------------------------------------
// ClipboardExport — serialize / deserialize clipboard history
// ---------------------------------------------------------------------------

/// Serialize clipboard history to a simple text format and parse it back.
///
/// Format per entry (separated by blank lines):
/// ```text
/// @@@ <mode> <timestamp>
/// <text>
/// ```
pub struct ClipboardExport;

impl ClipboardExport {
    /// Serialize a slice of entries to the export text format.
    pub fn serialize(entries: &[ClipboardEntry]) -> String {
        let mut out = String::new();
        for (i, entry) in entries.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&format!(
                "@@@ {} {}\n{}",
                entry.source_mode, entry.timestamp, entry.text
            ));
        }
        out
    }

    /// Parse entries from the export text format.
    /// Returns `None` if the format is invalid.
    pub fn deserialize(input: &str) -> Option<Vec<ClipboardEntry>> {
        if input.is_empty() {
            return Some(Vec::new());
        }
        let mut entries = Vec::new();
        // Split on the header marker
        let chunks: Vec<&str> = input.split("\n@@@").collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk = if i == 0 {
                chunk.strip_prefix("@@@").unwrap_or(chunk)
            } else {
                chunk
            };
            let chunk = chunk.trim_start();
            // First token is mode, second is timestamp, rest is text
            let first_newline = chunk.find('\n')?;
            let header = &chunk[..first_newline];
            let text = &chunk[first_newline + 1..];
            let mut parts = header.splitn(2, ' ');
            let mode_str = parts.next()?;
            let ts_str = parts.next()?;
            let mode = SourceMode::from_name(mode_str)?;
            let timestamp: u64 = ts_str.parse().ok()?;
            entries.push(ClipboardEntry {
                text: text.to_string(),
                timestamp,
                source_mode: mode,
            });
        }
        Some(entries)
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<&str> for ClipboardEntry {
    fn from(s: &str) -> Self {
        ClipboardEntry::new(s)
    }
}

impl From<String> for ClipboardEntry {
    fn from(s: String) -> Self {
        ClipboardEntry {
            text: s,
            timestamp: 0,
            source_mode: SourceMode::Normal,
        }
    }
}

impl From<&str> for SourceMode {
    /// Parse a mode string; defaults to `Normal` on unrecognized input.
    fn from(s: &str) -> Self {
        SourceMode::from_name(s).unwrap_or(SourceMode::Normal)
    }
}

// ---------------------------------------------------------------------------
// Hash impl for SourceMode (needed for HashMap key)
// ---------------------------------------------------------------------------

impl std::hash::Hash for SourceMode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

// ---------------------------------------------------------------------------
// Content type detection
// ---------------------------------------------------------------------------

/// Detected content type of clipboard text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// Plain prose or unknown text.
    PlainText,
    /// Looks like source code (contains braces, semicolons, keywords).
    Code,
    /// Looks like a URL.
    Url,
    /// Looks like a file path (Unix or Windows).
    FilePath,
    /// Looks like a numeric value.
    Numeric,
    /// Looks like whitespace-only or empty.
    Empty,
}

impl ContentType {
    /// Heuristic detection of the content type of a string.
    pub fn detect(text: &str) -> Self {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return ContentType::Empty;
        }
        // Numeric check (integer or float, optionally negative)
        if trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == '_')
            && trimmed.parse::<f64>().is_ok()
        {
            return ContentType::Numeric;
        }
        // URL check
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("ftp://")
            || trimmed.starts_with("ssh://")
        {
            return ContentType::Url;
        }
        // File path check
        if trimmed.starts_with('/')
            || trimmed.starts_with("~/")
            || trimmed.starts_with("./")
            || trimmed.starts_with("../")
            || (trimmed.len() >= 3
                && trimmed.as_bytes()[0].is_ascii_alphabetic()
                && trimmed.as_bytes()[1] == b':'
                && (trimmed.as_bytes()[2] == b'\\' || trimmed.as_bytes()[2] == b'/'))
        {
            return ContentType::FilePath;
        }
        // Code heuristic: look for common programming patterns
        let has_code_chars = trimmed.contains('{')
            || trimmed.contains('}')
            || trimmed.contains(';')
            || trimmed.contains("fn ")
            || trimmed.contains("let ")
            || trimmed.contains("if ")
            || trimmed.contains("def ")
            || trimmed.contains("class ")
            || trimmed.contains("import ");
        if has_code_chars {
            return ContentType::Code;
        }
        ContentType::PlainText
    }

    /// A short human-readable label for the content type.
    pub fn label(&self) -> &'static str {
        match self {
            ContentType::PlainText => "text",
            ContentType::Code => "code",
            ContentType::Url => "url",
            ContentType::FilePath => "path",
            ContentType::Numeric => "number",
            ContentType::Empty => "empty",
        }
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// Clipboard text normalization & transformation utilities
// ---------------------------------------------------------------------------

impl ClipboardTransform {
    /// Normalize all line endings to Unix-style `\n`.
    pub fn normalize_newlines(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Collapse runs of multiple blank lines into a single blank line.
    pub fn collapse_blank_lines(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut prev_blank = false;
        for line in text.split('\n') {
            let blank = line.trim().is_empty();
            if blank && prev_blank {
                continue;
            }
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
            prev_blank = blank;
        }
        result
    }

    /// Remove leading common indentation from all lines (dedent).
    pub fn dedent(text: &str) -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);
        lines
            .iter()
            .map(|l| {
                if l.len() >= min_indent {
                    &l[min_indent..]
                } else {
                    l.trim()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Re-indent text to use the given indentation string per level.
    /// Replaces leading whitespace proportionally based on the smallest
    /// non-zero indent found in the original text.
    pub fn reindent(text: &str, indent_str: &str) -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let base = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .filter(|&n| n > 0)
            .min()
            .unwrap_or(1);
        lines
            .iter()
            .map(|l| {
                let spaces = l.len() - l.trim_start().len();
                let level = spaces / base;
                let new_indent = indent_str.repeat(level);
                format!("{}{}", new_indent, l.trim_start())
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Convert tabs to spaces using the given tab width.
    pub fn tabs_to_spaces(text: &str, tab_width: usize) -> String {
        text.replace('\t', &" ".repeat(tab_width))
    }

    /// Convert leading spaces to tabs using the given tab width.
    pub fn spaces_to_tabs(text: &str, tab_width: usize) -> String {
        if tab_width == 0 {
            return text.to_string();
        }
        text.split('\n')
            .map(|line| {
                let spaces = line.len() - line.trim_start_matches(' ').len();
                let tabs = spaces / tab_width;
                let remaining = spaces % tab_width;
                let mut s = String::with_capacity(line.len());
                for _ in 0..tabs {
                    s.push('\t');
                }
                for _ in 0..remaining {
                    s.push(' ');
                }
                s.push_str(&line[spaces..]);
                s
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Wrap text at the specified column width, breaking at word boundaries.
    pub fn word_wrap(text: &str, width: usize) -> String {
        if width == 0 {
            return text.to_string();
        }
        let mut result = String::with_capacity(text.len() + text.len() / width);
        for paragraph in text.split('\n') {
            if !result.is_empty() {
                result.push('\n');
            }
            let mut col = 0usize;
            for (i, word) in paragraph.split_whitespace().enumerate() {
                if i > 0 && col + 1 + word.len() > width {
                    result.push('\n');
                    col = 0;
                } else if i > 0 {
                    result.push(' ');
                    col += 1;
                }
                result.push_str(word);
                col += word.len();
            }
        }
        result
    }

    /// Extract only lines matching a substring.
    pub fn grep_lines(text: &str, pattern: &str) -> String {
        text.split('\n')
            .filter(|l| l.contains(pattern))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Remove lines matching a substring (inverse grep).
    pub fn grep_v_lines(text: &str, pattern: &str) -> String {
        text.split('\n')
            .filter(|l| !l.contains(pattern))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Join all lines into a single line with the given separator.
    pub fn join_lines(text: &str, sep: &str) -> String {
        text.split('\n')
            .map(|l| l.trim_end_matches('\r'))
            .collect::<Vec<_>>()
            .join(sep)
    }

    /// Convert text to upper case.
    pub fn to_upper(text: &str) -> String {
        text.to_uppercase()
    }

    /// Convert text to lower case.
    pub fn to_lower(text: &str) -> String {
        text.to_lowercase()
    }
}

// ---------------------------------------------------------------------------
// Clipboard size limiter
// ---------------------------------------------------------------------------

/// Enforces size limits on clipboard content.
pub struct ClipboardSizeLimiter {
    max_bytes: usize,
    max_lines: usize,
}

impl ClipboardSizeLimiter {
    pub fn new(max_bytes: usize, max_lines: usize) -> Self {
        Self {
            max_bytes,
            max_lines,
        }
    }

    /// Check whether the text exceeds configured limits.
    pub fn exceeds_limit(&self, text: &str) -> bool {
        text.len() > self.max_bytes || text.split('\n').count() > self.max_lines
    }

    /// Truncate text to fit within byte and line limits.
    /// Returns the truncated text and whether truncation occurred.
    pub fn truncate(&self, text: &str) -> (String, bool) {
        let mut lines: Vec<&str> = text.split('\n').collect();
        let mut truncated = false;
        if lines.len() > self.max_lines {
            lines.truncate(self.max_lines);
            truncated = true;
        }
        let mut result = lines.join("\n");
        if result.len() > self.max_bytes {
            // Truncate at a char boundary
            let mut end = self.max_bytes;
            while end > 0 && !result.is_char_boundary(end) {
                end -= 1;
            }
            result.truncate(end);
            truncated = true;
        }
        (result, truncated)
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn max_lines(&self) -> usize {
        self.max_lines
    }
}

// ---------------------------------------------------------------------------
// Paste indentation adjuster
// ---------------------------------------------------------------------------

/// Adjusts indentation when pasting text into an editor context.
pub struct PasteIndenter;

impl PasteIndenter {
    /// Adjust the indentation of pasted text so that its first line aligns
    /// with `target_indent`, and subsequent lines are shifted by the same
    /// amount.
    pub fn adjust(text: &str, target_indent: &str) -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            return String::new();
        }
        let first_indent_len = lines[0].len() - lines[0].trim_start().len();
        lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    format!("{}{}", target_indent, line.trim_start())
                } else {
                    let cur_indent = line.len() - line.trim_start().len();
                    let extra = cur_indent.saturating_sub(first_indent_len);
                    let extra_spaces = " ".repeat(extra);
                    format!("{}{}{}", target_indent, extra_spaces, line.trim_start())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Strip all leading indentation from every line.
    pub fn flatten(text: &str) -> String {
        text.split('\n')
            .map(|l| l.trim_start())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---------------------------------------------------------------------------
// Clipboard metadata
// ---------------------------------------------------------------------------

/// Rich metadata about a clipboard entry, computed lazily.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipboardMetadata {
    pub content_type: ContentType,
    pub byte_count: usize,
    pub char_count: usize,
    pub line_count: usize,
    pub word_count: usize,
    pub is_multiline: bool,
    pub has_trailing_newline: bool,
}

impl ClipboardMetadata {
    /// Compute metadata from raw text.
    pub fn from_text(text: &str) -> Self {
        Self {
            content_type: ContentType::detect(text),
            byte_count: text.len(),
            char_count: text.chars().count(),
            line_count: if text.is_empty() {
                0
            } else {
                text.split('\n').count()
            },
            word_count: text.split_whitespace().count(),
            is_multiline: text.contains('\n'),
            has_trailing_newline: text.ends_with('\n'),
        }
    }

    /// Compute metadata from a clipboard entry.
    pub fn from_entry(entry: &ClipboardEntry) -> Self {
        Self::from_text(&entry.text)
    }
}

impl fmt::Display for ClipboardMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}B, {} chars, {} lines, {} words",
            self.content_type, self.byte_count, self.char_count, self.line_count, self.word_count
        )
    }
}

// ---------------------------------------------------------------------------
// Clipboard batch operations on ClipboardService
// ---------------------------------------------------------------------------

impl ClipboardService {
    /// Write multiple texts at once, assigning sequential timestamps starting
    /// from `base_timestamp`.
    pub fn write_batch(
        &mut self,
        texts: &[&str],
        base_timestamp: u64,
        source_mode: SourceMode,
    ) {
        for (i, text) in texts.iter().enumerate() {
            self.write_entry((*text).to_string(), base_timestamp + i as u64, source_mode);
        }
    }

    /// Keep only entries whose text satisfies a predicate.
    pub fn retain<F: Fn(&ClipboardEntry) -> bool>(&mut self, pred: F) {
        self.history.retain(|e| pred(e));
    }

    /// Replace all occurrences of `from` with `to` across the entire history.
    /// Returns the number of entries that were modified.
    pub fn replace_in_history(&mut self, from: &str, to: &str) -> usize {
        let mut count = 0;
        for entry in &mut self.history {
            if entry.text.contains(from) {
                entry.text = entry.text.replace(from, to);
                count += 1;
            }
        }
        count
    }

    /// Return the n most recent entries (newest first).
    pub fn recent(&self, n: usize) -> Vec<&ClipboardEntry> {
        self.history.iter().rev().take(n).collect()
    }

    /// Merge the text of all history entries into a single string, separated
    /// by the given separator.
    pub fn merge_history(&self, separator: &str) -> String {
        self.history
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join(separator)
    }
}

// ---------------------------------------------------------------------------
// ClipboardHistory
// ---------------------------------------------------------------------------

/// Ring buffer of clipboard entries.
#[derive(Debug, Clone)]
pub struct ClipboardHistory {
    entries: Vec<String>,
    max_entries: usize,
}

impl ClipboardHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn push(&mut self, text: String) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(text);
    }

    pub fn get_at(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn recent(&self, count: usize) -> Vec<&str> {
        self.entries.iter().rev().take(count).map(|s| s.as_str()).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    pub fn contains(&self, text: &str) -> bool {
        self.entries.iter().any(|e| e == text)
    }

    pub fn remove_at(&mut self, index: usize) -> Option<String> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    pub fn deduplicate(&mut self) {
        let mut seen = Vec::new();
        self.entries.retain(|e| {
            if seen.contains(e) {
                false
            } else {
                seen.push(e.clone());
                true
            }
        });
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ClipboardTransformer
// ---------------------------------------------------------------------------

/// Transform clipboard text.
pub struct ClipboardTransformer;

impl ClipboardTransformer {
    pub fn trim(text: &str) -> String {
        text.trim().to_string()
    }

    pub fn collapse_whitespace(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut prev_ws = false;
        for ch in text.chars() {
            if ch.is_whitespace() {
                if !prev_ws {
                    result.push(' ');
                }
                prev_ws = true;
            } else {
                result.push(ch);
                prev_ws = false;
            }
        }
        result
    }

    pub fn escape_html(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    pub fn unescape_html(text: &str) -> String {
        text.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
    }

    pub fn to_single_line(text: &str) -> String {
        text.lines().collect::<Vec<_>>().join(" ")
    }

    pub fn normalize_newlines(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }
}

// ---------------------------------------------------------------------------
// ClipboardMetadata
// ---------------------------------------------------------------------------

/// Metadata attached to a clipboard entry.
#[derive(Debug, Clone)]
pub struct ClipboardMetadataV2 {
    pub source_file: Option<String>,
    pub line: Option<u32>,
    pub timestamp: u64,
    pub is_whole_line: bool,
    pub language: Option<String>,
}

impl ClipboardMetadataV2 {
    pub fn new(timestamp: u64) -> Self {
        Self {
            source_file: None,
            line: None,
            timestamp,
            is_whole_line: false,
            language: None,
        }
    }

    pub fn with_source(mut self, file: &str, line: u32) -> Self {
        self.source_file = Some(file.to_string());
        self.line = Some(line);
        self
    }

    pub fn with_language(mut self, lang: &str) -> Self {
        self.language = Some(lang.to_string());
        self
    }

    pub fn matches_filter(&self, language: Option<&str>, file_pattern: Option<&str>) -> bool {
        if let Some(lang) = language {
            if self.language.as_deref() != Some(lang) {
                return false;
            }
        }
        if let Some(pattern) = file_pattern {
            if let Some(ref src) = self.source_file {
                if !src.contains(pattern) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}


// ---------------------------------------------------------------------------
// wb_clipboard – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbClipboardLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbClipboardPanelState {
    pub region: XWbClipboardLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbClipboardPanelState {
    pub fn new(region: XWbClipboardLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_clipboard_total_visible_area(panels: &[XWbClipboardPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_clipboard_count_in_region(
    panels: &[XWbClipboardPanelState],
    region: XWbClipboardLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_clipboard_widest_panel(panels: &[XWbClipboardPanelState]) -> Option<&XWbClipboardPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_clipboard_collapse_region(
    panels: &mut [XWbClipboardPanelState],
    region: XWbClipboardLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbClipboardLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbClipboardLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// wb_clipboard – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbClipboardClipboardEntryKind {
    Text,
    Image,
    File,
    Rich,
}

impl YWbClipboardClipboardEntryKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Text => 0,
            Self::Image => 1,
            Self::File => 2,
            Self::Rich => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Image => "Image",
            Self::File => "File",
            Self::Rich => "Rich",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbClipboardClipboardEntryKind] {
        &[
            YWbClipboardClipboardEntryKind::Text,
            YWbClipboardClipboardEntryKind::Image,
            YWbClipboardClipboardEntryKind::File,
            YWbClipboardClipboardEntryKind::Rich,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbClipboardClipboardEntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks clipboard stack data.
#[derive(Debug, Clone)]
pub struct YWbClipboardClipboardStack {
    pub entries: Vec<String>,
    pub capacity: usize,
    pub write_count: u64,
}

impl YWbClipboardClipboardStack {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            capacity: 0,
            write_count: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbClipboardClipboardStack({}: {:?})", "entries", self.entries)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_clipboard_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_clipboard_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_clipboard_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_clipboard_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_clipboard_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_clipboard_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_clipboard_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_clipboard_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_clipboard – Extended clipboard transform helpers
// ---------------------------------------------------------------------------

/// Priority levels for clipboard transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbClipboardPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbClipboardPriority {
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
    pub fn all_asc() -> [ZWbClipboardPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbClipboardPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks clipboard transform data.
#[derive(Debug, Clone)]
pub struct ZWbClipboardClipboardTransform {
    pub rules: Vec<(String, String)>,
    pub chain_count: usize,
    pub reversible: bool,
}

impl ZWbClipboardClipboardTransform {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            chain_count: 0,
            reversible: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.rules.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbClipboardClipboardTransform[chain_count={:?}, reversible={:?}]", self.chain_count, self.reversible)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.reversible = !c.reversible;
        c
    }
}

/// Compute a simple rolling hash for clipboard transform.
pub fn z_wb_clipboard_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_clipboard_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_clipboard_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_clipboard_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_wb_clipboard_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_clipboard_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_clipboard_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 100
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer100 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer100 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_100(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_100<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_100<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_100(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_100(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 203
// ---------------------------------------------------------------------------

/// Generic object pool `Xc203Pool<T>`.
pub struct Xc203Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc203Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc203PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc203Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc203PoolStats {
        Xc203PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc203Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc203Scheduler`.
pub struct Xc203Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc203Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc203Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_203 hash for the given byte slice.
pub fn xc_203_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_203 convention.
pub fn xc_203_reverse(s: &str) -> String {
    s.chars().rev().collect()
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
    fn clear_history_works() {
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
    fn get_history_by_mode_works() {
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
    fn total_text_size_works() {
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
    fn undo_last_write_works() {
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

    #[test]
    fn clipboard_ring_cycle() {
        let mut ring = ClipboardRing::new(3);
        ring.push(ClipboardEntry { text: "a".into(), timestamp: 1, source_mode: SourceMode::Normal });
        ring.push(ClipboardEntry { text: "b".into(), timestamp: 2, source_mode: SourceMode::Normal });
        ring.push(ClipboardEntry { text: "c".into(), timestamp: 3, source_mode: SourceMode::Normal });

        let e = ring.next().unwrap();
        assert_eq!(e.text, "c");
        let e = ring.next().unwrap();
        assert_eq!(e.text, "a");
        let e = ring.next().unwrap();
        assert_eq!(e.text, "b");
    }

    #[test]
    fn clipboard_ring_prev() {
        let mut ring = ClipboardRing::new(3);
        ring.push(ClipboardEntry { text: "x".into(), timestamp: 1, source_mode: SourceMode::Normal });
        ring.push(ClipboardEntry { text: "y".into(), timestamp: 2, source_mode: SourceMode::Normal });
        let e = ring.prev().unwrap();
        assert_eq!(e.text, "y");
        let e = ring.prev().unwrap();
        assert_eq!(e.text, "x");
        let e = ring.prev().unwrap();
        assert_eq!(e.text, "y");
    }

    #[test]
    fn clipboard_ring_max_size() {
        let mut ring = ClipboardRing::new(2);
        ring.push(ClipboardEntry { text: "a".into(), timestamp: 1, source_mode: SourceMode::Normal });
        ring.push(ClipboardEntry { text: "b".into(), timestamp: 2, source_mode: SourceMode::Normal });
        ring.push(ClipboardEntry { text: "c".into(), timestamp: 3, source_mode: SourceMode::Normal });
        assert_eq!(ring.len(), 2);
    }

    #[test]
    fn formatter_html_escape() {
        let text = "<div class=\"test\">hello & 'world'</div>";
        let escaped = ClipboardFormatter::format(text, ClipboardFormat::HtmlEscaped);
        assert_eq!(escaped, "&lt;div class=&quot;test&quot;&gt;hello &amp; &#39;world&#39;&lt;/div&gt;");
    }

    #[test]
    fn formatter_markdown() {
        let text = "fn main() {}";
        let md = ClipboardFormatter::format(text, ClipboardFormat::Markdown);
        assert!(md.starts_with("```\n"));
        assert!(md.ends_with("\n```"));
        assert!(md.contains("fn main() {}"));
    }

    #[test]
    fn diff_entries() {
        let a = ClipboardEntry { text: "line1\nline2\nline3".into(), timestamp: 1, source_mode: SourceMode::Normal };
        let b = ClipboardEntry { text: "line1\nchanged\nline3".into(), timestamp: 2, source_mode: SourceMode::Normal };
        let diff = clipboard_diff(&a, &b);
        assert!(diff.has_changes());
        assert_eq!(diff.lines_unchanged, 2);
        assert_eq!(diff.lines_added, 1);
        assert_eq!(diff.lines_removed, 1);
    }

    #[test]
    fn diff_equal() {
        let a = ClipboardEntry { text: "same".into(), timestamp: 1, source_mode: SourceMode::Normal };
        let b = ClipboardEntry { text: "same".into(), timestamp: 2, source_mode: SourceMode::Normal };
        let diff = clipboard_diff(&a, &b);
        assert!(!diff.has_changes());
        assert!(diff.text_equal);
    }

    // -- clipboard_monitor tests --------------------------------------------

    #[test]
    fn monitor_records_change() {
        let mut mon = ClipboardMonitor::new(10);
        let recorded = mon.record("hello".into(), 1, SourceMode::Normal);
        assert!(recorded);
        assert_eq!(mon.change_count(), 1);
    }

    #[test]
    fn monitor_skips_duplicate() {
        let mut mon = ClipboardMonitor::new(10);
        mon.record("hello".into(), 1, SourceMode::Normal);
        let recorded = mon.record("hello".into(), 2, SourceMode::Normal);
        assert!(!recorded);
        assert_eq!(mon.change_count(), 1);
    }

    #[test]
    fn monitor_tracks_old_text() {
        let mut mon = ClipboardMonitor::new(10);
        mon.record("first".into(), 1, SourceMode::Normal);
        mon.record("second".into(), 2, SourceMode::Normal);
        let last = mon.last_change().unwrap();
        assert_eq!(last.old_text.as_deref(), Some("first"));
        assert_eq!(last.new_text, "second");
    }

    #[test]
    fn monitor_respects_max_changes() {
        let mut mon = ClipboardMonitor::new(2);
        mon.record("a".into(), 1, SourceMode::Normal);
        mon.record("b".into(), 2, SourceMode::Normal);
        mon.record("c".into(), 3, SourceMode::Normal);
        assert_eq!(mon.change_count(), 2);
        assert_eq!(mon.changes()[0].new_text, "b");
    }

    #[test]
    fn monitor_changes_in_range() {
        let mut mon = ClipboardMonitor::new(10);
        mon.record("a".into(), 10, SourceMode::Normal);
        mon.record("b".into(), 20, SourceMode::Normal);
        mon.record("c".into(), 30, SourceMode::Normal);
        let range = mon.changes_in_range(15, 25);
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].new_text, "b");
    }

    #[test]
    fn monitor_clear() {
        let mut mon = ClipboardMonitor::new(10);
        mon.record("test".into(), 1, SourceMode::Normal);
        mon.clear();
        assert_eq!(mon.change_count(), 0);
        assert!(mon.last_change().is_none());
    }

    #[test]
    fn monitor_change_frequency() {
        let mut mon = ClipboardMonitor::new(10);
        mon.record("a".into(), 0, SourceMode::Normal);
        mon.record("b".into(), 10, SourceMode::Normal);
        mon.record("c".into(), 20, SourceMode::Normal);
        let freq = mon.change_frequency();
        assert!((freq - 0.15).abs() < 0.01);
    }

    #[test]
    fn test_source_mode_all() {
        assert_eq!(SourceMode::all().len(), 4);
    }

    #[test]
    fn test_source_mode_from_name() {
        assert_eq!(SourceMode::from_name("normal"), Some(SourceMode::Normal));
        assert_eq!(SourceMode::from_name("v"), Some(SourceMode::Visual));
        assert_eq!(SourceMode::from_name("vb"), Some(SourceMode::VisualBlock));
        assert_eq!(SourceMode::from_name("bogus"), None);
    }

    #[test]
    fn test_source_mode_is_visual() {
        assert!(!SourceMode::Normal.is_visual());
        assert!(SourceMode::Visual.is_visual());
        assert!(SourceMode::VisualBlock.is_visual());
    }

    #[test]
    fn test_source_mode_display_and_default() {
        assert_eq!(format!("{}", SourceMode::Visual), "Visual");
        assert_eq!(SourceMode::default(), SourceMode::Normal);
    }

    #[test]
    fn test_source_mode_char() {
        assert_eq!(SourceMode::Normal.mode_char(), 'N');
        assert_eq!(SourceMode::VisualBlock.mode_char(), 'B');
    }

    #[test]
    fn test_clipboard_entry_new() {
        let e = ClipboardEntry::new("hello");
        assert_eq!(e.text, "hello");
        assert_eq!(e.source_mode, SourceMode::Normal);
        assert_eq!(e.char_count(), 5);
        assert_eq!(e.line_count(), 1);
        assert!(!e.is_multiline());
    }

    #[test]
    fn test_clipboard_entry_multiline() {
        let e = ClipboardEntry::new("line1\nline2\nline3");
        assert!(e.is_multiline());
        assert_eq!(e.line_count(), 3);
    }

    #[test]
    fn test_clipboard_entry_preview() {
        let e = ClipboardEntry::new("a".repeat(100));
        let p = e.preview(20);
        assert!(p.len() <= 20);
        assert!(p.ends_with("..."));
    }

    #[test]
    fn test_clipboard_entry_display_with_mode() {
        let e = ClipboardEntry::with_mode("test", SourceMode::Visual);
        let s = format!("{e}");
        assert!(s.contains("Visual"));
        assert!(s.contains("test"));
    }

    #[test]
    fn test_clipboard_stats_from_entries() {
        let entries = vec![
            ClipboardEntry::new("hello"),
            ClipboardEntry::new("multi\nline"),
            ClipboardEntry::with_mode("visual", SourceMode::Visual),
        ];
        assert_eq!(entries.len(), 3);
        assert_eq!(entries.iter().filter(|e| e.is_multiline()).count(), 1);
    }

    // -----------------------------------------------------------------------
    // New tests for ClipboardSearch, detailed_diff, ClipboardStats extensions,
    // ClipboardExport, From impls, and glob matching
    // -----------------------------------------------------------------------

    #[test]
    fn search_by_substring() {
        let mut svc = ClipboardService::new(10);
        svc.write_entry("fn main() {}".into(), 1, SourceMode::Normal);
        svc.write_entry("let x = 42;".into(), 2, SourceMode::Visual);
        svc.write_entry("fn helper() {}".into(), 3, SourceMode::Normal);
        let search = ClipboardSearch::from_service(&svc);
        let results = search.by_substring("fn ");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "fn main() {}");
        assert_eq!(results[1].text, "fn helper() {}");
    }

    #[test]
    fn search_by_glob_pattern() {
        let entries = vec![
            ClipboardEntry::new("hello world"),
            ClipboardEntry::new("hello rust"),
            ClipboardEntry::new("goodbye world"),
        ];
        let search = ClipboardSearch::new(&entries);
        let results = search.by_glob("hello*");
        assert_eq!(results.len(), 2);
        let results2 = search.by_glob("*world");
        assert_eq!(results2.len(), 2);
        let results3 = search.by_glob("hello ?ust");
        assert_eq!(results3.len(), 1);
    }

    #[test]
    fn search_combined_query() {
        let mut svc = ClipboardService::new(10);
        svc.write_entry("alpha".into(), 10, SourceMode::Normal);
        svc.write_entry("alpha beta".into(), 20, SourceMode::Visual);
        svc.write_entry("gamma".into(), 30, SourceMode::Normal);
        let search = ClipboardSearch::from_service(&svc);
        // Filter by substring + mode + time range
        let results = search.query(
            Some("alpha"),
            Some(SourceMode::Normal),
            Some(5),
            Some(25),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "alpha");
    }

    #[test]
    fn detailed_diff_lines() {
        let a = "line1\nline2\nline3";
        let b = "line1\nmodified\nline3\nline4";
        let diff = detailed_diff(a, b);
        assert_eq!(diff[0], DiffLine::Context("line1".into()));
        assert_eq!(diff[1], DiffLine::Removed("line2".into()));
        assert_eq!(diff[2], DiffLine::Added("modified".into()));
        assert_eq!(diff[3], DiffLine::Context("line3".into()));
        assert_eq!(diff[4], DiffLine::Added("line4".into()));
        let formatted = format_diff(&diff);
        assert!(formatted.contains("- line2"));
        assert!(formatted.contains("+ modified"));
        assert!(formatted.contains("+ line4"));
    }

    #[test]
    fn clipboard_export_roundtrip() {
        let entries = vec![
            ClipboardEntry {
                text: "hello world".into(),
                timestamp: 100,
                source_mode: SourceMode::Normal,
            },
            ClipboardEntry {
                text: "multi\nline\ntext".into(),
                timestamp: 200,
                source_mode: SourceMode::Visual,
            },
        ];
        let serialized = ClipboardExport::serialize(&entries);
        let deserialized = ClipboardExport::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].text, "hello world");
        assert_eq!(deserialized[0].timestamp, 100);
        assert_eq!(deserialized[0].source_mode, SourceMode::Normal);
        assert_eq!(deserialized[1].text, "multi\nline\ntext");
        assert_eq!(deserialized[1].timestamp, 200);
        assert_eq!(deserialized[1].source_mode, SourceMode::Visual);
    }

    #[test]
    fn clipboard_stats_per_mode_and_timestamp() {
        let mut svc = ClipboardService::new(10);
        svc.write_entry("a".into(), 10, SourceMode::Normal);
        svc.write_entry("bb".into(), 20, SourceMode::Visual);
        svc.write_entry("ccc".into(), 30, SourceMode::Normal);
        let per_mode = ClipboardStats::entries_per_mode(&svc);
        assert_eq!(per_mode[&SourceMode::Normal], 2);
        assert_eq!(per_mode[&SourceMode::Visual], 1);
        assert_eq!(ClipboardStats::most_recent_timestamp(&svc), Some(30));
        let avg = ClipboardStats::avg_bytes_f64(&svc);
        assert!((avg - 2.0).abs() < 0.01);
        let stats = ClipboardStats::from_service(&svc);
        let display = format!("{stats}");
        assert!(display.contains("entries=3"));
    }

    #[test]
    fn from_impls_for_entry_and_mode() {
        let entry: ClipboardEntry = "quick text".into();
        assert_eq!(entry.text, "quick text");
        assert_eq!(entry.source_mode, SourceMode::Normal);
        let entry2: ClipboardEntry = String::from("owned").into();
        assert_eq!(entry2.text, "owned");
        let mode: SourceMode = "visual".into();
        assert_eq!(mode, SourceMode::Visual);
        let mode2: SourceMode = "unknown".into();
        assert_eq!(mode2, SourceMode::Normal);
    }

    // -----------------------------------------------------------------------
    // Tests for new functionality
    // -----------------------------------------------------------------------

    #[test]
    fn content_type_detection() {
        assert_eq!(ContentType::detect(""), ContentType::Empty);
        assert_eq!(ContentType::detect("   "), ContentType::Empty);
        assert_eq!(ContentType::detect("42"), ContentType::Numeric);
        assert_eq!(ContentType::detect("-3.14"), ContentType::Numeric);
        assert_eq!(ContentType::detect("https://example.com"), ContentType::Url);
        assert_eq!(ContentType::detect("ftp://files.example.com"), ContentType::Url);
        assert_eq!(ContentType::detect("/usr/bin/ls"), ContentType::FilePath);
        assert_eq!(ContentType::detect("~/Documents/file.txt"), ContentType::FilePath);
        assert_eq!(ContentType::detect("fn main() { }"), ContentType::Code);
        assert_eq!(ContentType::detect("let x = 5;"), ContentType::Code);
        assert_eq!(ContentType::detect("hello world"), ContentType::PlainText);
        // Display
        assert_eq!(ContentType::Code.label(), "code");
        assert_eq!(format!("{}", ContentType::Url), "url");
    }

    #[test]
    fn normalize_newlines_works() {
        assert_eq!(
            ClipboardTransform::normalize_newlines("a\r\nb\rc\nd"),
            "a\nb\nc\nd"
        );
    }

    #[test]
    fn collapse_blank_lines_works() {
        let input = "a\n\n\n\nb\n\nc";
        assert_eq!(
            ClipboardTransform::collapse_blank_lines(input),
            "a\n\nb\n\nc"
        );
    }

    #[test]
    fn dedent_text() {
        let input = "    fn foo() {\n        bar();\n    }";
        assert_eq!(
            ClipboardTransform::dedent(input),
            "fn foo() {\n    bar();\n}"
        );
    }

    #[test]
    fn reindent_with_tabs() {
        let input = "fn foo() {\n    bar();\n}";
        let result = ClipboardTransform::reindent(input, "\t");
        assert_eq!(result, "fn foo() {\n\tbar();\n}");
    }

    #[test]
    fn tabs_to_spaces_and_back() {
        let input = "\tif true {\n\t\treturn;\n\t}";
        let spaces = ClipboardTransform::tabs_to_spaces(input, 4);
        assert!(spaces.starts_with("    if true"));
        assert!(spaces.contains("        return;"));
        let back = ClipboardTransform::spaces_to_tabs(&spaces, 4);
        assert_eq!(back, input);
    }

    #[test]
    fn word_wrap_at_boundary() {
        let input = "the quick brown fox jumps over the lazy dog";
        let wrapped = ClipboardTransform::word_wrap(input, 20);
        for line in wrapped.split('\n') {
            assert!(line.len() <= 20, "line too long: '{line}'");
        }
        // All words are preserved
        assert_eq!(
            wrapped.split_whitespace().count(),
            input.split_whitespace().count()
        );
    }

    #[test]
    fn grep_and_grep_v_lines() {
        let input = "apple\nbanana\napricot\ncherry";
        assert_eq!(ClipboardTransform::grep_lines(input, "ap"), "apple\napricot");
        assert_eq!(
            ClipboardTransform::grep_v_lines(input, "ap"),
            "banana\ncherry"
        );
    }

    #[test]
    fn join_lines_with_separator() {
        assert_eq!(
            ClipboardTransform::join_lines("a\nb\nc", " | "),
            "a | b | c"
        );
    }

    #[test]
    fn to_upper_and_lower() {
        assert_eq!(ClipboardTransform::to_upper("Hello"), "HELLO");
        assert_eq!(ClipboardTransform::to_lower("Hello"), "hello");
    }

    #[test]
    fn size_limiter_truncation() {
        let limiter = ClipboardSizeLimiter::new(10, 100);
        assert!(!limiter.exceeds_limit("short"));
        assert!(limiter.exceeds_limit("this is a long string"));
        let (truncated, did_truncate) = limiter.truncate("this is a long string");
        assert!(did_truncate);
        assert!(truncated.len() <= 10);
        // Line limit
        let line_limiter = ClipboardSizeLimiter::new(10000, 3);
        let many_lines = "a\nb\nc\nd\ne";
        assert!(line_limiter.exceeds_limit(many_lines));
        let (truncated, did_truncate) = line_limiter.truncate(many_lines);
        assert!(did_truncate);
        assert_eq!(truncated.split('\n').count(), 3);
    }

    #[test]
    fn paste_indenter_adjust() {
        let code = "if true {\n    return 1;\n}";
        let adjusted = PasteIndenter::adjust(code, "        ");
        let lines: Vec<&str> = adjusted.split('\n').collect();
        assert!(lines[0].starts_with("        if true"));
        assert!(lines[1].starts_with("        "));
        assert!(lines[1].contains("return 1;"));
        // Extra indentation of line 2 (4 spaces relative to line 1) is preserved
        let indent1 = lines[0].len() - lines[0].trim_start().len();
        let indent2 = lines[1].len() - lines[1].trim_start().len();
        assert!(indent2 > indent1);
    }

    #[test]
    fn paste_indenter_flatten() {
        let indented = "    a\n        b\n    c";
        assert_eq!(PasteIndenter::flatten(indented), "a\nb\nc");
    }

    #[test]
    fn clipboard_metadata_from_text() {
        let meta = ClipboardMetadata::from_text("fn main() {\n    println!(\"hi\");\n}\n");
        assert_eq!(meta.content_type, ContentType::Code);
        assert_eq!(meta.line_count, 4);
        assert!(meta.is_multiline);
        assert!(meta.has_trailing_newline);
        assert!(meta.word_count > 0);
        let display = format!("{meta}");
        assert!(display.contains("code"));
    }

    #[test]
    fn clipboard_metadata_from_entry() {
        let entry = ClipboardEntry::new("https://example.com");
        let meta = ClipboardMetadata::from_entry(&entry);
        assert_eq!(meta.content_type, ContentType::Url);
        assert!(!meta.is_multiline);
    }

    #[test]
    fn service_write_batch() {
        let mut svc = ClipboardService::new(10);
        svc.write_batch(&["a", "b", "c"], 100, SourceMode::Visual);
        assert_eq!(svc.history_count(), 3);
        assert_eq!(svc.get_history()[0].timestamp, 100);
        assert_eq!(svc.get_history()[1].timestamp, 101);
        assert_eq!(svc.get_history()[2].timestamp, 102);
        assert!(svc.get_history().iter().all(|e| e.source_mode == SourceMode::Visual));
    }

    #[test]
    fn service_retain_entries() {
        let mut svc = ClipboardService::new(10);
        svc.write_batch(&["short", "a very long string here", "hi"], 1, SourceMode::Normal);
        svc.retain(|e| e.text.len() <= 10);
        assert_eq!(svc.history_count(), 2);
        assert!(svc.get_history().iter().all(|e| e.text.len() <= 10));
    }

    #[test]
    fn service_replace_in_history() {
        let mut svc = ClipboardService::new(10);
        svc.write_text("hello world".into(), 1);
        svc.write_text("world peace".into(), 2);
        svc.write_text("nothing".into(), 3);
        let count = svc.replace_in_history("world", "earth");
        assert_eq!(count, 2);
        assert_eq!(svc.get_history()[0].text, "hello earth");
        assert_eq!(svc.get_history()[1].text, "earth peace");
        assert_eq!(svc.get_history()[2].text, "nothing");
    }

    #[test]
    fn service_recent_entries() {
        let mut svc = ClipboardService::new(10);
        svc.write_batch(&["a", "b", "c", "d"], 1, SourceMode::Normal);
        let recent = svc.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "d");
        assert_eq!(recent[1].text, "c");
    }

    #[test]
    fn service_merge_history() {
        let mut svc = ClipboardService::new(10);
        svc.write_batch(&["alpha", "beta", "gamma"], 1, SourceMode::Normal);
        assert_eq!(svc.merge_history(", "), "alpha, beta, gamma");
        assert_eq!(svc.merge_history("\n"), "alpha\nbeta\ngamma");
    }

    // -- ClipboardHistory --------------------------------------------------

    #[test]
    fn history_push_and_get() {
        let mut h = ClipboardHistory::new(5);
        h.push("hello".into());
        assert_eq!(h.get_at(0), Some("hello"));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn history_max_entries() {
        let mut h = ClipboardHistory::new(2);
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        assert_eq!(h.len(), 2);
        assert_eq!(h.get_at(0), Some("b"));
    }

    #[test]
    fn history_recent() {
        let mut h = ClipboardHistory::new(10);
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        assert_eq!(h.recent(2), vec!["c", "b"]);
    }

    #[test]
    fn history_deduplicate() {
        let mut h = ClipboardHistory::new(10);
        h.push("a".into());
        h.push("b".into());
        h.push("a".into());
        h.deduplicate();
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn history_remove_at() {
        let mut h = ClipboardHistory::new(10);
        h.push("x".into());
        h.push("y".into());
        assert_eq!(h.remove_at(0), Some("x".into()));
        assert_eq!(h.len(), 1);
    }

    // -- ClipboardTransformer ----------------------------------------------

    #[test]
    fn transformer_trim() {
        assert_eq!(ClipboardTransformer::trim("  hello  "), "hello");
    }

    #[test]
    fn transformer_collapse_whitespace() {
        assert_eq!(ClipboardTransformer::collapse_whitespace("a  b\t\tc"), "a b c");
    }

    #[test]
    fn transformer_escape_html() {
        assert_eq!(ClipboardTransformer::escape_html("<b>\"hi\"</b>"), "&lt;b&gt;&quot;hi&quot;&lt;/b&gt;");
    }

    #[test]
    fn transformer_unescape_html() {
        assert_eq!(ClipboardTransformer::unescape_html("&lt;b&gt;"), "<b>");
    }

    #[test]
    fn transformer_to_single_line() {
        assert_eq!(ClipboardTransformer::to_single_line("a\nb\nc"), "a b c");
    }

    #[test]
    fn transformer_normalize_newlines() {
        assert_eq!(ClipboardTransformer::normalize_newlines("a\r\nb\rc"), "a\nb\nc");
    }

    // -- ClipboardMetadataV2 -------------------------------------------------

    #[test]
    fn metadata_matches_filter() {
        let meta = ClipboardMetadataV2::new(100)
            .with_source("src/main.rs", 10)
            .with_language("rust");
        assert!(meta.matches_filter(Some("rust"), None));
        assert!(!meta.matches_filter(Some("python"), None));
        assert!(meta.matches_filter(None, Some("main.rs")));
    }

    #[test]
    fn metadata_no_filter() {
        let meta = ClipboardMetadataV2::new(0);
        assert!(meta.matches_filter(None, None));
    }


    // -- wb_clipboard additional tests -------------------------------------------

    #[test]
    fn x_wb_clipboard_panel_state_new() {
        let p = XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbClipboardLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_clipboard_panel_area() {
        let p = XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_clipboard_panel_toggle() {
        let mut p = XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_clipboard_panel_resize() {
        let mut p = XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_clipboard_panel_is_narrow() {
        let mut p = XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_clipboard_total_visible_area_basic() {
        let panels = vec![
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "a"),
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_clipboard_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_clipboard_total_visible_area_hidden() {
        let mut panels = vec![
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "a"),
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_clipboard_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_clipboard_count_in_region_basic() {
        let panels = vec![
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "a"),
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "b"),
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_clipboard_count_in_region(&panels, XWbClipboardLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_clipboard_count_in_region(&panels, XWbClipboardLayoutRegion::Editor), 1);
        assert_eq!(x_wb_clipboard_count_in_region(&panels, XWbClipboardLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_clipboard_widest_panel_basic() {
        let mut panels = vec![
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "narrow"),
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_clipboard_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_clipboard_collapse_region_basic() {
        let mut panels = vec![
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "a"),
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Sidebar, "b"),
            XWbClipboardPanelState::new(XWbClipboardLayoutRegion::Editor, "c"),
        ];
        x_wb_clipboard_collapse_region(&mut panels, XWbClipboardLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_clipboard_layout_constraint_clamp() {
        let lc = XWbClipboardLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_clipboard_layout_constraint_satisfied() {
        let lc = XWbClipboardLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_clipboard_widest_panel_empty() {
        let panels: Vec<XWbClipboardPanelState> = vec![];
        assert!(x_wb_clipboard_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_clipboard_layout_region_eq() {
        assert_eq!(XWbClipboardLayoutRegion::Sidebar, XWbClipboardLayoutRegion::Sidebar);
        assert_ne!(XWbClipboardLayoutRegion::Sidebar, XWbClipboardLayoutRegion::Panel);
    }


    // -- wb_clipboard extended domain tests ----------------------------------------

    #[test]
    fn y_wb_clipboard_enum_index() {
        assert_eq!(YWbClipboardClipboardEntryKind::Text.index(), 0);
        assert_eq!(YWbClipboardClipboardEntryKind::Image.index(), 1);
        assert_eq!(YWbClipboardClipboardEntryKind::File.index(), 2);
        assert_eq!(YWbClipboardClipboardEntryKind::Rich.index(), 3);
    }

    #[test]
    fn y_wb_clipboard_enum_label() {
        assert_eq!(YWbClipboardClipboardEntryKind::Text.label(), "Text");
        assert_eq!(YWbClipboardClipboardEntryKind::Image.label(), "Image");
        assert_eq!(YWbClipboardClipboardEntryKind::File.label(), "File");
        assert_eq!(YWbClipboardClipboardEntryKind::Rich.label(), "Rich");
    }

    #[test]
    fn y_wb_clipboard_enum_all() {
        let all = YWbClipboardClipboardEntryKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_clipboard_enum_is_default() {
        assert!(YWbClipboardClipboardEntryKind::Text.is_default());
        assert!(!YWbClipboardClipboardEntryKind::Rich.is_default());
    }

    #[test]
    fn y_wb_clipboard_enum_display() {
        assert_eq!(format!("{}", YWbClipboardClipboardEntryKind::Text), "Text");
    }

    #[test]
    fn y_wb_clipboard_struct_new() {
        let s = YWbClipboardClipboardStack::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_wb_clipboard_struct_clear() {
        let mut s = YWbClipboardClipboardStack::new();
        s.entries.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_wb_clipboard_fingerprint_deterministic() {
        let h1 = y_wb_clipboard_fingerprint("hello");
        let h2 = y_wb_clipboard_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_clipboard_fingerprint("a"), y_wb_clipboard_fingerprint("b"));
    }

    #[test]
    fn y_wb_clipboard_truncate_short() {
        assert_eq!(y_wb_clipboard_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_clipboard_truncate_long() {
        let r = y_wb_clipboard_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_clipboard_normalize_key_basic() {
        assert_eq!(y_wb_clipboard_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_clipboard_split_path_basic() {
        let parts = y_wb_clipboard_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_clipboard_count_occurrences_basic() {
        assert_eq!(y_wb_clipboard_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_clipboard_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_clipboard_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_clipboard_in_range_basic() {
        assert!(y_wb_clipboard_in_range(5, 1, 10));
        assert!(y_wb_clipboard_in_range(1, 1, 10));
        assert!(y_wb_clipboard_in_range(10, 1, 10));
        assert!(!y_wb_clipboard_in_range(0, 1, 10));
        assert!(!y_wb_clipboard_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_clipboard_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_clipboard_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_clipboard_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_clipboard_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_clipboard Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_clipboard_priority_weight() {
        assert_eq!(ZWbClipboardPriority::Idle.weight(), 0);
        assert_eq!(ZWbClipboardPriority::Normal.weight(), 2);
        assert_eq!(ZWbClipboardPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_clipboard_priority_label() {
        assert_eq!(ZWbClipboardPriority::Low.label(), "low");
        assert_eq!(ZWbClipboardPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_clipboard_priority_is_elevated() {
        assert!(!ZWbClipboardPriority::Normal.is_elevated());
        assert!(ZWbClipboardPriority::High.is_elevated());
        assert!(ZWbClipboardPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_clipboard_priority_display() {
        assert_eq!(format!("{}", ZWbClipboardPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_clipboard_priority_all_asc() {
        let all = ZWbClipboardPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbClipboardPriority::Idle);
        assert_eq!(all[4], ZWbClipboardPriority::Realtime);
    }

    #[test]
    fn z_wb_clipboard_struct_new() {
        let s = ZWbClipboardClipboardTransform::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_clipboard_struct_toggled_clone() {
        let s = ZWbClipboardClipboardTransform::new();
        let t = s.toggled_clone();
        assert_ne!(s.reversible, t.reversible);
    }

    #[test]
    fn z_wb_clipboard_rolling_hash_deterministic() {
        let h1 = z_wb_clipboard_rolling_hash(b"test");
        let h2 = z_wb_clipboard_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_clipboard_rolling_hash(b"a"), z_wb_clipboard_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_clipboard_pad_to_basic() {
        assert_eq!(z_wb_clipboard_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_clipboard_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_clipboard_is_identifier_basic() {
        assert!(z_wb_clipboard_is_identifier("foo_bar"));
        assert!(z_wb_clipboard_is_identifier("abc123"));
        assert!(!z_wb_clipboard_is_identifier(""));
        assert!(!z_wb_clipboard_is_identifier("has space"));
    }

    #[test]
    fn z_wb_clipboard_levenshtein_basic() {
        assert_eq!(z_wb_clipboard_levenshtein("", ""), 0);
        assert_eq!(z_wb_clipboard_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_clipboard_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_clipboard_unique_words_basic() {
        let w = z_wb_clipboard_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_clipboard_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_clipboard_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_clipboard_common_prefix_basic() {
        assert_eq!(z_wb_clipboard_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_clipboard_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_clipboard_struct_clear() {
        let mut s = ZWbClipboardClipboardTransform::new();
        s.rules.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_clipboard_rolling_hash_empty() {
        let h = z_wb_clipboard_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_100_push_and_len() {
        let mut rb = super::XbRingBuffer100::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_100_overwrite() {
        let mut rb = super::XbRingBuffer100::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_100_get_out_of_bounds() {
        let rb = super::XbRingBuffer100::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_100_drain_all() {
        let mut rb = super::XbRingBuffer100::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_100_peek_front_back() {
        let mut rb = super::XbRingBuffer100::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_100_clear() {
        let mut rb = super::XbRingBuffer100::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_100_capacity() {
        let rb = super::XbRingBuffer100::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_100_basic() {
        let h = super::xb_fnv1a_100(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_100(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_100_different_inputs() {
        let h1 = super::xb_fnv1a_100(b"abc");
        let h2 = super::xb_fnv1a_100(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_100_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_100(&data);
        let dec = super::xb_rle_decode_100(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_100_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_100(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_100(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_100_values() {
        assert!((super::xb_clamp_100(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_100(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_100(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_100_values() {
        assert!((super::xb_lerp_100(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_100(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_100(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_100_wrap_around_twice() {
        let mut rb = super::XbRingBuffer100::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 203 ----

    #[test]
    fn xc_203_pool_new_empty() {
        let pool: super::Xc203Pool<i32> = super::Xc203Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_203_pool_release_acquire() {
        let mut pool = super::Xc203Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_203_pool_acquire_empty() {
        let mut pool: super::Xc203Pool<i32> = super::Xc203Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_203_pool_full() {
        let mut pool = super::Xc203Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_203_pool_drain() {
        let mut pool = super::Xc203Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_203_pool_stats() {
        let mut pool = super::Xc203Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_203_pool_clear() {
        let mut pool = super::Xc203Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_203_pool_shrink() {
        let mut pool = super::Xc203Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_203_pool_default() {
        let pool: super::Xc203Pool<String> = super::Xc203Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_203_pool_extend() {
        let mut pool = super::Xc203Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_203_pool_retain() {
        let mut pool = super::Xc203Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_203_scheduler_round_robin() {
        let mut sched = super::Xc203Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_203_scheduler_empty() {
        let mut sched = super::Xc203Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_203_scheduler_reset() {
        let mut sched = super::Xc203Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_203_scheduler_add_remove() {
        let mut sched = super::Xc203Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_203_scheduler_targets() {
        let sched = super::Xc203Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_203_hash_empty() {
        assert_eq!(super::xc_203_hash(b""), 5381);
    }

    #[test]
    fn xc_203_hash_data() {
        let h = super::xc_203_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_203_hash(b"hello"), h);
    }

    #[test]
    fn xc_203_reverse_str() {
        assert_eq!(super::xc_203_reverse("abc"), "cba");
        assert_eq!(super::xc_203_reverse(""), "");
    }

}
