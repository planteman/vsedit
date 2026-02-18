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

}
