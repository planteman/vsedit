//! Output panel view.

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
}
