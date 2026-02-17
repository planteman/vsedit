//! Output panel view.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur when working with output channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputError {
    ChannelNotFound(String),
    DuplicateChannel(String),
    ChannelEmpty,
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::ChannelNotFound(id) => write!(f, "channel not found: {id}"),
            OutputError::DuplicateChannel(id) => write!(f, "duplicate channel: {id}"),
            OutputError::ChannelEmpty => write!(f, "channel is empty"),
        }
    }
}

/// A single output channel that accumulates text lines.
pub struct OutputChannel {
    pub id: String,
    pub name: String,
    pub lines: Vec<String>,
    pub visible: bool,
    pub language_id: Option<String>,
}

impl OutputChannel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            lines: Vec::new(),
            visible: false,
            language_id: None,
        }
    }

    pub fn append(&mut self, text: &str) {
        if let Some(last) = self.lines.last_mut() {
            last.push_str(text);
        } else {
            self.lines.push(text.to_string());
        }
    }

    pub fn append_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a specific line by zero-based index.
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }

    /// Find all lines containing `pattern`, returning (line_index, line_text).
    pub fn search(&self, pattern: &str) -> Vec<(usize, &str)> {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(pattern))
            .map(|(i, line)| (i, line.as_str()))
            .collect()
    }

    /// Replace the entire content with new lines.
    pub fn replace(&mut self, lines: Vec<String>) {
        self.lines = lines;
    }

    /// Builder method to set the language_id.
    pub fn with_language(mut self, language_id: impl Into<String>) -> Self {
        self.language_id = Some(language_id.into());
        self
    }

    /// Return the last `n` lines.
    pub fn tail(&self, n: usize) -> Vec<&str> {
        let start = self.lines.len().saturating_sub(n);
        self.lines[start..].iter().map(|s| s.as_str()).collect()
    }
}

impl fmt::Display for OutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} lines)", self.name, self.lines.len())
    }
}

/// Service managing multiple output channels.
pub struct OutputService {
    pub channels: Vec<OutputChannel>,
    pub active_channel: Option<usize>,
}

impl OutputService {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            active_channel: None,
        }
    }

    /// Create a new channel and return its index.
    pub fn create_channel(&mut self, name: impl Into<String>) -> usize {
        let idx = self.channels.len();
        let id = format!("channel-{idx}");
        self.channels.push(OutputChannel::new(id, name));
        idx
    }

    pub fn get_channel(&self, id: &str) -> Option<&OutputChannel> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn get_channel_mut(&mut self, id: &str) -> Option<&mut OutputChannel> {
        self.channels.iter_mut().find(|c| c.id == id)
    }

    pub fn set_active(&mut self, id: &str) {
        if let Some(idx) = self.channels.iter().position(|c| c.id == id) {
            self.active_channel = Some(idx);
        }
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Remove a channel by id, returning it if found.
    pub fn remove_channel(&mut self, id: &str) -> Result<OutputChannel, OutputError> {
        let idx = self
            .channels
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        // Adjust active_channel index after removal.
        if let Some(active) = self.active_channel {
            if active == idx {
                self.active_channel = None;
            } else if active > idx {
                self.active_channel = Some(active - 1);
            }
        }
        Ok(self.channels.remove(idx))
    }

    /// Get a reference to the currently active channel.
    pub fn get_active_channel(&self) -> Option<&OutputChannel> {
        self.active_channel.and_then(|i| self.channels.get(i))
    }

    /// Find the first channel whose name matches.
    pub fn find_by_name(&self, name: &str) -> Option<&OutputChannel> {
        self.channels.iter().find(|c| c.name == name)
    }

    /// Clear the contents of every channel.
    pub fn clear_all(&mut self) {
        for ch in &mut self.channels {
            ch.clear();
        }
    }
}

impl Default for OutputService {
    fn default() -> Self {
        Self::new()
    }
}

/// Log severity level for structured output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// A structured log entry with level, timestamp and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub level: LogLevel,
    pub timestamp_ms: u64,
    pub message: String,
    pub source: Option<String>,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>, timestamp_ms: u64) -> Self {
        Self {
            level,
            timestamp_ms,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Format the entry as a log line.
    pub fn format(&self) -> String {
        let src = self.source.as_deref().unwrap_or("unknown");
        format!("[{} {}] {}: {}", self.timestamp_ms, self.level, src, self.message)
    }
}

impl LogEntry {
    /// Check whether this entry is at or above the given level.
    pub fn matches_level(&self, min: LogLevel) -> bool {
        self.level >= min
    }

    /// Check whether the message contains the given substring.
    pub fn contains(&self, pattern: &str) -> bool {
        self.message.contains(pattern)
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

// ---------------------------------------------------------------------------
// LogOutputChannel — structured logging output channel
// ---------------------------------------------------------------------------

/// An output channel that formats entries with timestamps and log levels.
///
/// Wraps an `OutputChannel` and provides `info()`, `warn()`, `error()`,
/// `debug()`, and `trace()` convenience methods.
pub struct LogOutputChannel {
    pub channel: OutputChannel,
    pub log_level: LogLevel,
    timestamp_counter: u64,
}

impl LogOutputChannel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            channel: OutputChannel::new(id, name),
            log_level: LogLevel::Info,
            timestamp_counter: 0,
        }
    }

    /// Set the minimum log level; messages below this level are discarded.
    pub fn set_log_level(&mut self, level: LogLevel) {
        self.log_level = level;
    }

    /// Advance the internal timestamp counter and return the new value.
    fn next_timestamp(&mut self) -> u64 {
        self.timestamp_counter += 1;
        self.timestamp_counter
    }

    /// Log a message at the given level, if it meets the threshold.
    /// Returns `true` if the message was appended.
    pub fn log(&mut self, level: LogLevel, message: &str) -> bool {
        if level < self.log_level {
            return false;
        }
        let ts = self.next_timestamp();
        let formatted = format!("[{ts} {level}] {message}");
        self.channel.append_line(&formatted);
        true
    }

    pub fn trace(&mut self, message: &str) -> bool {
        self.log(LogLevel::Trace, message)
    }

    pub fn debug(&mut self, message: &str) -> bool {
        self.log(LogLevel::Debug, message)
    }

    pub fn info(&mut self, message: &str) -> bool {
        self.log(LogLevel::Info, message)
    }

    pub fn warn(&mut self, message: &str) -> bool {
        self.log(LogLevel::Warn, message)
    }

    pub fn error(&mut self, message: &str) -> bool {
        self.log(LogLevel::Error, message)
    }

    /// Clear all log output.
    pub fn clear(&mut self) {
        self.channel.clear();
    }

    /// Return the underlying channel's line count.
    pub fn line_count(&self) -> usize {
        self.channel.line_count()
    }

    /// Return the full content.
    pub fn get_content(&self) -> String {
        self.channel.get_content()
    }

    /// Show the channel.
    pub fn show(&mut self) {
        self.channel.show();
    }

    /// Hide the channel.
    pub fn hide(&mut self) {
        self.channel.hide();
    }

    /// Return lines whose formatted log level is at or above `min`.
    pub fn filter_by_level(&self, min: LogLevel) -> Vec<&str> {
        let tags: Vec<&str> = [LogLevel::Trace, LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error]
            .iter()
            .filter(|&&l| l >= min)
            .map(|l| match l {
                LogLevel::Trace => "TRACE",
                LogLevel::Debug => "DEBUG",
                LogLevel::Info => "INFO",
                LogLevel::Warn => "WARN",
                LogLevel::Error => "ERROR",
            })
            .collect();
        self.channel
            .lines
            .iter()
            .filter(|line| tags.iter().any(|tag| line.contains(tag)))
            .map(|s| s.as_str())
            .collect()
    }

    /// Parse stored lines back into `LogEntry` values.
    /// Lines that do not match the expected `[ts LEVEL] message` format are skipped.
    pub fn entries(&self) -> Vec<LogEntry> {
        let mut result = Vec::new();
        for line in &self.channel.lines {
            if let Some(entry) = Self::parse_log_line(line) {
                result.push(entry);
            }
        }
        result
    }

    /// Try to parse a single formatted log line into a `LogEntry`.
    fn parse_log_line(line: &str) -> Option<LogEntry> {
        let rest = line.strip_prefix('[')?;
        let bracket_end = rest.find(']')?;
        let header = &rest[..bracket_end];
        let message = rest[bracket_end + 1..].trim_start().to_string();

        let mut parts = header.splitn(2, ' ');
        let ts_str = parts.next()?;
        let level_str = parts.next()?;

        let ts: u64 = ts_str.parse().ok()?;
        let level = match level_str {
            "TRACE" => LogLevel::Trace,
            "DEBUG" => LogLevel::Debug,
            "INFO" => LogLevel::Info,
            "WARN" => LogLevel::Warn,
            "ERROR" => LogLevel::Error,
            _ => return None,
        };
        Some(LogEntry::new(level, message, ts))
    }
}

