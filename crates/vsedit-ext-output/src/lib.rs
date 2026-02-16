//! Ext API: Output.
//!
//! RPC bridge between the extension host and the main thread for output channels.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_output";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OutputMessage {
    CreateChannel {
        name: String,
        language_id: Option<String>,
    },
    AppendLine {
        channel_id: String,
        line: String,
    },
    Clear {
        channel_id: String,
    },
    Show {
        channel_id: String,
        preserve_focus: bool,
    },
    Dispose {
        channel_id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputChannel {
    pub id: String,
    pub name: String,
    pub language_id: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogOutputChannel {
    pub id: String,
    pub name: String,
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

// ── Error Types ──

/// Errors that can occur during output channel operations.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputError {
    /// The referenced channel does not exist.
    ChannelNotFound(String),
    /// A channel with the given name already exists.
    DuplicateChannelName(String),
    /// The provided name is empty or invalid.
    InvalidName(String),
    /// The content exceeds the maximum buffer size.
    BufferOverflow { channel_id: String, max_bytes: usize },
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::ChannelNotFound(id) => write!(f, "output channel not found: {id}"),
            OutputError::DuplicateChannelName(name) => {
                write!(f, "output channel already exists: {name}")
            }
            OutputError::InvalidName(reason) => {
                write!(f, "invalid channel name: {reason}")
            }
            OutputError::BufferOverflow { channel_id, max_bytes } => {
                write!(f, "channel {channel_id} exceeded {max_bytes} byte limit")
            }
        }
    }
}

impl std::error::Error for OutputError {}

// ── LogLevel helpers ──

impl LogLevel {
    /// Numeric severity (higher = more severe).
    pub fn severity(self) -> u8 {
        match self {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warning => 3,
            LogLevel::Error => 4,
        }
    }

    /// Returns `true` if this level is at least as severe as `threshold`.
    pub fn is_enabled(self, threshold: LogLevel) -> bool {
        self.severity() >= threshold.severity()
    }

    /// Parse a log level from a case-insensitive string.
    pub fn from_str(s: &str) -> Option<LogLevel> {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Some(LogLevel::Trace),
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warning" | "warn" => Some(LogLevel::Warning),
            "error" => Some(LogLevel::Error),
            _ => None,
        }
    }

    /// Return all log level variants in severity order.
    pub fn all_levels() -> &'static [LogLevel] {
        &[
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warning,
            LogLevel::Error,
        ]
    }

    /// Human-readable label used for formatted log lines.
    pub fn label(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ── OutputChannel helpers ──

impl OutputChannel {
    /// Returns `true` if the channel has no content.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Number of lines currently in the buffer.
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            self.content.lines().count()
        }
    }

    /// Total byte length of the content buffer.
    pub fn byte_len(&self) -> usize {
        self.content.len()
    }

    /// Returns the last `n` lines of the content.
    pub fn tail_lines(&self, n: usize) -> Vec<&str> {
        let lines: Vec<&str> = self.content.lines().collect();
        let start = lines.len().saturating_sub(n);
        lines[start..].to_vec()
    }
}

impl fmt::Display for OutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} ({} lines)", self.id, self.name, self.line_count())
    }
}

// ── LogOutputChannel helpers ──

impl LogOutputChannel {
    /// Format a log message respecting the current log level filter.
    /// Returns `None` if the message level is below the channel threshold.
    pub fn format_message(&self, level: LogLevel, message: &str) -> Option<String> {
        if level.is_enabled(self.log_level) {
            Some(format!("[{}] {}: {}", self.name, level.label(), message))
        } else {
            None
        }
    }
}

impl fmt::Display for LogOutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} (level={})", self.id, self.name, self.log_level)
    }
}

// ── ChannelBuilder ──

/// Builder for constructing an `OutputChannel` with validation.
#[derive(Debug, Clone)]
pub struct ChannelBuilder {
    name: String,
    language_id: Option<String>,
    initial_content: Option<String>,
}

impl ChannelBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            language_id: None,
            initial_content: None,
        }
    }

    pub fn language_id(mut self, id: impl Into<String>) -> Self {
        self.language_id = Some(id.into());
        self
    }

    pub fn initial_content(mut self, content: impl Into<String>) -> Self {
        self.initial_content = Some(content.into());
        self
    }

    /// Validate and produce the channel name + options, returning an error if
    /// the name is empty or consists only of whitespace.
    pub fn validate(self) -> Result<(String, Option<String>, Option<String>), OutputError> {
        let trimmed = self.name.trim().to_string();
        if trimmed.is_empty() {
            return Err(OutputError::InvalidName("name must not be empty".into()));
        }
        Ok((trimmed, self.language_id, self.initial_content))
    }
}

