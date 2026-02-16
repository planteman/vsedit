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
}