impl fmt::Display for LogOutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LogOutputChannel({}, level={}, {} lines)",
            self.channel.name, self.log_level, self.channel.line_count())
    }
}

/// Statistics for an output channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChannelStats {
    pub total_lines: usize,
    pub total_chars: usize,
    pub longest_line_len: usize,
    pub empty_lines: usize,
}

impl OutputChannel {
    /// Compute statistics about the channel's content.
    pub fn stats(&self) -> OutputChannelStats {
        let total_lines = self.lines.len();
        let total_chars: usize = self.lines.iter().map(|l| l.len()).sum();
        let longest_line_len = self.lines.iter().map(|l| l.len()).max().unwrap_or(0);
        let empty_lines = self.lines.iter().filter(|l| l.trim().is_empty()).count();
        OutputChannelStats {
            total_lines,
            total_chars,
            longest_line_len,
            empty_lines,
        }
    }

    /// Return the first `n` lines.
    pub fn head(&self, n: usize) -> Vec<&str> {
        self.lines.iter().take(n).map(|s| s.as_str()).collect()
    }

    /// Filter lines that match a predicate, returning (index, line).
    pub fn filter_lines<F>(&self, predicate: F) -> Vec<(usize, &str)>
    where
        F: Fn(&str) -> bool,
    {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| predicate(line))
            .map(|(i, line)| (i, line.as_str()))
            .collect()
    }

    /// Count occurrences of a pattern across all lines.
    pub fn count_pattern(&self, pattern: &str) -> usize {
        self.lines.iter().map(|line| line.matches(pattern).count()).sum()
    }

    /// Return lines in reverse order.
    pub fn reversed_lines(&self) -> Vec<&str> {
        self.lines.iter().rev().map(|s| s.as_str()).collect()
    }

    /// Truncate to keep only the last `n` lines, removing older lines.
    pub fn truncate_to_tail(&mut self, n: usize) {
        if self.lines.len() > n {
            let start = self.lines.len() - n;
            self.lines = self.lines.split_off(start);
        }
    }

    /// Append a batch of lines at once.
    pub fn append_lines(&mut self, lines: &[&str]) {
        for line in lines {
            self.lines.push(line.to_string());
        }
    }

    /// Return a sub-slice of lines by start (inclusive) and end (exclusive) indices.
    /// Out-of-range indices are clamped silently.
    pub fn line_range(&self, start: usize, end: usize) -> Vec<&str> {
        let s = start.min(self.lines.len());
        let e = end.min(self.lines.len());
        self.lines[s..e].iter().map(|l| l.as_str()).collect()
    }

    /// Return the last line, if any.
    pub fn last_line(&self) -> Option<&str> {
        self.lines.last().map(|s| s.as_str())
    }
}

impl OutputService {
    /// Find all channels containing a specific text pattern.
    pub fn search_all_channels(&self, pattern: &str) -> Vec<(&OutputChannel, Vec<(usize, &str)>)> {
        self.channels
            .iter()
            .map(|ch| (ch, ch.search(pattern)))
            .filter(|(_, results)| !results.is_empty())
            .collect()
    }

    /// Return total line count across all channels.
    pub fn total_lines(&self) -> usize {
        self.channels.iter().map(|ch| ch.line_count()).sum()
    }

    /// Get visible channels.
    pub fn visible_channels(&self) -> Vec<&OutputChannel> {
        self.channels.iter().filter(|ch| ch.visible).collect()
    }

    /// Hide all channels.
    pub fn hide_all(&mut self) {
        for ch in &mut self.channels {
            ch.hide();
        }
    }

    /// Get channel names as a list.
    pub fn channel_names(&self) -> Vec<&str> {
        self.channels.iter().map(|ch| ch.name.as_str()).collect()
    }

    /// Get or create a channel by name. Returns the channel's id.
    /// If a channel with the given name already exists, its id is returned
    /// without creating a duplicate.
    pub fn get_or_create_channel(&mut self, name: &str) -> String {
        if let Some(ch) = self.channels.iter().find(|c| c.name == name) {
            return ch.id.clone();
        }
        let idx = self.create_channel(name);
        self.channels[idx].id.clone()
    }
}

// ---------------------------------------------------------------------------
// Output search
// ---------------------------------------------------------------------------

/// Represents a single match found in output channel content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSearchMatch {
    /// Zero-based line number where the match was found.
    pub line: usize,
    /// Byte offset within the line where the match starts.
    pub start: usize,
    /// Byte offset within the line where the match ends (exclusive).
    pub end: usize,
}

/// Searches within output channel content for occurrences of a pattern.
#[derive(Debug)]
pub struct OutputSearch {
    pattern: String,
    case_sensitive: bool,
}

impl OutputSearch {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            case_sensitive: true,
        }
    }

    pub fn case_insensitive(mut self) -> Self {
        self.case_sensitive = false;
        self
    }

    /// Find all occurrences of the pattern in the given lines.
    pub fn find_all(&self, lines: &[String]) -> Vec<OutputSearchMatch> {
        let mut results = Vec::new();
        for (line_idx, line) in lines.iter().enumerate() {
            let (haystack, needle);
            if self.case_sensitive {
                haystack = line.clone();
                needle = self.pattern.clone();
            } else {
                haystack = line.to_lowercase();
                needle = self.pattern.to_lowercase();
            }
            let mut start = 0;
            while let Some(pos) = haystack[start..].find(&needle) {
                let abs = start + pos;
                results.push(OutputSearchMatch {
                    line: line_idx,
                    start: abs,
                    end: abs + needle.len(),
                });
                start = abs + 1;
            }
        }
        results
    }

    /// Returns line numbers (zero-based) that contain at least one match.
    pub fn matching_lines(&self, lines: &[String]) -> Vec<usize> {
        let mut seen = Vec::new();
        for m in self.find_all(lines) {
            if seen.last() != Some(&m.line) {
                seen.push(m.line);
            }
        }
        seen
    }
}

// ---------------------------------------------------------------------------
// OutputViewFilter — filter output by channel and severity
// ---------------------------------------------------------------------------

/// Filters output by channel ID and minimum severity level.
pub struct OutputViewFilter {
    channel_ids: Vec<String>,
    min_severity: Option<LogLevel>,
}

impl OutputViewFilter {
    pub fn new() -> Self {
        Self {
            channel_ids: Vec::new(),
            min_severity: None,
        }
    }

    pub fn with_channel(mut self, channel_id: &str) -> Self {
        self.channel_ids.push(channel_id.to_string());
        self
    }

    pub fn with_min_severity(mut self, level: LogLevel) -> Self {
        self.min_severity = Some(level);
        self
    }

    pub fn matches_channel(&self, channel_id: &str) -> bool {
        if self.channel_ids.is_empty() {
            return true;
        }
        self.channel_ids.iter().any(|id| id == channel_id)
    }