// ── Bridge ──

/// Maximum content buffer size per channel (1 MiB).
pub const MAX_CHANNEL_BYTES: usize = 1024 * 1024;

pub struct OutputBridge {
    channels: Vec<OutputChannel>,
    next_id: u64,
}

impl OutputBridge {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_channel(&mut self, name: &str, language_id: Option<String>) -> String {
        let id = format!("output-{}", self.next_id);
        self.next_id += 1;
        self.channels.push(OutputChannel {
            id: id.clone(),
            name: name.to_string(),
            language_id,
            content: String::new(),
        });
        id
    }

    pub fn append_line(&mut self, channel_id: &str, line: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel_id) {
            if !ch.content.is_empty() {
                ch.content.push('\n');
            }
            ch.content.push_str(line);
        }
    }

    pub fn clear(&mut self, channel_id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel_id) {
            ch.content.clear();
        }
    }

    pub fn dispose(&mut self, channel_id: &str) {
        self.channels.retain(|c| c.id != channel_id);
    }

    pub fn get_channel(&self, id: &str) -> Option<&OutputChannel> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn handle_message(&mut self, msg: &OutputMessage) -> serde_json::Value {
        match msg {
            OutputMessage::CreateChannel { name, language_id } => {
                let id = self.create_channel(name, language_id.clone());
                serde_json::json!({"channelId": id})
            }
            OutputMessage::AppendLine { channel_id, line } => {
                self.append_line(channel_id, line);
                serde_json::json!({"appended": true})
            }
            OutputMessage::Clear { channel_id } => {
                self.clear(channel_id);
                serde_json::json!({"cleared": true})
            }
            OutputMessage::Show { channel_id, preserve_focus } => {
                let found = self.get_channel(channel_id).is_some();
                serde_json::json!({"shown": found, "preserveFocus": preserve_focus})
            }
            OutputMessage::Dispose { channel_id } => {
                self.dispose(channel_id);
                serde_json::json!({"disposed": true})
            }
        }
    }
}

impl OutputBridge {
    /// Create a channel via builder, with name validation.
    pub fn create_from_builder(
        &mut self,
        builder: ChannelBuilder,
    ) -> Result<String, OutputError> {
        let (name, language_id, initial_content) = builder.validate()?;
        let id = self.create_channel(&name, language_id);
        if let Some(content) = initial_content {
            self.append_line(&id, &content);
        }
        Ok(id)
    }

    /// Append text with buffer-size enforcement.
    pub fn append_checked(
        &mut self,
        channel_id: &str,
        line: &str,
    ) -> Result<(), OutputError> {
        let ch = self
            .channels
            .iter_mut()
            .find(|c| c.id == channel_id)
            .ok_or_else(|| OutputError::ChannelNotFound(channel_id.to_string()))?;

        let new_len = ch.content.len() + line.len() + 1;
        if new_len > MAX_CHANNEL_BYTES {
            return Err(OutputError::BufferOverflow {
                channel_id: channel_id.to_string(),
                max_bytes: MAX_CHANNEL_BYTES,
            });
        }
        if !ch.content.is_empty() {
            ch.content.push('\n');
        }
        ch.content.push_str(line);
        Ok(())
    }

    /// Return a snapshot list of all active channel ids.
    pub fn channel_ids(&self) -> Vec<&str> {
        self.channels.iter().map(|c| c.id.as_str()).collect()
    }

    /// Number of active channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Total bytes across all channel buffers.
    pub fn total_bytes(&self) -> usize {
        self.channels.iter().map(|c| c.content.len()).sum()
    }

    /// Search all channels for lines containing `needle`, returning
    /// `(channel_id, line_number, line_text)` triples.
    pub fn search(&self, needle: &str) -> Vec<(&str, usize, &str)> {
        let mut results = Vec::new();
        for ch in &self.channels {
            for (i, line) in ch.content.lines().enumerate() {
                if line.contains(needle) {
                    results.push((ch.id.as_str(), i, line));
                }
            }
        }
        results
    }
}

impl Default for OutputBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for OutputBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputBridge")
            .field("channel_count", &self.channels.len())
            .field("next_id", &self.next_id)
            .finish()
    }
}

