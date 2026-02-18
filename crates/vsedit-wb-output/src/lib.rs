//! Output panel channels.

use std::collections::HashMap;
use std::fmt;

/// Errors returned by output channel operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputError {
    /// The requested channel id was not found.
    ChannelNotFound(String),
    /// A channel with the given id already exists.
    DuplicateChannel(String),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::ChannelNotFound(id) => write!(f, "channel not found: {id}"),
            OutputError::DuplicateChannel(id) => write!(f, "duplicate channel: {id}"),
        }
    }
}

/// Descriptor for an output channel.
#[derive(Debug, Clone)]
pub struct OutputChannelDescriptor {
    pub id: String,
    pub name: String,
    pub language_id: Option<String>,
    pub log: bool,
}

impl OutputChannelDescriptor {
    /// Builder method to set the language id.
    pub fn with_language(mut self, language_id: &str) -> Self {
        self.language_id = Some(language_id.to_string());
        self
    }
}

impl fmt::Display for OutputChannelDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.id)?;
        if let Some(lang) = &self.language_id {
            write!(f, " [{lang}]")?;
        }
        Ok(())
    }
}

/// Internal state for an output channel.
#[derive(Debug, Clone)]
pub struct OutputChannelState {
    pub descriptor: OutputChannelDescriptor,
    pub content: String,
    pub visible: bool,
}

impl OutputChannelState {
    /// Returns the number of lines in the content.
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            self.content.lines().count()
        }
    }
}

impl fmt::Display for OutputChannelState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = if self.visible { "visible" } else { "hidden" };
        write!(f, "{} ({}, {} lines)", self.descriptor, vis, self.line_count())
    }
}

/// Service for managing output channels.
pub struct OutputChannelService {
    channels: Vec<OutputChannelState>,
    active: Option<String>,
}

impl OutputChannelService {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            active: None,
        }
    }

    pub fn create_channel(&mut self, descriptor: OutputChannelDescriptor) -> String {
        let id = descriptor.id.clone();
        self.channels.push(OutputChannelState {
            descriptor,
            content: String::new(),
            visible: false,
        });
        id
    }

    pub fn append(&mut self, id: &str, text: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.content.push_str(text);
        }
    }

    pub fn append_line(&mut self, id: &str, text: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.content.push_str(text);
            ch.content.push('\n');
        }
    }

    pub fn clear(&mut self, id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.content.clear();
        }
    }

    pub fn get_content(&self, id: &str) -> Option<&str> {
        self.channels
            .iter()
            .find(|c| c.descriptor.id == id)
            .map(|c| c.content.as_str())
    }

    pub fn show(&mut self, id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.visible = true;
        }
    }

    pub fn hide(&mut self, id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.visible = false;
        }
    }

    pub fn set_active(&mut self, id: &str) {
        if self.channels.iter().any(|c| c.descriptor.id == id) {
            self.active = Some(id.to_string());
        }
    }

    pub fn get_active(&self) -> Option<&OutputChannelState> {
        self.active
            .as_ref()
            .and_then(|id| self.channels.iter().find(|c| c.descriptor.id == *id))
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Remove a channel by id. Returns an error if the channel does not exist.
    pub fn remove_channel(&mut self, id: &str) -> Result<(), OutputError> {
        let pos = self
            .channels
            .iter()
            .position(|c| c.descriptor.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        self.channels.remove(pos);
        if self.active.as_deref() == Some(id) {
            self.active = None;
        }
        Ok(())
    }

    /// Get a reference to a channel's state by id.
    pub fn get_channel(&self, id: &str) -> Option<&OutputChannelState> {
        self.channels.iter().find(|c| c.descriptor.id == id)
    }

    /// Find the first channel whose name matches the given string.
    pub fn find_by_name(&self, name: &str) -> Option<&OutputChannelState> {
        self.channels.iter().find(|c| c.descriptor.name == name)
    }

    /// Replace all content in a channel. Returns an error if the channel does not exist.
    pub fn replace_content(&mut self, id: &str, content: &str) -> Result<(), OutputError> {
        let ch = self
            .channels
            .iter_mut()
            .find(|c| c.descriptor.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        ch.content = content.to_string();
        Ok(())
    }

    /// Get the number of lines in a channel's content.
    pub fn get_line_count(&self, id: &str) -> Result<usize, OutputError> {
        let ch = self
            .channels
            .iter()
            .find(|c| c.descriptor.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        Ok(ch.line_count())
    }

    /// Search all channels for content containing the given query string.
    /// Returns a list of (channel id, matching line) pairs.
    pub fn search_content(&self, query: &str) -> Vec<(&str, &str)> {
        let mut results = Vec::new();
        for ch in &self.channels {
            for line in ch.content.lines() {
                if line.contains(query) {
                    results.push((ch.descriptor.id.as_str(), line));
                }
            }
        }
        results
    }
}

impl Default for OutputChannelService {
    fn default() -> Self {
        Self::new()
    }
}

/// Severity level for output log entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputSeverity {
    Info,
    Warning,
    Error,
    Debug,
}

impl fmt::Display for OutputSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputSeverity::Info => write!(f, "INFO"),
            OutputSeverity::Warning => write!(f, "WARNING"),
            OutputSeverity::Error => write!(f, "ERROR"),
            OutputSeverity::Debug => write!(f, "DEBUG"),
        }
    }
}

/// A structured log entry associated with an output channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub severity: OutputSeverity,
    pub message: String,
    pub timestamp_ms: u64,
    pub channel_id: String,
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.severity, self.message)
    }
}

impl LogEntry {
    /// Create a new log entry with the given severity and message.
    pub fn new(
        severity: OutputSeverity,
        message: impl Into<String>,
        channel_id: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            message: message.into(),
            timestamp_ms: 0,
            channel_id: channel_id.into(),
        }
    }

    /// Builder method to set the timestamp.
    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp_ms = ts;
        self
    }

    /// Returns `true` if the severity is `Error`.
    pub fn is_error(&self) -> bool {
        self.severity == OutputSeverity::Error
    }

    /// Returns `true` if the severity is `Warning`.
    pub fn is_warning(&self) -> bool {
        self.severity == OutputSeverity::Warning
    }

    /// Returns `true` if this entry's severity is at or above `min_severity`.
    pub fn matches_filter(&self, min_severity: &OutputSeverity) -> bool {
        severity_rank(&self.severity) >= severity_rank(min_severity)
    }
}

/// Returns a numeric rank for severity ordering: Debug=0, Info=1, Warning=2, Error=3.
pub fn severity_rank(s: &OutputSeverity) -> u8 {
    match s {
        OutputSeverity::Debug => 0,
        OutputSeverity::Info => 1,
        OutputSeverity::Warning => 2,
        OutputSeverity::Error => 3,
    }
}

impl OutputChannelService {
    /// Return all channel ids.
    pub fn channel_ids(&self) -> Vec<&str> {
        self.channels.iter().map(|c| c.descriptor.id.as_str()).collect()
    }

    /// Return only visible channels.
    pub fn visible_channels(&self) -> Vec<&OutputChannelState> {
        self.channels.iter().filter(|c| c.visible).collect()
    }

    /// Sum line counts across all channels.
    pub fn total_line_count(&self) -> usize {
        self.channels.iter().map(|c| c.line_count()).sum()
    }

    /// Return the channel with the most content bytes.
    pub fn longest_channel(&self) -> Option<&OutputChannelState> {
        self.channels.iter().max_by_key(|c| c.content.len())
    }

    /// Clear content from all channels.
    pub fn clear_all(&mut self) {
        for ch in &mut self.channels {
            ch.content.clear();
        }
    }

    /// Hide all channels.
    pub fn hide_all(&mut self) {
        for ch in &mut self.channels {
            ch.visible = false;
        }
    }

    /// Show all channels.
    pub fn show_all(&mut self) {
        for ch in &mut self.channels {
            ch.visible = true;
        }
    }

    /// Return the content of a channel split into lines.
    pub fn get_content_lines(&self, id: &str) -> Option<Vec<&str>> {
        self.channels
            .iter()
            .find(|c| c.descriptor.id == id)
            .map(|c| c.content.lines().collect())
    }
}

/// Accumulated statistics for wb-output operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbOutputStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbOutputStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &WbOutputStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for WbOutputStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbOutputStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbOutputStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-output.
#[derive(Debug, Clone)]
pub struct WbOutputValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbOutputValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for WbOutputValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// output_channel_scroll_lock — freeze output scroll position
// ---------------------------------------------------------------------------

/// Scroll lock state for an output channel.
#[derive(Debug, Clone)]
pub struct ScrollLockState {
    pub channel_id: String,
    pub locked: bool,
    /// The line number the view is frozen at (when locked).
    pub frozen_line: Option<usize>,
    /// Total lines when lock was engaged.
    pub lines_at_lock: usize,
}

impl ScrollLockState {
    pub fn new(channel_id: &str) -> Self {
        Self {
            channel_id: channel_id.to_string(),
            locked: false,
            frozen_line: None,
            lines_at_lock: 0,
        }
    }

    /// Number of lines added since the lock was engaged.
    pub fn lines_since_lock(&self, current_line_count: usize) -> usize {
        if !self.locked {
            return 0;
        }
        current_line_count.saturating_sub(self.lines_at_lock)
    }
}

impl fmt::Display for ScrollLockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.locked {
            write!(f, "scroll-lock ON at line {:?}", self.frozen_line)
        } else {
            write!(f, "scroll-lock OFF")
        }
    }
}

/// Engage scroll lock on a channel, freezing at the current last line.
pub fn output_channel_scroll_lock(
    service: &OutputChannelService,
    channel_id: &str,
) -> Result<ScrollLockState, OutputError> {
    let state = service
        .get_channel(channel_id)
        .ok_or_else(|| OutputError::ChannelNotFound(channel_id.to_string()))?;
    let line_count = state.line_count();
    Ok(ScrollLockState {
        channel_id: channel_id.to_string(),
        locked: true,
        frozen_line: if line_count > 0 { Some(line_count - 1) } else { Some(0) },
        lines_at_lock: line_count,
    })
}

/// Disengage scroll lock.
pub fn output_channel_scroll_unlock(lock: &mut ScrollLockState) {
    lock.locked = false;
    lock.frozen_line = None;
}

/// Toggle scroll lock on/off.
pub fn output_channel_scroll_toggle(
    lock: &mut ScrollLockState,
    service: &OutputChannelService,
) -> Result<(), OutputError> {
    if lock.locked {
        output_channel_scroll_unlock(lock);
    } else {
        let new_lock = output_channel_scroll_lock(service, &lock.channel_id)?;
        *lock = new_lock;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Output channel statistics & iteration
// ---------------------------------------------------------------------------

impl OutputChannelService {
    /// Returns all channel names.
    pub fn channel_names(&self) -> Vec<&str> {
        self.channels.iter().map(|c| c.descriptor.name.as_str()).collect()
    }

    /// Returns channels sorted by line count (descending).
    pub fn channels_by_size(&self) -> Vec<&OutputChannelState> {
        let mut channels: Vec<&OutputChannelState> = self.channels.iter().collect();
        channels.sort_by(|a, b| b.line_count().cmp(&a.line_count()));
        channels
    }

    /// Returns the total character count across all channels.
    pub fn total_char_count(&self) -> usize {
        self.channels.iter().map(|c| c.content.len()).sum()
    }

    /// Returns true if any channel contains the given text.
    pub fn any_channel_contains(&self, text: &str) -> bool {
        self.channels.iter().any(|c| c.content.contains(text))
    }

    /// Returns shortest channel by line count.
    pub fn shortest_channel(&self) -> Option<&OutputChannelState> {
        self.channels.iter().min_by_key(|c| c.line_count())
    }
}

impl fmt::Display for OutputChannelService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutputChannelService({} channels, {} total lines)",
            self.channel_count(),
            self.total_line_count(),
        )
    }
}