    pub fn matches_severity(&self, level: LogLevel) -> bool {
        match self.min_severity {
            None => true,
            Some(min) => level >= min,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.channel_ids.is_empty() && self.min_severity.is_none()
    }
}

impl Default for OutputViewFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OutputViewSearch — search across output channels
// ---------------------------------------------------------------------------

/// A hit found by `OutputViewSearch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputViewSearchHit {
    pub channel_name: String,
    pub line_index: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

/// Searches for text across output channels.
pub struct OutputViewSearch {
    query: String,
    case_sensitive: bool,
}

impl OutputViewSearch {
    pub fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
            case_sensitive: true,
        }
    }

    pub fn case_sensitive(mut self, sensitive: bool) -> Self {
        self.case_sensitive = sensitive;
        self
    }

    pub fn search_channel(&self, channel: &OutputChannel) -> Vec<OutputViewSearchHit> {
        let needle = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };
        let mut hits = Vec::new();
        for (line_index, line) in channel.lines.iter().enumerate() {
            let haystack = if self.case_sensitive {
                line.clone()
            } else {
                line.to_lowercase()
            };
            let mut start = 0;
            while let Some(pos) = haystack[start..].find(&needle) {
                let abs = start + pos;
                hits.push(OutputViewSearchHit {
                    channel_name: channel.name.clone(),
                    line_index,
                    line_text: line.clone(),
                    match_start: abs,
                    match_end: abs + needle.len(),
                });
                start = abs + 1;
            }
        }
        hits
    }

    pub fn search_service(&self, service: &OutputService) -> Vec<OutputViewSearchHit> {
        let mut all = Vec::new();
        for ch in &service.channels {
            all.extend(self.search_channel(ch));
        }
        all
    }
}

// ---------------------------------------------------------------------------
// OutputViewTailState — follow tail of output
// ---------------------------------------------------------------------------

/// Tracks tail-follow state for an output view.
pub struct OutputViewTailState {
    pub enabled: bool,
    pub last_line_count: usize,
}

impl OutputViewTailState {
    pub fn new() -> Self {
        Self {
            enabled: true,
            last_line_count: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn is_following(&self) -> bool {
        self.enabled
    }

    /// Returns true if new lines were added since last update.
    pub fn update(&mut self, current_line_count: usize) -> bool {
        let had_new = current_line_count > self.last_line_count;
        self.last_line_count = current_line_count;
        had_new
    }

    pub fn new_lines_count(&self, current_line_count: usize) -> usize {
        current_line_count.saturating_sub(self.last_line_count)
    }
}

impl Default for OutputViewTailState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OutputRateLimiter — throttle log output to prevent flooding
// ---------------------------------------------------------------------------

/// Tracks message counts within a time window to enforce rate limits.
pub struct OutputRateLimiter {
    /// Maximum number of messages allowed within the window.
    pub max_messages: usize,
    /// Window duration in milliseconds.
    pub window_ms: u64,
    /// Timestamps (in ms) of messages accepted within the current window.
    timestamps: Vec<u64>,
    /// Number of messages that were dropped due to rate limiting.
    pub dropped_count: u64,
}

impl OutputRateLimiter {
    pub fn new(max_messages: usize, window_ms: u64) -> Self {
        Self {
            max_messages,
            window_ms,
            timestamps: Vec::new(),
            dropped_count: 0,
        }
    }

    /// Prune timestamps that fall outside the current window.
    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&ts| ts >= cutoff);
    }

    /// Try to accept a message at the given timestamp. Returns `true` if
    /// the message is allowed, `false` if it was rate-limited.
    pub fn try_accept(&mut self, now_ms: u64) -> bool {
        self.prune(now_ms);
        if self.timestamps.len() >= self.max_messages {
            self.dropped_count += 1;
            false
        } else {
            self.timestamps.push(now_ms);
            true
        }
    }

    /// Number of messages accepted in the current window.
    pub fn current_count(&self) -> usize {
        self.timestamps.len()
    }

    /// Remaining capacity in the current window (without pruning).
    pub fn remaining(&self) -> usize {
        self.max_messages.saturating_sub(self.timestamps.len())
    }

    /// Reset the limiter, clearing all state.
    pub fn reset(&mut self) {
        self.timestamps.clear();
        self.dropped_count = 0;
    }
}

// ---------------------------------------------------------------------------
// OutputFormatter — configurable line formatting
// ---------------------------------------------------------------------------

/// Controls how output lines are formatted before display.
#[derive(Debug, Clone)]
pub struct OutputFormatter {
    /// Whether to prepend a line number to each line.
    pub show_line_numbers: bool,
    /// Whether to prepend a timestamp to each line.
    pub show_timestamps: bool,
    /// Optional prefix string added before every line.
    pub prefix: Option<String>,
    /// Maximum line length; longer lines are truncated with an ellipsis.
    pub max_line_length: Option<usize>,
}

impl OutputFormatter {
    pub fn new() -> Self {
        Self {
            show_line_numbers: false,
            show_timestamps: false,
            prefix: None,
            max_line_length: None,
        }
    }

    pub fn with_line_numbers(mut self) -> Self {
        self.show_line_numbers = true;
        self
    }

    pub fn with_timestamps(mut self) -> Self {
        self.show_timestamps = true;
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn with_max_line_length(mut self, max: usize) -> Self {
        self.max_line_length = Some(max);
        self
    }

    /// Format a single line, given its zero-based index and an optional
    /// timestamp value.
    pub fn format_line(&self, index: usize, timestamp_ms: Option<u64>, line: &str) -> String {
        let mut parts = Vec::new();
        if self.show_line_numbers {
            parts.push(format!("{:>5}", index + 1));
        }
        if self.show_timestamps {
            if let Some(ts) = timestamp_ms {
                parts.push(format!("[{ts}]"));
            }
        }
        if let Some(ref pfx) = self.prefix {
            parts.push(pfx.clone());
        }
        parts.push(line.to_string());
        let mut result = parts.join(" ");
        if let Some(max) = self.max_line_length {
            if result.len() > max {
                result.truncate(max.saturating_sub(3));
                result.push_str("...");
            }
        }
        result
    }

    /// Format all lines in a channel, returning the formatted output.
    pub fn format_channel(&self, channel: &OutputChannel) -> Vec<String> {
        channel
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| self.format_line(i, None, line))
            .collect()
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OutputExporter — export channel content in various formats
// ---------------------------------------------------------------------------

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    PlainText,
    Json,
    Csv,
}

/// Exports output channel content to different string formats.
pub struct OutputExporter;

impl OutputExporter {
    /// Export a channel's content in the specified format.
    pub fn export(channel: &OutputChannel, format: ExportFormat) -> String {
        match format {
            ExportFormat::PlainText => channel.get_content(),
            ExportFormat::Json => Self::to_json(channel),
            ExportFormat::Csv => Self::to_csv(channel),
        }
    }

    fn to_json(channel: &OutputChannel) -> String {
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"channel\": \"{}\",\n", channel.name));
        out.push_str(&format!("  \"id\": \"{}\",\n", channel.id));
        out.push_str("  \"lines\": [\n");
        for (i, line) in channel.lines.iter().enumerate() {
            let escaped = line.replace('\\', "\\\\").replace('"', "\\\"");
            if i + 1 < channel.lines.len() {
                out.push_str(&format!("    \"{escaped}\",\n"));
            } else {
                out.push_str(&format!("    \"{escaped}\"\n"));
            }
        }
        out.push_str("  ]\n}");
        out
    }

    fn to_csv(channel: &OutputChannel) -> String {
        let mut out = String::from("line_number,content\n");
        for (i, line) in channel.lines.iter().enumerate() {
            let escaped = line.replace('"', "\"\"");
            out.push_str(&format!("{},\"{escaped}\"\n", i + 1));
        }
        out
    }

    /// Export all channels in a service as plain text, separated by headers.
    pub fn export_service(service: &OutputService, format: ExportFormat) -> String {
        let mut parts = Vec::new();
        for ch in &service.channels {
            let header = format!("=== {} ({}) ===", ch.name, ch.id);
            let body = Self::export(ch, format);
            parts.push(format!("{header}\n{body}"));
        }
        parts.join("\n\n")
    }
}

/// Count total lines across all channels in an `OutputService`.
pub fn total_line_count(service: &OutputService) -> usize {
    service.total_lines()
}

/// Find channels that contain a given pattern (case-insensitive).
pub fn channels_matching_pattern<'a>(
    service: &'a OutputService,
    pattern: &str,
) -> Vec<&'a OutputChannel> {
    let _lower = pattern.to_lowercase();
    service
        .search_all_channels(pattern)
        .into_iter()
        .filter(|(_, matches)| !matches.is_empty())
        .map(|(ch, _)| ch)
        .collect()
}

