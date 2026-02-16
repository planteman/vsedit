//! Structured logging service for vsedit.
//!
//! Equivalent to VS Code's `vs/platform/log/common/log.ts`.
//! Wraps the `tracing` crate to provide VS Code-compatible log levels and output channels.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// A log entry with structured metadata.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub channel: String,
    pub timestamp: u64,
    pub source: Option<String>,
    pub data: Option<HashMap<String, String>>,
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
            timestamp: now_epoch_ms(),
            source: None,
            data: None,
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

    /// Returns true if this level is at least as severe as `other`.
    pub fn is_at_least(&self, other: LogLevel) -> bool {
        *self >= other
    }

    /// Returns a static slice of all `LogLevel` variants in order.
    pub fn all() -> &'static [LogLevel] {
        &[
            LogLevel::Off,
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warning,
            LogLevel::Error,
        ]
    }
}

// --- LogEntry helpers ---

impl LogEntry {
    /// Create a new `LogEntry` with the given level, channel, and message. Timestamp is set to now.
    pub fn new(level: LogLevel, channel: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            channel: channel.into(),
            timestamp: now_epoch_ms(),
            source: None,
            data: None,
        }
    }

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

    /// Returns the number of error-level entries.
    pub fn error_count(&self) -> usize {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.is_error())
            .count()
    }

    /// Returns the number of warning-level entries.
    pub fn warning_count(&self) -> usize {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.is_warning())
            .count()
    }

    /// Returns entries matching the given level.
    pub fn entries_at_level(&self, level: LogLevel) -> Vec<LogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.level == level)
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

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Configuration for file-based logging with rotation.
#[derive(Debug, Clone)]
pub struct LogFileConfig {
    pub log_dir: PathBuf,
    pub file_name: String,
    /// Number of rotated log files to keep (default: 5).
    pub max_rotated_files: usize,
}

impl LogFileConfig {
    pub fn new(log_dir: impl Into<PathBuf>, file_name: impl Into<String>) -> Self {
        Self {
            log_dir: log_dir.into(),
            file_name: file_name.into(),
            max_rotated_files: 5,
        }
    }

    /// Default config: `~/.config/vsedit/logs/main.log`, keep 5 rotated files.
    pub fn default_config() -> Self {
        let log_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vsedit")
            .join("logs");
        Self::new(log_dir, "main.log")
    }

    pub fn log_file_path(&self) -> PathBuf {
        self.log_dir.join(&self.file_name)
    }

    /// Returns paths for rotated log files: main.log.1, main.log.2, etc.
    pub fn rotated_paths(&self) -> Vec<PathBuf> {
        (1..=self.max_rotated_files)
            .map(|i| self.log_dir.join(format!("{}.{}", self.file_name, i)))
            .collect()
    }
}

