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

// ── OutputMessage helpers ──

impl OutputMessage {
    /// Returns the channel_id referenced by this message, if any.
    /// `CreateChannel` does not reference an existing channel, so returns `None`.
    pub fn channel_id(&self) -> Option<&str> {
        match self {
            OutputMessage::CreateChannel { .. } => None,
            OutputMessage::AppendLine { channel_id, .. }
            | OutputMessage::Clear { channel_id }
            | OutputMessage::Show { channel_id, .. }
            | OutputMessage::Dispose { channel_id } => Some(channel_id),
        }
    }

    /// Returns `true` if this message mutates channel content.
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            OutputMessage::CreateChannel { .. }
                | OutputMessage::AppendLine { .. }
                | OutputMessage::Clear { .. }
                | OutputMessage::Dispose { .. }
        )
    }

    /// Returns a short human-readable label for the message variant.
    pub fn kind(&self) -> &'static str {
        match self {
            OutputMessage::CreateChannel { .. } => "create",
            OutputMessage::AppendLine { .. } => "append",
            OutputMessage::Clear { .. } => "clear",
            OutputMessage::Show { .. } => "show",
            OutputMessage::Dispose { .. } => "dispose",
        }
    }
}

impl fmt::Display for OutputMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputMessage::CreateChannel { name, language_id } => {
                write!(f, "CreateChannel(name={name}")?;
                if let Some(lid) = language_id {
                    write!(f, ", lang={lid}")?;
                }
                write!(f, ")")
            }
            OutputMessage::AppendLine { channel_id, line } => {
                let preview = if line.len() > 40 {
                    format!("{}…", &line[..39])
                } else {
                    line.clone()
                };
                write!(f, "AppendLine(ch={channel_id}, line={preview:?})")
            }
            OutputMessage::Clear { channel_id } => write!(f, "Clear(ch={channel_id})"),
            OutputMessage::Show { channel_id, preserve_focus } => {
                write!(f, "Show(ch={channel_id}, preserve={preserve_focus})")
            }
            OutputMessage::Dispose { channel_id } => write!(f, "Dispose(ch={channel_id})"),
        }
    }
}

// ── OutputChannel additional helpers ──

impl OutputChannel {
    /// Returns the first `n` lines of the content.
    pub fn head_lines(&self, n: usize) -> Vec<&str> {
        self.content.lines().take(n).collect()
    }

    /// Returns `true` if the content contains the given substring.
    pub fn contains(&self, needle: &str) -> bool {
        self.content.contains(needle)
    }

    /// Returns `true` if a language_id is set.
    pub fn has_language(&self) -> bool {
        self.language_id.is_some()
    }

    /// Returns lines matching a predicate.
    pub fn filter_lines<F: Fn(&str) -> bool>(&self, predicate: F) -> Vec<&str> {
        self.content.lines().filter(|l| predicate(l)).collect()
    }

    /// Append a line, returning the new total line count.
    pub fn push_line(&mut self, line: &str) -> usize {
        if !self.content.is_empty() {
            self.content.push('\n');
        }
        self.content.push_str(line);
        self.line_count()
    }

    /// Returns the content split into lines.
    pub fn lines(&self) -> Vec<&str> {
        if self.content.is_empty() {
            Vec::new()
        } else {
            self.content.lines().collect()
        }
    }
}

// ── LogOutputChannel additional helpers ──

impl LogOutputChannel {
    /// Returns `true` if the given level would be logged by this channel.
    pub fn would_log(&self, level: LogLevel) -> bool {
        level.is_enabled(self.log_level)
    }

    /// Create a new `LogOutputChannel` with the given parameters.
    pub fn new(id: impl Into<String>, name: impl Into<String>, log_level: LogLevel) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            log_level,
        }
    }

    /// Set the log level threshold, returning self for chaining in tests.
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.log_level = level;
        self
    }
}

// ── LogLevel additional helpers ──

impl LogLevel {
    /// Returns `true` if this is an error-class level (Warning or Error).
    pub fn is_error_class(self) -> bool {
        matches!(self, LogLevel::Warning | LogLevel::Error)
    }

    /// Returns `true` if this is a diagnostic-class level (Trace or Debug).
    pub fn is_diagnostic(self) -> bool {
        matches!(self, LogLevel::Trace | LogLevel::Debug)
    }

    /// Returns the next more severe level, or `None` if already at `Error`.
    pub fn escalate(self) -> Option<LogLevel> {
        match self {
            LogLevel::Trace => Some(LogLevel::Debug),
            LogLevel::Debug => Some(LogLevel::Info),
            LogLevel::Info => Some(LogLevel::Warning),
            LogLevel::Warning => Some(LogLevel::Error),
            LogLevel::Error => None,
        }
    }

    /// Returns the next less severe level, or `None` if already at `Trace`.
    pub fn deescalate(self) -> Option<LogLevel> {
        match self {
            LogLevel::Trace => None,
            LogLevel::Debug => Some(LogLevel::Trace),
            LogLevel::Info => Some(LogLevel::Debug),
            LogLevel::Warning => Some(LogLevel::Info),
            LogLevel::Error => Some(LogLevel::Warning),
        }
    }
}

// ── OutputError helpers ──

impl OutputError {
    /// Returns `true` if this error indicates a missing channel.
    pub fn is_not_found(&self) -> bool {
        matches!(self, OutputError::ChannelNotFound(_))
    }

    /// Returns `true` if this error is a buffer overflow.
    pub fn is_overflow(&self) -> bool {
        matches!(self, OutputError::BufferOverflow { .. })
    }

    /// Returns the channel id associated with this error, if any.
    pub fn channel_id(&self) -> Option<&str> {
        match self {
            OutputError::ChannelNotFound(id) => Some(id),
            OutputError::BufferOverflow { channel_id, .. } => Some(channel_id),
            _ => None,
        }
    }
}

// ── OutputBuffer additional helpers ──

impl OutputBuffer {
    /// Returns the total number of lines ever appended (buffered + flushed).
    pub fn total_appended(&self) -> usize {
        self.total_flushed + self.lines.len()
    }

    /// Returns how many more lines can be appended before the buffer is full.
    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.lines.len())
    }

    /// Resize the buffer capacity. Does not discard existing lines.
    pub fn set_capacity(&mut self, new_capacity: usize) {
        self.capacity = new_capacity.max(1);
    }
}

// ── OutputChannelMerger additional helpers ──

impl OutputChannelMerger {
    /// Search merged lines for content containing `needle`.
    pub fn search(&self, needle: &str) -> Vec<&MergedLine> {
        self.merged.iter().filter(|l| l.content.contains(needle)).collect()
    }

    /// Returns `true` if there are no merged lines.
    pub fn is_empty(&self) -> bool {
        self.merged.is_empty()
    }

    /// Returns the number of distinct source channels.
    pub fn channel_count(&self) -> usize {
        self.active_channels().len()
    }
}

// ── MergedLine helpers ──

impl MergedLine {
    /// Create a new merged line.
    pub fn new(source_channel: impl Into<String>, content: impl Into<String>, sequence: usize) -> Self {
        Self {
            source_channel: source_channel.into(),
            content: content.into(),
            sequence,
        }
    }

    /// Returns `true` if content contains the given substring.
    pub fn contains(&self, needle: &str) -> bool {
        self.content.contains(needle)
    }
}

// ── TimestampFormat helpers ──

impl TimestampFormat {
    /// Returns `true` if timestamps are disabled.
    pub fn is_none(self) -> bool {
        matches!(self, TimestampFormat::None)
    }