/// Return the names of channels that have at least `min_lines` lines.
pub fn channels_with_min_lines(service: &OutputService, min_lines: usize) -> Vec<String> {
    service
        .channel_names()
        .into_iter()
        .filter(|name| {
            service
                .find_by_name(name)
                .map_or(false, |ch| ch.line_count() >= min_lines)
        })
        .map(|s| s.to_string())
        .collect()
}

/// Compute the average line length (in chars) of a channel's content.
pub fn average_line_length(channel: &OutputChannel) -> f64 {
    let content = channel.get_content();
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return 0.0;
    }
    let total_chars: usize = lines.iter().map(|l| l.len()).sum();
    total_chars as f64 / lines.len() as f64
}

/// Deduplicate consecutive identical lines in a channel's output.
pub fn dedup_consecutive_lines(lines: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for line in lines {
        if result.last().map_or(true, |prev: &String| prev != line) {
            result.push(line.clone());
        }
    }
    result
}

/// Format a line count summary for an `OutputService`.
pub fn service_summary(service: &OutputService) -> String {
    let ch_count = service.channel_count();
    let total = service.total_lines();
    let visible = service.visible_channels().len();
    format!(
        "{} channels ({} visible), {} total lines",
        ch_count, visible, total
    )
}

/// Extract lines from a channel that match a log-level prefix like "[ERROR]" or "[WARN]".
pub fn extract_log_level_lines(channel: &OutputChannel, level_tag: &str) -> Vec<String> {
    let content = channel.get_content();
    content
        .lines()
        .filter(|l| l.contains(level_tag))
        .map(|l| l.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// OutputChannelSnapshot — frozen point-in-time capture of channel state
// ---------------------------------------------------------------------------

/// A frozen, cloneable snapshot of an `OutputChannel` at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChannelSnapshot {
    pub channel_id: String,
    pub channel_name: String,
    pub lines: Vec<String>,
    pub visible: bool,
}

impl OutputChannelSnapshot {
    /// Capture the current state of a channel.
    pub fn capture(channel: &OutputChannel) -> Self {
        Self {
            channel_id: channel.id.clone(),
            channel_name: channel.name.clone(),
            lines: channel.lines.clone(),
            visible: channel.visible,
        }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }
}

impl fmt::Display for OutputChannelSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Snapshot({}, {} lines)",
            self.channel_name,
            self.lines.len()
        )
    }
}

// ---------------------------------------------------------------------------
// OutputDiff — diff between two snapshots
// ---------------------------------------------------------------------------

/// The kind of change detected between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffKind {
    /// Lines were added at the end (contains the new lines).
    Appended(Vec<String>),
    /// Lines were removed from the end (contains count removed).
    Truncated(usize),
    /// Content changed in a way that is not a simple append or truncation.
    Changed,
    /// No differences.
    Unchanged,
}

/// Computes a simple diff between two `OutputChannelSnapshot` values
/// that belong to the same channel.
pub struct OutputDiff;

impl OutputDiff {
    /// Compare `before` and `after` snapshots, returning a `DiffKind`.
    pub fn diff(before: &OutputChannelSnapshot, after: &OutputChannelSnapshot) -> DiffKind {
        if before.lines == after.lines {
            return DiffKind::Unchanged;
        }
        // Check pure append: after starts with all of before's lines.
        if after.lines.len() > before.lines.len()
            && after.lines[..before.lines.len()] == before.lines[..]
        {
            let new_lines = after.lines[before.lines.len()..].to_vec();
            return DiffKind::Appended(new_lines);
        }
        // Check pure truncation: before starts with all of after's lines.
        if before.lines.len() > after.lines.len()
            && before.lines[..after.lines.len()] == after.lines[..]
        {
            let removed = before.lines.len() - after.lines.len();
            return DiffKind::Truncated(removed);
        }
        DiffKind::Changed
    }
}


// ---------------------------------------------------------------------------
// OutputViewWordWrap
// ---------------------------------------------------------------------------

pub struct OutputViewWordWrap {
    enabled: bool,
    wrap_column: usize,
}

impl OutputViewWordWrap {
    pub fn new(enabled: bool, wrap_column: usize) -> Self { Self { enabled, wrap_column } }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn toggle(&mut self) { self.enabled = !self.enabled; }
    pub fn wrap_column(&self) -> usize { self.wrap_column }
    pub fn set_wrap_column(&mut self, col: usize) { self.wrap_column = col; }

    pub fn wrap_line(&self, line: &str) -> Vec<String> {
        if !self.enabled || line.len() <= self.wrap_column {
            return vec![line.to_string()];
        }
        let mut result = Vec::new();
        let mut remaining = line;
        while remaining.len() > self.wrap_column {
            let (chunk, rest) = remaining.split_at(self.wrap_column);
            result.push(chunk.to_string());
            remaining = rest;
        }
        if !remaining.is_empty() { result.push(remaining.to_string()); }
        result
    }
}

impl Default for OutputViewWordWrap {
    fn default() -> Self { Self::new(false, 120) }
}

impl fmt::Display for OutputViewWordWrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WordWrap(enabled={}, col={})", self.enabled, self.wrap_column)
    }
}

// ---------------------------------------------------------------------------
// OutputViewTimestamp
// ---------------------------------------------------------------------------

pub struct OutputViewTimestamp;

impl OutputViewTimestamp {
    pub fn format_ms(ms: u64) -> String {
        let total_secs = ms / 1000;
        let millis = ms % 1000;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
    }

    pub fn format_elapsed(base_ms: u64, current_ms: u64) -> String {
        format!("+{}", Self::format_ms(current_ms.saturating_sub(base_ms)))
    }

    pub fn stamp_line(line: &str, timestamp_ms: u64) -> String {
        format!("[{}] {}", Self::format_ms(timestamp_ms), line)
    }
}

// ---------------------------------------------------------------------------
// OutputViewCopySelection
// ---------------------------------------------------------------------------

pub struct OutputViewCopySelection {
    start_line: usize,
    end_line: usize,
    selected_text: String,
}

impl OutputViewCopySelection {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start_line: start, end_line: end, selected_text: String::new() }
    }

    pub fn from_lines(lines: &[String], start: usize, end: usize) -> Self {
        let end = end.min(lines.len());
        let start = start.min(end);
        let text = lines[start..end].join("\n");
        Self { start_line: start, end_line: end, selected_text: text }
    }

    pub fn start_line(&self) -> usize { self.start_line }
    pub fn end_line(&self) -> usize { self.end_line }
    pub fn text(&self) -> &str { &self.selected_text }
    pub fn line_count(&self) -> usize { self.end_line.saturating_sub(self.start_line) }
    pub fn is_empty(&self) -> bool { self.selected_text.is_empty() }
}

impl fmt::Display for OutputViewCopySelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Selection(lines {}..{})", self.start_line, self.end_line)
    }
}

// ---------------------------------------------------------------------------
// OutputTextSearcher
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OutputTextSearchResult {
    pub line_index: usize,
    pub column: usize,
    pub length: usize,
    pub line_text: String,
}

pub struct OutputTextSearcher;

impl OutputTextSearcher {
    pub fn search(lines: &[String], pattern: &str) -> Vec<OutputTextSearchResult> {
        let pattern_lower = pattern.to_lowercase();
        let mut results = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&pattern_lower) {
                results.push(OutputTextSearchResult {
                    line_index: i, column: start + pos, length: pattern.len(), line_text: line.clone(),
                });
                start += pos + 1;
            }
        }
        results
    }

    pub fn count_matches(lines: &[String], pattern: &str) -> usize {
        Self::search(lines, pattern).len()
    }

    pub fn find_first(lines: &[String], pattern: &str) -> Option<OutputTextSearchResult> {
        Self::search(lines, pattern).into_iter().next()
    }
}