/// Initialize the output extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-output operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtOutputStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtOutputStats {
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
    pub fn merge(&mut self, other: &ExtOutputStats) {
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

impl Default for ExtOutputStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtOutputStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtOutputStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-output.
#[derive(Debug, Clone)]
pub struct ExtOutputValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtOutputValidator {
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

impl Default for ExtOutputValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OutputBridge extensions
// ---------------------------------------------------------------------------

impl OutputBridge {
    /// Append multiple lines at once to a channel.
    pub fn append_multiple_lines(&mut self, channel_id: &str, lines: &[&str]) {
        for line in lines {
            self.append_line(channel_id, line);
        }
    }

    /// Append a line with an auto-incrementing counter prefix. Returns the formatted line.
    pub fn append_timestamped(&mut self, channel_id: &str, line: &str) -> String {
        let counter = if let Some(ch) = self.channels.iter().find(|c| c.id == channel_id) {
            ch.line_count() + 1
        } else {
            0
        };
        let formatted = format!("[{:04}] {}", counter, line);
        self.append_line(channel_id, &formatted);
        formatted
    }

    /// Clear channel but preserve the first `preserve_lines` lines as a header.
    pub fn clear_with_options(&mut self, channel_id: &str, preserve_lines: usize) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel_id) {
            if preserve_lines == 0 {
                ch.content.clear();
                return;
            }
            let lines: Vec<&str> = ch.content.lines().collect();
            let kept = lines.iter().take(preserve_lines).copied().collect::<Vec<_>>();
            ch.content = kept.join("\n");
        }
    }

    /// Export a channel's content with header metadata.
    pub fn export_channel(&self, channel_id: &str) -> Result<String, OutputError> {
        let ch = self
            .channels
            .iter()
            .find(|c| c.id == channel_id)
            .ok_or_else(|| OutputError::ChannelNotFound(channel_id.to_string()))?;
        Ok(format!(
            "--- Channel: {} (id: {}) ---\n{}",
            ch.name, ch.id, ch.content
        ))
    }

    /// Find channels whose name matches the given string.
    pub fn find_channels_by_name(&self, name: &str) -> Vec<&OutputChannel> {
        self.channels.iter().filter(|c| c.name == name).collect()
    }

    /// Find channels whose language_id matches the given string.
    pub fn find_channels_by_language(&self, language_id: &str) -> Vec<&OutputChannel> {
        self.channels
            .iter()
            .filter(|c| c.language_id.as_deref() == Some(language_id))
            .collect()
    }

    /// Sum of line counts across all active channels.
    pub fn total_line_count(&self) -> usize {
        self.channels.iter().map(|c| c.line_count()).sum()
    }

    /// Clear content of all active channels.
    pub fn clear_all(&mut self) {
        for ch in &mut self.channels {
            ch.content.clear();
        }
    }

    /// Merge source channel content into target channel.
    pub fn merge_channels(&mut self, source_id: &str, target_id: &str) -> Result<(), OutputError> {
        let source_content = self
            .channels
            .iter()
            .find(|c| c.id == source_id)
            .ok_or_else(|| OutputError::ChannelNotFound(source_id.to_string()))?
            .content
            .clone();

        let target = self
            .channels
            .iter_mut()
            .find(|c| c.id == target_id)
            .ok_or_else(|| OutputError::ChannelNotFound(target_id.to_string()))?;

        if !target.content.is_empty() && !source_content.is_empty() {
            target.content.push('\n');
        }
        target.content.push_str(&source_content);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// OutputFormatter – configurable output templates
// ---------------------------------------------------------------------------

/// A formatter that applies configurable templates to output lines.
#[derive(Debug, Clone)]
pub struct OutputFormatter {
    /// Template string with placeholders: {message}, {timestamp}, {level}, {channel}
    template: String,
    timestamp_format: TimestampFormat,
}

/// How timestamps are formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampFormat {
    None,
    Seconds,
    Millis,
    Iso8601,
}

impl fmt::Display for TimestampFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Seconds => write!(f, "seconds"),
            Self::Millis => write!(f, "millis"),
            Self::Iso8601 => write!(f, "iso8601"),
        }
    }
}