    /// Parse from a case-insensitive string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Some(TimestampFormat::None),
            "seconds" | "secs" | "s" => Some(TimestampFormat::Seconds),
            "millis" | "ms" => Some(TimestampFormat::Millis),
            "iso8601" | "iso" => Some(TimestampFormat::Iso8601),
            _ => None,
        }
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

// ---------------------------------------------------------------------------
// OutputChannelLanguageMode - output channel language mode
// ---------------------------------------------------------------------------

/// Severity level for output channel language mode issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutputChannelLanguageModeSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for OutputChannelLanguageModeSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [OutputChannelLanguageMode].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChannelLanguageModeEntry {
    pub id: String,
    pub label: String,
    pub severity: OutputChannelLanguageModeSeverity,
    pub detail: Option<String>,
    pub channel_count: usize,
    enabled: bool,
}

impl OutputChannelLanguageModeEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: OutputChannelLanguageModeSeverity::Low,
            detail: None,
            channel_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: OutputChannelLanguageModeSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_channel_count(mut self, val: usize) -> Self {
        self.channel_count = val;
        self
    }

    pub fn has_language(&self) -> bool {
        self.enabled && self.severity >= OutputChannelLanguageModeSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.channel_count, det)
    }
}

impl fmt::Display for OutputChannelLanguageModeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [OutputChannelLanguageModeEntry] items.
#[derive(Debug, Clone)]
pub struct OutputChannelLanguageMode {
    entries: Vec<OutputChannelLanguageModeEntry>,
    name: String,
    capacity: usize,
}

impl OutputChannelLanguageMode {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: OutputChannelLanguageModeEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<OutputChannelLanguageModeEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&OutputChannelLanguageModeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn channel_count(&self) -> usize { self.entries.len() }

    pub fn has_language(&self) -> bool {
        self.entries.iter().any(|e| e.has_language())
    }

    pub fn entries_by_severity(&self, severity: OutputChannelLanguageModeSeverity) -> Vec<&OutputChannelLanguageModeEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= OutputChannelLanguageModeSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&OutputChannelLanguageModeEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&OutputChannelLanguageModeEntry> {
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
// OutputChannelBufferManager - output channel buffer manager
// ---------------------------------------------------------------------------

/// Configuration for [OutputChannelBufferManager].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChannelBufferManagerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub buffer_size: usize,
}

impl OutputChannelBufferManagerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, buffer_size: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_buffer_size(mut self, val: usize) -> Self { self.buffer_size = val; self }
}

impl Default for OutputChannelBufferManagerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [OutputChannelBufferManager].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChannelBufferManagerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl OutputChannelBufferManagerItem {
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