impl LogEntry {
    /// Returns a short one-line summary of this log entry.
    pub fn summary(&self) -> String {
        let msg = if self.message.len() > 60 {
            format!("{}...", &self.message[..57])
        } else {
            self.message.clone()
        };
        format!("[{}] {}", self.severity, msg)
    }

    /// Returns true if the severity is Info.
    pub fn is_info(&self) -> bool {
        matches!(self.severity, OutputSeverity::Info)
    }
}

impl OutputSeverity {
    /// Returns the numeric rank of this severity for comparison.
    pub fn rank(&self) -> u8 {
        severity_rank(self)
    }

    /// Returns true if this severity is at least as severe as `other`.
    pub fn at_least(&self, other: &OutputSeverity) -> bool {
        self.rank() >= other.rank()
    }
}

// ---------------------------------------------------------------------------
// OutputChannelFilter — filter output by severity or pattern
// ---------------------------------------------------------------------------

/// Filter configuration for an output channel.
#[derive(Debug, Clone)]
pub struct OutputChannelFilter {
    pub min_severity: Option<OutputSeverity>,
    pub pattern: Option<String>,
    pub exclude_pattern: Option<String>,
}

impl OutputChannelFilter {
    pub fn new() -> Self {
        Self {
            min_severity: None,
            pattern: None,
            exclude_pattern: None,
        }
    }

    pub fn with_severity(mut self, severity: OutputSeverity) -> Self {
        self.min_severity = Some(severity);
        self
    }

    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    pub fn with_exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_pattern = Some(pattern.into());
        self
    }

    /// Check if a log entry passes the filter.
    pub fn matches_entry(&self, entry: &LogEntry) -> bool {
        if let Some(ref min) = self.min_severity {
            if !entry.matches_filter(min) {
                return false;
            }
        }
        true
    }

    /// Check if a line of text passes the pattern filters.
    pub fn matches_line(&self, line: &str) -> bool {
        if let Some(ref pat) = self.pattern {
            if !line.contains(pat.as_str()) {
                return false;
            }
        }
        if let Some(ref excl) = self.exclude_pattern {
            if line.contains(excl.as_str()) {
                return false;
            }
        }
        true
    }

    /// Filter lines from channel content.
    pub fn filter_content<'a>(&self, content: &'a str) -> Vec<&'a str> {
        content.lines().filter(|l| self.matches_line(l)).collect()
    }
}

impl Default for OutputChannelFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OutputChannelExporter — export channel content
// ---------------------------------------------------------------------------

/// Export format for output channel content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    PlainText,
    Csv,
    Json,
}

/// Exports output channel content to various formats.
pub struct OutputChannelExporter;

impl OutputChannelExporter {
    /// Export raw content as plain text with optional line numbers.
    pub fn export_plain(content: &str, line_numbers: bool) -> String {
        if !line_numbers {
            return content.to_string();
        }
        content
            .lines()
            .enumerate()
            .map(|(i, l)| format!("{:>4}: {}", i + 1, l))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Export lines as CSV (line_number, content).
    pub fn export_csv(content: &str) -> String {
        let mut out = String::from("line,content\n");
        for (i, line) in content.lines().enumerate() {
            let escaped = line.replace('"', "\"\"");
            out.push_str(&format!("{},\"{}\"\n", i + 1, escaped));
        }
        out
    }

    /// Export lines as a JSON array of objects.
    pub fn export_json(content: &str) -> String {
        let entries: Vec<String> = content
            .lines()
            .enumerate()
            .map(|(i, l)| {
                let escaped = l.replace('\\', "\\\\").replace('"', "\\\"");
                format!("  {{\"line\": {}, \"text\": \"{}\"}}", i + 1, escaped)
            })
            .collect();
        format!("[\n{}\n]", entries.join(",\n"))
    }

    /// Export using the specified format.
    pub fn export(content: &str, format: ExportFormat) -> String {
        match format {
            ExportFormat::PlainText => Self::export_plain(content, true),
            ExportFormat::Csv => Self::export_csv(content),
            ExportFormat::Json => Self::export_json(content),
        }
    }
}

// ---------------------------------------------------------------------------
// OutputChannelSearch — search within output channels
// ---------------------------------------------------------------------------

/// A search match within a channel's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub channel_id: String,
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

/// Search across output channels.
pub struct OutputChannelSearch;

impl OutputChannelSearch {
    /// Find all occurrences of `query` in a channel's content.
    pub fn search_channel(channel_id: &str, content: &str, query: &str) -> Vec<SearchMatch> {
        let mut results = Vec::new();
        if query.is_empty() {
            return results;
        }
        for (line_num, line) in content.lines().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(query) {
                let abs_start = start + pos;
                results.push(SearchMatch {
                    channel_id: channel_id.to_string(),
                    line_number: line_num + 1,
                    line_content: line.to_string(),
                    match_start: abs_start,
                    match_end: abs_start + query.len(),
                });
                start = abs_start + query.len();
            }
        }
        results
    }

    /// Case-insensitive search.
    pub fn search_channel_ci(channel_id: &str, content: &str, query: &str) -> Vec<SearchMatch> {
        let lower_content = content.to_lowercase();
        let lower_query = query.to_lowercase();
        let mut results = Vec::new();
        if lower_query.is_empty() {
            return results;
        }
        for (line_num, (orig_line, lower_line)) in content.lines().zip(lower_content.lines()).enumerate() {
            let mut start = 0;
            while let Some(pos) = lower_line[start..].find(&lower_query) {
                let abs_start = start + pos;
                results.push(SearchMatch {
                    channel_id: channel_id.to_string(),
                    line_number: line_num + 1,
                    line_content: orig_line.to_string(),
                    match_start: abs_start,
                    match_end: abs_start + query.len(),
                });
                start = abs_start + query.len();
            }
        }
        results
    }

    /// Count total matches across all channels.
    pub fn count_matches(service: &OutputChannelService, query: &str) -> usize {
        let mut total = 0;
        for id in service.channel_ids() {
            if let Some(content) = service.get_content(id) {
                total += Self::search_channel(id, content, query).len();
            }
        }
        total
    }
}

// ---------------------------------------------------------------------------
// OutputChannelService — channel grouping
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// OutputChannelState — content manipulation helpers
// ---------------------------------------------------------------------------

impl OutputChannelState {
    /// Returns the byte length of the content.
    pub fn content_len(&self) -> usize {
        self.content.len()
    }

    /// Returns true if the content is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns a specific line by zero-based index, or `None` if out of range.
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.content.lines().nth(index)
    }

    /// Returns the last line of content, or `None` if empty.
    pub fn last_line(&self) -> Option<&str> {
        self.content.lines().last()
    }

    /// Returns true if content contains the given substring.
    pub fn contains(&self, needle: &str) -> bool {
        self.content.contains(needle)
    }

    /// Truncate content to the last `n` lines, discarding earlier lines.
    pub fn retain_last_lines(&mut self, n: usize) {
        let lines: Vec<&str> = self.content.lines().collect();
        if lines.len() <= n {
            return;
        }
        let start = lines.len() - n;
        let mut result = lines[start..].join("\n");
        if self.content.ends_with('\n') {
            result.push('\n');
        }
        self.content = result;
    }
}

// ---------------------------------------------------------------------------
// ScrollLockState — additional helpers
// ---------------------------------------------------------------------------

impl ScrollLockState {
    /// Returns true if the lock is currently engaged.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Returns the frozen line number, or 0 if not locked.
    pub fn frozen_line_or_zero(&self) -> usize {
        self.frozen_line.unwrap_or(0)
    }

    /// Reset lock state to unlocked defaults.
    pub fn reset(&mut self) {
        self.locked = false;
        self.frozen_line = None;
        self.lines_at_lock = 0;
    }
}

// ---------------------------------------------------------------------------
// LogStore — structured log entry storage and querying
// ---------------------------------------------------------------------------

/// A store for structured log entries with filtering and querying capabilities.
#[derive(Debug, Clone)]
pub struct LogStore {
    entries: Vec<LogEntry>,
    max_entries: Option<usize>,
}

impl LogStore {
    /// Create a new empty log store with no entry limit.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: None,
        }
    }

    /// Create a log store with a maximum number of entries (oldest evicted first).
    pub fn with_capacity(max: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: Some(max),
        }
    }

    /// Add a log entry. If a capacity limit is set, evicts the oldest entry when full.
    pub fn push(&mut self, entry: LogEntry) {
        if let Some(max) = self.max_entries {
            if self.entries.len() >= max && max > 0 {
                self.entries.remove(0);
            }
        }
        self.entries.push(entry);
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get all entries.
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Get entries filtered by minimum severity.
    pub fn filter_by_severity(&self, min: &OutputSeverity) -> Vec<&LogEntry> {
        self.entries.iter().filter(|e| e.matches_filter(min)).collect()
    }

    /// Get entries for a specific channel.
    pub fn filter_by_channel(&self, channel_id: &str) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.channel_id == channel_id)
            .collect()
    }

    /// Get entries matching both a channel id and minimum severity.
    pub fn filter(&self, channel_id: &str, min_severity: &OutputSeverity) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.channel_id == channel_id && e.matches_filter(min_severity))
            .collect()
    }

    /// Count entries by severity.
    pub fn count_by_severity(&self, severity: &OutputSeverity) -> usize {
        self.entries.iter().filter(|e| e.severity == *severity).count()
    }

    /// Get the most recent entry, or `None` if empty.
    pub fn last(&self) -> Option<&LogEntry> {
        self.entries.last()
    }

    /// Get entries whose message contains the given substring.
    pub fn search(&self, query: &str) -> Vec<&LogEntry> {
        self.entries.iter().filter(|e| e.message.contains(query)).collect()
    }

    /// Return entries within the given timestamp range (inclusive).
    pub fn in_time_range(&self, start_ms: u64, end_ms: u64) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp_ms >= start_ms && e.timestamp_ms <= end_ms)
            .collect()
    }

    /// Return all error entries.
    pub fn errors(&self) -> Vec<&LogEntry> {
        self.filter_by_severity(&OutputSeverity::Error)
    }

    /// Return all warning entries.
    pub fn warnings(&self) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.severity == OutputSeverity::Warning)
            .collect()
    }
}

impl Default for LogStore {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LogStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LogStore({} entries)", self.entries.len())
    }
}

/// A named group of output channels.
#[derive(Debug, Clone)]
pub struct OutputChannelGroup {
    pub name: String,
    pub channel_ids: Vec<String>,
}

impl OutputChannelGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            channel_ids: Vec::new(),
        }
    }

    pub fn add(&mut self, id: impl Into<String>) {
        self.channel_ids.push(id.into());
    }

    pub fn remove(&mut self, id: &str) {
        self.channel_ids.retain(|i| i != id);
    }

    pub fn contains(&self, id: &str) -> bool {
        self.channel_ids.iter().any(|i| i == id)
    }

    pub fn len(&self) -> usize {
        self.channel_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.channel_ids.is_empty()
    }

    /// Total line count across grouped channels.
    pub fn total_lines(&self, service: &OutputChannelService) -> usize {
        self.channel_ids
            .iter()
            .filter_map(|id| service.get_line_count(id).ok())
            .sum()
    }
}


// ---------------------------------------------------------------------------
// OutputLanguageMode -- syntax highlighting mode
// ---------------------------------------------------------------------------

pub struct OutputLanguageMode {
    pub channel_id: String,
    pub language_id: String,
}

impl OutputLanguageMode {
    pub fn new(channel_id: impl Into<String>, language_id: impl Into<String>) -> Self {
        Self { channel_id: channel_id.into(), language_id: language_id.into() }
    }

    pub fn is_log(&self) -> bool { self.language_id == "log" }
    pub fn is_plain(&self) -> bool { self.language_id == "plaintext" }
}

impl fmt::Display for OutputLanguageMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.channel_id, self.language_id)
    }
}

/// Manages language modes for output channels.
pub struct OutputLanguageModeRegistry {
    modes: std::collections::HashMap<String, String>,
}

impl OutputLanguageModeRegistry {
    pub fn new() -> Self { Self { modes: std::collections::HashMap::new() } }

