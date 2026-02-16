//! Structured logging service for vsedit.
//!
//! Equivalent to VS Code's `vs/platform/log/common/log.ts`.
//! Wraps the `tracing` crate to provide VS Code-compatible log levels and output channels.

use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Log levels matching VS Code's LogLevel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum LogLevel {
    Off = 0,
    Trace = 1,
    Debug = 2,
    Info = 3,
    Warning = 4,
    Error = 5,
}

impl LogLevel {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Off,
            1 => Self::Trace,
            2 => Self::Debug,
            3 => Self::Info,
            4 => Self::Warning,
            5 => Self::Error,
            _ => Self::Info,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Logger trait for vsedit services.
pub trait ILogger: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);
    fn get_level(&self) -> LogLevel;
    fn set_level(&self, level: LogLevel);

    fn trace(&self, message: &str) {
        self.log(LogLevel::Trace, message);
    }

    fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    fn warning(&self, message: &str) {
        self.log(LogLevel::Warning, message);
    }

    fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
}

/// A logger that writes to a named output channel using tracing.
pub struct Logger {
    channel: String,
    level: AtomicU8,
}

impl Logger {
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            level: AtomicU8::new(LogLevel::Info as u8),
        }
    }

    pub fn channel(&self) -> &str {
        &self.channel
    }
}

impl ILogger for Logger {
    fn log(&self, level: LogLevel, message: &str) {
        let current = LogLevel::from_u8(self.level.load(Ordering::Relaxed));
        if level < current {
            return;
        }

        match level {
            LogLevel::Trace => tracing::trace!(channel = %self.channel, "{}", message),
            LogLevel::Debug => tracing::debug!(channel = %self.channel, "{}", message),
            LogLevel::Info => tracing::info!(channel = %self.channel, "{}", message),
            LogLevel::Warning => tracing::warn!(channel = %self.channel, "{}", message),
            LogLevel::Error => tracing::error!(channel = %self.channel, "{}", message),
            LogLevel::Off => {}
        }
    }

    fn get_level(&self) -> LogLevel {
        LogLevel::from_u8(self.level.load(Ordering::Relaxed))
    }

    fn set_level(&self, level: LogLevel) {
        self.level.store(level as u8, Ordering::Relaxed);
    }
}

/// A logger that collects log entries in memory (for testing or output channels).
pub struct BufferedLogger {
    channel: String,
    level: AtomicU8,
    entries: std::sync::Mutex<Vec<LogEntry>>,
}

/// A log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub channel: String,
}

impl BufferedLogger {
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            level: AtomicU8::new(LogLevel::Trace as u8),
            entries: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

impl ILogger for BufferedLogger {
    fn log(&self, level: LogLevel, message: &str) {
        let current = LogLevel::from_u8(self.level.load(Ordering::Relaxed));
        if level < current {
            return;
        }
        self.entries.lock().unwrap().push(LogEntry {
            level,
            message: message.to_string(),
            channel: self.channel.clone(),
        });
    }

    fn get_level(&self) -> LogLevel {
        LogLevel::from_u8(self.level.load(Ordering::Relaxed))
    }

