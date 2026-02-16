//! Output panel channels.

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
}