impl OutputFormatter {
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
            timestamp_format: TimestampFormat::None,
        }
    }

    pub fn with_timestamp(mut self, format: TimestampFormat) -> Self {
        self.timestamp_format = format;
        self
    }

    pub fn template(&self) -> &str {
        &self.template
    }

    pub fn timestamp_format(&self) -> TimestampFormat {
        self.timestamp_format
    }

    /// Format a message using the template.
    pub fn format(&self, message: &str, level: Option<LogLevel>, channel: Option<&str>) -> String {
        let ts = self.format_timestamp();
        let level_str = level.map(|l| l.label()).unwrap_or("");
        let channel_str = channel.unwrap_or("");
        self.template
            .replace("{message}", message)
            .replace("{timestamp}", &ts)
            .replace("{level}", level_str)
            .replace("{channel}", channel_str)
    }

    fn format_timestamp(&self) -> String {
        match self.timestamp_format {
            TimestampFormat::None => String::new(),
            TimestampFormat::Seconds => {
                let d = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                format!("{}", d.as_secs())
            }
            TimestampFormat::Millis => {
                let d = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                format!("{}", d.as_millis())
            }
            TimestampFormat::Iso8601 => {
                // Simple approximation without chrono
                let d = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                format!("{}s", d.as_secs())
            }
        }
    }

    /// Default formatter: "[{level}] {message}"
    pub fn default_formatter() -> Self {
        Self::new("[{level}] {message}")
    }

    /// Verbose formatter with channel and timestamp.
    pub fn verbose_formatter() -> Self {
        Self::new("[{timestamp}] [{channel}] [{level}] {message}")
            .with_timestamp(TimestampFormat::Seconds)
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::default_formatter()
    }
}

// ---------------------------------------------------------------------------
// OutputBuffer – buffering with flush intervals
// ---------------------------------------------------------------------------

/// Buffers output lines and flushes when capacity or interval is reached.
#[derive(Debug, Clone)]
pub struct OutputBuffer {
    lines: Vec<String>,
    capacity: usize,
    total_flushed: usize,
}

impl OutputBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: Vec::new(),
            capacity: capacity.max(1),
            total_flushed: 0,
        }
    }

    /// Append a line. Returns `true` if the buffer is now full and should be flushed.
    pub fn append(&mut self, line: impl Into<String>) -> bool {
        self.lines.push(line.into());
        self.lines.len() >= self.capacity
    }

    /// Take all buffered lines (draining the buffer).
    pub fn flush(&mut self) -> Vec<String> {
        self.total_flushed += self.lines.len();
        std::mem::take(&mut self.lines)
    }

    /// Number of currently buffered lines.
    pub fn buffered_count(&self) -> usize {
        self.lines.len()
    }

    /// Total lines flushed since creation.
    pub fn total_flushed(&self) -> usize {
        self.total_flushed
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Is the buffer at capacity?
    pub fn is_full(&self) -> bool {
        self.lines.len() >= self.capacity
    }

    /// Peek at buffered lines without flushing.
    pub fn peek(&self) -> &[String] {
        &self.lines
    }

    /// Clear without counting as flushed.
    pub fn discard(&mut self) {
        self.lines.clear();
    }
}

impl fmt::Display for OutputBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutputBuffer({}/{} lines, {} flushed)",
            self.lines.len(),
            self.capacity,
            self.total_flushed
        )
    }
}

// ---------------------------------------------------------------------------
// OutputChannelMerger – merge multiple channels into one
// ---------------------------------------------------------------------------

/// Merges output from multiple channels into a single unified stream.
#[derive(Debug, Clone)]
pub struct MergedLine {
    pub source_channel: String,
    pub content: String,
    pub sequence: usize,
}

impl fmt::Display for MergedLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.source_channel, self.content)
    }
}

/// Merges output from multiple channels into a unified view.
#[derive(Debug, Clone)]
pub struct OutputChannelMerger {
    merged: Vec<MergedLine>,
    sequence: usize,
    channel_filter: Option<Vec<String>>,
}

impl OutputChannelMerger {
    pub fn new() -> Self {
        Self {
            merged: Vec::new(),
            sequence: 0,
            channel_filter: None,
        }
    }

    /// Only include lines from these channels.
    pub fn with_filter(mut self, channels: Vec<String>) -> Self {
        self.channel_filter = Some(channels);
        self
    }

    /// Append a line from a specific channel.
    pub fn append(&mut self, channel: impl Into<String>, content: impl Into<String>) {
        let ch = channel.into();
        if let Some(ref filter) = self.channel_filter {
            if !filter.contains(&ch) {
                return;
            }
        }
        self.merged.push(MergedLine {
            source_channel: ch,
            content: content.into(),
            sequence: self.sequence,
        });
        self.sequence += 1;
    }

    /// All merged lines in order.
    pub fn lines(&self) -> &[MergedLine] {
        &self.merged
    }