    fn set_level(&self, level: LogLevel) {
        self.level.store(level as u8, Ordering::Relaxed);
    }
}

/// Service that manages loggers for different channels.
pub struct LogService {
    default_level: AtomicU8,
    loggers: std::sync::Mutex<std::collections::HashMap<String, Arc<Logger>>>,
}

impl LogService {
    pub fn new(default_level: LogLevel) -> Self {
        Self {
            default_level: AtomicU8::new(default_level as u8),
            loggers: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn get_logger(&self, channel: &str) -> Arc<Logger> {
        let mut loggers = self.loggers.lock().unwrap();
        loggers
            .entry(channel.to_string())
            .or_insert_with(|| {
                let logger = Logger::new(channel);
                logger.set_level(LogLevel::from_u8(
                    self.default_level.load(Ordering::Relaxed),
                ));
                Arc::new(logger)
            })
            .clone()
    }

    pub fn set_default_level(&self, level: LogLevel) {
        self.default_level.store(level as u8, Ordering::Relaxed);
    }
}

// --- Additional LogLevel methods ---

impl LogLevel {
    /// Returns true if this level would be logged given a minimum threshold.
    pub fn is_enabled_at(&self, threshold: LogLevel) -> bool {
        *self >= threshold
    }
}

// --- LogEntry helpers ---

impl LogEntry {
    /// Returns true if this entry is an error.
    pub fn is_error(&self) -> bool {
        self.level == LogLevel::Error
    }

    /// Returns true if this entry is a warning.
    pub fn is_warning(&self) -> bool {
        self.level == LogLevel::Warning
    }

    /// Returns a formatted string: "[LEVEL] channel: message".
    pub fn formatted(&self) -> String {
        format!(
            "[{}] {}: {}",
            self.level.as_str().to_uppercase(),
            self.channel,
            self.message
        )
    }
}

impl std::fmt::Display for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            self.level.as_str().to_uppercase(),
            self.channel,
            self.message
        )
    }
}

impl PartialEq for LogEntry {
    fn eq(&self, other: &Self) -> bool {
        self.level == other.level
            && self.channel == other.channel
            && self.message == other.message
    }
}

impl Eq for LogEntry {}

// --- LogFilter ---

/// Filter criteria for log entries.
#[derive(Debug, Clone)]
pub struct LogFilter {
    /// Minimum level to include (None = any level).
    pub level: Option<LogLevel>,
    /// Substring pattern to match against the channel name.
    pub channel_pattern: Option<String>,
    /// Substring pattern to match against the message.
    pub message_pattern: Option<String>,
}

impl LogFilter {
    /// Create a filter with no criteria (matches everything).
    pub fn new() -> Self {
        Self {
            level: None,
            channel_pattern: None,
            message_pattern: None,
        }
    }

    /// Set the minimum level filter.
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = Some(level);
        self
    }

    /// Set the channel substring pattern.
    pub fn with_channel(mut self, pattern: impl Into<String>) -> Self {
        self.channel_pattern = Some(pattern.into());
        self
    }

    /// Set the message substring pattern.
    pub fn with_message(mut self, pattern: impl Into<String>) -> Self {
        self.message_pattern = Some(pattern.into());
        self
    }

    /// Returns true if the given entry matches all filter criteria.
    pub fn matches(&self, entry: &LogEntry) -> bool {
        if let Some(min_level) = self.level {
            if entry.level < min_level {
                return false;
            }
        }
        if let Some(ref pat) = self.channel_pattern {
            if !entry.channel.contains(pat.as_str()) {
                return false;
            }
        }
        if let Some(ref pat) = self.message_pattern {
            if !entry.message.contains(pat.as_str()) {
                return false;
            }
        }
        true
    }
}

impl Default for LogFilter {
    fn default() -> Self {
        Self::new()
    }
}

// --- BufferedLogger additional methods ---

impl BufferedLogger {
    /// Returns the number of buffered entries.
    pub fn entry_count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// Returns entries matching the given filter.
    pub fn filter(&self, f: &LogFilter) -> Vec<LogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| f.matches(e))
            .cloned()
            .collect()
    }

    /// Returns all error-level entries.
    pub fn errors(&self) -> Vec<LogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.is_error())
            .cloned()
            .collect()
    }

    /// Returns all warning-level entries.
    pub fn warnings(&self) -> Vec<LogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.is_warning())
            .cloned()
            .collect()
    }

    /// Drains all entries, returning them and leaving the buffer empty.
    pub fn drain(&self) -> Vec<LogEntry> {
        let mut entries = self.entries.lock().unwrap();
        std::mem::take(&mut *entries)
    }
}

// --- LogService additional methods ---

impl LogService {
    /// Returns a sorted list of all registered channel names.
    pub fn channel_names(&self) -> Vec<String> {
        let loggers = self.loggers.lock().unwrap();
        let mut names: Vec<String> = loggers.keys().cloned().collect();
        names.sort();
        names
    }

    /// Returns the number of registered loggers.
    pub fn logger_count(&self) -> usize {
        self.loggers.lock().unwrap().len()
    }

