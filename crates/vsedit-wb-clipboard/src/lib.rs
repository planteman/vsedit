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
}