    pub fn set_mode(&mut self, channel_id: impl Into<String>, language_id: impl Into<String>) {
        self.modes.insert(channel_id.into(), language_id.into());
    }

    pub fn get_mode(&self, channel_id: &str) -> Option<&str> {
        self.modes.get(channel_id).map(|s| s.as_str())
    }

    pub fn remove_mode(&mut self, channel_id: &str) -> bool {
        self.modes.remove(channel_id).is_some()
    }

    pub fn len(&self) -> usize { self.modes.len() }
    pub fn is_empty(&self) -> bool { self.modes.is_empty() }
}

impl Default for OutputLanguageModeRegistry { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// OutputChannelGroupByExtension
// ---------------------------------------------------------------------------

pub struct OutputChannelGroupByExtension {
    groups: std::collections::HashMap<String, Vec<String>>,
}

impl OutputChannelGroupByExtension {
    pub fn new() -> Self { Self { groups: std::collections::HashMap::new() } }

    pub fn add_channel(&mut self, extension: impl Into<String>, channel_id: impl Into<String>) {
        self.groups.entry(extension.into()).or_default().push(channel_id.into());
    }

    pub fn channels_for_extension(&self, ext: &str) -> Vec<&str> {
        self.groups.get(ext).map(|v| v.iter().map(|s| s.as_str()).collect()).unwrap_or_default()
    }

    pub fn extension_count(&self) -> usize { self.groups.len() }
    pub fn total_channels(&self) -> usize { self.groups.values().map(|v| v.len()).sum() }
}

impl Default for OutputChannelGroupByExtension { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// OutputLogLevelFilter
// ---------------------------------------------------------------------------

pub struct OutputLogLevelFilter {
    min_severity: OutputSeverity,
}

impl OutputLogLevelFilter {
    pub fn new(min: OutputSeverity) -> Self { Self { min_severity: min } }

    pub fn passes(&self, severity: &OutputSeverity) -> bool {
        severity_rank(severity) >= severity_rank(&self.min_severity)
    }

    pub fn filter_entries<'a>(&self, entries: &'a [LogEntry]) -> Vec<&'a LogEntry> {
        entries.iter().filter(|e| self.passes(&e.severity)).collect()
    }

    pub fn min_severity(&self) -> &OutputSeverity { &self.min_severity }

    pub fn set_min_severity(&mut self, severity: OutputSeverity) { self.min_severity = severity; }
}

impl Default for OutputLogLevelFilter {
    fn default() -> Self { Self::new(OutputSeverity::Info) }
}

impl fmt::Display for OutputLogLevelFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LogLevelFilter(min={})", self.min_severity)
    }
}

// ---------------------------------------------------------------------------
// OutputChannelAppendOptimizer
// ---------------------------------------------------------------------------

pub struct OutputChannelAppendOptimizer {
    buffer: String,
    flush_threshold: usize,
    total_appends: u64,
    total_flushes: u64,
}

impl OutputChannelAppendOptimizer {
    pub fn new(flush_threshold: usize) -> Self {
        Self { buffer: String::new(), flush_threshold, total_appends: 0, total_flushes: 0 }
    }

    pub fn append(&mut self, text: &str) -> Option<String> {
        self.buffer.push_str(text);
        self.total_appends += 1;
        if self.buffer.len() >= self.flush_threshold {
            self.total_flushes += 1;
            Some(std::mem::take(&mut self.buffer))
        } else {
            None
        }
    }

    pub fn flush(&mut self) -> Option<String> {
        if self.buffer.is_empty() { None }
        else {
            self.total_flushes += 1;
            Some(std::mem::take(&mut self.buffer))
        }
    }

    pub fn buffered_len(&self) -> usize { self.buffer.len() }
    pub fn total_appends(&self) -> u64 { self.total_appends }
    pub fn total_flushes(&self) -> u64 { self.total_flushes }

    pub fn efficiency(&self) -> f64 {
        if self.total_appends == 0 { return 1.0; }
        1.0 - (self.total_flushes as f64 / self.total_appends as f64)
    }
}

impl Default for OutputChannelAppendOptimizer {
    fn default() -> Self { Self::new(4096) }
}

impl fmt::Display for OutputChannelAppendOptimizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AppendOptimizer(buffered={}, flushes={})", self.buffered_len(), self.total_flushes)
    }
}


// === Output Scroll Position Saver ===

/// Output Scroll Position Saver implementation.
#[derive(Debug, Clone)]
pub struct OutputScrollPositionSaver {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: OutputScrollPositionSaverStats,
}