    /// Removes a logger by channel name. Returns true if it existed.
    pub fn remove_logger(&self, channel: &str) -> bool {
        self.loggers.lock().unwrap().remove(channel).is_some()
    }
}

// --- LogFormatter trait ---

/// Trait for formatting log entries into strings.
pub trait LogFormatter: Send + Sync {
    fn format(&self, entry: &LogEntry) -> String;
}

/// Simple text formatter: "[LEVEL] channel: message".
pub struct SimpleFormatter;

impl LogFormatter for SimpleFormatter {
    fn format(&self, entry: &LogEntry) -> String {
        entry.formatted()
    }
}

/// JSON formatter: {"level":"...","channel":"...","message":"..."}.
pub struct JsonFormatter;

impl LogFormatter for JsonFormatter {
    fn format(&self, entry: &LogEntry) -> String {
        format!(
            r#"{{"level":"{}","channel":"{}","message":"{}"}}"#,
            entry.level.as_str(),
            entry.channel.replace('"', "\\\""),
            entry.message.replace('"', "\\\""),
        )
    }
}

// --- LogRotation ---

/// Tracks entry count limits and provides rotation logic.
pub struct LogRotation {
    max_entries: usize,
    current_count: usize,
}

impl LogRotation {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            current_count: 0,
        }
    }

    /// Record that an entry was added. Returns true if rotation is needed.
    pub fn record_entry(&mut self) -> bool {
        self.current_count += 1;
        self.current_count >= self.max_entries
    }

    /// Reset the counter after rotation.
    pub fn reset(&mut self) {
        self.current_count = 0;
    }

    /// Returns the current entry count.
    pub fn current_count(&self) -> usize {
        self.current_count
    }

    /// Returns the maximum entries before rotation.
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Returns true if rotation is needed right now.
    pub fn needs_rotation(&self) -> bool {
        self.current_count >= self.max_entries
    }
}

// --- LogStats ---

/// Counts of log entries per level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogStats {
    pub trace_count: usize,
    pub debug_count: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
}

impl LogStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute stats from a slice of log entries.
    pub fn from_entries(entries: &[LogEntry]) -> Self {
        let mut stats = Self::new();
        for entry in entries {
            match entry.level {
                LogLevel::Trace => stats.trace_count += 1,
                LogLevel::Debug => stats.debug_count += 1,
                LogLevel::Info => stats.info_count += 1,
                LogLevel::Warning => stats.warning_count += 1,
                LogLevel::Error => stats.error_count += 1,
                LogLevel::Off => {}
            }
        }
        stats
    }

    /// Total number of entries across all levels.
    pub fn total(&self) -> usize {
        self.trace_count + self.debug_count + self.info_count + self.warning_count + self.error_count
    }
}

impl BufferedLogger {
    /// Compute statistics for the buffered entries.
    pub fn stats(&self) -> LogStats {
        let entries = self.entries.lock().unwrap();
        LogStats::from_entries(&entries)
    }
}