/// Initialize global logging with tracing subscriber.
///
/// Configures tracing output to stderr. Call once at startup.
pub fn init_logging(level: LogLevel, _log_file: Option<&LogFileConfig>) {
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

/// Initialize tracing subscriber (call once at startup).
pub fn init_tracing(level: LogLevel) {
    init_logging(level, None);
}

/// Change the runtime log level for a `LogService`.
pub fn set_log_level(service: &LogService, level: LogLevel) {
    service.set_default_level(level);
}

// --- Log file rotation ---

/// Manages log file rotation on disk.
pub struct LogFileRotator {
    config: LogFileConfig,
}

impl LogFileRotator {
    pub fn new(config: LogFileConfig) -> Self {
        Self { config }
    }

    /// Rotate log files: main.log -> main.log.1, main.log.1 -> main.log.2, etc.
    /// Removes files beyond `max_rotated_files`.
    pub fn rotate(&self) -> std::io::Result<()> {
        let base = self.config.log_file_path();
        // Remove the oldest if it exists
        let oldest = self.config.log_dir.join(format!(
            "{}.{}",
            self.config.file_name, self.config.max_rotated_files
        ));
        if oldest.exists() {
            std::fs::remove_file(&oldest)?;
        }
        // Shift existing rotated files
        for i in (1..self.config.max_rotated_files).rev() {
            let from = self
                .config
                .log_dir
                .join(format!("{}.{}", self.config.file_name, i));
            let to = self
                .config
                .log_dir
                .join(format!("{}.{}", self.config.file_name, i + 1));
            if from.exists() {
                std::fs::rename(&from, &to)?;
            }
        }
        // Move current log to .1
        if base.exists() {
            let first_rotated = self
                .config
                .log_dir
                .join(format!("{}.1", self.config.file_name));
            std::fs::rename(&base, &first_rotated)?;
        }
        Ok(())
    }

    /// Returns the number of existing rotated log files.
    pub fn rotated_file_count(&self) -> usize {
        self.config
            .rotated_paths()
            .iter()
            .filter(|p| p.exists())
            .count()
    }

    pub fn config(&self) -> &LogFileConfig {
        &self.config
    }
}

// --- Developer log viewer ---

/// Represents a developer-facing log output panel (e.g., "Log (Main)").
pub struct LogViewer {
    channel_name: String,
    entries: Vec<LogEntry>,
    max_entries: usize,
}

impl LogViewer {
    pub fn new(channel_name: impl Into<String>) -> Self {
        Self {
            channel_name: channel_name.into(),
            entries: Vec::new(),
            max_entries: 10_000,
        }
    }

    /// Add a log entry to the viewer.
    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Filter entries by level, source substring, and time range.
    pub fn filter(
        &self,
        min_level: Option<LogLevel>,
        source_pattern: Option<&str>,
        time_start: Option<u64>,
        time_end: Option<u64>,
    ) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(min) = min_level {
                    if e.level < min {
                        return false;
                    }
                }
                if let Some(pat) = source_pattern {
                    match &e.source {
                        Some(src) if src.contains(pat) => {}
                        None if e.channel.contains(pat) => {}
                        _ => return false,
                    }
                }
                if let Some(start) = time_start {
                    if e.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = time_end {
                    if e.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn channel_name(&self) -> &str {
        &self.channel_name
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
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
        let entry = LogEntry::new(LogLevel::Error, "ch", "bad");
        assert!(entry.is_error());
        assert!(!entry.is_warning());
    }

    #[test]
    fn log_entry_is_warning() {
        let entry = LogEntry::new(LogLevel::Warning, "ch", "careful");
        assert!(entry.is_warning());
        assert!(!entry.is_error());
    }

    #[test]
    fn log_entry_formatted() {
        let entry = LogEntry::new(LogLevel::Info, "editor", "hello");
        assert_eq!(entry.formatted(), "[INFO] editor: hello");
    }

    #[test]
    fn log_entry_display() {
        let entry = LogEntry::new(LogLevel::Error, "core", "fail");
        assert_eq!(entry.to_string(), "[ERROR] core: fail");
    }

    #[test]
    fn log_entry_partial_eq() {
        let a = LogEntry::new(LogLevel::Info, "ch", "msg");
        let b = LogEntry::new(LogLevel::Info, "ch", "msg");
        let c = LogEntry::new(LogLevel::Error, "ch", "msg");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn log_filter_matches_all() {
        let filter = LogFilter::new();
        let entry = LogEntry::new(LogLevel::Trace, "any", "anything");
        assert!(filter.matches(&entry));
    }

    #[test]
    fn log_filter_by_level() {
        let filter = LogFilter::new().with_level(LogLevel::Warning);
        let info_entry = LogEntry::new(LogLevel::Info, "ch", "lo");
        let warn_entry = LogEntry::new(LogLevel::Warning, "ch", "hi");
        assert!(!filter.matches(&info_entry));
        assert!(filter.matches(&warn_entry));
    }

    #[test]
    fn log_filter_by_channel_and_message() {
        let filter = LogFilter::new()
            .with_channel("editor")
            .with_message("save");
        let entry_match = LogEntry::new(LogLevel::Info, "editor.core", "file save ok");
        let entry_no_match = LogEntry::new(LogLevel::Info, "terminal", "file save ok");
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
        let entry = LogEntry::new(LogLevel::Debug, "fmt_ch", "test msg");
        assert_eq!(fmt.format(&entry), "[DEBUG] fmt_ch: test msg");
    }

    #[test]
    fn json_formatter_output() {
        let fmt = JsonFormatter;
        let entry = LogEntry::new(LogLevel::Info, "app", "hello");
        let json = fmt.format(&entry);
        assert_eq!(json, r#"{"level":"info","channel":"app","message":"hello"}"#);
    }

    #[test]
    fn json_formatter_escapes_quotes() {
        let fmt = JsonFormatter;
        let entry = LogEntry::new(LogLevel::Error, "ch", r#"say "hi""#);
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
            LogEntry::new(LogLevel::Info, "c", "a"),
            LogEntry::new(LogLevel::Info, "c", "b"),
            LogEntry::new(LogLevel::Error, "c", "e"),
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

    // --- New feature tests ---

    #[test]
    fn log_entry_new_has_timestamp() {
        let entry = LogEntry::new(LogLevel::Info, "ch", "msg");
        assert!(entry.timestamp > 0);
        assert!(entry.source.is_none());
        assert!(entry.data.is_none());
    }

    #[test]
    fn log_entry_with_source_and_data() {
        let mut entry = LogEntry::new(LogLevel::Debug, "editor", "file opened");
        entry.source = Some("editor.core".to_string());
        let mut data = HashMap::new();
        data.insert("path".to_string(), "/tmp/test.rs".to_string());
        entry.data = Some(data);
        assert_eq!(entry.source.as_deref(), Some("editor.core"));
        assert_eq!(entry.data.as_ref().unwrap().get("path").unwrap(), "/tmp/test.rs");
    }

    #[test]
    fn log_file_config_default() {
        let cfg = LogFileConfig::default_config();
        assert!(cfg.log_file_path().to_string_lossy().contains("vsedit"));
        assert_eq!(cfg.file_name, "main.log");
        assert_eq!(cfg.max_rotated_files, 5);
    }

    #[test]
    fn log_file_config_rotated_paths() {
        let cfg = LogFileConfig::new("/tmp/logs", "app.log");
        let paths = cfg.rotated_paths();
        assert_eq!(paths.len(), 5);
        assert!(paths[0].to_string_lossy().contains("app.log.1"));
        assert!(paths[4].to_string_lossy().contains("app.log.5"));
    }

    #[test]
    fn log_file_rotator_rotate() {
        let dir = std::env::temp_dir().join("vsedit-log-test-rotate");
        let _ = std::fs::create_dir_all(&dir);
        let cfg = LogFileConfig::new(&dir, "test.log");
        let main_path = cfg.log_file_path();
        std::fs::write(&main_path, "log content").unwrap();

        let rotator = LogFileRotator::new(cfg);
        rotator.rotate().unwrap();

        assert!(!main_path.exists());
        assert!(dir.join("test.log.1").exists());
        assert_eq!(rotator.rotated_file_count(), 1);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_viewer_push_and_filter() {
        let mut viewer = LogViewer::new("Log (Main)");
        viewer.push(LogEntry::new(LogLevel::Info, "editor", "opened file"));
        viewer.push(LogEntry::new(LogLevel::Error, "editor", "save failed"));
        viewer.push(LogEntry::new(LogLevel::Debug, "terminal", "resize"));

        assert_eq!(viewer.entry_count(), 3);
        assert_eq!(viewer.channel_name(), "Log (Main)");

        // Filter by level
        let errors = viewer.filter(Some(LogLevel::Error), None, None, None);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "save failed");

        // Filter by source
        let editor_entries = viewer.filter(None, Some("editor"), None, None);
        assert_eq!(editor_entries.len(), 2);
    }

    #[test]
    fn log_viewer_max_entries() {
        let mut viewer = LogViewer::new("test");
        viewer.max_entries = 3;
        for i in 0..5 {
            viewer.push(LogEntry::new(LogLevel::Info, "ch", format!("msg{i}")));
        }
        assert_eq!(viewer.entry_count(), 3);
        assert_eq!(viewer.entries()[0].message, "msg2");
    }

    #[test]
    fn log_viewer_clear() {
        let mut viewer = LogViewer::new("test");
        viewer.push(LogEntry::new(LogLevel::Info, "ch", "msg"));
        viewer.clear();
        assert_eq!(viewer.entry_count(), 0);
    }

    #[test]
    fn init_logging_does_not_panic() {
        // Just verify it doesn't panic (tracing can only be initialized once)
        init_logging(LogLevel::Info, None);
    }

    #[test]
    fn set_log_level_changes_default() {
        let svc = LogService::new(LogLevel::Info);
        set_log_level(&svc, LogLevel::Debug);
        let logger = svc.get_logger("test-channel");
        assert_eq!(logger.get_level(), LogLevel::Debug);
    }

    #[test]
    fn buffered_logger_entries_have_timestamps() {
        let logger = BufferedLogger::new("ts-test");
        logger.info("hello");
        let entries = logger.entries();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].timestamp > 0);
    }

    #[test]
    fn log_level_is_at_least() {
        assert!(LogLevel::Error.is_at_least(LogLevel::Warning));
        assert!(LogLevel::Warning.is_at_least(LogLevel::Warning));
        assert!(!LogLevel::Info.is_at_least(LogLevel::Warning));
        assert!(LogLevel::Trace.is_at_least(LogLevel::Off));
        assert!(!LogLevel::Off.is_at_least(LogLevel::Trace));
    }

    #[test]
    fn log_level_all_variants() {
        let all = LogLevel::all();
        assert_eq!(all.len(), 6);
        assert_eq!(all[0], LogLevel::Off);
        assert_eq!(all[5], LogLevel::Error);
        // Verify ordering is ascending
        for w in all.windows(2) {
            assert!(w[0] <= w[1]);
        }
    }

    #[test]
    fn buffered_logger_error_count() {
        let logger = BufferedLogger::new("test");
        logger.info("ok");
        logger.error("e1");
        logger.warning("w1");
        logger.error("e2");
        assert_eq!(logger.error_count(), 2);
    }

    #[test]
    fn buffered_logger_warning_count() {
        let logger = BufferedLogger::new("test");
        logger.warning("w1");
        logger.warning("w2");
        logger.info("i1");
        logger.error("e1");
        assert_eq!(logger.warning_count(), 2);
    }

    #[test]
    fn buffered_logger_entries_at_level() {
        let logger = BufferedLogger::new("test");
        logger.trace("t1");
        logger.info("i1");
        logger.info("i2");
        logger.error("e1");
        let infos = logger.entries_at_level(LogLevel::Info);
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].message, "i1");
        assert_eq!(infos[1].message, "i2");
        let traces = logger.entries_at_level(LogLevel::Trace);
        assert_eq!(traces.len(), 1);
        let warnings = logger.entries_at_level(LogLevel::Warning);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn log_entry_display_all_levels() {
        for level in LogLevel::all() {
            if *level == LogLevel::Off {
                continue;
            }
            let entry = LogEntry::new(*level, "ch", "msg");
            let display = entry.to_string();
            assert!(display.starts_with('['));
            assert!(display.contains("ch: msg"));
        }
    }

    #[test]
    fn buffered_logger_counts_after_clear() {
        let logger = BufferedLogger::new("test");
        logger.error("e1");
        logger.warning("w1");
        assert_eq!(logger.error_count(), 1);
        assert_eq!(logger.warning_count(), 1);
        logger.clear();
        assert_eq!(logger.error_count(), 0);
        assert_eq!(logger.warning_count(), 0);
        assert!(logger.entries_at_level(LogLevel::Error).is_empty());
    }

    #[test]
    fn log_viewer_filter_by_time_range() {
        let mut viewer = LogViewer::new("time-test");
        let mut e1 = LogEntry::new(LogLevel::Info, "ch", "early");
        e1.timestamp = 100;
        let mut e2 = LogEntry::new(LogLevel::Info, "ch", "middle");
        e2.timestamp = 200;
        let mut e3 = LogEntry::new(LogLevel::Info, "ch", "late");
        e3.timestamp = 300;
        viewer.push(e1);
        viewer.push(e2);
        viewer.push(e3);

        let results = viewer.filter(None, None, Some(150), Some(250));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "middle");
    }
}