/// Statistics for OutputScrollPositionSaver.
#[derive(Debug, Clone, Default)]
pub struct OutputScrollPositionSaverStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl OutputScrollPositionSaverStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl OutputScrollPositionSaver {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: OutputScrollPositionSaverStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &OutputScrollPositionSaverStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for OutputScrollPositionSaver {
    fn default() -> Self {
        Self::new()
    }
}

// === Output Clear Handler ===

/// Priority level for OutputClearHandler items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutputClearHandlerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl OutputClearHandlerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for OutputClearHandlerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Output Clear Handler implementation.
#[derive(Debug, Clone)]
pub struct OutputClearHandler {
    items: Vec<OutputClearHandlerItem>,
    max_items: usize,
    default_priority: OutputClearHandlerPriority,
}

/// A single item in OutputClearHandler.
#[derive(Debug, Clone)]
pub struct OutputClearHandlerItem {
    pub id: String,
    pub label: String,
    pub priority: OutputClearHandlerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl OutputClearHandlerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: OutputClearHandlerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: OutputClearHandlerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl OutputClearHandler {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: OutputClearHandlerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: OutputClearHandlerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<OutputClearHandlerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&OutputClearHandlerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: OutputClearHandlerPriority) -> Vec<&OutputClearHandlerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&OutputClearHandlerItem> {
        let mut sorted: Vec<&OutputClearHandlerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&OutputClearHandlerItem> {
        let mut sorted: Vec<&OutputClearHandlerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&OutputClearHandlerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: OutputClearHandlerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> OutputClearHandlerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &OutputClearHandlerItem> {
        self.items.iter()
    }
}

impl Default for OutputClearHandler {
    fn default() -> Self {
        Self::new()
    }
}


// ─── OutBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for output lines.
#[derive(Debug, Clone)]
pub struct OutBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> OutBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for OutBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── OutBld Builder & Validator ─────────────────────────────

/// Builder for constructing output channel configurations.
#[derive(Debug, Clone)]
pub struct OutBldBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl OutBldBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<OutBldCfg, OutBldBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(OutBldBuildErr { errors }); }
        Ok(OutBldCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated output channel configuration.
#[derive(Debug, Clone)]
pub struct OutBldCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl OutBldCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &OutBldCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for OutBldCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutBldCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct OutBldBuildErr { pub errors: Vec<String> }

impl fmt::Display for OutBldBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutBldBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for OutBldBuildErr {}



// ---------------------------------------------------------------------------
// wb_output – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for workbench output panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbOutputOutputChannelKind {
    Log,
    Terminal,
    Debug,
    Extension,
}

impl YWbOutputOutputChannelKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Log => 0,
            Self::Terminal => 1,
            Self::Debug => 2,
            Self::Extension => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Log => "Log",
            Self::Terminal => "Terminal",
            Self::Debug => "Debug",
            Self::Extension => "Extension",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbOutputOutputChannelKind] {
        &[
            YWbOutputOutputChannelKind::Log,
            YWbOutputOutputChannelKind::Terminal,
            YWbOutputOutputChannelKind::Debug,
            YWbOutputOutputChannelKind::Extension,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbOutputOutputChannelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks output rotation data.
#[derive(Debug, Clone)]
pub struct YWbOutputOutputRotation {
    pub max_bytes: u64,
    pub current_bytes: u64,
    pub rotated: bool,
}

impl YWbOutputOutputRotation {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            max_bytes: 0,
            current_bytes: 0,
            rotated: false,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbOutputOutputRotation({}: {:?})", "max_bytes", self.max_bytes)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_output_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_output_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_output_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_output_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_output_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_output_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_output_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_output_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_output – Extended output highlighter helpers
// ---------------------------------------------------------------------------

/// Priority levels for output highlighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbOutputPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbOutputPriority {
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
    pub fn all_asc() -> [ZWbOutputPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbOutputPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks output highlighter data.
#[derive(Debug, Clone)]
pub struct ZWbOutputOutputHighlighter {
    pub patterns: Vec<(String, String)>,
    pub enabled: bool,
    pub match_count: u64,
}

impl ZWbOutputOutputHighlighter {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            enabled: false,
            match_count: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.patterns.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbOutputOutputHighlighter[enabled={:?}, match_count={:?}]", self.enabled, self.match_count)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for output highlighter.
pub fn z_wb_output_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_output_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_output_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_output_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_wb_output_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_output_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_output_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 218
// ---------------------------------------------------------------------------

/// Generic object pool `Xc218Pool<T>`.
pub struct Xc218Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc218Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc218PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc218Pool<T> {
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
    pub fn stats(&self) -> Xc218PoolStats {
        Xc218PoolStats {
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

impl<T> Default for Xc218Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc218Scheduler`.
pub struct Xc218Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc218Scheduler {
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

impl Default for Xc218Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_218 hash for the given byte slice.
pub fn xc_218_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_218 convention.
pub fn xc_218_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_19 deepening: state machine + event bus ---

/// States for the Xd19 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd19State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd19State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd19Transition {
    pub from: Xd19State,
    pub to: Xd19State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd19StateMachine {
    current: Xd19State,
    history: Vec<Xd19Transition>,
    step_counter: usize,
}

impl Xd19StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd19State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd19State {
        self.current
    }

    pub fn history(&self) -> &[Xd19Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd19State) -> Result<Xd19State, String> {
        let allowed = match (self.current, target) {
            (Xd19State::Idle, Xd19State::Running) => true,
            (Xd19State::Running, Xd19State::Paused) => true,
            (Xd19State::Running, Xd19State::Done) => true,
            (Xd19State::Paused, Xd19State::Running) => true,
            (Xd19State::Paused, Xd19State::Done) => true,
            (Xd19State::Done, Xd19State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_19: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd19Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd19SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd19State> {
        let prefix = "Xd19SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd19State::Idle),
            "Running" => Some(Xd19State::Running),
            "Paused" => Some(Xd19State::Paused),
            "Done" => Some(Xd19State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd19State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd19 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd19Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd19Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd19HandlerFn = Box<dyn Fn(&Xd19Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd19EventBus {
    handlers: Vec<(usize, Option<String>, Xd19HandlerFn)>,
    next_id: usize,
    published: Vec<Xd19Event>,
}

impl Xd19EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd19Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd19Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd19Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd19Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #17
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf17Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf17TrieNode {
    children: std::collections::HashMap<char, Xf17TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf17Trie {
    root: Xf17TrieNode,
    count: usize,
}

impl Xf17Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf17TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf17TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf17TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf17BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf17BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 217).
pub struct Xh217SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh217SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 259 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 217).
pub struct Xh217BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh217BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 217).
pub struct Xi217Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi217Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi217Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi217Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 217).
pub struct Xi217IntervalTree {
    xi_intervals: Vec<Xi217Interval>,
}

impl Xi217IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi217Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi217Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi217Interval) -> Vec<&Xi217Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi217Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi217Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi217Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi217Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi217Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi217Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 217) ---

/// Disjoint set / union-find for crate 217.
pub struct Xj217UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj217UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ217_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 217.
pub struct Xj217BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj217BTreeNode<K, V>>>,
    len: usize,
}

struct Xj217BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj217BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj217BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ217_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ217_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj217BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj217BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj217BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj217BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_217 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk217SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk217SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk217DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk217DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_217).
#[derive(Debug, Clone)]
pub struct Xl217Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl217Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_217).
#[derive(Debug, Clone)]
pub struct Xl217SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl217SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm217MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm217MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm217Tokenizer {
    text: String,
}

impl Xm217Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 217.
pub struct Xn217Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn217Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 217 -----

#[derive(Debug, Clone)]
struct Xn217AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn217AvlNode<K, V>>>,
    right: Option<Box<Xn217AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 217.
#[derive(Debug, Clone)]
pub struct Xn217AVL<K, V> {
    root: Option<Box<Xn217AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn217AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn217AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn217AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn217AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn217AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn217AvlNode<K, V>>) -> Box<Xn217AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn217AvlNode<K, V>>) -> Box<Xn217AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn217AvlNode<K, V>>) -> Box<Xn217AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn217AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn217AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn217AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn217AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn217AvlNode<K, V>>) -> &Xn217AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn217AvlNode<K, V>>) -> (Box<Xn217AvlNode<K, V>>, Option<Box<Xn217AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn217AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn217AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn217AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn217AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn217AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn217AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn217AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo217RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo217Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo217RBNode<K, V> {
    key: K,
    value: V,
    color: Xo217Color,
    left: Option<Box<Xo217RBNode<K, V>>>,
    right: Option<Box<Xo217RBNode<K, V>>>,
}

/// A red-black tree map for crate 217.
#[derive(Debug, Clone)]
pub struct Xo217RedBlack<K, V> {
    root: Option<Box<Xo217RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo217RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo217Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo217RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo217RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo217RBNode {
                    key, value, color: Xo217Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo217RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo217Color::Red)
    }

    fn xo_balance(mut h: Box<Xo217RBNode<K, V>>) -> Box<Xo217RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo217Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo217RBNode<K, V>>) -> Box<Xo217RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo217Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo217RBNode<K, V>>) -> Box<Xo217RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo217Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo217RBNode<K, V>>) {
        h.color = Xo217Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo217Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo217Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo217Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo217RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo217RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo217RBNode<K, V>) -> (K, V, Option<Box<Xo217RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo217RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo217Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo217RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo217ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 217.
#[derive(Debug, Clone)]
pub struct Xo217ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo217ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo217#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo217#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 217).
#[derive(Debug)]
pub struct Xp217SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp217Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp217Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp217Node<K, V>>>,
    xp_right: Option<Box<Xp217Node<K, V>>>,
}

impl<K: Ord, V> Xp217Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp217SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp217SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp217Node<K, V>>>, key: &K) -> Option<Box<Xp217Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp217Node<K, V>>) -> Box<Xp217Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp217Node<K, V>>) -> Box<Xp217Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp217Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp217Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp217Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq217Treap ---------------

use std::cmp::Ordering as Xq217Ord;

struct Xq217TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq217TreapNode<K, V>>>,
    right: Option<Box<Xq217TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq217Treap<K, V> {
    root: Option<Box<Xq217TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq217TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_217_size<K, V>(node: &Option<Box<Xq217TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_217_update_size<K, V>(node: &mut Xq217TreapNode<K, V>) {
    node.size = 1 + xq_217_size(&node.left) + xq_217_size(&node.right);
}

fn xq_217_rotate_right<K, V>(mut node: Box<Xq217TreapNode<K, V>>) -> Box<Xq217TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_217_update_size(&mut node);
    left.right = Some(node);
    xq_217_update_size(&mut left);
    left
}

fn xq_217_rotate_left<K, V>(mut node: Box<Xq217TreapNode<K, V>>) -> Box<Xq217TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_217_update_size(&mut node);
    right.left = Some(node);
    xq_217_update_size(&mut right);
    right
}

fn xq_217_insert_node<K: Ord, V>(
    node: Option<Box<Xq217TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq217TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq217TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq217Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq217Ord::Less => {
                let (new_left, old) = xq_217_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_217_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_217_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq217Ord::Greater => {
                let (new_right, old) = xq_217_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_217_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_217_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_217_remove_node<K: Ord, V>(
    node: Option<Box<Xq217TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq217TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq217Ord::Less => {
                let (new_left, old) = xq_217_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_217_update_size(&mut n);
                (Some(n), old)
            }
            Xq217Ord::Greater => {
                let (new_right, old) = xq_217_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_217_update_size(&mut n);
                (Some(n), old)
            }
            Xq217Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_217_rotate_right(n);
                    let (new_right, old) = xq_217_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_217_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_217_rotate_left(n);
                    let (new_left, old) = xq_217_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_217_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_217_find_min<K, V>(node: &Option<Box<Xq217TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_217_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_217_find_max<K, V>(node: &Option<Box<Xq217TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_217_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_217_rank<K: Ord, V>(node: &Option<Box<Xq217TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq217Ord::Less => xq_217_rank(&n.left, key),
            Xq217Ord::Equal => xq_217_size(&n.left),
            Xq217Ord::Greater => 1 + xq_217_size(&n.left) + xq_217_rank(&n.right, key),
        },
    }
}

fn xq_217_kth<K, V>(node: &Option<Box<Xq217TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_217_size(&n.left);
        if k < left_size {
            xq_217_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_217_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_217_in_order<K: Clone, V>(node: &Option<Box<Xq217TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_217_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_217_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq217Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 217 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_217_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq217Ord::Equal => return Some(&n.value),
                Xq217Ord::Less => cur = &n.left,
                Xq217Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_217_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_217_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_217_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_217_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_217_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_217_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_217_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq217VEBTree ---------------

pub struct Xq217VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq217VEBTree>>,
    clusters: Vec<Option<Box<Xq217VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq217VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq217VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq217VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr217KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr217KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr217BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr217KDNode {
    xr_point: Xr217KDPoint,
    xr_left: Option<Box<Xr217KDNode>>,
    xr_right: Option<Box<Xr217KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr217KDTree {
    xr_root: Option<Box<Xr217KDNode>>,
    xr_size: usize,
}

impl Xr217KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr217KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr217KDNode>>,
        point: Xr217KDPoint,
        depth: usize,
    ) -> Box<Xr217KDNode> {
        match node {
            None => Box::new(Xr217KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr217KDPoint) -> Option<Xr217KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr217KDNode>,
        query: &Xr217KDPoint,
        depth: usize,
        best: &mut Xr217KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr217KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr217KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr217KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr217KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr217KDNode>>, pts: &mut Vec<Xr217KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr217KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr217BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr217BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(id: &str) -> OutputChannelDescriptor {
        OutputChannelDescriptor {
            id: id.to_string(),
            name: id.to_string(),
            language_id: None,
            log: false,
        }
    }

    fn named_desc(id: &str, name: &str) -> OutputChannelDescriptor {
        OutputChannelDescriptor {
            id: id.to_string(),
            name: name.to_string(),
            language_id: None,
            log: false,
        }
    }

    #[test]
    fn create_and_append() {
        let mut svc = OutputChannelService::new();
        let id = svc.create_channel(desc("out"));
        svc.append(&id, "hello ");
        svc.append_line(&id, "world");
        assert_eq!(svc.get_content(&id), Some("hello world\n"));
    }

    #[test]
    fn clear_content() {
        let mut svc = OutputChannelService::new();
        let id = svc.create_channel(desc("out"));
        svc.append(&id, "data");
        svc.clear(&id);
        assert_eq!(svc.get_content(&id), Some(""));
    }

    #[test]
    fn active_channel() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        assert!(svc.get_active().is_none());
        svc.set_active("b");
        assert_eq!(svc.get_active().unwrap().descriptor.id, "b");
        assert_eq!(svc.channel_count(), 2);
    }

    #[test]
    fn output_error_display() {
        let e1 = OutputError::ChannelNotFound("x".into());
        assert_eq!(e1.to_string(), "channel not found: x");
        let e2 = OutputError::DuplicateChannel("y".into());
        assert_eq!(e2.to_string(), "duplicate channel: y");
    }

    #[test]
    fn descriptor_display_without_language() {
        let d = desc("log");
        assert_eq!(d.to_string(), "log(log)");
    }

    #[test]
    fn descriptor_display_with_language() {
        let d = desc("log").with_language("rust");
        assert_eq!(d.to_string(), "log(log) [rust]");
    }

    #[test]
    fn with_language_builder() {
        let d = desc("ch").with_language("json");
        assert_eq!(d.language_id.as_deref(), Some("json"));
    }

    #[test]
    fn state_display_and_line_count() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("s"));
        svc.append_line("s", "line1");
        svc.append_line("s", "line2");
        let ch = svc.get_channel("s").unwrap();
        assert_eq!(ch.line_count(), 2);
        assert_eq!(ch.to_string(), "s(s) (hidden, 2 lines)");
    }

    #[test]
    fn remove_channel_success() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("r"));
        assert_eq!(svc.channel_count(), 1);
        svc.remove_channel("r").unwrap();
        assert_eq!(svc.channel_count(), 0);
    }

    #[test]
    fn remove_channel_clears_active() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.set_active("a");
        svc.remove_channel("a").unwrap();
        assert!(svc.get_active().is_none());
    }

    #[test]
    fn remove_channel_not_found() {
        let mut svc = OutputChannelService::new();
        let err = svc.remove_channel("missing").unwrap_err();
        assert_eq!(err, OutputError::ChannelNotFound("missing".into()));
    }

    #[test]
    fn get_channel_and_find_by_name() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(named_desc("id1", "Build Output"));
        assert!(svc.get_channel("id1").is_some());
        assert!(svc.get_channel("nope").is_none());
        assert_eq!(
            svc.find_by_name("Build Output").unwrap().descriptor.id,
            "id1"
        );
        assert!(svc.find_by_name("Other").is_none());
    }

    #[test]
    fn replace_content_success_and_error() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("rc"));
        svc.append("rc", "old");
        svc.replace_content("rc", "new").unwrap();
        assert_eq!(svc.get_content("rc"), Some("new"));
        let err = svc.replace_content("bad", "x").unwrap_err();
        assert_eq!(err, OutputError::ChannelNotFound("bad".into()));
    }

    #[test]
    fn get_line_count_and_empty() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("lc"));
        assert_eq!(svc.get_line_count("lc").unwrap(), 0);
        svc.append_line("lc", "a");
        svc.append_line("lc", "b");
        svc.append_line("lc", "c");
        assert_eq!(svc.get_line_count("lc").unwrap(), 3);
        let err = svc.get_line_count("no").unwrap_err();
        assert_eq!(err, OutputError::ChannelNotFound("no".into()));
    }

    #[test]
    fn search_content_across_channels() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        svc.append_line("a", "error: something failed");
        svc.append_line("a", "info: all good");
        svc.append_line("b", "error: another failure");
        svc.append_line("b", "debug: trace");
        let results = svc.search_content("error");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a");
        assert_eq!(results[1].0, "b");
    }

    #[test]
    fn search_content_no_matches() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("x"));
        svc.append_line("x", "hello world");
        let results = svc.search_content("zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_output_severity_display() {
        assert_eq!(OutputSeverity::Info.to_string(), "INFO");
        assert_eq!(OutputSeverity::Warning.to_string(), "WARNING");
        assert_eq!(OutputSeverity::Error.to_string(), "ERROR");
        assert_eq!(OutputSeverity::Debug.to_string(), "DEBUG");
    }

    #[test]
    fn test_log_entry_new_and_display() {
        let entry = LogEntry::new(OutputSeverity::Info, "hello", "ch1");
        assert_eq!(entry.severity, OutputSeverity::Info);
        assert_eq!(entry.message, "hello");
        assert_eq!(entry.channel_id, "ch1");
        assert_eq!(entry.timestamp_ms, 0);
        assert_eq!(entry.to_string(), "[INFO] hello");
    }

    #[test]
    fn test_log_entry_with_timestamp() {
        let entry = LogEntry::new(OutputSeverity::Warning, "warn", "ch1")
            .with_timestamp(12345);
        assert_eq!(entry.timestamp_ms, 12345);
        assert_eq!(entry.to_string(), "[WARNING] warn");
    }

    #[test]
    fn test_log_entry_is_error_and_warning() {
        let err = LogEntry::new(OutputSeverity::Error, "fail", "ch1");
        assert!(err.is_error());
        assert!(!err.is_warning());

        let warn = LogEntry::new(OutputSeverity::Warning, "careful", "ch1");
        assert!(!warn.is_error());
        assert!(warn.is_warning());

        let info = LogEntry::new(OutputSeverity::Info, "ok", "ch1");
        assert!(!info.is_error());
        assert!(!info.is_warning());
    }

    #[test]
    fn test_severity_rank_ordering() {
        assert!(severity_rank(&OutputSeverity::Debug) < severity_rank(&OutputSeverity::Info));
        assert!(severity_rank(&OutputSeverity::Info) < severity_rank(&OutputSeverity::Warning));
        assert!(severity_rank(&OutputSeverity::Warning) < severity_rank(&OutputSeverity::Error));
        assert_eq!(severity_rank(&OutputSeverity::Debug), 0);
        assert_eq!(severity_rank(&OutputSeverity::Info), 1);
        assert_eq!(severity_rank(&OutputSeverity::Warning), 2);
        assert_eq!(severity_rank(&OutputSeverity::Error), 3);
    }

    #[test]
    fn test_log_entry_matches_filter() {
        let debug_entry = LogEntry::new(OutputSeverity::Debug, "d", "ch1");
        let info_entry = LogEntry::new(OutputSeverity::Info, "i", "ch1");
        let warn_entry = LogEntry::new(OutputSeverity::Warning, "w", "ch1");
        let error_entry = LogEntry::new(OutputSeverity::Error, "e", "ch1");

        // Debug entry only matches Debug filter
        assert!(debug_entry.matches_filter(&OutputSeverity::Debug));
        assert!(!debug_entry.matches_filter(&OutputSeverity::Info));

        // Error entry matches all filters
        assert!(error_entry.matches_filter(&OutputSeverity::Debug));
        assert!(error_entry.matches_filter(&OutputSeverity::Info));
        assert!(error_entry.matches_filter(&OutputSeverity::Warning));
        assert!(error_entry.matches_filter(&OutputSeverity::Error));

        // Info entry matches Debug and Info
        assert!(info_entry.matches_filter(&OutputSeverity::Debug));
        assert!(info_entry.matches_filter(&OutputSeverity::Info));
        assert!(!info_entry.matches_filter(&OutputSeverity::Warning));

        // Warning entry matches Debug, Info, Warning
        assert!(warn_entry.matches_filter(&OutputSeverity::Warning));
        assert!(!warn_entry.matches_filter(&OutputSeverity::Error));
    }

    #[test]
    fn test_channel_ids() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("alpha"));
        svc.create_channel(desc("beta"));
        svc.create_channel(desc("gamma"));
        let ids = svc.channel_ids();
        assert_eq!(ids, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_visible_channels() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        svc.create_channel(desc("c"));
        assert!(svc.visible_channels().is_empty());
        svc.show("a");
        svc.show("c");
        let visible = svc.visible_channels();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].descriptor.id, "a");
        assert_eq!(visible[1].descriptor.id, "c");
    }

    #[test]
    fn test_total_line_count() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        assert_eq!(svc.total_line_count(), 0);
        svc.append_line("a", "line1");
        svc.append_line("a", "line2");
        svc.append_line("b", "line1");
        assert_eq!(svc.total_line_count(), 3);
    }

    #[test]
    fn test_longest_channel() {
        let mut svc = OutputChannelService::new();
        assert!(svc.longest_channel().is_none());
        svc.create_channel(desc("short"));
        svc.create_channel(desc("long"));
        svc.append("short", "hi");
        svc.append("long", "this is a much longer string");
        let longest = svc.longest_channel().unwrap();
        assert_eq!(longest.descriptor.id, "long");
    }

    #[test]
    fn test_clear_all() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        svc.append("a", "data-a");
        svc.append("b", "data-b");
        svc.clear_all();
        assert_eq!(svc.get_content("a"), Some(""));
        assert_eq!(svc.get_content("b"), Some(""));
    }

    #[test]
    fn test_hide_all_and_show_all() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        svc.show("a");
        svc.show("b");
        assert_eq!(svc.visible_channels().len(), 2);

        svc.hide_all();
        assert!(svc.visible_channels().is_empty());

        svc.show_all();
        assert_eq!(svc.visible_channels().len(), 2);
    }

    #[test]
    fn test_get_content_lines() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("ch"));
        svc.append_line("ch", "first");
        svc.append_line("ch", "second");
        svc.append_line("ch", "third");
        let lines = svc.get_content_lines("ch").unwrap();
        assert_eq!(lines, vec!["first", "second", "third"]);
        assert!(svc.get_content_lines("missing").is_none());
    }

    #[test]
    fn wb_output_stats_new_defaults() {
        let stats = WbOutputStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_output_stats_record_success() {
        let mut stats = WbOutputStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_output_stats_record_failure() {
        let mut stats = WbOutputStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_output_stats_reset() {
        let mut stats = WbOutputStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_output_stats_merge() {
        let mut a = WbOutputStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbOutputStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn wb_output_stats_display() {
        let mut stats = WbOutputStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_output_stats_default() {
        let stats = WbOutputStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_output_validator_accepts_valid_name() {
        let v = WbOutputValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_output_validator_rejects_empty() {
        let v = WbOutputValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_output_validator_rejects_too_long() {
        let v = WbOutputValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_output_validator_forbidden_prefix() {
        let v = WbOutputValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_output_validator_allowed_chars() {
        let v = WbOutputValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_output_validator_range() {
        let v = WbOutputValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_output_sanitize_removes_control() {
        let result = WbOutputValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_output_truncate_short_string() {
        assert_eq!(WbOutputValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_output_truncate_long_string() {
        let result = WbOutputValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_output_is_ascii_printable() {
        assert!(WbOutputValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbOutputValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- output_channel_scroll_lock tests -----------------------------------

    fn make_output_service_with_content() -> OutputChannelService {
        let mut svc = OutputChannelService::new();
        svc.create_channel(OutputChannelDescriptor {
            id: "ch1".into(),
            name: "Build".into(),
            language_id: None,
            log: false,
        });
        svc.append("ch1", "line1\nline2\nline3\n");
        svc
    }

    #[test]
    fn scroll_lock_engages() {
        let svc = make_output_service_with_content();
        let lock = output_channel_scroll_lock(&svc, "ch1").unwrap();
        assert!(lock.locked);
        assert!(lock.frozen_line.is_some());
        assert!(lock.lines_at_lock > 0);
    }

    #[test]
    fn scroll_lock_channel_not_found() {
        let svc = OutputChannelService::new();
        let result = output_channel_scroll_lock(&svc, "missing");
        assert!(result.is_err());
    }

    #[test]
    fn scroll_unlock() {
        let svc = make_output_service_with_content();
        let mut lock = output_channel_scroll_lock(&svc, "ch1").unwrap();
        output_channel_scroll_unlock(&mut lock);
        assert!(!lock.locked);
        assert!(lock.frozen_line.is_none());
    }

    #[test]
    fn scroll_toggle() {
        let svc = make_output_service_with_content();
        let mut lock = ScrollLockState::new("ch1");
        output_channel_scroll_toggle(&mut lock, &svc).unwrap();
        assert!(lock.locked);
        output_channel_scroll_toggle(&mut lock, &svc).unwrap();
        assert!(!lock.locked);
    }

    #[test]
    fn scroll_lock_lines_since() {
        let lock = ScrollLockState {
            channel_id: "ch1".into(),
            locked: true,
            frozen_line: Some(3),
            lines_at_lock: 4,
        };
        assert_eq!(lock.lines_since_lock(10), 6);
    }

    #[test]
    fn scroll_lock_display() {
        let lock = ScrollLockState::new("ch1");
        assert!(format!("{lock}").contains("OFF"));
    }

    #[test]
    fn output_service_display() {
        let svc = OutputChannelService::new();
        let s = svc.to_string();
        assert!(s.contains("0 channels"));
    }

    #[test]
    fn output_channel_names() {
        let mut svc = OutputChannelService::new();
        let d = desc("git");
        svc.create_channel(d);
        let names = svc.channel_names();
        assert!(names.contains(&"git"));
    }

    #[test]
    fn output_total_char_count() {
        let mut svc = OutputChannelService::new();
        let d = desc("test");
        let id = svc.create_channel(d);
        svc.append(&id, "hello");
        assert_eq!(svc.total_char_count(), 5);
    }

    #[test]
    fn output_any_channel_contains() {
        let mut svc = OutputChannelService::new();
        let d = desc("t");
        let id = svc.create_channel(d);
        svc.append(&id, "error: failed");
        assert!(svc.any_channel_contains("error"));
        assert!(!svc.any_channel_contains("warning"));
    }

    #[test]
    fn log_entry_summary_short() {
        let entry = LogEntry::new(OutputSeverity::Info, "hello world", "ch1");
        let s = entry.summary();
        assert!(s.contains("[INFO]"));
        assert!(s.contains("hello world"));
    }

    #[test]
    fn output_severity_at_least() {
        assert!(OutputSeverity::Error.at_least(&OutputSeverity::Warning));
        assert!(OutputSeverity::Warning.at_least(&OutputSeverity::Info));
        assert!(!OutputSeverity::Info.at_least(&OutputSeverity::Error));
    }

    // --- New tests ---

    #[test]
    fn output_channel_filter_by_pattern() {
        let filter = OutputChannelFilter::new()
            .with_pattern("error")
            .with_exclude("debug");
        let content = "error: something failed\ndebug: error trace\ninfo: ok\nerror: again";
        let filtered = filter.filter_content(content);
        assert_eq!(filtered.len(), 2);
        assert!(filtered[0].contains("something failed"));
        assert!(filtered[1].contains("again"));
    }

    #[test]
    fn output_channel_filter_severity() {
        let filter = OutputChannelFilter::new().with_severity(OutputSeverity::Warning);
        let info = LogEntry::new(OutputSeverity::Info, "hello", "ch");
        let warn = LogEntry::new(OutputSeverity::Warning, "watch out", "ch");
        let err = LogEntry::new(OutputSeverity::Error, "fail", "ch");
        assert!(!filter.matches_entry(&info));
        assert!(filter.matches_entry(&warn));
        assert!(filter.matches_entry(&err));
    }

    #[test]
    fn output_channel_exporter_plain_text() {
        let content = "line one\nline two";
        let plain = OutputChannelExporter::export_plain(content, true);
        assert!(plain.contains("   1: line one"));
        assert!(plain.contains("   2: line two"));
        let no_nums = OutputChannelExporter::export_plain(content, false);
        assert_eq!(no_nums, content);
    }

    #[test]
    fn output_channel_exporter_csv() {
        let content = "hello\nworld";
        let csv = OutputChannelExporter::export_csv(content);
        assert!(csv.starts_with("line,content\n"));
        assert!(csv.contains("1,\"hello\""));
        assert!(csv.contains("2,\"world\""));
    }

    #[test]
    fn output_channel_exporter_json() {
        let content = "first\nsecond";
        let json = OutputChannelExporter::export_json(content);
        assert!(json.starts_with("["));
        assert!(json.ends_with("]"));
        assert!(json.contains("\"line\": 1"));
        assert!(json.contains("\"text\": \"first\""));
    }

    #[test]
    fn output_channel_search_basic() {
        let matches = OutputChannelSearch::search_channel("ch1", "error: foo\nok\nerror: bar", "error");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_number, 1);
        assert_eq!(matches[0].match_start, 0);
        assert_eq!(matches[1].line_number, 3);
    }

    #[test]
    fn output_channel_search_case_insensitive() {
        let matches = OutputChannelSearch::search_channel_ci("ch1", "Error: FOO\nok\nERROR: bar", "error");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn output_channel_search_count_across_channels() {
        let mut svc = OutputChannelService::new();
        let id1 = svc.create_channel(desc("ch1"));
        let id2 = svc.create_channel(desc("ch2"));
        svc.append(&id1, "error here\nok");
        svc.append(&id2, "another error\nmore error");
        let count = OutputChannelSearch::count_matches(&svc, "error");
        assert_eq!(count, 3);
    }

    #[test]
    fn output_channel_group_operations() {
        let mut group = OutputChannelGroup::new("Build");
        group.add("rust");
        group.add("cargo");
        assert_eq!(group.len(), 2);
        assert!(group.contains("rust"));
        group.remove("rust");
        assert!(!group.contains("rust"));
        assert_eq!(group.len(), 1);
    }

    #[test]
    fn output_channel_group_total_lines() {
        let mut svc = OutputChannelService::new();
        let id1 = svc.create_channel(desc("a"));
        let id2 = svc.create_channel(desc("b"));
        svc.append_line(&id1, "line1");
        svc.append_line(&id1, "line2");
        svc.append_line(&id2, "line3");

        let mut group = OutputChannelGroup::new("all");
        group.add("a");
        group.add("b");
        assert_eq!(group.total_lines(&svc), 3);
    }

    #[test]
    fn output_exporter_format_dispatch() {
        let content = "hello";
        let plain = OutputChannelExporter::export(content, ExportFormat::PlainText);
        assert!(plain.contains("1:"));
        let csv = OutputChannelExporter::export(content, ExportFormat::Csv);
        assert!(csv.contains("line,content"));
        let json = OutputChannelExporter::export(content, ExportFormat::Json);
        assert!(json.contains("\"text\""));
    }

    // --- OutputChannelState helpers ---

    #[test]
    fn channel_state_content_helpers() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("h"));
        assert!(svc.get_channel("h").unwrap().is_empty());
        assert_eq!(svc.get_channel("h").unwrap().content_len(), 0);

        svc.append_line("h", "alpha");
        svc.append_line("h", "beta");
        svc.append_line("h", "gamma");

        let ch = svc.get_channel("h").unwrap();
        assert!(!ch.is_empty());
        assert_eq!(ch.get_line(0), Some("alpha"));
        assert_eq!(ch.get_line(1), Some("beta"));
        assert_eq!(ch.get_line(5), None);
        assert_eq!(ch.last_line(), Some("gamma"));
        assert!(ch.contains("beta"));
        assert!(!ch.contains("delta"));
    }

    #[test]
    fn channel_state_retain_last_lines() {
        let mut state = OutputChannelState {
            descriptor: desc("r"),
            content: "a\nb\nc\nd\ne\n".to_string(),
            visible: false,
        };
        state.retain_last_lines(3);
        assert_eq!(state.line_count(), 3);
        assert_eq!(state.get_line(0), Some("c"));
        assert_eq!(state.get_line(2), Some("e"));
    }

    #[test]
    fn channel_state_retain_when_fewer_lines() {
        let mut state = OutputChannelState {
            descriptor: desc("r"),
            content: "a\nb\n".to_string(),
            visible: false,
        };
        state.retain_last_lines(10);
        assert_eq!(state.line_count(), 2);
    }

    // --- ScrollLockState helpers ---

    #[test]
    fn scroll_lock_state_helpers() {
        let mut lock = ScrollLockState::new("ch1");
        assert!(!lock.is_locked());
        assert_eq!(lock.frozen_line_or_zero(), 0);

        lock.locked = true;
        lock.frozen_line = Some(42);
        lock.lines_at_lock = 50;
        assert!(lock.is_locked());
        assert_eq!(lock.frozen_line_or_zero(), 42);

        lock.reset();
        assert!(!lock.is_locked());
        assert_eq!(lock.frozen_line_or_zero(), 0);
        assert_eq!(lock.lines_at_lock, 0);
    }

    // --- LogStore tests ---

    #[test]
    fn log_store_push_and_len() {
        let mut store = LogStore::new();
        assert!(store.is_empty());
        store.push(LogEntry::new(OutputSeverity::Info, "msg1", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Error, "msg2", "ch1"));
        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());
    }

    #[test]
    fn log_store_capacity_eviction() {
        let mut store = LogStore::with_capacity(3);
        store.push(LogEntry::new(OutputSeverity::Info, "a", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Info, "b", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Info, "c", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Info, "d", "ch1"));
        assert_eq!(store.len(), 3);
        // Oldest entry "a" should have been evicted
        assert_eq!(store.entries()[0].message, "b");
        assert_eq!(store.entries()[2].message, "d");
    }

    #[test]
    fn log_store_filter_by_severity() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Debug, "d", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Info, "i", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Warning, "w", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Error, "e", "ch1"));

        let warnings_up = store.filter_by_severity(&OutputSeverity::Warning);
        assert_eq!(warnings_up.len(), 2);
        assert_eq!(warnings_up[0].message, "w");
        assert_eq!(warnings_up[1].message, "e");
    }

    #[test]
    fn log_store_filter_by_channel() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Info, "a", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Info, "b", "ch2"));
        store.push(LogEntry::new(OutputSeverity::Info, "c", "ch1"));

        let ch1 = store.filter_by_channel("ch1");
        assert_eq!(ch1.len(), 2);
        let ch2 = store.filter_by_channel("ch2");
        assert_eq!(ch2.len(), 1);
    }

    #[test]
    fn log_store_combined_filter() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Debug, "d", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Error, "e1", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Error, "e2", "ch2"));
        store.push(LogEntry::new(OutputSeverity::Info, "i", "ch1"));

        let result = store.filter("ch1", &OutputSeverity::Warning);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message, "e1");
    }

    #[test]
    fn log_store_count_by_severity() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Error, "e1", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Error, "e2", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Info, "i", "ch1"));
        assert_eq!(store.count_by_severity(&OutputSeverity::Error), 2);
        assert_eq!(store.count_by_severity(&OutputSeverity::Info), 1);
        assert_eq!(store.count_by_severity(&OutputSeverity::Warning), 0);
    }

    #[test]
    fn log_store_search_and_last() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Info, "build started", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Error, "build failed: timeout", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Info, "build retry", "ch1"));

        let matches = store.search("build");
        assert_eq!(matches.len(), 3);
        let timeout = store.search("timeout");
        assert_eq!(timeout.len(), 1);
        assert!(timeout[0].is_error());

        let last = store.last().unwrap();
        assert_eq!(last.message, "build retry");
    }

    #[test]
    fn log_store_time_range() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Info, "a", "ch1").with_timestamp(100));
        store.push(LogEntry::new(OutputSeverity::Info, "b", "ch1").with_timestamp(200));
        store.push(LogEntry::new(OutputSeverity::Info, "c", "ch1").with_timestamp(300));
        store.push(LogEntry::new(OutputSeverity::Info, "d", "ch1").with_timestamp(400));

        let range = store.in_time_range(150, 350);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].message, "b");
        assert_eq!(range[1].message, "c");
    }

    #[test]
    fn log_store_errors_and_warnings() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Info, "i", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Warning, "w", "ch1"));
        store.push(LogEntry::new(OutputSeverity::Error, "e", "ch1"));

        assert_eq!(store.errors().len(), 1);
        assert_eq!(store.warnings().len(), 1);
    }

    #[test]
    fn log_store_clear_and_display() {
        let mut store = LogStore::new();
        store.push(LogEntry::new(OutputSeverity::Info, "x", "ch1"));
        assert_eq!(store.to_string(), "LogStore(1 entries)");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.to_string(), "LogStore(0 entries)");
    }

    #[test]
    fn log_store_default() {
        let store = LogStore::default();
        assert!(store.is_empty());
    }


    #[test]
    fn language_mode_basic() {
        let mode = OutputLanguageMode::new("ch1", "log");
        assert!(mode.is_log());
        assert!(!mode.is_plain());
    }

    #[test]
    fn language_mode_display() {
        let mode = OutputLanguageMode::new("ch1", "rust");
        assert!(format!("{mode}").contains("ch1:rust"));
    }

    #[test]
    fn language_mode_registry() {
        let mut reg = OutputLanguageModeRegistry::new();
        reg.set_mode("ch1", "log");
        assert_eq!(reg.get_mode("ch1"), Some("log"));
        assert!(reg.remove_mode("ch1"));
        assert!(reg.is_empty());
    }

    #[test]
    fn group_by_extension() {
        let mut g = OutputChannelGroupByExtension::new();
        g.add_channel("rust-analyzer", "ra-output");
        g.add_channel("rust-analyzer", "ra-trace");
        g.add_channel("eslint", "eslint-output");
        assert_eq!(g.channels_for_extension("rust-analyzer").len(), 2);
        assert_eq!(g.extension_count(), 2);
        assert_eq!(g.total_channels(), 3);
    }

    #[test]
    fn log_level_filter_basic() {
        let filter = OutputLogLevelFilter::new(OutputSeverity::Warning);
        assert!(filter.passes(&OutputSeverity::Error));
        assert!(filter.passes(&OutputSeverity::Warning));
        assert!(!filter.passes(&OutputSeverity::Info));
    }

    #[test]
    fn log_level_filter_entries() {
        let entries = vec![
            LogEntry::new(OutputSeverity::Info, "info msg", "ch1"),
            LogEntry::new(OutputSeverity::Error, "error msg", "ch1"),
        ];
        let filter = OutputLogLevelFilter::new(OutputSeverity::Warning);
        assert_eq!(filter.filter_entries(&entries).len(), 1);
    }

    #[test]
    fn log_level_filter_display() {
        let f = OutputLogLevelFilter::default();
        assert!(format!("{f}").contains("min="));
    }

    #[test]
    fn append_optimizer_buffering() {
        let mut opt = OutputChannelAppendOptimizer::new(10);
        assert!(opt.append("hi").is_none());
        assert_eq!(opt.buffered_len(), 2);
    }

    #[test]
    fn append_optimizer_flush_threshold() {
        let mut opt = OutputChannelAppendOptimizer::new(5);
        let result = opt.append("hello world");
        assert!(result.is_some());
    }

    #[test]
    fn append_optimizer_manual_flush() {
        let mut opt = OutputChannelAppendOptimizer::new(100);
        opt.append("data");
        let flushed = opt.flush();
        assert!(flushed.is_some());
        assert_eq!(opt.buffered_len(), 0);
    }

    #[test]
    fn append_optimizer_efficiency() {
        let mut opt = OutputChannelAppendOptimizer::new(100);
        opt.append("a");
        opt.append("b");
        opt.append("c");
        assert!(opt.efficiency() > 0.9);
    }

    #[test]
    fn append_optimizer_display() {
        let opt = OutputChannelAppendOptimizer::default();
        assert!(format!("{opt}").contains("buffered=0"));
    }

    #[test]
    fn group_by_extension_empty() {
        let g = OutputChannelGroupByExtension::new();
        assert!(g.channels_for_extension("x").is_empty());
    }


    #[test]
    fn outputScrollPositionSaver_new() {
        let s = OutputScrollPositionSaver::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn outputScrollPositionSaver_add_contains() {
        let mut s = OutputScrollPositionSaver::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn outputScrollPositionSaver_add_duplicate() {
        let mut s = OutputScrollPositionSaver::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn outputScrollPositionSaver_remove() {
        let mut s = OutputScrollPositionSaver::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn outputScrollPositionSaver_capacity() {
        let s = OutputScrollPositionSaver::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn outputScrollPositionSaver_search() {
        let mut s = OutputScrollPositionSaver::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn outputScrollPositionSaver_stats() {
        let mut s = OutputScrollPositionSaver::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn outputClearHandler_new() {
        let m = OutputClearHandler::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn outputClearHandler_add_find() {
        let mut m = OutputClearHandler::new();
        m.add(OutputClearHandlerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn outputClearHandler_priority_filter() {
        let mut m = OutputClearHandler::new();
        m.add(OutputClearHandlerItem::new("a", "A").with_priority(OutputClearHandlerPriority::High));
        m.add(OutputClearHandlerItem::new("b", "B").with_priority(OutputClearHandlerPriority::Low));
        m.add(OutputClearHandlerItem::new("c", "C").with_priority(OutputClearHandlerPriority::High));
        assert_eq!(m.by_priority(OutputClearHandlerPriority::High).len(), 2);
    }

    #[test]
    fn outputClearHandler_remove() {
        let mut m = OutputClearHandler::new();
        m.add(OutputClearHandlerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn outputClearHandler_search() {
        let mut m = OutputClearHandler::new();
        m.add(OutputClearHandlerItem::new("id1", "Hello World"));
        m.add(OutputClearHandlerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn outputClearHandler_total_weight() {
        let mut m = OutputClearHandler::new();
        m.add(OutputClearHandlerItem::new("a", "A").with_priority(OutputClearHandlerPriority::Critical));
        m.add(OutputClearHandlerItem::new("b", "B").with_priority(OutputClearHandlerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn outputClearHandler_capacity_limit() {
        let mut m = OutputClearHandler::new().with_max_items(2);
        m.add(OutputClearHandlerItem::new("1", "one"));
        m.add(OutputClearHandlerItem::new("2", "two"));
        assert!(!m.add(OutputClearHandlerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn outputClearHandler_sorted_by_priority() {
        let mut m = OutputClearHandler::new();
        m.add(OutputClearHandlerItem::new("lo", "Low").with_priority(OutputClearHandlerPriority::Low));
        m.add(OutputClearHandlerItem::new("hi", "High").with_priority(OutputClearHandlerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn outputClearHandler_item_metadata() {
        let mut item = OutputClearHandlerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn outputScrollPositionSaver_enabled_toggle() {
        let mut s = OutputScrollPositionSaver::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn outputClearHandler_priority_display() {
        assert_eq!(format!("{}", OutputClearHandlerPriority::High), "high");
        assert_eq!(format!("{}", OutputClearHandlerPriority::Low), "low");
    }


    #[test]
    fn outbuf_ringbuf_push_get() {
        let mut rb = OutBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn outbuf_ringbuf_overflow() {
        let mut rb = OutBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn outbuf_ringbuf_clear() {
        let mut rb = OutBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn outbuf_ringbuf_newest_oldest() {
        let mut rb = OutBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn outbuf_ringbuf_to_vec() {
        let mut rb = OutBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn outbuf_ringbuf_is_full() {
        let mut rb = OutBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn outbld_builder_valid() {
        let cfg = OutBldBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn outbld_builder_empty_name() {
        let r = OutBldBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn outbld_builder_bad_priority() {
        assert!(OutBldBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn outbld_builder_zero_max() {
        assert!(OutBldBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn outbld_cfg_merge() {
        let mut a = OutBldBuilder::new("a").property("x", "1").build().unwrap();
        let b = OutBldBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn outbld_cfg_display() {
        let cfg = OutBldBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- wb_output extended domain tests ----------------------------------------

    #[test]
    fn y_wb_output_enum_index() {
        assert_eq!(YWbOutputOutputChannelKind::Log.index(), 0);
        assert_eq!(YWbOutputOutputChannelKind::Terminal.index(), 1);
        assert_eq!(YWbOutputOutputChannelKind::Debug.index(), 2);
        assert_eq!(YWbOutputOutputChannelKind::Extension.index(), 3);
    }

    #[test]
    fn y_wb_output_enum_label() {
        assert_eq!(YWbOutputOutputChannelKind::Log.label(), "Log");
        assert_eq!(YWbOutputOutputChannelKind::Terminal.label(), "Terminal");
        assert_eq!(YWbOutputOutputChannelKind::Debug.label(), "Debug");
        assert_eq!(YWbOutputOutputChannelKind::Extension.label(), "Extension");
    }

    #[test]
    fn y_wb_output_enum_all() {
        let all = YWbOutputOutputChannelKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_output_enum_is_default() {
        assert!(YWbOutputOutputChannelKind::Log.is_default());
        assert!(!YWbOutputOutputChannelKind::Extension.is_default());
    }

    #[test]
    fn y_wb_output_enum_display() {
        assert_eq!(format!("{}", YWbOutputOutputChannelKind::Log), "Log");
    }

    #[test]
    fn y_wb_output_struct_new() {
        let s = YWbOutputOutputRotation::new();
        let _ = s.summary();
    }

    #[test]
    fn y_wb_output_fingerprint_deterministic() {
        let h1 = y_wb_output_fingerprint("hello");
        let h2 = y_wb_output_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_output_fingerprint("a"), y_wb_output_fingerprint("b"));
    }

    #[test]
    fn y_wb_output_truncate_short() {
        assert_eq!(y_wb_output_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_output_truncate_long() {
        let r = y_wb_output_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_output_normalize_key_basic() {
        assert_eq!(y_wb_output_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_output_split_path_basic() {
        let parts = y_wb_output_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_output_count_occurrences_basic() {
        assert_eq!(y_wb_output_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_output_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_output_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_output_in_range_basic() {
        assert!(y_wb_output_in_range(5, 1, 10));
        assert!(y_wb_output_in_range(1, 1, 10));
        assert!(y_wb_output_in_range(10, 1, 10));
        assert!(!y_wb_output_in_range(0, 1, 10));
        assert!(!y_wb_output_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_output_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_output_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_output_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_output_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_output Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_output_priority_weight() {
        assert_eq!(ZWbOutputPriority::Idle.weight(), 0);
        assert_eq!(ZWbOutputPriority::Normal.weight(), 2);
        assert_eq!(ZWbOutputPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_output_priority_label() {
        assert_eq!(ZWbOutputPriority::Low.label(), "low");
        assert_eq!(ZWbOutputPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_output_priority_is_elevated() {
        assert!(!ZWbOutputPriority::Normal.is_elevated());
        assert!(ZWbOutputPriority::High.is_elevated());
        assert!(ZWbOutputPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_output_priority_display() {
        assert_eq!(format!("{}", ZWbOutputPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_output_priority_all_asc() {
        let all = ZWbOutputPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbOutputPriority::Idle);
        assert_eq!(all[4], ZWbOutputPriority::Realtime);
    }

    #[test]
    fn z_wb_output_struct_new() {
        let s = ZWbOutputOutputHighlighter::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_output_struct_toggled_clone() {
        let s = ZWbOutputOutputHighlighter::new();
        let t = s.toggled_clone();
        let _ = t.match_count;
    }

    #[test]
    fn z_wb_output_rolling_hash_deterministic() {
        let h1 = z_wb_output_rolling_hash(b"test");
        let h2 = z_wb_output_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_output_rolling_hash(b"a"), z_wb_output_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_output_pad_to_basic() {
        assert_eq!(z_wb_output_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_output_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_output_is_identifier_basic() {
        assert!(z_wb_output_is_identifier("foo_bar"));
        assert!(z_wb_output_is_identifier("abc123"));
        assert!(!z_wb_output_is_identifier(""));
        assert!(!z_wb_output_is_identifier("has space"));
    }

    #[test]
    fn z_wb_output_levenshtein_basic() {
        assert_eq!(z_wb_output_levenshtein("", ""), 0);
        assert_eq!(z_wb_output_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_output_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_output_unique_words_basic() {
        let w = z_wb_output_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_output_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_output_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_output_common_prefix_basic() {
        assert_eq!(z_wb_output_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_output_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_output_struct_clear() {
        let mut s = ZWbOutputOutputHighlighter::new();
        s.patterns.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_output_rolling_hash_empty() {
        let h = z_wb_output_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 218 ----

    #[test]
    fn xc_218_pool_new_empty() {
        let pool: super::Xc218Pool<i32> = super::Xc218Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_218_pool_release_acquire() {
        let mut pool = super::Xc218Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_218_pool_acquire_empty() {
        let mut pool: super::Xc218Pool<i32> = super::Xc218Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_218_pool_full() {
        let mut pool = super::Xc218Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_218_pool_drain() {
        let mut pool = super::Xc218Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_218_pool_stats() {
        let mut pool = super::Xc218Pool::new(8);
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
    fn xc_218_pool_clear() {
        let mut pool = super::Xc218Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_218_pool_shrink() {
        let mut pool = super::Xc218Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_218_pool_default() {
        let pool: super::Xc218Pool<String> = super::Xc218Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_218_pool_extend() {
        let mut pool = super::Xc218Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_218_pool_retain() {
        let mut pool = super::Xc218Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_218_scheduler_round_robin() {
        let mut sched = super::Xc218Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_218_scheduler_empty() {
        let mut sched = super::Xc218Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_218_scheduler_reset() {
        let mut sched = super::Xc218Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_218_scheduler_add_remove() {
        let mut sched = super::Xc218Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_218_scheduler_targets() {
        let sched = super::Xc218Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_218_hash_empty() {
        assert_eq!(super::xc_218_hash(b""), 5381);
    }

    #[test]
    fn xc_218_hash_data() {
        let h = super::xc_218_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_218_hash(b"hello"), h);
    }

    #[test]
    fn xc_218_reverse_str() {
        assert_eq!(super::xc_218_reverse("abc"), "cba");
        assert_eq!(super::xc_218_reverse(""), "");
    }


    // --- xd_19 deepening tests ---

    #[test]
    fn xd_19_sm_initial_state() {
        let sm = Xd19StateMachine::new();
        assert_eq!(sm.current_state(), Xd19State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_19_sm_valid_idle_to_running() {
        let mut sm = Xd19StateMachine::new();
        assert!(sm.transition(Xd19State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd19State::Running);
    }

    #[test]
    fn xd_19_sm_valid_running_to_paused() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        assert!(sm.transition(Xd19State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd19State::Paused);
    }

    #[test]
    fn xd_19_sm_valid_running_to_done() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        assert!(sm.transition(Xd19State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd19State::Done);
    }

    #[test]
    fn xd_19_sm_valid_paused_to_running() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        sm.transition(Xd19State::Paused).unwrap();
        assert!(sm.transition(Xd19State::Running).is_ok());
    }

    #[test]
    fn xd_19_sm_valid_done_to_idle() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        sm.transition(Xd19State::Done).unwrap();
        assert!(sm.transition(Xd19State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd19State::Idle);
    }

    #[test]
    fn xd_19_sm_invalid_idle_to_done() {
        let mut sm = Xd19StateMachine::new();
        assert!(sm.transition(Xd19State::Done).is_err());
    }

    #[test]
    fn xd_19_sm_invalid_idle_to_paused() {
        let mut sm = Xd19StateMachine::new();
        assert!(sm.transition(Xd19State::Paused).is_err());
    }

    #[test]
    fn xd_19_sm_history_tracking() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        sm.transition(Xd19State::Paused).unwrap();
        sm.transition(Xd19State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd19State::Idle);
        assert_eq!(sm.history()[0].to, Xd19State::Running);
        assert_eq!(sm.history()[1].from, Xd19State::Running);
        assert_eq!(sm.history()[2].to, Xd19State::Done);
    }

    #[test]
    fn xd_19_sm_serialize_deserialize() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd19StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd19State::Running));
    }

    #[test]
    fn xd_19_sm_deserialize_invalid() {
        assert_eq!(Xd19StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_19_sm_reset() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd19State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_19_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd19EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd19Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_19_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd19EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd19Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd19Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_19_bus_unsubscribe() {
        let mut bus = Xd19EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_19_event_kind_and_payload() {
        let e = Xd19Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd19Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_19_bus_clear_history() {
        let mut bus = Xd19EventBus::new();
        bus.publish(Xd19Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_19_sm_step_counter_increments() {
        let mut sm = Xd19StateMachine::new();
        sm.transition(Xd19State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd19State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #17 --

    #[test]
    fn xf17_trie_insert_search() {
        let mut t = Xf17Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf17_trie_starts_with() {
        let mut t = Xf17Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf17_trie_remove() {
        let mut t = Xf17Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf17_trie_word_count() {
        let mut t = Xf17Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf17_trie_longest_prefix() {
        let mut t = Xf17Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf17_trie_all_words() {
        let mut t = Xf17Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf17_trie_autocomplete() {
        let mut t = Xf17Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf17_trie_empty_search() {
        let t = Xf17Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf17_bloom_add_contains() {
        let mut bf = Xf17BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf17_bloom_probably_absent() {
        let bf = Xf17BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf17_bloom_false_positive_rate() {
        let mut bf = Xf17BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf17_bloom_clear() {
        let mut bf = Xf17BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf17_bloom_union() {
        let mut a = Xf17BloomFilter::xf_new(512, 2);
        let mut b = Xf17BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf17_bloom_intersection_estimate() {
        let mut a = Xf17BloomFilter::xf_new(512, 2);
        let mut b = Xf17BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf17_bloom_union_size_mismatch() {
        let a = Xf17BloomFilter::xf_new(256, 2);
        let b = Xf17BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh217_skip_insert_contains() {
        let mut sl = super::Xh217SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh217_skip_remove() {
        let mut sl = super::Xh217SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh217_skip_len() {
        let mut sl = super::Xh217SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh217_skip_range_query() {
        let mut sl = super::Xh217SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh217_skip_floor_ceiling() {
        let mut sl = super::Xh217SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh217_skip_rank() {
        let mut sl = super::Xh217SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh217_skip_empty() {
        let sl = super::Xh217SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh217_skip_duplicates() {
        let mut sl = super::Xh217SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh217_bitset_set_test() {
        let mut bs = super::Xh217BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh217_bitset_clear_count() {
        let mut bs = super::Xh217BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh217_bitset_and_or_xor() {
        let mut a = super::Xh217BitSet::xh_new(128);
        let mut b = super::Xh217BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh217_bitset_iter_ones() {
        let mut bs = super::Xh217BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh217_bitset_first_last() {
        let mut bs = super::Xh217BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh217_bitset_empty() {
        let bs = super::Xh217BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi217_deque_push_pop_back() {
        let mut dq = super::Xi217Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi217_deque_push_pop_front() {
        let mut dq = super::Xi217Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi217_deque_mixed_ops() {
        let mut dq = super::Xi217Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi217_deque_get_and_split() {
        let mut dq = super::Xi217Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi217_deque_rotate_left() {
        let mut dq = super::Xi217Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi217_deque_rotate_right() {
        let mut dq = super::Xi217Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi217_deque_grow() {
        let mut dq = super::Xi217Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi217_deque_empty() {
        let dq = super::Xi217Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi217_interval_tree_insert_query() {
        let mut tree = super::Xi217IntervalTree::xi_new();
        tree.xi_insert(super::Xi217Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi217Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi217Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi217_interval_tree_overlap() {
        let mut tree = super::Xi217IntervalTree::xi_new();
        tree.xi_insert(super::Xi217Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi217Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi217Interval::xi_new(12, 20));
        let q = super::Xi217Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi217_interval_tree_remove() {
        let mut tree = super::Xi217IntervalTree::xi_new();
        tree.xi_insert(super::Xi217Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi217Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi217_interval_tree_gaps() {
        let mut tree = super::Xi217IntervalTree::xi_new();
        tree.xi_insert(super::Xi217Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi217Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi217Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi217Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi217Interval::xi_new(8, 10));
    }

    #[test]
    fn xi217_interval_tree_merge() {
        let mut tree = super::Xi217IntervalTree::xi_new();
        tree.xi_insert(super::Xi217Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi217Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi217Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi217Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi217Interval::xi_new(10, 15));
    }

    #[test]
    fn xi217_interval_tree_all() {
        let mut tree = super::Xi217IntervalTree::xi_new();
        tree.xi_insert(super::Xi217Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi217Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi217_interval_tree_empty() {
        let tree = super::Xi217IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi217_interval_tree_contains_point() {
        let iv = super::Xi217Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 217) ---

    #[test]
    fn xj_217_uf_make_and_find() {
        let mut uf = super::Xj217UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_217_uf_union_connected() {
        let mut uf = super::Xj217UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_217_uf_component_count() {
        let mut uf = super::Xj217UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_217_uf_component_size() {
        let mut uf = super::Xj217UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_217_uf_largest_component() {
        let mut uf = super::Xj217UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_217_uf_many_elements() {
        let mut uf = super::Xj217UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_217_uf_separate_components() {
        let mut uf = super::Xj217UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_217_uf_path_compression() {
        let mut uf = super::Xj217UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_217_bt_insert_get() {
        let mut bt = super::Xj217BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_217_bt_contains_len() {
        let mut bt = super::Xj217BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_217_bt_replace() {
        let mut bt = super::Xj217BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_217_bt_remove() {
        let mut bt = super::Xj217BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_217_bt_keys_values() {
        let mut bt = super::Xj217BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_217_bt_range() {
        let mut bt = super::Xj217BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_217_bt_min_max() {
        let mut bt = super::Xj217BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_217_bt_many_inserts() {
        let mut bt = super::Xj217BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_217 segment tree tests ---

    #[test]
    fn xk_217_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk217SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_217_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk217SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_217_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk217SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_217_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk217SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_217_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk217SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_217_st_single_element() {
        let data = vec![42];
        let st = super::Xk217SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_217_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk217SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_217_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk217SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_217 disjoint intervals tests ---

    #[test]
    fn xk_217_di_add_and_count() {
        let mut di = super::Xk217DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_217_di_merge_overlap() {
        let mut di = super::Xk217DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_217_di_contains() {
        let mut di = super::Xk217DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_217_di_remove() {
        let mut di = super::Xk217DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_217_di_covered_length() {
        let mut di = super::Xk217DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_217_di_gaps() {
        let mut di = super::Xk217DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_217_di_merge_adjacent() {
        let mut di = super::Xk217DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_217_di_empty() {
        let di = super::Xk217DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_217_rope_new_empty() {
        let rope = super::Xl217Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_217_rope_from_str() {
        let rope = super::Xl217Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_217_rope_insert_at() {
        let mut rope = super::Xl217Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_217_rope_delete_range() {
        let mut rope = super::Xl217Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_217_rope_char_at() {
        let rope = super::Xl217Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_217_rope_split_concat() {
        let rope = super::Xl217Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_217_rope_line_count() {
        let rope = super::Xl217Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_217_rope_line_at() {
        let rope = super::Xl217Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_217_sa_build_and_search() {
        let sa = super::Xl217SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_217_sa_count() {
        let sa = super::Xl217SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_217_sa_longest_repeated() {
        let sa = super::Xl217SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_217_sa_all_positions() {
        let sa = super::Xl217SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_217_sa_len() {
        let sa = super::Xl217SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_217_sa_empty() {
        let sa = super::Xl217SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_217_rope_slice() {
        let rope = super::Xl217Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_217_sa_search_start() {
        let sa = super::Xl217SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_217_sparse_set_get() {
        let mut m = super::Xm217MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_217_sparse_row_col() {
        let mut m = super::Xm217MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_217_sparse_transpose() {
        let mut m = super::Xm217MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_217_sparse_multiply_vec() {
        let mut m = super::Xm217MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_217_sparse_nnz_density() {
        let mut m = super::Xm217MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_217_sparse_clear() {
        let mut m = super::Xm217MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_217_sparse_overwrite_zero() {
        let mut m = super::Xm217MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_217_tokenizer_basic() {
        let t = super::Xm217Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_217_tokenizer_count() {
        let t = super::Xm217Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_217_tokenizer_unique() {
        let t = super::Xm217Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_217_tokenizer_frequency() {
        let t = super::Xm217Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_217_tokenizer_delimiter() {
        let t = super::Xm217Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_217_tokenizer_whitespace() {
        let t = super::Xm217Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_217_tokenizer_empty() {
        let t = super::Xm217Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 217 ----

    #[test]
    fn xn_217_fenwick_prefix_sum() {
        let mut ft = super::Xn217Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_217_fenwick_range_sum() {
        let mut ft = super::Xn217Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_217_fenwick_point_query() {
        let mut ft = super::Xn217Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_217_fenwick_len() {
        let ft = super::Xn217Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_217_fenwick_multiple_updates() {
        let mut ft = super::Xn217Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_217_fenwick_single_element() {
        let mut ft = super::Xn217Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_217_fenwick_find_kth() {
        let mut ft = super::Xn217Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_217_fenwick_negative_delta() {
        let mut ft = super::Xn217Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 217 ----

    #[test]
    fn xn_217_avl_insert_get() {
        let mut m = super::Xn217AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_217_avl_remove() {
        let mut m = super::Xn217AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_217_avl_in_order() {
        let mut m = super::Xn217AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_217_avl_min_max() {
        let mut m = super::Xn217AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_217_avl_floor_ceiling() {
        let mut m = super::Xn217AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_217_avl_height_balanced() {
        let mut m = super::Xn217AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_217_avl_overwrite() {
        let mut m = super::Xn217AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_217_avl_empty() {
        let m: super::Xn217AVL<i32, i32> = super::Xn217AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo217RedBlack tests ---

    #[test]
    fn xo_217_rb_insert_and_get() {
        let mut tree = super::Xo217RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_217_rb_len_and_empty() {
        let mut tree = super::Xo217RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_217_rb_min_max() {
        let mut tree = super::Xo217RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_217_rb_contains() {
        let mut tree = super::Xo217RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_217_rb_remove() {
        let mut tree = super::Xo217RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_217_rb_in_order() {
        let mut tree = super::Xo217RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_217_rb_black_height() {
        let mut tree = super::Xo217RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_217_rb_overwrite() {
        let mut tree = super::Xo217RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo217ConsistentHash tests ---

    #[test]
    fn xo_217_ch_add_and_count() {
        let mut ring = super::Xo217ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_217_ch_remove_node() {
        let mut ring = super::Xo217ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_217_ch_get_node() {
        let mut ring = super::Xo217ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_217_ch_empty_ring() {
        let ring = super::Xo217ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_217_ch_distribution() {
        let mut ring = super::Xo217ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_217_ch_rebalance() {
        let mut ring = super::Xo217ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_217_ch_virtual_nodes() {
        let mut ring = super::Xo217ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_217_ch_consistent_lookup() {
        let mut ring = super::Xo217ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_217_splay_insert_get() {
        let mut t = super::Xp217SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_217_splay_remove() {
        let mut t = super::Xp217SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_217_splay_count_increases() {
        let mut t = super::Xp217SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_217_splay_depth() {
        let mut t = super::Xp217SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_217_splay_len_empty() {
        let t = super::Xp217SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_217_splay_min_max() {
        let mut t = super::Xp217SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_217_splay_overwrite() {
        let mut t = super::Xp217SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_217_splay_remove_missing() {
        let mut t = super::Xp217SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_217 treap tests ----
    #[test]
    fn xq_217_treap_empty() {
        let t = super::Xq217Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_217_treap_insert_get() {
        let mut t = super::Xq217Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_217_treap_overwrite() {
        let mut t = super::Xq217Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_217_treap_remove() {
        let mut t = super::Xq217Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_217_treap_min_max() {
        let mut t = super::Xq217Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_217_treap_rank() {
        let mut t = super::Xq217Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_217_treap_kth() {
        let mut t = super::Xq217Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_217_treap_in_order() {
        let mut t = super::Xq217Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_217 VEB tree tests ----
    #[test]
    fn xq_217_veb_empty() {
        let v = super::Xq217VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_217_veb_insert_contains() {
        let mut v = super::Xq217VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_217_veb_min_max() {
        let mut v = super::Xq217VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_217_veb_delete() {
        let mut v = super::Xq217VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_217_veb_successor() {
        let mut v = super::Xq217VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_217_veb_predecessor() {
        let mut v = super::Xq217VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_217_veb_count() {
        let mut v = super::Xq217VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_217_veb_duplicate_insert() {
        let mut v = super::Xq217VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_217_kdtree_empty() {
        let tree = super::Xr217KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_217_kdtree_insert_one() {
        let mut tree = super::Xr217KDTree::xr_new();
        tree.xr_insert(super::Xr217KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_217_kdtree_insert_multiple() {
        let mut tree = super::Xr217KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr217KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_217_kdtree_nearest_neighbor() {
        let mut tree = super::Xr217KDTree::xr_new();
        tree.xr_insert(super::Xr217KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr217KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr217KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_217_kdtree_nn_empty() {
        let tree = super::Xr217KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr217KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_217_kdtree_range_search() {
        let mut tree = super::Xr217KDTree::xr_new();
        tree.xr_insert(super::Xr217KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr217KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr217KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_217_kdtree_range_empty() {
        let mut tree = super::Xr217KDTree::xr_new();
        tree.xr_insert(super::Xr217KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_217_kdtree_all_points() {
        let mut tree = super::Xr217KDTree::xr_new();
        tree.xr_insert(super::Xr217KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr217KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_217_kdtree_depth() {
        let mut tree = super::Xr217KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr217KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_217_kdtree_bounding_box() {
        let mut tree = super::Xr217KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr217KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr217KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}