/// Initialize tracing subscriber (call once at startup).
pub fn init_tracing(level: LogLevel) {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;

    let filter = match level {
        LogLevel::Off => "off",
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warning => "warn",
        LogLevel::Error => "error",
    };

    let _ = fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Error);
    }

    #[test]
    fn buffered_logger_collects() {
        let logger = BufferedLogger::new("test");
        logger.info("hello");
        logger.warning("warn");
        logger.error("err");

        let entries = logger.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].level, LogLevel::Info);
        assert_eq!(entries[0].message, "hello");
        assert_eq!(entries[1].level, LogLevel::Warning);
        assert_eq!(entries[2].level, LogLevel::Error);
    }

    #[test]
    fn buffered_logger_respects_level() {
        let logger = BufferedLogger::new("test");
        logger.set_level(LogLevel::Warning);
        logger.trace("skip");
        logger.debug("skip");
        logger.info("skip");
        logger.warning("keep");
        logger.error("keep");

        assert_eq!(logger.entries().len(), 2);
    }

    #[test]
    fn log_service_creates_loggers() {
        let svc = LogService::new(LogLevel::Info);
        let l1 = svc.get_logger("editor");
        let l2 = svc.get_logger("editor");
        assert_eq!(l1.channel(), l2.channel());
        assert_eq!(l1.get_level(), LogLevel::Info);
    }

    #[test]
    fn log_level_from_u8_roundtrip() {
        for level in [
            LogLevel::Off,
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warning,
            LogLevel::Error,
        ] {
            assert_eq!(LogLevel::from_u8(level as u8), level);
        }
    }

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn log_entry_is_error() {
        let entry = LogEntry {
            level: LogLevel::Error,
            message: "bad".into(),
            channel: "ch".into(),
        };
        assert!(entry.is_error());
        assert!(!entry.is_warning());
    }

    #[test]
    fn log_entry_is_warning() {
        let entry = LogEntry {
            level: LogLevel::Warning,
            message: "careful".into(),
            channel: "ch".into(),
        };
        assert!(entry.is_warning());
        assert!(!entry.is_error());
    }

    #[test]
    fn log_entry_formatted() {
        let entry = LogEntry {
            level: LogLevel::Info,
            message: "hello".into(),
            channel: "editor".into(),
        };
        assert_eq!(entry.formatted(), "[INFO] editor: hello");
    }

    #[test]
    fn log_entry_display() {
        let entry = LogEntry {
            level: LogLevel::Error,
            message: "fail".into(),
            channel: "core".into(),
        };
        assert_eq!(entry.to_string(), "[ERROR] core: fail");
    }

    #[test]
    fn log_entry_partial_eq() {
        let a = LogEntry {
            level: LogLevel::Info,
            message: "msg".into(),
            channel: "ch".into(),
        };
        let b = LogEntry {
            level: LogLevel::Info,
            message: "msg".into(),
            channel: "ch".into(),
        };
        let c = LogEntry {
            level: LogLevel::Error,
            message: "msg".into(),
            channel: "ch".into(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn log_filter_matches_all() {
        let filter = LogFilter::new();
        let entry = LogEntry {
            level: LogLevel::Trace,
            message: "anything".into(),
            channel: "any".into(),
        };
        assert!(filter.matches(&entry));
    }

    #[test]
    fn log_filter_by_level() {
        let filter = LogFilter::new().with_level(LogLevel::Warning);
        let info_entry = LogEntry {
            level: LogLevel::Info,
            message: "lo".into(),
            channel: "ch".into(),
        };
        let warn_entry = LogEntry {
            level: LogLevel::Warning,
            message: "hi".into(),
            channel: "ch".into(),
        };
        assert!(!filter.matches(&info_entry));
        assert!(filter.matches(&warn_entry));
    }

    #[test]
    fn log_filter_by_channel_and_message() {
        let filter = LogFilter::new()
            .with_channel("editor")
            .with_message("save");
        let entry_match = LogEntry {
            level: LogLevel::Info,
            message: "file save ok".into(),
            channel: "editor.core".into(),
        };
        let entry_no_match = LogEntry {
            level: LogLevel::Info,
            message: "file save ok".into(),
            channel: "terminal".into(),
        };
        assert!(filter.matches(&entry_match));
        assert!(!filter.matches(&entry_no_match));
    }

    #[test]
    fn buffered_logger_entry_count() {
        let logger = BufferedLogger::new("test");
        assert_eq!(logger.entry_count(), 0);
        logger.info("a");
        logger.info("b");
        assert_eq!(logger.entry_count(), 2);
    }

    #[test]
    fn buffered_logger_errors_and_warnings() {
        let logger = BufferedLogger::new("test");
        logger.info("info msg");
        logger.warning("warn msg");
        logger.error("err msg");
        logger.error("err2");

        assert_eq!(logger.errors().len(), 2);
        assert_eq!(logger.warnings().len(), 1);
        assert_eq!(logger.warnings()[0].message, "warn msg");
    }

    #[test]
    fn buffered_logger_filter() {
        let logger = BufferedLogger::new("svc");
        logger.info("request started");
        logger.error("request failed");
        logger.info("request ended");

        let filter = LogFilter::new().with_message("request").with_level(LogLevel::Error);
        let results = logger.filter(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "request failed");
    }

    #[test]
    fn buffered_logger_drain() {
        let logger = BufferedLogger::new("test");
        logger.info("a");
        logger.info("b");
        let drained = logger.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(logger.entry_count(), 0);
    }

    #[test]
    fn buffered_logger_stats() {
        let logger = BufferedLogger::new("test");
        logger.trace("t");
        logger.debug("d");
        logger.info("i");
        logger.warning("w");
        logger.error("e");
        logger.error("e2");

        let stats = logger.stats();
        assert_eq!(stats.trace_count, 1);
        assert_eq!(stats.debug_count, 1);
        assert_eq!(stats.info_count, 1);
        assert_eq!(stats.warning_count, 1);
        assert_eq!(stats.error_count, 2);
        assert_eq!(stats.total(), 6);
    }

    #[test]
    fn log_service_channel_names_and_count() {
        let svc = LogService::new(LogLevel::Info);
        svc.get_logger("zebra");
        svc.get_logger("alpha");
        svc.get_logger("middle");

        assert_eq!(svc.logger_count(), 3);
        let names = svc.channel_names();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn log_service_remove_logger() {
        let svc = LogService::new(LogLevel::Info);
        svc.get_logger("editor");
        svc.get_logger("terminal");
        assert_eq!(svc.logger_count(), 2);

        assert!(svc.remove_logger("editor"));
        assert_eq!(svc.logger_count(), 1);
        assert!(!svc.remove_logger("nonexistent"));
    }

    #[test]
    fn log_level_is_enabled_at() {
        assert!(LogLevel::Error.is_enabled_at(LogLevel::Warning));
        assert!(LogLevel::Warning.is_enabled_at(LogLevel::Warning));
        assert!(!LogLevel::Info.is_enabled_at(LogLevel::Warning));
        assert!(LogLevel::Trace.is_enabled_at(LogLevel::Trace));
    }

    #[test]
    fn simple_formatter_output() {
        let fmt = SimpleFormatter;
        let entry = LogEntry {
            level: LogLevel::Debug,
            message: "test msg".into(),
            channel: "fmt_ch".into(),
        };
        assert_eq!(fmt.format(&entry), "[DEBUG] fmt_ch: test msg");
    }

    #[test]
    fn json_formatter_output() {
        let fmt = JsonFormatter;
        let entry = LogEntry {
            level: LogLevel::Info,
            message: "hello".into(),
            channel: "app".into(),
        };
        let json = fmt.format(&entry);
        assert_eq!(json, r#"{"level":"info","channel":"app","message":"hello"}"#);
    }

    #[test]
    fn json_formatter_escapes_quotes() {
        let fmt = JsonFormatter;
        let entry = LogEntry {
            level: LogLevel::Error,
            message: r#"say "hi""#.into(),
            channel: "ch".into(),
        };
        let json = fmt.format(&entry);
        assert!(json.contains(r#"say \"hi\""#));
    }

    #[test]
    fn log_rotation_basic() {
        let mut rot = LogRotation::new(3);
        assert!(!rot.needs_rotation());
        assert!(!rot.record_entry());
        assert!(!rot.record_entry());
        assert!(rot.record_entry()); // 3rd entry triggers rotation
        assert!(rot.needs_rotation());
        assert_eq!(rot.current_count(), 3);

        rot.reset();
        assert_eq!(rot.current_count(), 0);
        assert!(!rot.needs_rotation());
    }

    #[test]
    fn log_stats_from_entries() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "a".into(), channel: "c".into() },
            LogEntry { level: LogLevel::Info, message: "b".into(), channel: "c".into() },
            LogEntry { level: LogLevel::Error, message: "e".into(), channel: "c".into() },
        ];
        let stats = LogStats::from_entries(&entries);
        assert_eq!(stats.info_count, 2);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.total(), 3);
        assert_eq!(stats.trace_count, 0);
    }

    #[test]
    fn log_stats_empty() {
        let stats = LogStats::new();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats, LogStats::default());
    }
}