    pub fn is_buffer_full(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for OutputChannelBufferManagerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [OutputChannelBufferManagerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct OutputChannelBufferManager {
    config: OutputChannelBufferManagerConfig,
    items: Vec<OutputChannelBufferManagerItem>,
}

impl OutputChannelBufferManager {
    pub fn new(config: OutputChannelBufferManagerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: OutputChannelBufferManagerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<OutputChannelBufferManagerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&OutputChannelBufferManagerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn buffer_size(&self) -> usize { self.items.len() }

    pub fn is_buffer_full(&self) -> bool {
        self.items.iter().any(|i| i.is_buffer_full())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&OutputChannelBufferManagerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&OutputChannelBufferManagerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &OutputChannelBufferManagerConfig {
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
// ext_output – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtOutputActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtOutputActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtOutputRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtOutputRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_output_collect_sequences(envelopes: &[XExtOutputRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_output_filter_by_method<'a>(
    envelopes: &'a [XExtOutputRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtOutputRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_output_dedup_by_seq(envelopes: Vec<XExtOutputRpcEnvelope>) -> Vec<XExtOutputRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_output_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtOutputApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtOutputApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtOutputApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}



// ---------------------------------------------------------------------------
// ext_output – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension output channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtOutputExtOutputEncoding {
    Utf8,
    Ascii,
    Latin1,
    Raw,
}

impl YExtOutputExtOutputEncoding {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Utf8 => 0,
            Self::Ascii => 1,
            Self::Latin1 => 2,
            Self::Raw => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Utf8 => "Utf8",
            Self::Ascii => "Ascii",
            Self::Latin1 => "Latin1",
            Self::Raw => "Raw",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtOutputExtOutputEncoding] {
        &[
            YExtOutputExtOutputEncoding::Utf8,
            YExtOutputExtOutputEncoding::Ascii,
            YExtOutputExtOutputEncoding::Latin1,
            YExtOutputExtOutputEncoding::Raw,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtOutputExtOutputEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks extension output buffer data.
#[derive(Debug, Clone)]
pub struct YExtOutputExtOutputBuffer {
    pub lines: Vec<String>,
    pub byte_count: u64,
    pub channel_id: String,
}

impl YExtOutputExtOutputBuffer {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            byte_count: 0,
            channel_id: String::new(),
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtOutputExtOutputBuffer({}: {:?})", "lines", self.lines)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_output_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_output_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_output_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_output_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_output_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_output_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_output_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_output_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_output – Extended extension output ring helpers
// ---------------------------------------------------------------------------

/// Priority levels for extension output ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtOutputPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtOutputPriority {
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
    pub fn all_asc() -> [ZExtOutputPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtOutputPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks extension output ring data.
#[derive(Debug, Clone)]
pub struct ZExtOutputExtOutputRing {
    pub segments: Vec<(u64, usize)>,
    pub capacity: usize,
    pub wrap_count: u64,
}

impl ZExtOutputExtOutputRing {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            capacity: 0,
            wrap_count: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtOutputExtOutputRing[capacity={:?}, wrap_count={:?}]", self.capacity, self.wrap_count)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for extension output ring.
pub fn z_ext_output_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_output_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_output_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_output_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_ext_output_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_output_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_output_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 77
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer77 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer77 {
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
pub fn xb_fnv1a_77(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_77<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_77<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_77(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_77(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 65
// ---------------------------------------------------------------------------

/// Generic object pool `Xc65Pool<T>`.
pub struct Xc65Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc65Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc65PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc65Pool<T> {
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
    pub fn stats(&self) -> Xc65PoolStats {
        Xc65PoolStats {
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

impl<T> Default for Xc65Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc65Scheduler`.
pub struct Xc65Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc65Scheduler {
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

impl Default for Xc65Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_65 hash for the given byte slice.
pub fn xc_65_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_65 convention.
pub fn xc_65_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe90 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe90Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe90PipelineError {
    pub stage: Xe90Stage,
    pub message: String,
}

impl std::fmt::Display for Xe90PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe90Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe90Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError>>>,
    stage_names: Vec<Xe90Stage>,
}

impl Xe90Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe90Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe90Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe90Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe90Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe90Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe90CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe90CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe90Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe90CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe90CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe90Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe90CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_90_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe90CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_90_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe90CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_90_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> {
    Ok(data)
}

pub fn xe_90_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_90_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_90_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_90_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe90PipelineError> {
    Err(Xe90PipelineError {
        stage: Xe90Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_88: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg88Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg88Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg88Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_88: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg88Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg88Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg88Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg88Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 64).
pub struct Xh64SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh64SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 106 as u64,
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

/// A compact bit set supporting boolean operations (variant 64).
pub struct Xh64BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh64BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 64).
pub struct Xi64Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi64Deque<T> {
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
pub struct Xi64Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi64Interval {
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

/// A simple interval tree (variant 64).
pub struct Xi64IntervalTree {
    xi_intervals: Vec<Xi64Interval>,
}

impl Xi64IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi64Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi64Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi64Interval) -> Vec<&Xi64Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi64Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi64Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi64Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi64Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi64Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi64Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 65) ---

/// Disjoint set / union-find for crate 65.
pub struct Xj65UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj65UnionFind {
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

const XJ65_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 65.
pub struct Xj65BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj65BTreeNode<K, V>>>,
    len: usize,
}

struct Xj65BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj65BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj65BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ65_BTREE_ORDER - 1
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
        let mid = XJ65_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj65BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj65BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj65BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj65BTreeNode::xj_new_leaf();
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


// --- xk_64 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk64SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk64SegmentTree {
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
pub struct Xk64DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk64DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_65).
#[derive(Debug, Clone)]
pub struct Xl65Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl65Rope {
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

/// Suffix array for efficient string searching (xl_65).
#[derive(Debug, Clone)]
pub struct Xl65SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl65SuffixArray {
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
pub struct Xm65MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm65MatrixSparse {
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
pub struct Xm65Tokenizer {
    text: String,
}

impl Xm65Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 64.
pub struct Xn64Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn64Fenwick {
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

// ----- AVL tree map — crate 64 -----

#[derive(Debug, Clone)]
struct Xn64AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn64AvlNode<K, V>>>,
    right: Option<Box<Xn64AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 64.
#[derive(Debug, Clone)]
pub struct Xn64AVL<K, V> {
    root: Option<Box<Xn64AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn64AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn64AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn64AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn64AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn64AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn64AvlNode<K, V>>) -> Box<Xn64AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn64AvlNode<K, V>>) -> Box<Xn64AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn64AvlNode<K, V>>) -> Box<Xn64AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn64AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn64AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn64AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn64AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn64AvlNode<K, V>>) -> &Xn64AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn64AvlNode<K, V>>) -> (Box<Xn64AvlNode<K, V>>, Option<Box<Xn64AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn64AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn64AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn64AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn64AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn64AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn64AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn64AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo64RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo64Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo64RBNode<K, V> {
    key: K,
    value: V,
    color: Xo64Color,
    left: Option<Box<Xo64RBNode<K, V>>>,
    right: Option<Box<Xo64RBNode<K, V>>>,
}

/// A red-black tree map for crate 64.
#[derive(Debug, Clone)]
pub struct Xo64RedBlack<K, V> {
    root: Option<Box<Xo64RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo64RedBlack<K, V> {
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
            r.color = Xo64Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo64RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo64RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo64RBNode {
                    key, value, color: Xo64Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo64RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo64Color::Red)
    }

    fn xo_balance(mut h: Box<Xo64RBNode<K, V>>) -> Box<Xo64RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo64Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo64RBNode<K, V>>) -> Box<Xo64RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo64Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo64RBNode<K, V>>) -> Box<Xo64RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo64Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo64RBNode<K, V>>) {
        h.color = Xo64Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo64Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo64Color::Black; }
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
            r.color = Xo64Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo64RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo64RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo64RBNode<K, V>) -> (K, V, Option<Box<Xo64RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo64RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo64Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo64RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo64ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 64.
#[derive(Debug, Clone)]
pub struct Xo64ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo64ConsistentHash {
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
            let vkey = format!("{}#xo64#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo64#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 64).
#[derive(Debug)]
pub struct Xp64SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp64Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp64Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp64Node<K, V>>>,
    xp_right: Option<Box<Xp64Node<K, V>>>,
}

impl<K: Ord, V> Xp64Node<K, V> {
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

impl<K: Ord, V> Default for Xp64SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp64SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp64Node<K, V>>>, key: &K) -> Option<Box<Xp64Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp64Node<K, V>>) -> Box<Xp64Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp64Node<K, V>>) -> Box<Xp64Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp64Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp64Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp64Node::xp_new(key, val));
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


// --------------- Xq64Treap ---------------

use std::cmp::Ordering as Xq64Ord;

struct Xq64TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq64TreapNode<K, V>>>,
    right: Option<Box<Xq64TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq64Treap<K, V> {
    root: Option<Box<Xq64TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq64TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_64_size<K, V>(node: &Option<Box<Xq64TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_64_update_size<K, V>(node: &mut Xq64TreapNode<K, V>) {
    node.size = 1 + xq_64_size(&node.left) + xq_64_size(&node.right);
}

fn xq_64_rotate_right<K, V>(mut node: Box<Xq64TreapNode<K, V>>) -> Box<Xq64TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_64_update_size(&mut node);
    left.right = Some(node);
    xq_64_update_size(&mut left);
    left
}

fn xq_64_rotate_left<K, V>(mut node: Box<Xq64TreapNode<K, V>>) -> Box<Xq64TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_64_update_size(&mut node);
    right.left = Some(node);
    xq_64_update_size(&mut right);
    right
}

fn xq_64_insert_node<K: Ord, V>(
    node: Option<Box<Xq64TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq64TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq64TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq64Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq64Ord::Less => {
                let (new_left, old) = xq_64_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_64_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_64_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq64Ord::Greater => {
                let (new_right, old) = xq_64_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_64_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_64_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_64_remove_node<K: Ord, V>(
    node: Option<Box<Xq64TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq64TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq64Ord::Less => {
                let (new_left, old) = xq_64_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_64_update_size(&mut n);
                (Some(n), old)
            }
            Xq64Ord::Greater => {
                let (new_right, old) = xq_64_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_64_update_size(&mut n);
                (Some(n), old)
            }
            Xq64Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_64_rotate_right(n);
                    let (new_right, old) = xq_64_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_64_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_64_rotate_left(n);
                    let (new_left, old) = xq_64_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_64_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_64_find_min<K, V>(node: &Option<Box<Xq64TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_64_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_64_find_max<K, V>(node: &Option<Box<Xq64TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_64_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_64_rank<K: Ord, V>(node: &Option<Box<Xq64TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq64Ord::Less => xq_64_rank(&n.left, key),
            Xq64Ord::Equal => xq_64_size(&n.left),
            Xq64Ord::Greater => 1 + xq_64_size(&n.left) + xq_64_rank(&n.right, key),
        },
    }
}

fn xq_64_kth<K, V>(node: &Option<Box<Xq64TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_64_size(&n.left);
        if k < left_size {
            xq_64_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_64_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_64_in_order<K: Clone, V>(node: &Option<Box<Xq64TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_64_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_64_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq64Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 64 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_64_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq64Ord::Equal => return Some(&n.value),
                Xq64Ord::Less => cur = &n.left,
                Xq64Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_64_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_64_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_64_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_64_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_64_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_64_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_64_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq64VEBTree ---------------

pub struct Xq64VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq64VEBTree>>,
    clusters: Vec<Option<Box<Xq64VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq64VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq64VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq64VEBTree::xq_new(self.sqrt_lo)));
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

    // ── New tests ──

    #[test]
    fn output_message_channel_id() {
        let create = OutputMessage::CreateChannel {
            name: "Build".into(),
            language_id: None,
        };
        assert_eq!(create.channel_id(), None);

        let append = OutputMessage::AppendLine {
            channel_id: "ch1".into(),
            line: "hi".into(),
        };
        assert_eq!(append.channel_id(), Some("ch1"));

        let clear = OutputMessage::Clear { channel_id: "ch2".into() };
        assert_eq!(clear.channel_id(), Some("ch2"));

        let show = OutputMessage::Show {
            channel_id: "ch3".into(),
            preserve_focus: false,
        };
        assert_eq!(show.channel_id(), Some("ch3"));

        let dispose = OutputMessage::Dispose { channel_id: "ch4".into() };
        assert_eq!(dispose.channel_id(), Some("ch4"));
    }

    #[test]
    fn output_message_is_mutating() {
        let create = OutputMessage::CreateChannel {
            name: "X".into(),
            language_id: None,
        };
        assert!(create.is_mutating());

        let show = OutputMessage::Show {
            channel_id: "ch1".into(),
            preserve_focus: true,
        };
        assert!(!show.is_mutating());

        let append = OutputMessage::AppendLine {
            channel_id: "ch1".into(),
            line: "x".into(),
        };
        assert!(append.is_mutating());
    }

    #[test]
    fn output_message_kind() {
        let msg = OutputMessage::Clear { channel_id: "ch1".into() };
        assert_eq!(msg.kind(), "clear");

        let msg2 = OutputMessage::CreateChannel { name: "A".into(), language_id: None };
        assert_eq!(msg2.kind(), "create");
    }

    #[test]
    fn output_message_display() {
        let msg = OutputMessage::CreateChannel {
            name: "Build".into(),
            language_id: Some("log".into()),
        };
        let s = format!("{msg}");
        assert!(s.contains("CreateChannel"));
        assert!(s.contains("Build"));
        assert!(s.contains("log"));

        let msg2 = OutputMessage::Clear { channel_id: "ch1".into() };
        assert!(format!("{msg2}").contains("Clear"));

        let msg3 = OutputMessage::Show {
            channel_id: "ch1".into(),
            preserve_focus: true,
        };
        assert!(format!("{msg3}").contains("preserve=true"));
    }

    #[test]
    fn output_channel_head_lines() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "Test".into(),
            language_id: None,
            content: "one\ntwo\nthree\nfour".into(),
        };
        assert_eq!(ch.head_lines(2), vec!["one", "two"]);
        assert_eq!(ch.head_lines(10).len(), 4);
        assert_eq!(ch.head_lines(0), Vec::<&str>::new());
    }

    #[test]
    fn output_channel_contains() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "T".into(),
            language_id: None,
            content: "error: something failed".into(),
        };
        assert!(ch.contains("error"));
        assert!(ch.contains("failed"));
        assert!(!ch.contains("success"));
    }

    #[test]
    fn output_channel_has_language() {
        let with = OutputChannel {
            id: "o1".into(),
            name: "T".into(),
            language_id: Some("rust".into()),
            content: String::new(),
        };
        assert!(with.has_language());

        let without = OutputChannel {
            id: "o2".into(),
            name: "T".into(),
            language_id: None,
            content: String::new(),
        };
        assert!(!without.has_language());
    }

    #[test]
    fn output_channel_filter_lines() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "Logs".into(),
            language_id: None,
            content: "INFO: ok\nERROR: bad\nINFO: done\nWARN: slow".into(),
        };
        let errors = ch.filter_lines(|l| l.starts_with("ERROR"));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], "ERROR: bad");

        let infos = ch.filter_lines(|l| l.starts_with("INFO"));
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn output_channel_push_line() {
        let mut ch = OutputChannel {
            id: "o1".into(),
            name: "T".into(),
            language_id: None,
            content: String::new(),
        };
        assert_eq!(ch.push_line("first"), 1);
        assert_eq!(ch.push_line("second"), 2);
        assert_eq!(ch.content, "first\nsecond");
    }

    #[test]
    fn output_channel_lines_method() {
        let ch = OutputChannel {
            id: "o1".into(),
            name: "T".into(),
            language_id: None,
            content: "a\nb\nc".into(),
        };
        assert_eq!(ch.lines(), vec!["a", "b", "c"]);

        let empty = OutputChannel {
            id: "o2".into(),
            name: "E".into(),
            language_id: None,
            content: String::new(),
        };
        assert!(empty.lines().is_empty());
    }

    #[test]
    fn log_output_channel_new_and_would_log() {
        let ch = LogOutputChannel::new("l1", "Server", LogLevel::Warning);
        assert_eq!(ch.id, "l1");
        assert_eq!(ch.name, "Server");
        assert!(ch.would_log(LogLevel::Error));
        assert!(ch.would_log(LogLevel::Warning));
        assert!(!ch.would_log(LogLevel::Info));
        assert!(!ch.would_log(LogLevel::Debug));
    }

    #[test]
    fn log_output_channel_with_level() {
        let ch = LogOutputChannel::new("l1", "S", LogLevel::Error)
            .with_level(LogLevel::Debug);
        assert_eq!(ch.log_level, LogLevel::Debug);
        assert!(ch.would_log(LogLevel::Info));
    }

    #[test]
    fn log_level_is_error_class() {
        assert!(LogLevel::Error.is_error_class());
        assert!(LogLevel::Warning.is_error_class());
        assert!(!LogLevel::Info.is_error_class());
        assert!(!LogLevel::Debug.is_error_class());
        assert!(!LogLevel::Trace.is_error_class());
    }

    #[test]
    fn log_level_is_diagnostic() {
        assert!(LogLevel::Trace.is_diagnostic());
        assert!(LogLevel::Debug.is_diagnostic());
        assert!(!LogLevel::Info.is_diagnostic());
        assert!(!LogLevel::Warning.is_diagnostic());
        assert!(!LogLevel::Error.is_diagnostic());
    }

    #[test]
    fn log_level_escalate_and_deescalate() {
        assert_eq!(LogLevel::Trace.escalate(), Some(LogLevel::Debug));
        assert_eq!(LogLevel::Debug.escalate(), Some(LogLevel::Info));
        assert_eq!(LogLevel::Info.escalate(), Some(LogLevel::Warning));
        assert_eq!(LogLevel::Warning.escalate(), Some(LogLevel::Error));
        assert_eq!(LogLevel::Error.escalate(), None);

        assert_eq!(LogLevel::Error.deescalate(), Some(LogLevel::Warning));
        assert_eq!(LogLevel::Warning.deescalate(), Some(LogLevel::Info));
        assert_eq!(LogLevel::Info.deescalate(), Some(LogLevel::Debug));
        assert_eq!(LogLevel::Debug.deescalate(), Some(LogLevel::Trace));
        assert_eq!(LogLevel::Trace.deescalate(), None);
    }

    #[test]
    fn log_level_escalate_deescalate_roundtrip() {
        for &level in LogLevel::all_levels() {
            if let Some(up) = level.escalate() {
                assert_eq!(up.deescalate(), Some(level));
            }
            if let Some(down) = level.deescalate() {
                assert_eq!(down.escalate(), Some(level));
            }
        }
    }

    #[test]
    fn output_error_predicates() {
        let not_found = OutputError::ChannelNotFound("ch1".into());
        assert!(not_found.is_not_found());
        assert!(!not_found.is_overflow());
        assert_eq!(not_found.channel_id(), Some("ch1"));

        let overflow = OutputError::BufferOverflow {
            channel_id: "ch2".into(),
            max_bytes: 1024,
        };
        assert!(!overflow.is_not_found());
        assert!(overflow.is_overflow());
        assert_eq!(overflow.channel_id(), Some("ch2"));

        let invalid = OutputError::InvalidName("bad".into());
        assert!(!invalid.is_not_found());
        assert!(!invalid.is_overflow());
        assert_eq!(invalid.channel_id(), None);

        let dup = OutputError::DuplicateChannelName("dup".into());
        assert_eq!(dup.channel_id(), None);
    }

    #[test]
    fn output_buffer_remaining_capacity() {
        let mut buf = OutputBuffer::new(5);
        assert_eq!(buf.remaining_capacity(), 5);
        buf.append("a");
        buf.append("b");
        assert_eq!(buf.remaining_capacity(), 3);
    }

    #[test]
    fn output_buffer_total_appended() {
        let mut buf = OutputBuffer::new(2);
        buf.append("a");
        buf.append("b");
        buf.flush();
        buf.append("c");
        assert_eq!(buf.total_appended(), 3); // 2 flushed + 1 buffered
    }

    #[test]
    fn output_buffer_set_capacity() {
        let mut buf = OutputBuffer::new(10);
        assert_eq!(buf.capacity(), 10);
        buf.set_capacity(5);
        assert_eq!(buf.capacity(), 5);
        buf.set_capacity(0); // should clamp to 1
        assert_eq!(buf.capacity(), 1);
    }

    #[test]
    fn output_merger_search() {
        let mut merger = OutputChannelMerger::new();
        merger.append("build", "compiling main.rs");
        merger.append("build", "error: type mismatch");
        merger.append("test", "test passed");
        merger.append("build", "error: missing semicolon");

        let errors = merger.search("error");
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|l| l.source_channel == "build"));
    }

    #[test]
    fn output_merger_is_empty_and_channel_count() {
        let mut merger = OutputChannelMerger::new();
        assert!(merger.is_empty());
        assert_eq!(merger.channel_count(), 0);

        merger.append("ch1", "line");
        merger.append("ch2", "line");
        assert!(!merger.is_empty());
        assert_eq!(merger.channel_count(), 2);
    }

    #[test]
    fn merged_line_new_and_contains() {
        let ml = MergedLine::new("build", "compiling OK", 42);
        assert_eq!(ml.source_channel, "build");
        assert_eq!(ml.content, "compiling OK");
        assert_eq!(ml.sequence, 42);
        assert!(ml.contains("OK"));
        assert!(!ml.contains("ERROR"));
    }

    #[test]
    fn timestamp_format_is_none() {
        assert!(TimestampFormat::None.is_none());
        assert!(!TimestampFormat::Seconds.is_none());
        assert!(!TimestampFormat::Millis.is_none());
        assert!(!TimestampFormat::Iso8601.is_none());
    }

    #[test]
    fn timestamp_format_from_str() {
        assert_eq!(TimestampFormat::from_str("none"), Some(TimestampFormat::None));
        assert_eq!(TimestampFormat::from_str("seconds"), Some(TimestampFormat::Seconds));
        assert_eq!(TimestampFormat::from_str("secs"), Some(TimestampFormat::Seconds));
        assert_eq!(TimestampFormat::from_str("s"), Some(TimestampFormat::Seconds));
        assert_eq!(TimestampFormat::from_str("millis"), Some(TimestampFormat::Millis));
        assert_eq!(TimestampFormat::from_str("ms"), Some(TimestampFormat::Millis));
        assert_eq!(TimestampFormat::from_str("iso8601"), Some(TimestampFormat::Iso8601));
        assert_eq!(TimestampFormat::from_str("ISO"), Some(TimestampFormat::Iso8601));
        assert_eq!(TimestampFormat::from_str("unknown"), None);
    }

    #[test]
    fn timestamp_format_display() {
        assert_eq!(format!("{}", TimestampFormat::None), "none");
        assert_eq!(format!("{}", TimestampFormat::Seconds), "seconds");
        assert_eq!(format!("{}", TimestampFormat::Millis), "millis");
        assert_eq!(format!("{}", TimestampFormat::Iso8601), "iso8601");
    }

    #[test]
    fn output_message_display_long_line_truncation() {
        let long = "a".repeat(100);
        let msg = OutputMessage::AppendLine {
            channel_id: "ch1".into(),
            line: long,
        };
        let display = format!("{msg}");
        assert!(display.contains("AppendLine"));
        assert!(display.contains("…"));
    }

#[test]
    fn outputchannellanguagemode_severity_ordering() {
        assert!(OutputChannelLanguageModeSeverity::Critical > OutputChannelLanguageModeSeverity::High);
        assert!(OutputChannelLanguageModeSeverity::High > OutputChannelLanguageModeSeverity::Medium);
        assert!(OutputChannelLanguageModeSeverity::Medium > OutputChannelLanguageModeSeverity::Low);
    }

    #[test]
    fn outputchannellanguagemode_severity_display() {
        assert_eq!(OutputChannelLanguageModeSeverity::Low.to_string(), "low");
        assert_eq!(OutputChannelLanguageModeSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn outputchannellanguagemode_entry_creation() {
        let e = OutputChannelLanguageModeEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, OutputChannelLanguageModeSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn outputchannellanguagemode_entry_builder() {
        let e = OutputChannelLanguageModeEntry::new("e2", "Entry 2")
            .with_severity(OutputChannelLanguageModeSeverity::High)
            .with_detail("some detail")
            .with_channel_count(42);
        assert_eq!(e.severity, OutputChannelLanguageModeSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.channel_count, 42);
    }

    #[test]
    fn outputchannellanguagemode_entry_enable_disable() {
        let mut e = OutputChannelLanguageModeEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn outputchannellanguagemode_add_and_count() {
        let mut mgr = OutputChannelLanguageMode::new("test");
        mgr.add(OutputChannelLanguageModeEntry::new("a", "A"));
        mgr.add(OutputChannelLanguageModeEntry::new("b", "B").with_severity(OutputChannelLanguageModeSeverity::High));
        assert_eq!(mgr.channel_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn outputchannellanguagemode_remove() {
        let mut mgr = OutputChannelLanguageMode::new("test");
        mgr.add(OutputChannelLanguageModeEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn outputchannellanguagemode_capacity() {
        let mut mgr = OutputChannelLanguageMode::new("test").with_capacity(1);
        assert!(mgr.add(OutputChannelLanguageModeEntry::new("a", "A")));
        assert!(!mgr.add(OutputChannelLanguageModeEntry::new("b", "B")));
    }

    #[test]
    fn outputchannellanguagemode_sorted_by_severity() {
        let mut mgr = OutputChannelLanguageMode::new("test");
        mgr.add(OutputChannelLanguageModeEntry::new("lo", "Low"));
        mgr.add(OutputChannelLanguageModeEntry::new("hi", "High").with_severity(OutputChannelLanguageModeSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, OutputChannelLanguageModeSeverity::Critical);
    }

    #[test]
    fn outputchannellanguagemode_summary() {
        let mgr = OutputChannelLanguageMode::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn outputchannelbuffermanager_config_defaults() {
        let cfg = OutputChannelBufferManagerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn outputchannelbuffermanager_item_creation() {
        let item = OutputChannelBufferManagerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn outputchannelbuffermanager_add_and_get() {
        let mut mgr = OutputChannelBufferManager::new(OutputChannelBufferManagerConfig::new("test"));
        mgr.add(OutputChannelBufferManagerItem::new("k1", "v1"));
        assert_eq!(mgr.buffer_size(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn outputchannelbuffermanager_remove_item() {
        let mut mgr = OutputChannelBufferManager::new(OutputChannelBufferManagerConfig::new("test"));
        mgr.add(OutputChannelBufferManagerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn outputchannelbuffermanager_sorted_by_priority() {
        let mut mgr = OutputChannelBufferManager::new(OutputChannelBufferManagerConfig::new("test"));
        mgr.add(OutputChannelBufferManagerItem::new("lo", "low").with_priority(1));
        mgr.add(OutputChannelBufferManagerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn outputchannelbuffermanager_items_with_tag() {
        let mut mgr = OutputChannelBufferManager::new(OutputChannelBufferManagerConfig::new("test"));
        mgr.add(OutputChannelBufferManagerItem::new("a", "1").with_tag("x"));
        mgr.add(OutputChannelBufferManagerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn outputchannelbuffermanager_report() {
        let mgr = OutputChannelBufferManager::new(OutputChannelBufferManagerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    // -- ext_output additional tests -------------------------------------------

    #[test]
    fn x_ext_output_activation_parse_language() {
        let ak = XExtOutputActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtOutputActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_output_activation_parse_command() {
        let ak = XExtOutputActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtOutputActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_output_activation_parse_star() {
        assert_eq!(XExtOutputActivationKind::parse("*"), Some(XExtOutputActivationKind::Star));
    }

    #[test]
    fn x_ext_output_activation_parse_unknown() {
        assert!(XExtOutputActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_output_activation_parse_workspace() {
        let ak = XExtOutputActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtOutputActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_output_rpc_envelope_basic() {
        let env = XExtOutputRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_output_rpc_envelope_response() {
        let env = XExtOutputRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_output_rpc_payload_checksum() {
        let env = XExtOutputRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_output_collect_sequences_works() {
        let envs = vec![
            XExtOutputRpcEnvelope::new(10, "a", ""),
            XExtOutputRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_output_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_output_filter_by_method_works() {
        let envs = vec![
            XExtOutputRpcEnvelope::new(1, "textDocument/open", ""),
            XExtOutputRpcEnvelope::new(2, "workspace/config", ""),
            XExtOutputRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_output_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_output_dedup_by_seq_works() {
        let envs = vec![
            XExtOutputRpcEnvelope::new(1, "a", "first"),
            XExtOutputRpcEnvelope::new(1, "a", "second"),
            XExtOutputRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_output_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_output_negotiate_capabilities_basic() {
        let result = x_ext_output_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_output_api_version_satisfies() {
        let v1 = XExtOutputApiVersion::new(1, 80, 0);
        let min = XExtOutputApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_output_api_version_display() {
        let v = XExtOutputApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_output_api_version_ord() {
        let v1 = XExtOutputApiVersion::new(1, 0, 0);
        let v2 = XExtOutputApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    // -- ext_output extended domain tests ----------------------------------------

    #[test]
    fn y_ext_output_enum_index() {
        assert_eq!(YExtOutputExtOutputEncoding::Utf8.index(), 0);
        assert_eq!(YExtOutputExtOutputEncoding::Ascii.index(), 1);
        assert_eq!(YExtOutputExtOutputEncoding::Latin1.index(), 2);
        assert_eq!(YExtOutputExtOutputEncoding::Raw.index(), 3);
    }

    #[test]
    fn y_ext_output_enum_label() {
        assert_eq!(YExtOutputExtOutputEncoding::Utf8.label(), "Utf8");
        assert_eq!(YExtOutputExtOutputEncoding::Ascii.label(), "Ascii");
        assert_eq!(YExtOutputExtOutputEncoding::Latin1.label(), "Latin1");
        assert_eq!(YExtOutputExtOutputEncoding::Raw.label(), "Raw");
    }

    #[test]
    fn y_ext_output_enum_all() {
        let all = YExtOutputExtOutputEncoding::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_output_enum_is_default() {
        assert!(YExtOutputExtOutputEncoding::Utf8.is_default());
        assert!(!YExtOutputExtOutputEncoding::Raw.is_default());
    }

    #[test]
    fn y_ext_output_enum_display() {
        assert_eq!(format!("{}", YExtOutputExtOutputEncoding::Utf8), "Utf8");
    }

    #[test]
    fn y_ext_output_struct_new() {
        let s = YExtOutputExtOutputBuffer::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_ext_output_struct_clear() {
        let mut s = YExtOutputExtOutputBuffer::new();
        s.lines.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_ext_output_fingerprint_deterministic() {
        let h1 = y_ext_output_fingerprint("hello");
        let h2 = y_ext_output_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_output_fingerprint("a"), y_ext_output_fingerprint("b"));
    }

    #[test]
    fn y_ext_output_truncate_short() {
        assert_eq!(y_ext_output_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_output_truncate_long() {
        let r = y_ext_output_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_output_normalize_key_basic() {
        assert_eq!(y_ext_output_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_output_split_path_basic() {
        let parts = y_ext_output_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_output_count_occurrences_basic() {
        assert_eq!(y_ext_output_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_output_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_output_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_output_in_range_basic() {
        assert!(y_ext_output_in_range(5, 1, 10));
        assert!(y_ext_output_in_range(1, 1, 10));
        assert!(y_ext_output_in_range(10, 1, 10));
        assert!(!y_ext_output_in_range(0, 1, 10));
        assert!(!y_ext_output_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_output_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_output_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_output_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_output_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_output Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_output_priority_weight() {
        assert_eq!(ZExtOutputPriority::Idle.weight(), 0);
        assert_eq!(ZExtOutputPriority::Normal.weight(), 2);
        assert_eq!(ZExtOutputPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_output_priority_label() {
        assert_eq!(ZExtOutputPriority::Low.label(), "low");
        assert_eq!(ZExtOutputPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_output_priority_is_elevated() {
        assert!(!ZExtOutputPriority::Normal.is_elevated());
        assert!(ZExtOutputPriority::High.is_elevated());
        assert!(ZExtOutputPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_output_priority_display() {
        assert_eq!(format!("{}", ZExtOutputPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_output_priority_all_asc() {
        let all = ZExtOutputPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtOutputPriority::Idle);
        assert_eq!(all[4], ZExtOutputPriority::Realtime);
    }

    #[test]
    fn z_ext_output_struct_new() {
        let s = ZExtOutputExtOutputRing::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_output_struct_toggled_clone() {
        let s = ZExtOutputExtOutputRing::new();
        let t = s.toggled_clone();
        let _ = t.wrap_count;
    }

    #[test]
    fn z_ext_output_rolling_hash_deterministic() {
        let h1 = z_ext_output_rolling_hash(b"test");
        let h2 = z_ext_output_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_output_rolling_hash(b"a"), z_ext_output_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_output_pad_to_basic() {
        assert_eq!(z_ext_output_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_output_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_output_is_identifier_basic() {
        assert!(z_ext_output_is_identifier("foo_bar"));
        assert!(z_ext_output_is_identifier("abc123"));
        assert!(!z_ext_output_is_identifier(""));
        assert!(!z_ext_output_is_identifier("has space"));
    }

    #[test]
    fn z_ext_output_levenshtein_basic() {
        assert_eq!(z_ext_output_levenshtein("", ""), 0);
        assert_eq!(z_ext_output_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_output_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_output_unique_words_basic() {
        let w = z_ext_output_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_output_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_output_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_output_common_prefix_basic() {
        assert_eq!(z_ext_output_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_output_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_output_struct_clear() {
        let mut s = ZExtOutputExtOutputRing::new();
        s.segments.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_output_rolling_hash_empty() {
        let h = z_ext_output_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_77_push_and_len() {
        let mut rb = super::XbRingBuffer77::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_77_overwrite() {
        let mut rb = super::XbRingBuffer77::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_77_get_out_of_bounds() {
        let rb = super::XbRingBuffer77::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_77_drain_all() {
        let mut rb = super::XbRingBuffer77::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_77_peek_front_back() {
        let mut rb = super::XbRingBuffer77::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_77_clear() {
        let mut rb = super::XbRingBuffer77::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_77_capacity() {
        let rb = super::XbRingBuffer77::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_77_basic() {
        let h = super::xb_fnv1a_77(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_77(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_77_different_inputs() {
        let h1 = super::xb_fnv1a_77(b"abc");
        let h2 = super::xb_fnv1a_77(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_77_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_77(&data);
        let dec = super::xb_rle_decode_77(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_77_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_77(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_77(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_77_values() {
        assert!((super::xb_clamp_77(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_77(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_77(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_77_values() {
        assert!((super::xb_lerp_77(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_77(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_77(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_77_wrap_around_twice() {
        let mut rb = super::XbRingBuffer77::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 65 ----

    #[test]
    fn xc_65_pool_new_empty() {
        let pool: super::Xc65Pool<i32> = super::Xc65Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_65_pool_release_acquire() {
        let mut pool = super::Xc65Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_65_pool_acquire_empty() {
        let mut pool: super::Xc65Pool<i32> = super::Xc65Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_65_pool_full() {
        let mut pool = super::Xc65Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_65_pool_drain() {
        let mut pool = super::Xc65Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_65_pool_stats() {
        let mut pool = super::Xc65Pool::new(8);
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
    fn xc_65_pool_clear() {
        let mut pool = super::Xc65Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_65_pool_shrink() {
        let mut pool = super::Xc65Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_65_pool_default() {
        let pool: super::Xc65Pool<String> = super::Xc65Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_65_pool_extend() {
        let mut pool = super::Xc65Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_65_pool_retain() {
        let mut pool = super::Xc65Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_65_scheduler_round_robin() {
        let mut sched = super::Xc65Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_65_scheduler_empty() {
        let mut sched = super::Xc65Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_65_scheduler_reset() {
        let mut sched = super::Xc65Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_65_scheduler_add_remove() {
        let mut sched = super::Xc65Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_65_scheduler_targets() {
        let sched = super::Xc65Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_65_hash_empty() {
        assert_eq!(super::xc_65_hash(b""), 5381);
    }

    #[test]
    fn xc_65_hash_data() {
        let h = super::xc_65_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_65_hash(b"hello"), h);
    }

    #[test]
    fn xc_65_reverse_str() {
        assert_eq!(super::xc_65_reverse("abc"), "cba");
        assert_eq!(super::xc_65_reverse(""), "");
    }


    #[test]
    fn xe_90_pipeline_empty() {
        let p = super::Xe90Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_90_pipeline_parse_stage() {
        let p = super::Xe90Pipeline::new()
            .add_parse(super::xe_90_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_90_pipeline_transform_double() {
        let p = super::Xe90Pipeline::new()
            .add_transform(super::xe_90_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_90_pipeline_validate_reverse() {
        let p = super::Xe90Pipeline::new()
            .add_validate(super::xe_90_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_90_pipeline_emit_filter() {
        let p = super::Xe90Pipeline::new()
            .add_emit(super::xe_90_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_90_pipeline_multi_stage() {
        let p = super::Xe90Pipeline::new()
            .add_parse(super::xe_90_pipeline_identity)
            .add_transform(super::xe_90_pipeline_double)
            .add_validate(super::xe_90_pipeline_reverse)
            .add_emit(super::xe_90_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_90_pipeline_error_propagation() {
        let p = super::Xe90Pipeline::new()
            .add_parse(super::xe_90_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe90Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_90_pipeline_compose() {
        let p1 = super::Xe90Pipeline::new()
            .add_parse(super::xe_90_pipeline_identity);
        let p2 = super::Xe90Pipeline::new()
            .add_transform(super::xe_90_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_90_pipeline_error_display() {
        let e = super::Xe90PipelineError {
            stage: super::Xe90Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_90_cache_put_get() {
        let mut c = super::Xe90Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_90_cache_miss() {
        let mut c: super::Xe90Cache<&str, i32> = super::Xe90Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_90_cache_ttl_expiry() {
        let mut c = super::Xe90Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_90_cache_evict() {
        let mut c = super::Xe90Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_90_cache_capacity() {
        let mut c = super::Xe90Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_90_cache_stats() {
        let mut c = super::Xe90Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_90_cache_clear() {
        let mut c = super::Xe90Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_88 graph tests ------------------------------------------------

    #[test]
    fn xg_88_graph_empty() {
        let g = super::Xg88Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_88_graph_add_node() {
        let mut g = super::Xg88Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_88_graph_add_edge() {
        let mut g = super::Xg88Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_88_graph_neighbors() {
        let mut g = super::Xg88Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_88_graph_has_path() {
        let mut g = super::Xg88Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_88_graph_self_path() {
        let g = super::Xg88Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_88_graph_topo_sort() {
        let mut g = super::Xg88Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_88_graph_cycle_detect_false() {
        let mut g = super::Xg88Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_88_graph_cycle_detect_true() {
        let mut g = super::Xg88Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_88 heap tests -------------------------------------------------

    #[test]
    fn xg_88_heap_empty() {
        let h: super::Xg88Heap<i32> = super::Xg88Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_88_heap_push_pop() {
        let mut h = super::Xg88Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_88_heap_peek() {
        let mut h = super::Xg88Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_88_heap_drain_sorted() {
        let mut h = super::Xg88Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_88_heap_merge() {
        let mut a = super::Xg88Heap::new();
        let mut b = super::Xg88Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_88_heap_default() {
        let h: super::Xg88Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_88_graph_default() {
        let g: super::Xg88Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh64_skip_insert_contains() {
        let mut sl = super::Xh64SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh64_skip_remove() {
        let mut sl = super::Xh64SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh64_skip_len() {
        let mut sl = super::Xh64SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh64_skip_range_query() {
        let mut sl = super::Xh64SkipList::xh_new(4);
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
    fn xh64_skip_floor_ceiling() {
        let mut sl = super::Xh64SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh64_skip_rank() {
        let mut sl = super::Xh64SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh64_skip_empty() {
        let sl = super::Xh64SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh64_skip_duplicates() {
        let mut sl = super::Xh64SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh64_bitset_set_test() {
        let mut bs = super::Xh64BitSet::xh_new(256);
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
    fn xh64_bitset_clear_count() {
        let mut bs = super::Xh64BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh64_bitset_and_or_xor() {
        let mut a = super::Xh64BitSet::xh_new(128);
        let mut b = super::Xh64BitSet::xh_new(128);
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
    fn xh64_bitset_iter_ones() {
        let mut bs = super::Xh64BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh64_bitset_first_last() {
        let mut bs = super::Xh64BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh64_bitset_empty() {
        let bs = super::Xh64BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi64_deque_push_pop_back() {
        let mut dq = super::Xi64Deque::xi_new(4);
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
    fn xi64_deque_push_pop_front() {
        let mut dq = super::Xi64Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi64_deque_mixed_ops() {
        let mut dq = super::Xi64Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi64_deque_get_and_split() {
        let mut dq = super::Xi64Deque::xi_new(8);
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
    fn xi64_deque_rotate_left() {
        let mut dq = super::Xi64Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi64_deque_rotate_right() {
        let mut dq = super::Xi64Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi64_deque_grow() {
        let mut dq = super::Xi64Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi64_deque_empty() {
        let dq = super::Xi64Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi64_interval_tree_insert_query() {
        let mut tree = super::Xi64IntervalTree::xi_new();
        tree.xi_insert(super::Xi64Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi64Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi64Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi64_interval_tree_overlap() {
        let mut tree = super::Xi64IntervalTree::xi_new();
        tree.xi_insert(super::Xi64Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi64Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi64Interval::xi_new(12, 20));
        let q = super::Xi64Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi64_interval_tree_remove() {
        let mut tree = super::Xi64IntervalTree::xi_new();
        tree.xi_insert(super::Xi64Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi64Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi64_interval_tree_gaps() {
        let mut tree = super::Xi64IntervalTree::xi_new();
        tree.xi_insert(super::Xi64Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi64Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi64Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi64Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi64Interval::xi_new(8, 10));
    }

    #[test]
    fn xi64_interval_tree_merge() {
        let mut tree = super::Xi64IntervalTree::xi_new();
        tree.xi_insert(super::Xi64Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi64Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi64Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi64Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi64Interval::xi_new(10, 15));
    }

    #[test]
    fn xi64_interval_tree_all() {
        let mut tree = super::Xi64IntervalTree::xi_new();
        tree.xi_insert(super::Xi64Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi64Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi64_interval_tree_empty() {
        let tree = super::Xi64IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi64_interval_tree_contains_point() {
        let iv = super::Xi64Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 65) ---

    #[test]
    fn xj_65_uf_make_and_find() {
        let mut uf = super::Xj65UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_65_uf_union_connected() {
        let mut uf = super::Xj65UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_65_uf_component_count() {
        let mut uf = super::Xj65UnionFind::xj_new();
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
    fn xj_65_uf_component_size() {
        let mut uf = super::Xj65UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_65_uf_largest_component() {
        let mut uf = super::Xj65UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_65_uf_many_elements() {
        let mut uf = super::Xj65UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_65_uf_separate_components() {
        let mut uf = super::Xj65UnionFind::xj_new();
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
    fn xj_65_uf_path_compression() {
        let mut uf = super::Xj65UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_65_bt_insert_get() {
        let mut bt = super::Xj65BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_65_bt_contains_len() {
        let mut bt = super::Xj65BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_65_bt_replace() {
        let mut bt = super::Xj65BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_65_bt_remove() {
        let mut bt = super::Xj65BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_65_bt_keys_values() {
        let mut bt = super::Xj65BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_65_bt_range() {
        let mut bt = super::Xj65BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_65_bt_min_max() {
        let mut bt = super::Xj65BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_65_bt_many_inserts() {
        let mut bt = super::Xj65BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_64 segment tree tests ---

    #[test]
    fn xk_64_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk64SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_64_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk64SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_64_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk64SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_64_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk64SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_64_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk64SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_64_st_single_element() {
        let data = vec![42];
        let st = super::Xk64SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_64_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk64SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_64_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk64SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_64 disjoint intervals tests ---

    #[test]
    fn xk_64_di_add_and_count() {
        let mut di = super::Xk64DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_64_di_merge_overlap() {
        let mut di = super::Xk64DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_64_di_contains() {
        let mut di = super::Xk64DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_64_di_remove() {
        let mut di = super::Xk64DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_64_di_covered_length() {
        let mut di = super::Xk64DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_64_di_gaps() {
        let mut di = super::Xk64DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_64_di_merge_adjacent() {
        let mut di = super::Xk64DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_64_di_empty() {
        let di = super::Xk64DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_65_rope_new_empty() {
        let rope = super::Xl65Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_65_rope_from_str() {
        let rope = super::Xl65Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_65_rope_insert_at() {
        let mut rope = super::Xl65Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_65_rope_delete_range() {
        let mut rope = super::Xl65Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_65_rope_char_at() {
        let rope = super::Xl65Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_65_rope_split_concat() {
        let rope = super::Xl65Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_65_rope_line_count() {
        let rope = super::Xl65Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_65_rope_line_at() {
        let rope = super::Xl65Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_65_sa_build_and_search() {
        let sa = super::Xl65SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_65_sa_count() {
        let sa = super::Xl65SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_65_sa_longest_repeated() {
        let sa = super::Xl65SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_65_sa_all_positions() {
        let sa = super::Xl65SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_65_sa_len() {
        let sa = super::Xl65SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_65_sa_empty() {
        let sa = super::Xl65SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_65_rope_slice() {
        let rope = super::Xl65Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_65_sa_search_start() {
        let sa = super::Xl65SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_65_sparse_set_get() {
        let mut m = super::Xm65MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_65_sparse_row_col() {
        let mut m = super::Xm65MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_65_sparse_transpose() {
        let mut m = super::Xm65MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_65_sparse_multiply_vec() {
        let mut m = super::Xm65MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_65_sparse_nnz_density() {
        let mut m = super::Xm65MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_65_sparse_clear() {
        let mut m = super::Xm65MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_65_sparse_overwrite_zero() {
        let mut m = super::Xm65MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_65_tokenizer_basic() {
        let t = super::Xm65Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_65_tokenizer_count() {
        let t = super::Xm65Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_65_tokenizer_unique() {
        let t = super::Xm65Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_65_tokenizer_frequency() {
        let t = super::Xm65Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_65_tokenizer_delimiter() {
        let t = super::Xm65Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_65_tokenizer_whitespace() {
        let t = super::Xm65Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_65_tokenizer_empty() {
        let t = super::Xm65Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 64 ----

    #[test]
    fn xn_64_fenwick_prefix_sum() {
        let mut ft = super::Xn64Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_64_fenwick_range_sum() {
        let mut ft = super::Xn64Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_64_fenwick_point_query() {
        let mut ft = super::Xn64Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_64_fenwick_len() {
        let ft = super::Xn64Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_64_fenwick_multiple_updates() {
        let mut ft = super::Xn64Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_64_fenwick_single_element() {
        let mut ft = super::Xn64Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_64_fenwick_find_kth() {
        let mut ft = super::Xn64Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_64_fenwick_negative_delta() {
        let mut ft = super::Xn64Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 64 ----

    #[test]
    fn xn_64_avl_insert_get() {
        let mut m = super::Xn64AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_64_avl_remove() {
        let mut m = super::Xn64AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_64_avl_in_order() {
        let mut m = super::Xn64AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_64_avl_min_max() {
        let mut m = super::Xn64AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_64_avl_floor_ceiling() {
        let mut m = super::Xn64AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_64_avl_height_balanced() {
        let mut m = super::Xn64AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_64_avl_overwrite() {
        let mut m = super::Xn64AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_64_avl_empty() {
        let m: super::Xn64AVL<i32, i32> = super::Xn64AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo64RedBlack tests ---

    #[test]
    fn xo_64_rb_insert_and_get() {
        let mut tree = super::Xo64RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_64_rb_len_and_empty() {
        let mut tree = super::Xo64RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_64_rb_min_max() {
        let mut tree = super::Xo64RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_64_rb_contains() {
        let mut tree = super::Xo64RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_64_rb_remove() {
        let mut tree = super::Xo64RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_64_rb_in_order() {
        let mut tree = super::Xo64RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_64_rb_black_height() {
        let mut tree = super::Xo64RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_64_rb_overwrite() {
        let mut tree = super::Xo64RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo64ConsistentHash tests ---

    #[test]
    fn xo_64_ch_add_and_count() {
        let mut ring = super::Xo64ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_64_ch_remove_node() {
        let mut ring = super::Xo64ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_64_ch_get_node() {
        let mut ring = super::Xo64ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_64_ch_empty_ring() {
        let ring = super::Xo64ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_64_ch_distribution() {
        let mut ring = super::Xo64ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_64_ch_rebalance() {
        let mut ring = super::Xo64ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_64_ch_virtual_nodes() {
        let mut ring = super::Xo64ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_64_ch_consistent_lookup() {
        let mut ring = super::Xo64ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_64_splay_insert_get() {
        let mut t = super::Xp64SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_64_splay_remove() {
        let mut t = super::Xp64SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_64_splay_count_increases() {
        let mut t = super::Xp64SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_64_splay_depth() {
        let mut t = super::Xp64SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_64_splay_len_empty() {
        let t = super::Xp64SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_64_splay_min_max() {
        let mut t = super::Xp64SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_64_splay_overwrite() {
        let mut t = super::Xp64SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_64_splay_remove_missing() {
        let mut t = super::Xp64SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_64 treap tests ----
    #[test]
    fn xq_64_treap_empty() {
        let t = super::Xq64Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_64_treap_insert_get() {
        let mut t = super::Xq64Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_64_treap_overwrite() {
        let mut t = super::Xq64Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_64_treap_remove() {
        let mut t = super::Xq64Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_64_treap_min_max() {
        let mut t = super::Xq64Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_64_treap_rank() {
        let mut t = super::Xq64Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_64_treap_kth() {
        let mut t = super::Xq64Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_64_treap_in_order() {
        let mut t = super::Xq64Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_64 VEB tree tests ----
    #[test]
    fn xq_64_veb_empty() {
        let v = super::Xq64VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_64_veb_insert_contains() {
        let mut v = super::Xq64VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_64_veb_min_max() {
        let mut v = super::Xq64VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_64_veb_delete() {
        let mut v = super::Xq64VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_64_veb_successor() {
        let mut v = super::Xq64VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_64_veb_predecessor() {
        let mut v = super::Xq64VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_64_veb_count() {
        let mut v = super::Xq64VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_64_veb_duplicate_insert() {
        let mut v = super::Xq64VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