    /// Lines from a specific channel only.
    pub fn lines_from(&self, channel: &str) -> Vec<&MergedLine> {
        self.merged
            .iter()
            .filter(|l| l.source_channel == channel)
            .collect()
    }

    /// Total line count.
    pub fn line_count(&self) -> usize {
        self.merged.len()
    }

    /// Distinct channels that have contributed lines.
    pub fn active_channels(&self) -> Vec<&str> {
        let mut channels: Vec<&str> = self
            .merged
            .iter()
            .map(|l| l.source_channel.as_str())
            .collect();
        channels.sort();
        channels.dedup();
        channels
    }

    /// Clear all merged output.
    pub fn clear(&mut self) {
        self.merged.clear();
    }

    /// Tail N lines.
    pub fn tail(&self, n: usize) -> &[MergedLine] {
        let start = self.merged.len().saturating_sub(n);
        &self.merged[start..]
    }
}

impl Default for OutputChannelMerger {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OutputChannelMerger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutputChannelMerger({} lines, {} channels)",
            self.merged.len(),
            self.active_channels().len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = OutputMessage::AppendLine {
            channel_id: "ch1".into(),
            line: "hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: OutputMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn channel_serialization() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "Build".into(),
            language_id: Some("log".into()),
            content: "line1".into(),
        };
        let json = serde_json::to_string(&ch).unwrap();
        let back: OutputChannel = serde_json::from_str(&json).unwrap();
        assert_eq!(ch, back);
    }

    #[test]
    fn bridge_create_and_append() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Build", None);
        bridge.append_line(&id, "line 1");
        bridge.append_line(&id, "line 2");
        let ch = bridge.get_channel(&id).unwrap();
        assert_eq!(ch.content, "line 1\nline 2");
    }

    #[test]
    fn bridge_clear() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Test", None);
        bridge.append_line(&id, "data");
        bridge.clear(&id);
        let ch = bridge.get_channel(&id).unwrap();
        assert!(ch.content.is_empty());
    }

    #[test]
    fn bridge_dispose() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Temp", None);
        bridge.dispose(&id);
        assert!(bridge.get_channel(&id).is_none());
    }

    #[test]
    fn log_level_severity_order() {
        assert!(LogLevel::Error.severity() > LogLevel::Warning.severity());
        assert!(LogLevel::Warning.severity() > LogLevel::Info.severity());
        assert!(LogLevel::Info.severity() > LogLevel::Debug.severity());
        assert!(LogLevel::Debug.severity() > LogLevel::Trace.severity());
    }

    #[test]
    fn log_level_is_enabled() {
        assert!(LogLevel::Error.is_enabled(LogLevel::Info));
        assert!(LogLevel::Info.is_enabled(LogLevel::Info));
        assert!(!LogLevel::Debug.is_enabled(LogLevel::Info));
    }

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Warning.to_string(), "WARN");
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
    }

    #[test]
    fn output_channel_line_count() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "Test".into(),
            language_id: None,
            content: "a\nb\nc".into(),
        };
        assert_eq!(ch.line_count(), 3);
        assert_eq!(ch.byte_len(), 5);
    }

    #[test]
    fn output_channel_tail_lines() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "Test".into(),
            language_id: None,
            content: "one\ntwo\nthree\nfour".into(),
        };
        assert_eq!(ch.tail_lines(2), vec!["three", "four"]);
        assert_eq!(ch.tail_lines(10).len(), 4);
    }

    #[test]
    fn output_channel_display() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "Build".into(),
            language_id: None,
            content: "a\nb".into(),
        };
        let display = format!("{ch}");
        assert!(display.contains("Build"));
        assert!(display.contains("2 lines"));
    }

    #[test]
    fn log_output_channel_format_message() {
        let log_ch = LogOutputChannel {
            id: "log1".into(),
            name: "Server".into(),
            log_level: LogLevel::Warning,
        };
        assert!(log_ch.format_message(LogLevel::Error, "boom").is_some());
        assert!(log_ch.format_message(LogLevel::Debug, "detail").is_none());
        let msg = log_ch.format_message(LogLevel::Warning, "caution").unwrap();
        assert!(msg.contains("WARN"));
        assert!(msg.contains("caution"));
    }

    #[test]
    fn channel_builder_success() {
        let mut bridge = OutputBridge::new();
        let builder = ChannelBuilder::new("Build")
            .language_id("log")
            .initial_content("starting...");
        let id = bridge.create_from_builder(builder).unwrap();
        let ch = bridge.get_channel(&id).unwrap();
        assert_eq!(ch.name, "Build");
        assert_eq!(ch.language_id.as_deref(), Some("log"));
        assert_eq!(ch.content, "starting...");
    }

    #[test]
    fn channel_builder_empty_name_rejected() {
        let mut bridge = OutputBridge::new();
        let builder = ChannelBuilder::new("   ");
        let err = bridge.create_from_builder(builder).unwrap_err();
        assert_eq!(err, OutputError::InvalidName("name must not be empty".into()));
    }

    #[test]
    fn bridge_append_checked_unknown_channel() {
        let mut bridge = OutputBridge::new();
        let err = bridge.append_checked("no-such", "text").unwrap_err();
        assert!(matches!(err, OutputError::ChannelNotFound(_)));
    }

    #[test]
    fn bridge_channel_count_and_ids() {
        let mut bridge = OutputBridge::new();
        assert_eq!(bridge.channel_count(), 0);
        let id1 = bridge.create_channel("A", None);
        let id2 = bridge.create_channel("B", None);
        assert_eq!(bridge.channel_count(), 2);
        let ids = bridge.channel_ids();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }

    #[test]
    fn bridge_search() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Logs", None);
        bridge.append_line(&id, "INFO: server started");
        bridge.append_line(&id, "ERROR: disk full");
        bridge.append_line(&id, "INFO: request handled");
        let hits = bridge.search("ERROR");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, 1); // line index
        assert!(hits[0].2.contains("disk full"));
    }

    #[test]
    fn bridge_total_bytes() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("X", None);
        bridge.append_line(&id, "hello");
        assert_eq!(bridge.total_bytes(), 5);
    }

    #[test]
    fn output_error_display() {
        let err = OutputError::ChannelNotFound("ch-42".into());
        assert_eq!(err.to_string(), "output channel not found: ch-42");

        let err2 = OutputError::BufferOverflow {
            channel_id: "ch-1".into(),
            max_bytes: 1024,
        };
        assert!(err2.to_string().contains("1024"));
    }

    #[test]
    fn bridge_handle_message_show_missing() {
        let mut bridge = OutputBridge::new();
        let result = bridge.handle_message(&OutputMessage::Show {
            channel_id: "nonexistent".into(),
            preserve_focus: true,
        });
        assert_eq!(result["shown"], false);
    }

    #[test]
    fn bridge_debug_impl() {
        let bridge = OutputBridge::new();
        let dbg = format!("{bridge:?}");
        assert!(dbg.contains("OutputBridge"));
        assert!(dbg.contains("channel_count"));
    }

    #[test]
    fn ext_output_stats_new_defaults() {
        let stats = ExtOutputStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_output_stats_record_success() {
        let mut stats = ExtOutputStats::new();
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
    fn ext_output_stats_record_failure() {
        let mut stats = ExtOutputStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_output_stats_reset() {
        let mut stats = ExtOutputStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_output_stats_merge() {
        let mut a = ExtOutputStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtOutputStats::new();
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
    fn ext_output_stats_display() {
        let mut stats = ExtOutputStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_output_stats_default() {
        let stats = ExtOutputStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_output_validator_accepts_valid_name() {
        let v = ExtOutputValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_output_validator_rejects_empty() {
        let v = ExtOutputValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_output_validator_rejects_too_long() {
        let v = ExtOutputValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_output_validator_forbidden_prefix() {
        let v = ExtOutputValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_output_validator_allowed_chars() {
        let v = ExtOutputValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_output_validator_range() {
        let v = ExtOutputValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_output_sanitize_removes_control() {
        let result = ExtOutputValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_output_truncate_short_string() {
        assert_eq!(ExtOutputValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_output_truncate_long_string() {
        let result = ExtOutputValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_output_is_ascii_printable() {
        assert!(ExtOutputValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtOutputValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn append_multiple_lines_at_once() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Test", None);
        bridge.append_multiple_lines(&id, &["line1", "line2", "line3"]);
        let ch = bridge.get_channel(&id).unwrap();
        assert_eq!(ch.line_count(), 3);
        assert_eq!(ch.content, "line1\nline2\nline3");
    }

    #[test]
    fn append_timestamped_increments() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Log", None);
        let line1 = bridge.append_timestamped(&id, "hello");
        assert_eq!(line1, "[0001] hello");
        let line2 = bridge.append_timestamped(&id, "world");
        assert_eq!(line2, "[0002] world");
    }

    #[test]
    fn clear_with_options_preserve_header() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Build", None);
        bridge.append_multiple_lines(&id, &["=== Header ===", "line1", "line2", "line3"]);
        bridge.clear_with_options(&id, 1);
        let ch = bridge.get_channel(&id).unwrap();
        assert_eq!(ch.content, "=== Header ===");
        assert_eq!(ch.line_count(), 1);
    }

    #[test]
    fn clear_with_options_preserve_zero() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("X", None);
        bridge.append_line(&id, "data");
        bridge.clear_with_options(&id, 0);
        let ch = bridge.get_channel(&id).unwrap();
        assert!(ch.content.is_empty());
    }

    #[test]
    fn export_channel_format() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("Build", None);
        bridge.append_line(&id, "compiling...");
        let export = bridge.export_channel(&id).unwrap();
        assert!(export.contains("--- Channel: Build"));
        assert!(export.contains(&format!("(id: {})", id)));
        assert!(export.contains("compiling..."));
    }

    #[test]
    fn export_channel_not_found() {
        let bridge = OutputBridge::new();
        let err = bridge.export_channel("nonexistent").unwrap_err();
        assert!(matches!(err, OutputError::ChannelNotFound(_)));
    }

    #[test]
    fn find_channels_by_name_matches() {
        let mut bridge = OutputBridge::new();
        bridge.create_channel("Build", None);
        bridge.create_channel("Build", None);
        bridge.create_channel("Test", None);
        let found = bridge.find_channels_by_name("Build");
        assert_eq!(found.len(), 2);
        let found_test = bridge.find_channels_by_name("Test");
        assert_eq!(found_test.len(), 1);
        let found_none = bridge.find_channels_by_name("Missing");
        assert!(found_none.is_empty());
    }

    #[test]
    fn merge_channels_success() {
        let mut bridge = OutputBridge::new();
        let src = bridge.create_channel("Source", None);
        let tgt = bridge.create_channel("Target", None);
        bridge.append_line(&src, "source line");
        bridge.append_line(&tgt, "target line");
        bridge.merge_channels(&src, &tgt).unwrap();
        let target = bridge.get_channel(&tgt).unwrap();
        assert_eq!(target.content, "target line\nsource line");
    }

    #[test]
    fn merge_channels_not_found() {
        let mut bridge = OutputBridge::new();
        let id = bridge.create_channel("A", None);
        assert!(bridge.merge_channels("nope", &id).is_err());
        assert!(bridge.merge_channels(&id, "nope").is_err());
    }

    #[test]
    fn log_level_from_str_valid() {
        assert_eq!(LogLevel::from_str("trace"), Some(LogLevel::Trace));
        assert_eq!(LogLevel::from_str("DEBUG"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("Info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("warning"), Some(LogLevel::Warning));
        assert_eq!(LogLevel::from_str("warn"), Some(LogLevel::Warning));
        assert_eq!(LogLevel::from_str("ERROR"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str("unknown"), None);
        assert_eq!(LogLevel::from_str(""), None);
    }

    #[test]
    fn log_level_all_levels_order() {
        let levels = LogLevel::all_levels();
        assert_eq!(levels.len(), 5);
        assert_eq!(levels[0], LogLevel::Trace);
        assert_eq!(levels[4], LogLevel::Error);
        for window in levels.windows(2) {
            assert!(window[0].severity() < window[1].severity());
        }
    }

    #[test]
    fn output_channel_is_empty() {
        let empty_ch = OutputChannel {
            id: "e1".into(),
            name: "Empty".into(),
            language_id: None,
            content: String::new(),
        };
        assert!(empty_ch.is_empty());
        let nonempty_ch = OutputChannel {
            id: "e2".into(),
            name: "Full".into(),
            language_id: None,
            content: "data".into(),
        };
        assert!(!nonempty_ch.is_empty());
    }

    #[test]
    fn bridge_find_channels_by_language() {
        let mut bridge = OutputBridge::new();
        bridge.create_channel("Build", Some("log".into()));
        bridge.create_channel("Test", Some("log".into()));
        bridge.create_channel("Debug", Some("json".into()));
        bridge.create_channel("Plain", None);
        let log_channels = bridge.find_channels_by_language("log");
        assert_eq!(log_channels.len(), 2);
        let json_channels = bridge.find_channels_by_language("json");
        assert_eq!(json_channels.len(), 1);
        assert_eq!(json_channels[0].name, "Debug");
        let none = bridge.find_channels_by_language("xml");
        assert!(none.is_empty());
    }

    #[test]
    fn bridge_total_line_count() {
        let mut bridge = OutputBridge::new();
        let id1 = bridge.create_channel("A", None);
        let id2 = bridge.create_channel("B", None);
        assert_eq!(bridge.total_line_count(), 0);
        bridge.append_line(&id1, "line1");
        bridge.append_line(&id1, "line2");
        bridge.append_line(&id2, "line3");
        assert_eq!(bridge.total_line_count(), 3);
    }

    #[test]
    fn bridge_clear_all() {
        let mut bridge = OutputBridge::new();
        let id1 = bridge.create_channel("A", None);
        let id2 = bridge.create_channel("B", None);
        bridge.append_line(&id1, "data1");
        bridge.append_line(&id2, "data2");
        assert!(bridge.total_bytes() > 0);
        bridge.clear_all();
        assert_eq!(bridge.total_bytes(), 0);
        assert_eq!(bridge.total_line_count(), 0);
        // channels still exist, just cleared
        assert_eq!(bridge.channel_count(), 2);
    }

    #[test]
    fn log_level_display_all_variants() {
        assert_eq!(format!("{}", LogLevel::Trace), "TRACE");
        assert_eq!(format!("{}", LogLevel::Debug), "DEBUG");
        assert_eq!(format!("{}", LogLevel::Info), "INFO");
        assert_eq!(format!("{}", LogLevel::Warning), "WARN");
        assert_eq!(format!("{}", LogLevel::Error), "ERROR");
    }

    // --- New tests for formatter, buffer, merger ---

    #[test]
    fn output_formatter_basic() {
        let fmt = OutputFormatter::new("[{level}] {message}");
        let result = fmt.format("hello", Some(LogLevel::Info), None);
        assert_eq!(result, "[INFO] hello");
    }

    #[test]
    fn output_formatter_with_channel() {
        let fmt = OutputFormatter::new("[{channel}] {message}");
        let result = fmt.format("test", None, Some("build"));
        assert_eq!(result, "[build] test");
    }

    #[test]
    fn output_formatter_default() {
        let fmt = OutputFormatter::default_formatter();
        assert!(fmt.template().contains("{level}"));
        assert!(fmt.template().contains("{message}"));
    }

    #[test]
    fn output_buffer_flush_on_capacity() {
        let mut buf = OutputBuffer::new(3);
        assert!(!buf.append("line1"));
        assert!(!buf.append("line2"));
        assert!(buf.append("line3")); // full
        assert_eq!(buf.buffered_count(), 3);
        let lines = buf.flush();
        assert_eq!(lines.len(), 3);
        assert!(buf.is_empty());
        assert_eq!(buf.total_flushed(), 3);
    }

    #[test]
    fn output_buffer_peek_and_discard() {
        let mut buf = OutputBuffer::new(10);
        buf.append("a");
        buf.append("b");
        assert_eq!(buf.peek().len(), 2);
        buf.discard();
        assert!(buf.is_empty());
        assert_eq!(buf.total_flushed(), 0); // discard doesn't count
    }

    #[test]
    fn output_merger_basic() {
        let mut merger = OutputChannelMerger::new();
        merger.append("build", "compiling...");
        merger.append("test", "running tests");
        merger.append("build", "done");
        assert_eq!(merger.line_count(), 3);
        assert_eq!(merger.lines_from("build").len(), 2);
        let channels = merger.active_channels();
        assert!(channels.contains(&"build"));
        assert!(channels.contains(&"test"));
    }

    #[test]
    fn output_merger_filter() {
        let mut merger = OutputChannelMerger::new()
            .with_filter(vec!["build".to_string()]);
        merger.append("build", "ok");
        merger.append("test", "filtered out");
        assert_eq!(merger.line_count(), 1);
    }

    #[test]
    fn output_merger_tail() {
        let mut merger = OutputChannelMerger::new();
        for i in 0..10 {
            merger.append("ch", format!("line {i}"));
        }
        let tail = merger.tail(3);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].content, "line 7");
    }

    #[test]
    fn output_merger_display() {
        let merger = OutputChannelMerger::new();
        assert!(format!("{merger}").contains("0 lines"));
    }

    #[test]
    fn merged_line_display() {
        let line = MergedLine {
            source_channel: "build".to_string(),
            content: "ok".to_string(),
            sequence: 0,
        };
        assert_eq!(format!("{line}"), "[build] ok");
    }
}