// ---------------------------------------------------------------------------
// OutputAutoScrollToggle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OutputAutoScrollToggle {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl OutputAutoScrollToggle {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for OutputAutoScrollToggle {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for OutputAutoScrollToggle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "OutputAutoScrollToggle({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// OutputClearConfirm
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OutputClearConfirm {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl OutputClearConfirm {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for OutputClearConfirm {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for OutputClearConfirm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "OutputClearConfirm({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// OutputAutoScrollToggleSnapshot — point-in-time snapshot of OutputAutoScrollToggle state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OutputAutoScrollToggleSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl OutputAutoScrollToggleSnapshot {
    pub fn capture(source: &OutputAutoScrollToggle, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for OutputAutoScrollToggleSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// OutputClearConfirmStats — aggregate statistics for OutputClearConfirm
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct OutputClearConfirmStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl OutputClearConfirmStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for OutputClearConfirmStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// OutputAutoScrollToggleConfig — configuration for OutputAutoScrollToggle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OutputAutoScrollToggleConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl OutputAutoScrollToggleConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for OutputAutoScrollToggleConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for OutputAutoScrollToggleConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_append_and_content() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("line 1");
        ch.append_line("line 2");
        assert_eq!(ch.line_count(), 2);
        assert_eq!(ch.get_content(), "line 1\nline 2");
    }

    #[test]
    fn channel_clear() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("hello");
        ch.clear();
        assert_eq!(ch.line_count(), 0);
        assert_eq!(ch.get_content(), "");
    }

    #[test]
    fn channel_visibility() {
        let mut ch = OutputChannel::new("ch1", "Build");
        assert!(!ch.visible);
        ch.show();
        assert!(ch.visible);
        ch.hide();
        assert!(!ch.visible);
    }

    #[test]
    fn service_create_and_find() {
        let mut svc = OutputService::new();
        let idx = svc.create_channel("Build");
        assert_eq!(idx, 0);
        assert_eq!(svc.channel_count(), 1);
        assert!(svc.get_channel("channel-0").is_some());
        assert!(svc.get_channel("nonexistent").is_none());
    }

    #[test]
    fn service_set_active() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.set_active("channel-1");
        assert_eq!(svc.active_channel, Some(1));
    }

    #[test]
    fn output_error_display() {
        assert_eq!(
            OutputError::ChannelNotFound("x".into()).to_string(),
            "channel not found: x"
        );
        assert_eq!(
            OutputError::DuplicateChannel("x".into()).to_string(),
            "duplicate channel: x"
        );
        assert_eq!(OutputError::ChannelEmpty.to_string(), "channel is empty");
    }

    #[test]
    fn channel_display() {
        let mut ch = OutputChannel::new("ch1", "Build");
        assert_eq!(ch.to_string(), "Build (0 lines)");
        ch.append_line("hello");
        ch.append_line("world");
        assert_eq!(ch.to_string(), "Build (2 lines)");
    }

    #[test]
    fn channel_get_line() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("alpha");
        ch.append_line("beta");
        assert_eq!(ch.get_line(0), Some("alpha"));
        assert_eq!(ch.get_line(1), Some("beta"));
        assert_eq!(ch.get_line(2), None);
    }

    #[test]
    fn channel_search() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("error: something failed");
        ch.append_line("info: all good");
        ch.append_line("error: another failure");
        let results = ch.search("error");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], (0, "error: something failed"));
        assert_eq!(results[1], (2, "error: another failure"));
        assert!(ch.search("warning").is_empty());
    }

    #[test]
    fn channel_replace() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("old line");
        ch.replace(vec!["new line 1".into(), "new line 2".into()]);
        assert_eq!(ch.line_count(), 2);
        assert_eq!(ch.get_line(0), Some("new line 1"));
    }

    #[test]
    fn channel_with_language() {
        let ch = OutputChannel::new("ch1", "Build").with_language("rust");
        assert_eq!(ch.language_id.as_deref(), Some("rust"));
    }

    #[test]
    fn channel_tail() {
        let mut ch = OutputChannel::new("ch1", "Log");
        for i in 0..10 {
            ch.append_line(&format!("line {i}"));
        }
        let last3 = ch.tail(3);
        assert_eq!(last3, vec!["line 7", "line 8", "line 9"]);
        assert_eq!(ch.tail(20).len(), 10);
        assert!(OutputChannel::new("ch2", "Empty").tail(5).is_empty());
    }

    #[test]
    fn service_remove_channel() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.set_active("channel-1");
        let removed = svc.remove_channel("channel-0").unwrap();
        assert_eq!(removed.name, "Build");
        assert_eq!(svc.channel_count(), 1);
        // active index should shift down
        assert_eq!(svc.active_channel, Some(0));
        assert!(svc.remove_channel("nonexistent").is_err());
    }

    #[test]
    fn service_get_active_channel() {
        let mut svc = OutputService::new();
        assert!(svc.get_active_channel().is_none());
        svc.create_channel("Build");
        svc.set_active("channel-0");
        assert_eq!(svc.get_active_channel().unwrap().name, "Build");
    }

    #[test]
    fn service_find_by_name() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        assert_eq!(svc.find_by_name("Tests").unwrap().id, "channel-1");
        assert!(svc.find_by_name("Nonexistent").is_none());
    }

    #[test]
    fn service_clear_all() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.get_channel_mut("channel-0").unwrap().append_line("hello");
        svc.get_channel_mut("channel-1").unwrap().append_line("world");
        svc.clear_all();
        assert_eq!(svc.get_channel("channel-0").unwrap().line_count(), 0);
        assert_eq!(svc.get_channel("channel-1").unwrap().line_count(), 0);
    }

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn log_entry_format() {
        let entry = LogEntry::new(LogLevel::Error, "something failed", 12345)
            .with_source("build");
        assert_eq!(entry.format(), "[12345 ERROR] build: something failed");
        assert_eq!(entry.to_string(), "[12345 ERROR] build: something failed");
    }

    #[test]
    fn log_entry_no_source() {
        let entry = LogEntry::new(LogLevel::Info, "hello", 100);
        assert!(entry.format().contains("unknown"));
    }

    #[test]
    fn channel_stats() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("hello world");
        ch.append_line("");
        ch.append_line("short");
        let stats = ch.stats();
        assert_eq!(stats.total_lines, 3);
        assert_eq!(stats.longest_line_len, 11);
        assert_eq!(stats.empty_lines, 1);
        assert_eq!(stats.total_chars, 16);
    }

    #[test]
    fn channel_stats_empty() {
        let ch = OutputChannel::new("ch1", "Log");
        let stats = ch.stats();
        assert_eq!(stats.total_lines, 0);
        assert_eq!(stats.longest_line_len, 0);
    }

    #[test]
    fn channel_head() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("a");
        ch.append_line("b");
        ch.append_line("c");
        assert_eq!(ch.head(2), vec!["a", "b"]);
        assert_eq!(ch.head(10).len(), 3);
    }

    #[test]
    fn channel_filter_lines() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("error: fail");
        ch.append_line("info: ok");
        ch.append_line("error: crash");
        let errors = ch.filter_lines(|l| l.starts_with("error"));
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].0, 0);
        assert_eq!(errors[1].0, 2);
    }

    #[test]
    fn channel_count_pattern() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("error error error");
        ch.append_line("no issues");
        ch.append_line("error");
        assert_eq!(ch.count_pattern("error"), 4);
        assert_eq!(ch.count_pattern("missing"), 0);
    }

    #[test]
    fn channel_reversed_lines() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("a");
        ch.append_line("b");
        ch.append_line("c");
        assert_eq!(ch.reversed_lines(), vec!["c", "b", "a"]);
    }

    #[test]
    fn channel_truncate_to_tail() {
        let mut ch = OutputChannel::new("ch1", "Log");
        for i in 0..10 {
            ch.append_line(&format!("line {i}"));
        }
        ch.truncate_to_tail(3);
        assert_eq!(ch.line_count(), 3);
        assert_eq!(ch.get_line(0), Some("line 7"));
    }

    #[test]
    fn channel_append_lines_batch() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_lines(&["a", "b", "c"]);
        assert_eq!(ch.line_count(), 3);
        assert_eq!(ch.get_line(1), Some("b"));
    }

    #[test]
    fn service_search_all_channels() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.get_channel_mut("channel-0").unwrap().append_line("error: build failed");
        svc.get_channel_mut("channel-1").unwrap().append_line("all tests passed");
        let results = svc.search_all_channels("error");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name, "Build");
    }

    #[test]
    fn service_total_lines() {
        let mut svc = OutputService::new();
        svc.create_channel("A");
        svc.create_channel("B");
        svc.get_channel_mut("channel-0").unwrap().append_line("x");
        svc.get_channel_mut("channel-1").unwrap().append_line("y");
        svc.get_channel_mut("channel-1").unwrap().append_line("z");
        assert_eq!(svc.total_lines(), 3);
    }

    #[test]
    fn service_visible_channels() {
        let mut svc = OutputService::new();
        svc.create_channel("A");
        svc.create_channel("B");
        svc.get_channel_mut("channel-0").unwrap().show();
        assert_eq!(svc.visible_channels().len(), 1);
        assert_eq!(svc.visible_channels()[0].name, "A");
    }

    #[test]
    fn service_hide_all() {
        let mut svc = OutputService::new();
        svc.create_channel("A");
        svc.create_channel("B");
        svc.get_channel_mut("channel-0").unwrap().show();
        svc.get_channel_mut("channel-1").unwrap().show();
        svc.hide_all();
        assert!(svc.visible_channels().is_empty());
    }

    #[test]
    fn service_channel_names() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        assert_eq!(svc.channel_names(), vec!["Build", "Tests"]);
    }

    // -- LogOutputChannel tests --

    #[test]
    fn log_channel_creation() {
        let log_ch = LogOutputChannel::new("log1", "Server Log");
        assert_eq!(log_ch.channel.name, "Server Log");
        assert_eq!(log_ch.log_level, LogLevel::Info);
        assert_eq!(log_ch.line_count(), 0);
    }

    #[test]
    fn log_channel_info_warn_error() {
        let mut log_ch = LogOutputChannel::new("log1", "Build");
        log_ch.info("started");
        log_ch.warn("deprecated API");
        log_ch.error("compilation failed");
        assert_eq!(log_ch.line_count(), 3);
        let content = log_ch.get_content();
        assert!(content.contains("INFO"));
        assert!(content.contains("WARN"));
        assert!(content.contains("ERROR"));
        assert!(content.contains("started"));
        assert!(content.contains("compilation failed"));
    }

    #[test]
    fn log_channel_filters_below_threshold() {
        let mut log_ch = LogOutputChannel::new("log1", "Build");
        log_ch.set_log_level(LogLevel::Warn);
        assert!(!log_ch.debug("should be filtered"));
        assert!(!log_ch.info("should be filtered"));
        assert!(log_ch.warn("should appear"));
        assert!(log_ch.error("should appear"));
        assert_eq!(log_ch.line_count(), 2);
    }

    #[test]
    fn log_channel_trace_and_debug() {
        let mut log_ch = LogOutputChannel::new("log1", "Debug");
        log_ch.set_log_level(LogLevel::Trace);
        assert!(log_ch.trace("tracing something"));
        assert!(log_ch.debug("debugging something"));
        assert_eq!(log_ch.line_count(), 2);
        let content = log_ch.get_content();
        assert!(content.contains("TRACE"));
        assert!(content.contains("DEBUG"));
    }

    #[test]
    fn log_channel_clear() {
        let mut log_ch = LogOutputChannel::new("log1", "Test");
        log_ch.info("msg");
        log_ch.clear();
        assert_eq!(log_ch.line_count(), 0);
    }

    #[test]
    fn log_channel_show_hide() {
        let mut log_ch = LogOutputChannel::new("log1", "Test");
        assert!(!log_ch.channel.visible);
        log_ch.show();
        assert!(log_ch.channel.visible);
        log_ch.hide();
        assert!(!log_ch.channel.visible);
    }

    #[test]
    fn log_channel_display() {
        let mut log_ch = LogOutputChannel::new("log1", "Server");
        log_ch.info("hi");
        let display = log_ch.to_string();
        assert!(display.contains("LogOutputChannel"));
        assert!(display.contains("Server"));
        assert!(display.contains("1 lines"));
    }

    #[test]
    fn log_channel_timestamps_increment() {
        let mut log_ch = LogOutputChannel::new("log1", "Test");
        log_ch.info("first");
        log_ch.info("second");
        let content = log_ch.get_content();
        // First log should have timestamp 1, second should have 2
        assert!(content.contains("[1 INFO]"));
        assert!(content.contains("[2 INFO]"));
    }

    // -- OutputSearch tests ------------------------------------------------

    #[test]
    fn search_finds_pattern() {
        let lines = vec!["hello world".into(), "goodbye world".into()];
        let search = OutputSearch::new("world");
        let matches = search.find_all(&lines);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 0);
        assert_eq!(matches[1].line, 1);
    }

    #[test]
    fn search_case_insensitive() {
        let lines = vec!["Hello World".into()];
        let search = OutputSearch::new("hello").case_insensitive();
        let matches = search.find_all(&lines);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start, 0);
    }

    #[test]
    fn search_no_matches() {
        let lines = vec!["foo bar".into()];
        let search = OutputSearch::new("baz");
        assert!(search.find_all(&lines).is_empty());
    }

    #[test]
    fn search_multiple_matches_per_line() {
        let lines = vec!["aaa".into()];
        let search = OutputSearch::new("a");
        let matches = search.find_all(&lines);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn search_matching_lines() {
        let lines = vec![
            "error: something".into(),
            "info: ok".into(),
            "error: another".into(),
        ];
        let search = OutputSearch::new("error");
        let ml = search.matching_lines(&lines);
        assert_eq!(ml, vec![0, 2]);
    }

    // -- OutputViewFilter tests --

    #[test]
    fn filter_empty_matches_all() {
        let f = OutputViewFilter::new();
        assert!(f.is_empty());
        assert!(f.matches_channel("any"));
        assert!(f.matches_severity(LogLevel::Trace));
    }

    #[test]
    fn filter_by_channel() {
        let f = OutputViewFilter::new()
            .with_channel("ch-1")
            .with_channel("ch-2");
        assert!(!f.is_empty());
        assert!(f.matches_channel("ch-1"));
        assert!(f.matches_channel("ch-2"));
        assert!(!f.matches_channel("ch-3"));
    }

    #[test]
    fn filter_by_severity() {
        let f = OutputViewFilter::new().with_min_severity(LogLevel::Warn);
        assert!(!f.is_empty());
        assert!(!f.matches_severity(LogLevel::Trace));
        assert!(!f.matches_severity(LogLevel::Debug));
        assert!(!f.matches_severity(LogLevel::Info));
        assert!(f.matches_severity(LogLevel::Warn));
        assert!(f.matches_severity(LogLevel::Error));
    }

    // -- OutputViewSearch tests --

    #[test]
    fn view_search_channel() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("error: compilation failed");
        ch.append_line("info: done");
        ch.append_line("error: link failed");
        let search = OutputViewSearch::new("error");
        let hits = search.search_channel(&ch);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].channel_name, "Build");
        assert_eq!(hits[0].line_index, 0);
        assert_eq!(hits[0].match_start, 0);
        assert_eq!(hits[1].line_index, 2);
    }

    #[test]
    fn view_search_case_insensitive() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("ERROR: big problem");
        let search = OutputViewSearch::new("error").case_sensitive(false);
        let hits = search.search_channel(&ch);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].match_start, 0);
        assert_eq!(hits[0].match_end, 5);
    }

    #[test]
    fn view_search_service() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.get_channel_mut("channel-0").unwrap().append_line("error here");
        svc.get_channel_mut("channel-1").unwrap().append_line("error there");
        let search = OutputViewSearch::new("error");
        let hits = search.search_service(&svc);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].channel_name, "Build");
        assert_eq!(hits[1].channel_name, "Tests");
    }

    // -- OutputViewTailState tests --

    #[test]
    fn tail_state_defaults() {
        let ts = OutputViewTailState::new();
        assert!(ts.is_following());
        assert_eq!(ts.last_line_count, 0);
    }

    #[test]
    fn tail_state_toggle() {
        let mut ts = OutputViewTailState::new();
        assert!(ts.is_following());
        ts.toggle();
        assert!(!ts.is_following());
        ts.toggle();
        assert!(ts.is_following());
    }

    #[test]
    fn tail_state_update_detects_new_lines() {
        let mut ts = OutputViewTailState::new();
        assert!(ts.update(5));
        assert_eq!(ts.last_line_count, 5);
        assert!(!ts.update(5));
        assert!(ts.update(8));
        assert_eq!(ts.last_line_count, 8);
    }

    #[test]
    fn tail_state_new_lines_count() {
        let mut ts = OutputViewTailState::new();
        ts.update(10);
        assert_eq!(ts.new_lines_count(15), 5);
        assert_eq!(ts.new_lines_count(10), 0);
        assert_eq!(ts.new_lines_count(5), 0);
    }

    // -- OutputRateLimiter tests --

    #[test]
    fn rate_limiter_accepts_within_limit() {
        let mut rl = OutputRateLimiter::new(3, 1000);
        assert!(rl.try_accept(100));
        assert!(rl.try_accept(200));
        assert!(rl.try_accept(300));
        assert!(!rl.try_accept(400)); // 4th message within window → rejected
        assert_eq!(rl.dropped_count, 1);
        assert_eq!(rl.current_count(), 3);
        assert_eq!(rl.remaining(), 0);
    }

    #[test]
    fn rate_limiter_window_expiry() {
        let mut rl = OutputRateLimiter::new(2, 100);
        assert!(rl.try_accept(10));
        assert!(rl.try_accept(20));
        assert!(!rl.try_accept(30)); // full
        // After the window expires, old timestamps are pruned
        assert!(rl.try_accept(200)); // 200 - 100 = 100 cutoff; 10 and 20 pruned
        assert_eq!(rl.current_count(), 1);
    }

    #[test]
    fn rate_limiter_reset() {
        let mut rl = OutputRateLimiter::new(2, 1000);
        rl.try_accept(10);
        rl.try_accept(20);
        rl.try_accept(30); // dropped
        assert_eq!(rl.dropped_count, 1);
        rl.reset();
        assert_eq!(rl.dropped_count, 0);
        assert_eq!(rl.current_count(), 0);
        assert_eq!(rl.remaining(), 2);
    }

    // -- OutputFormatter tests --

    #[test]
    fn formatter_plain() {
        let fmt = OutputFormatter::new();
        let result = fmt.format_line(0, None, "hello");
        assert_eq!(result, "hello");
    }

    #[test]
    fn formatter_with_line_numbers_and_prefix() {
        let fmt = OutputFormatter::new().with_line_numbers().with_prefix("|");
        let result = fmt.format_line(0, None, "hello");
        assert_eq!(result, "    1 | hello");
    }

    #[test]
    fn formatter_truncation() {
        let fmt = OutputFormatter::new().with_max_line_length(10);
        let result = fmt.format_line(0, None, "this is a very long line");
        assert_eq!(result.len(), 10);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn formatter_with_timestamps() {
        let fmt = OutputFormatter::new().with_timestamps();
        let result = fmt.format_line(0, Some(42), "msg");
        assert_eq!(result, "[42] msg");
    }

    #[test]
    fn formatter_format_channel() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("alpha");
        ch.append_line("beta");
        let fmt = OutputFormatter::new().with_line_numbers();
        let lines = fmt.format_channel(&ch);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("1"));
        assert!(lines[1].contains("2"));
    }

    // -- OutputExporter tests --

    #[test]
    fn exporter_plain_text() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("line one");
        ch.append_line("line two");
        let exported = OutputExporter::export(&ch, ExportFormat::PlainText);
        assert_eq!(exported, "line one\nline two");
    }

    #[test]
    fn exporter_json() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("hello");
        let json = OutputExporter::export(&ch, ExportFormat::Json);
        assert!(json.contains("\"channel\": \"Build\""));
        assert!(json.contains("\"hello\""));
    }

    #[test]
    fn exporter_csv() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("hello");
        ch.append_line("world");
        let csv = OutputExporter::export(&ch, ExportFormat::Csv);
        assert!(csv.starts_with("line_number,content\n"));
        assert!(csv.contains("1,\"hello\""));
        assert!(csv.contains("2,\"world\""));
    }

    #[test]
    fn exporter_service() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.get_channel_mut("channel-0").unwrap().append_line("ok");
        let exported = OutputExporter::export_service(&svc, ExportFormat::PlainText);
        assert!(exported.contains("=== Build (channel-0) ==="));
        assert!(exported.contains("ok"));
    }

    #[test]
    fn total_line_count_sums_channels() {
        let mut svc = OutputService::new();
        svc.create_channel("A");
        svc.create_channel("B");
        svc.get_channel_mut("channel-0").unwrap().append_line("line1");
        svc.get_channel_mut("channel-1").unwrap().append_line("line2");
        svc.get_channel_mut("channel-1").unwrap().append_line("line3");
        assert_eq!(total_line_count(&svc), 3);
    }

    #[test]
    fn channels_with_min_lines_filters() {
        let mut svc = OutputService::new();
        svc.create_channel("Big");
        svc.create_channel("Small");
        svc.get_channel_mut("channel-0").unwrap().append_line("a");
        svc.get_channel_mut("channel-0").unwrap().append_line("b");
        svc.get_channel_mut("channel-1").unwrap().append_line("c");
        let result = channels_with_min_lines(&svc, 2);
        assert_eq!(result, vec!["Big"]);
    }

    #[test]
    fn average_line_length_computes() {
        let mut ch = OutputChannel::new("ch", "Test");
        ch.append_line("abc");
        ch.append_line("abcdef");
        let avg = average_line_length(&ch);
        assert!((avg - 4.5).abs() < 0.01);
    }

    #[test]
    fn average_line_length_empty() {
        let ch = OutputChannel::new("ch", "Empty");
        assert_eq!(average_line_length(&ch), 0.0);
    }

    #[test]
    fn dedup_consecutive_lines_works() {
        let lines: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "b".into(), "a".into()];
        let result = dedup_consecutive_lines(&lines);
        assert_eq!(result, vec!["a", "b", "a"]);
    }

    #[test]
    fn dedup_consecutive_lines_empty() {
        assert!(dedup_consecutive_lines(&[]).is_empty());
    }

    #[test]
    fn service_summary_format() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        let s = service_summary(&svc);
        assert!(s.contains("1 channels"));
        assert!(s.contains("0 total lines"));
    }

    #[test]
    fn extract_log_level_lines_filters() {
        let mut ch = OutputChannel::new("ch", "Log");
        ch.append_line("[ERROR] something broke");
        ch.append_line("[INFO] all good");
        ch.append_line("[ERROR] another error");
        let errors = extract_log_level_lines(&ch, "[ERROR]");
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("something broke"));
    }

    // -- New functionality tests --

    #[test]
    fn log_entry_matches_level_and_contains() {
        let entry = LogEntry::new(LogLevel::Warn, "disk almost full", 500);
        assert!(entry.matches_level(LogLevel::Info));
        assert!(entry.matches_level(LogLevel::Warn));
        assert!(!entry.matches_level(LogLevel::Error));
        assert!(entry.contains("almost"));
        assert!(!entry.contains("memory"));
    }

    #[test]
    fn channel_line_range_and_last_line() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_lines(&["a", "b", "c", "d", "e"]);
        assert_eq!(ch.line_range(1, 4), vec!["b", "c", "d"]);
        assert_eq!(ch.line_range(3, 100), vec!["d", "e"]);
        assert!(ch.line_range(10, 20).is_empty());
        assert_eq!(ch.last_line(), Some("e"));

        let empty = OutputChannel::new("ch2", "Empty");
        assert_eq!(empty.last_line(), None);
    }

    #[test]
    fn service_get_or_create_channel_idempotent() {
        let mut svc = OutputService::new();
        let id1 = svc.get_or_create_channel("Build");
        let id2 = svc.get_or_create_channel("Build");
        assert_eq!(id1, id2);
        assert_eq!(svc.channel_count(), 1);
        let id3 = svc.get_or_create_channel("Tests");
        assert_ne!(id1, id3);
        assert_eq!(svc.channel_count(), 2);
    }

    #[test]
    fn log_channel_filter_by_level() {
        let mut log_ch = LogOutputChannel::new("log1", "Server");
        log_ch.set_log_level(LogLevel::Trace);
        log_ch.trace("t");
        log_ch.debug("d");
        log_ch.info("i");
        log_ch.warn("w");
        log_ch.error("e");
        assert_eq!(log_ch.line_count(), 5);

        let warn_and_above = log_ch.filter_by_level(LogLevel::Warn);
        assert_eq!(warn_and_above.len(), 2);
        assert!(warn_and_above[0].contains("WARN"));
        assert!(warn_and_above[1].contains("ERROR"));
    }

    #[test]
    fn log_channel_entries_roundtrip() {
        let mut log_ch = LogOutputChannel::new("log1", "Build");
        log_ch.info("compiling");
        log_ch.warn("deprecated");
        log_ch.error("failed");

        let entries = log_ch.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].level, LogLevel::Info);
        assert_eq!(entries[0].message, "compiling");
        assert_eq!(entries[0].timestamp_ms, 1);
        assert_eq!(entries[1].level, LogLevel::Warn);
        assert_eq!(entries[2].level, LogLevel::Error);
        assert_eq!(entries[2].timestamp_ms, 3);
    }

    #[test]
    fn snapshot_capture_and_display() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("line 1");
        ch.append_line("line 2");
        ch.show();
        let snap = OutputChannelSnapshot::capture(&ch);
        assert_eq!(snap.channel_id, "ch1");
        assert_eq!(snap.line_count(), 2);
        assert!(snap.visible);
        assert_eq!(snap.get_content(), "line 1\nline 2");
        assert!(snap.to_string().contains("Snapshot"));

        // Modifying the channel after capture doesn't affect the snapshot.
        ch.append_line("line 3");
        assert_eq!(snap.line_count(), 2);
    }

    #[test]
    fn output_diff_detects_changes() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_lines(&["a", "b"]);
        let snap1 = OutputChannelSnapshot::capture(&ch);

        // Unchanged
        let snap_same = OutputChannelSnapshot::capture(&ch);
        assert_eq!(OutputDiff::diff(&snap1, &snap_same), DiffKind::Unchanged);

        // Appended
        ch.append_line("c");
        ch.append_line("d");
        let snap2 = OutputChannelSnapshot::capture(&ch);
        match OutputDiff::diff(&snap1, &snap2) {
            DiffKind::Appended(new) => {
                assert_eq!(new, vec!["c", "d"]);
            }
            other => panic!("expected Appended, got {:?}", other),
        }

        // Truncated
        match OutputDiff::diff(&snap2, &snap1) {
            DiffKind::Truncated(n) => assert_eq!(n, 2),
            other => panic!("expected Truncated, got {:?}", other),
        }

        // Changed (replace content entirely)
        ch.replace(vec!["x".into(), "y".into()]);
        let snap3 = OutputChannelSnapshot::capture(&ch);
        assert_eq!(OutputDiff::diff(&snap1, &snap3), DiffKind::Changed);
    }


    #[test]
    fn word_wrap_disabled() {
        let wrap = OutputViewWordWrap::new(false, 10);
        assert_eq!(wrap.wrap_line("hello world"), vec!["hello world"]);
    }

    #[test]
    fn word_wrap_enabled() {
        let wrap = OutputViewWordWrap::new(true, 5);
        let lines = wrap.wrap_line("hello world!");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn word_wrap_toggle() {
        let mut wrap = OutputViewWordWrap::new(false, 80);
        wrap.toggle();
        assert!(wrap.is_enabled());
    }

    #[test]
    fn timestamp_format_ms() {
        assert_eq!(OutputViewTimestamp::format_ms(0), "00:00:00.000");
        assert_eq!(OutputViewTimestamp::format_ms(3661234), "01:01:01.234");
    }

    #[test]
    fn timestamp_format_elapsed() {
        let s = OutputViewTimestamp::format_elapsed(1000, 2500);
        assert!(s.starts_with('+'));
    }

    #[test]
    fn timestamp_stamp_line() {
        let line = OutputViewTimestamp::stamp_line("hello", 1000);
        assert!(line.starts_with('['));
        assert!(line.contains("hello"));
    }

    #[test]
    fn copy_selection_basic() {
        let lines: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let sel = OutputViewCopySelection::from_lines(&lines, 0, 2);
        assert_eq!(sel.line_count(), 2);
    }

    #[test]
    fn copy_selection_display() {
        let sel = OutputViewCopySelection::new(0, 5);
        assert!(format!("{sel}").contains("0..5"));
    }

    #[test]
    fn text_searcher_basic() {
        let lines: Vec<String> = vec!["hello world".into(), "world hello".into()];
        let results = OutputTextSearcher::search(&lines, "hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn text_searcher_count() {
        let lines: Vec<String> = vec!["aaa".into(), "aa".into()];
        assert_eq!(OutputTextSearcher::count_matches(&lines, "a"), 5);
    }

    #[test]
    fn text_searcher_case_insensitive() {
        let lines: Vec<String> = vec!["Hello World".into()];
        assert_eq!(OutputTextSearcher::search(&lines, "hello").len(), 1);
    }

    #[test]
    fn word_wrap_display() {
        let wrap = OutputViewWordWrap::default();
        assert!(format!("{wrap}").contains("enabled=false"));
    }


    #[test] fn outputAutoScrollToggle_new() { let s = OutputAutoScrollToggle::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn outputAutoScrollToggle_add() { let mut s = OutputAutoScrollToggle::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn outputAutoScrollToggle_remove() { let mut s = OutputAutoScrollToggle::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn outputAutoScrollToggle_config() { let mut s = OutputAutoScrollToggle::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn outputAutoScrollToggle_nav() { let mut s = OutputAutoScrollToggle::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn outputAutoScrollToggle_filter() { let mut s = OutputAutoScrollToggle::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn outputAutoScrollToggle_display() { assert!(format!("{}", OutputAutoScrollToggle::new()).contains("OutputAutoScrollToggle")); }
    #[test] fn outputClearConfirm_new() { let s = OutputClearConfirm::new(); assert!(s.is_empty()); }
    #[test] fn outputClearConfirm_add() { let mut s = OutputClearConfirm::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn outputClearConfirm_active() { let mut s = OutputClearConfirm::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn outputClearConfirm_error() { let mut s = OutputClearConfirm::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn outputClearConfirm_rm_group() { let mut s = OutputClearConfirm::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn outputClearConfirm_display() { assert!(format!("{}", OutputClearConfirm::new()).contains("OutputClearConfirm")); }


    #[test] fn outputAutoScrollToggle_snap_capture() {
        let s = OutputAutoScrollToggle::new();
        let snap = OutputAutoScrollToggleSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn outputAutoScrollToggle_snap_stale() {
        let s = OutputAutoScrollToggle::new();
        let snap = OutputAutoScrollToggleSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn outputAutoScrollToggle_snap_diff() {
        let s = OutputAutoScrollToggle::new();
        let s1v = OutputAutoScrollToggleSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn outputAutoScrollToggle_snap_display() {
        let s = OutputAutoScrollToggle::new();
        let snap = OutputAutoScrollToggleSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn outputClearConfirm_stats_record() {
        let mut st = OutputClearConfirmStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn outputClearConfirm_stats_hit_ratio() {
        let mut st = OutputClearConfirmStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn outputClearConfirm_stats_merge() {
        let mut a = OutputClearConfirmStats::new();
        a.total_adds = 5;
        let mut b = OutputClearConfirmStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn outputClearConfirm_stats_display() {
        let st = OutputClearConfirmStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn outputAutoScrollToggle_config_default() {
        let c = OutputAutoScrollToggleConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn outputAutoScrollToggle_config_builder() {
        let c = OutputAutoScrollToggleConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn outputAutoScrollToggle_config_labels() {
        let mut c = OutputAutoScrollToggleConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn outputAutoScrollToggle_config_cleanup_threshold() {
        let c = OutputAutoScrollToggleConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn outputAutoScrollToggle_config_display() {
        assert!(format!("{}", OutputAutoScrollToggleConfig::new()).contains("Config"));
    }
    #[test] fn outputClearConfirm_stats_peaks() {
        let mut st = OutputClearConfirmStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

}
