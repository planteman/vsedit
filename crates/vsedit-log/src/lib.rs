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

    /// Search entries by message substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&LogEntry> {
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.message.to_lowercase().contains(&lower))
            .collect()
    }

    /// Return entries grouped by log level.
    pub fn group_by_level(&self) -> HashMap<String, Vec<&LogEntry>> {
        let mut groups: HashMap<String, Vec<&LogEntry>> = HashMap::new();
        for entry in &self.entries {
            groups
                .entry(entry.level.as_str().to_string())
                .or_default()
                .push(entry);
        }
        groups
    }
}

// ---------------------------------------------------------------------------
// StructuredLogEntry
// ---------------------------------------------------------------------------

/// A log entry with structured key-value metadata.
#[derive(Debug, Clone)]
pub struct StructuredLogEntry {
    pub level: LogLevel,
    pub message: String,
    pub channel: String,
    pub timestamp: u64,
    pub fields: HashMap<String, String>,
}

impl StructuredLogEntry {
    pub fn new(level: LogLevel, channel: &str, message: &str) -> Self {
        Self {
            level,
            message: message.to_string(),
            channel: channel.to_string(),
            timestamp: now_epoch_ms(),
            fields: HashMap::new(),
        }
    }

    /// Add a key-value field.
    pub fn with_field(mut self, key: &str, value: &str) -> Self {
        self.fields.insert(key.to_string(), value.to_string());
        self
    }

    /// Get a field value by key.
    pub fn get_field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    /// Format as a JSON string.
    pub fn to_json(&self) -> String {
        let fields_json: Vec<String> = self
            .fields
            .iter()
            .map(|(k, v)| format!(r#""{}":"{}""#, k.replace('"', "\\\""), v.replace('"', "\\\"")))
            .collect();
        format!(
            r#"{{"level":"{}","channel":"{}","message":"{}","fields":{{{}}}}}"#,
            self.level.as_str(),
            self.channel.replace('"', "\\\""),
            self.message.replace('"', "\\\""),
            fields_json.join(",")
        )
    }

    /// Convert to a plain LogEntry (fields stored in data map).
    pub fn to_log_entry(&self) -> LogEntry {
        LogEntry {
            level: self.level,
            message: self.message.clone(),
            channel: self.channel.clone(),
            timestamp: self.timestamp,
            source: None,
            data: if self.fields.is_empty() {
                None
            } else {
                Some(self.fields.clone())
            },
        }
    }
}

// ---------------------------------------------------------------------------
// LogExporter
// ---------------------------------------------------------------------------

/// Export log entries to different string formats.
pub struct LogExporter;

impl LogExporter {
    /// Export entries as newline-delimited plain text.
    pub fn to_text(entries: &[LogEntry]) -> String {
        entries
            .iter()
            .map(|e| e.formatted())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Export entries as a JSON array string.
    pub fn to_json(entries: &[LogEntry]) -> String {
        let items: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    r#"{{"level":"{}","channel":"{}","message":"{}","timestamp":{}}}"#,
                    e.level.as_str(),
                    e.channel.replace('"', "\\\""),
                    e.message.replace('"', "\\\""),
                    e.timestamp
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Export entries as CSV (level,channel,message,timestamp).
    pub fn to_csv(entries: &[LogEntry]) -> String {
        let mut lines = vec!["level,channel,message,timestamp".to_string()];
        for e in entries {
            lines.push(format!(
                "{},{},{},{}",
                e.level.as_str(),
                e.channel.replace(',', ";"),
                e.message.replace(',', ";"),
                e.timestamp,
            ));
        }
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// LogAggregator
// ---------------------------------------------------------------------------

/// Aggregate log entries by grouping criteria.
#[derive(Debug, Default)]
pub struct LogAggregator {
    groups: HashMap<String, Vec<LogEntry>>,
}

impl LogAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Group entries by channel.
    pub fn group_by_channel(entries: &[LogEntry]) -> Self {
        let mut agg = Self::new();
        for entry in entries {
            agg.groups
                .entry(entry.channel.clone())
                .or_default()
                .push(entry.clone());
        }
        agg
    }

    /// Group entries by log level.
    pub fn group_by_level(entries: &[LogEntry]) -> Self {
        let mut agg = Self::new();
        for entry in entries {
            agg.groups
                .entry(entry.level.as_str().to_string())
                .or_default()
                .push(entry.clone());
        }
        agg
    }

    /// Group entries by a message pattern prefix (first word).
    pub fn group_by_first_word(entries: &[LogEntry]) -> Self {
        let mut agg = Self::new();
        for entry in entries {
            let key = entry
                .message
                .split_whitespace()
                .next()
                .unwrap_or("(empty)")
                .to_string();
            agg.groups.entry(key).or_default().push(entry.clone());
        }
        agg
    }

    /// Get group names.
    pub fn group_names(&self) -> Vec<&str> {
        self.groups.keys().map(String::as_str).collect()
    }

    /// Get entries in a specific group.
    pub fn get_group(&self, name: &str) -> Option<&[LogEntry]> {
        self.groups.get(name).map(Vec::as_slice)
    }

    /// Number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total entries across all groups.
    pub fn total_entries(&self) -> usize {
        self.groups.values().map(Vec::len).sum()
    }
}

/// Count entries matching a specific log level across a slice of entries.
pub fn count_at_level(entries: &[LogEntry], level: LogLevel) -> usize {
    entries.iter().filter(|e| e.level == level).count()
}

/// Extract unique channel names from a slice of log entries.
pub fn unique_channels(entries: &[LogEntry]) -> Vec<String> {
    let mut channels: Vec<String> = entries.iter().map(|e| e.channel.clone()).collect();
    channels.sort();
    channels.dedup();
    channels
}

/// Filter entries to only those within a time range (inclusive).
pub fn entries_in_time_range(entries: &[LogEntry], start: u64, end: u64) -> Vec<&LogEntry> {
    entries
        .iter()
        .filter(|e| e.timestamp >= start && e.timestamp <= end)
        .collect()
}

/// Find the most recent entry (highest timestamp) in a slice.
pub fn most_recent_entry(entries: &[LogEntry]) -> Option<&LogEntry> {
    entries.iter().max_by_key(|e| e.timestamp)
}

/// Summarize entries: return a tuple (trace, debug, info, warning, error) counts.
pub fn level_summary(entries: &[LogEntry]) -> (usize, usize, usize, usize, usize) {
    let trace = count_at_level(entries, LogLevel::Trace);
    let debug = count_at_level(entries, LogLevel::Debug);
    let info = count_at_level(entries, LogLevel::Info);
    let warning = count_at_level(entries, LogLevel::Warning);
    let error = count_at_level(entries, LogLevel::Error);
    (trace, debug, info, warning, error)
}

/// Group entries by channel, returning a map of channel -> entries.
pub fn group_by_channel(entries: &[LogEntry]) -> HashMap<String, Vec<&LogEntry>> {
    let mut map: HashMap<String, Vec<&LogEntry>> = HashMap::new();
    for entry in entries {
        map.entry(entry.channel.clone()).or_default().push(entry);
    }
    map
}

/// Check if any entry in the slice is an error.
pub fn has_errors(entries: &[LogEntry]) -> bool {
    entries.iter().any(|e| e.level == LogLevel::Error)
}

/// Check if any entry contains a substring in its message.
pub fn has_message_containing(entries: &[LogEntry], substring: &str) -> bool {
    entries.iter().any(|e| e.message.contains(substring))
}

/// Return entries sorted by timestamp (oldest first).
pub fn sort_by_time(entries: &mut [LogEntry]) {
    entries.sort_by_key(|e| e.timestamp);
}

// ---------------------------------------------------------------------------
// LogRateLimiter
// ---------------------------------------------------------------------------

/// Rate limiter that tracks per-key timestamps and suppresses messages that
/// arrive faster than the configured interval (in milliseconds).
pub struct LogRateLimiter {
    interval_ms: u64,
    last_seen: std::sync::Mutex<HashMap<String, u64>>,
}

impl LogRateLimiter {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_seen: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` if the message should be allowed through.
    /// `key` is typically a deduplication key (e.g. message template or channel).
    pub fn allow(&self, key: &str, now_ms: u64) -> bool {
        let mut map = self.last_seen.lock().unwrap();
        match map.get(key) {
            Some(&last) if now_ms.saturating_sub(last) < self.interval_ms => false,
            _ => {
                map.insert(key.to_string(), now_ms);
                true
            }
        }
    }

    /// Reset all rate-limit state.
    pub fn reset(&self) {
        self.last_seen.lock().unwrap().clear();
    }

    /// Number of distinct keys currently tracked.
    pub fn tracked_key_count(&self) -> usize {
        self.last_seen.lock().unwrap().len()
    }
}

// ---------------------------------------------------------------------------
// LogBuffer – fixed-capacity ring buffer for log entries
// ---------------------------------------------------------------------------

/// A bounded ring buffer that evicts the oldest entry when full.
pub struct LogBuffer {
    entries: Vec<LogEntry>,
    capacity: usize,
    total_pushed: usize,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "LogBuffer capacity must be > 0");
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            total_pushed: 0,
        }
    }

    /// Push an entry; if the buffer is full the oldest entry is dropped.
    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
        self.total_pushed += 1;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Total number of entries ever pushed (including evicted ones).
    pub fn total_pushed(&self) -> usize {
        self.total_pushed
    }

    /// Number of entries that were evicted due to capacity limits.
    pub fn evicted_count(&self) -> usize {
        self.total_pushed.saturating_sub(self.entries.len())
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Drain all entries, returning them.
    pub fn drain(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut self.entries)
    }

    /// Clear the buffer without returning entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the newest entry (last pushed).
    pub fn newest(&self) -> Option<&LogEntry> {
        self.entries.last()
    }

    /// Return the oldest entry still in the buffer.
    pub fn oldest(&self) -> Option<&LogEntry> {
        self.entries.first()
    }
}

// ---------------------------------------------------------------------------
// LogEntry – additional helpers
// ---------------------------------------------------------------------------

impl LogEntry {
    /// Attach a data field to this entry (builder style).
    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Attach a source to this entry (builder style).
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Returns true if this entry's message contains `needle` (case-insensitive).
    pub fn message_contains_ci(&self, needle: &str) -> bool {
        self.message.to_lowercase().contains(&needle.to_lowercase())
    }

    /// Get a data field value by key.
    pub fn get_data(&self, key: &str) -> Option<&str> {
        self.data.as_ref().and_then(|d| d.get(key).map(String::as_str))
    }

    /// Returns all data field keys, sorted.
    pub fn data_keys(&self) -> Vec<&str> {
        match &self.data {
            Some(d) => {
                let mut keys: Vec<&str> = d.keys().map(String::as_str).collect();
                keys.sort();
                keys
            }
            None => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// CompactFormatter
// ---------------------------------------------------------------------------

/// Compact single-line formatter: "LEVEL channel msg" (no brackets, minimal).
pub struct CompactFormatter;

impl LogFormatter for CompactFormatter {
    fn format(&self, entry: &LogEntry) -> String {
        format!(
            "{} {} {}",
            entry.level.as_str().to_uppercase(),
            entry.channel,
            entry.message,
        )
    }
}

// ---------------------------------------------------------------------------
// PrettyFormatter
// ---------------------------------------------------------------------------

/// Multi-line pretty formatter with optional data fields.
pub struct PrettyFormatter;

impl LogFormatter for PrettyFormatter {
    fn format(&self, entry: &LogEntry) -> String {
        let mut out = format!(
            "--- {} ---\nchannel: {}\nmessage: {}",
            entry.level.as_str().to_uppercase(),
            entry.channel,
            entry.message,
        );
        if let Some(ref src) = entry.source {
            out.push_str(&format!("\nsource:  {src}"));
        }
        if let Some(ref data) = entry.data {
            let mut keys: Vec<&String> = data.keys().collect();
            keys.sort();
            for k in keys {
                out.push_str(&format!("\n  {k}: {}", data[k]));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// LogLevel – parse from string
// ---------------------------------------------------------------------------

impl LogLevel {
    /// Parse a log level from a case-insensitive string.
    /// Returns `None` for unrecognised values.
    pub fn from_str_ci(s: &str) -> Option<LogLevel> {
        match s.to_lowercase().as_str() {
            "off" => Some(LogLevel::Off),
            "trace" => Some(LogLevel::Trace),
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warning" | "warn" => Some(LogLevel::Warning),
            "error" => Some(LogLevel::Error),
            _ => None,
        }
    }

    /// Iterator over loggable levels (excludes `Off`).
    pub fn loggable() -> &'static [LogLevel] {
        &[
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warning,
            LogLevel::Error,
        ]
    }
}

// ---------------------------------------------------------------------------
// Free functions – advanced entry analysis
// ---------------------------------------------------------------------------

/// Deduplicate consecutive entries that have the same level and message,
/// keeping only the first occurrence of each run.
pub fn dedup_consecutive(entries: &[LogEntry]) -> Vec<&LogEntry> {
    let mut result: Vec<&LogEntry> = Vec::new();
    for entry in entries {
        let dominated = result.last().map_or(false, |prev: &&LogEntry| {
            prev.level == entry.level && prev.message == entry.message
        });
        if !dominated {
            result.push(entry);
        }
    }
    result
}

/// Return entries whose message matches any of the provided substrings.
pub fn entries_matching_any<'a>(entries: &'a [LogEntry], patterns: &[&str]) -> Vec<&'a LogEntry> {
    entries
        .iter()
        .filter(|e| patterns.iter().any(|p| e.message.contains(p)))
        .collect()
}

/// Partition entries into two vecs: (matching, non_matching) based on a filter.
pub fn partition_by_filter<'a>(entries: &'a [LogEntry], filter: &LogFilter) -> (Vec<&'a LogEntry>, Vec<&'a LogEntry>) {
    let mut matching = Vec::new();
    let mut rest = Vec::new();
    for e in entries {
        if filter.matches(e) {
            matching.push(e);
        } else {
            rest.push(e);
        }
    }
    (matching, rest)
}

/// Compute a histogram: how many entries exist per channel.
pub fn channel_histogram(entries: &[LogEntry]) -> HashMap<String, usize> {
    let mut map: HashMap<String, usize> = HashMap::new();
    for e in entries {
        *map.entry(e.channel.clone()).or_insert(0) += 1;
    }
    map
}

/// Find the earliest entry (lowest timestamp).
pub fn earliest_entry(entries: &[LogEntry]) -> Option<&LogEntry> {
    entries.iter().min_by_key(|e| e.timestamp)
}

/// Return the time span (min_ts, max_ts) covered by a set of entries.
/// Returns `None` if the slice is empty.
pub fn time_span(entries: &[LogEntry]) -> Option<(u64, u64)> {
    let min = entries.iter().map(|e| e.timestamp).min()?;
    let max = entries.iter().map(|e| e.timestamp).max()?;
    Some((min, max))
}

/// Extract all unique data-field keys across a set of entries.
pub fn all_data_keys(entries: &[LogEntry]) -> Vec<String> {
    let mut keys: Vec<String> = entries
        .iter()
        .filter_map(|e| e.data.as_ref())
        .flat_map(|d| d.keys().cloned())
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

// ---------------------------------------------------------------------------
// LogOutputRotator – rotates log files by size/count
// ---------------------------------------------------------------------------

/// Policy describing when and how to rotate log files.
#[derive(Debug, Clone)]
pub struct RotationPolicy {
    /// Maximum size in bytes before rotation is triggered.
    pub max_bytes: u64,
    /// Maximum number of rotated files to keep.
    pub max_files: usize,
    /// Suffix pattern for rotated files (e.g. ".1", ".2", …).
    pub suffix_style: RotationSuffixStyle,
}

/// How rotated file names are generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationSuffixStyle {
    /// Numeric suffix: log.1, log.2, …
    Numeric,
    /// Timestamp-based suffix.
    Timestamp,
}

impl RotationPolicy {
    pub fn new(max_bytes: u64, max_files: usize) -> Self {
        Self {
            max_bytes,
            max_files,
            suffix_style: RotationSuffixStyle::Numeric,
        }
    }

    pub fn with_suffix_style(mut self, style: RotationSuffixStyle) -> Self {
        self.suffix_style = style;
        self
    }
}

/// Manages log file rotation logic (does not touch the filesystem directly).
#[derive(Debug)]
pub struct LogOutputRotator {
    policy: RotationPolicy,
    current_size: u64,
    rotation_count: usize,
    history: Vec<RotationRecord>,
}

/// Record of a single rotation event.
#[derive(Debug, Clone)]
pub struct RotationRecord {
    pub old_name: String,
    pub new_name: String,
    pub rotated_at_bytes: u64,
    pub sequence: usize,
}

impl LogOutputRotator {
    pub fn new(policy: RotationPolicy) -> Self {
        Self {
            policy,
            current_size: 0,
            rotation_count: 0,
            history: Vec::new(),
        }
    }

    /// Record that `bytes` were written.  Returns `true` if rotation is needed.
    pub fn record_write(&mut self, bytes: u64) -> bool {
        self.current_size += bytes;
        self.needs_rotation()
    }

    /// Whether the current file has exceeded the size limit.
    pub fn needs_rotation(&self) -> bool {
        self.current_size >= self.policy.max_bytes
    }

    /// Perform a rotation (logically): resets size counter, bumps sequence,
    /// and returns the name of the rotated file.
    pub fn rotate(&mut self, base_name: &str) -> Option<String> {
        if !self.needs_rotation() {
            return None;
        }
        self.rotation_count += 1;
        let new_name = match self.policy.suffix_style {
            RotationSuffixStyle::Numeric => format!("{base_name}.{}", self.rotation_count),
            RotationSuffixStyle::Timestamp => format!("{base_name}.ts{}", self.rotation_count),
        };
        self.history.push(RotationRecord {
            old_name: base_name.to_string(),
            new_name: new_name.clone(),
            rotated_at_bytes: self.current_size,
            sequence: self.rotation_count,
        });
        self.current_size = 0;
        Some(new_name)
    }

    /// Names of files that should be pruned (oldest first) to stay within `max_files`.
    pub fn files_to_prune(&self) -> Vec<String> {
        if self.history.len() <= self.policy.max_files {
            return Vec::new();
        }
        let excess = self.history.len() - self.policy.max_files;
        self.history[..excess].iter().map(|r| r.new_name.clone()).collect()
    }

    pub fn rotation_count(&self) -> usize { self.rotation_count }
    pub fn current_size(&self) -> u64 { self.current_size }
    pub fn policy(&self) -> &RotationPolicy { &self.policy }
    pub fn history(&self) -> &[RotationRecord] { &self.history }
}

// ---------------------------------------------------------------------------
// LogLevelRuntimeAdjuster – adjusts log levels at runtime
// ---------------------------------------------------------------------------

/// Tracks per-channel runtime log level overrides.
#[derive(Debug)]
pub struct LogLevelRuntimeAdjuster {
    global_level: LogLevel,
    channel_overrides: HashMap<String, LogLevel>,
    change_log: Vec<LevelChangeRecord>,
}

/// Record of a log level change.
#[derive(Debug, Clone)]
pub struct LevelChangeRecord {
    pub channel: Option<String>,
    pub old_level: LogLevel,
    pub new_level: LogLevel,
    pub timestamp: u64,
}

impl LogLevelRuntimeAdjuster {
    pub fn new(global_level: LogLevel) -> Self {
        Self {
            global_level,
            channel_overrides: HashMap::new(),
            change_log: Vec::new(),
        }
    }

    /// Set the global log level, recording the change.
    pub fn set_global(&mut self, level: LogLevel, timestamp: u64) {
        let old = self.global_level;
        self.global_level = level;
        self.change_log.push(LevelChangeRecord {
            channel: None,
            old_level: old,
            new_level: level,
            timestamp,
        });
    }

    /// Set a channel-specific override.
    pub fn set_channel(&mut self, channel: impl Into<String>, level: LogLevel, timestamp: u64) {
        let channel = channel.into();
        let old = self.effective_level(&channel);
        self.change_log.push(LevelChangeRecord {
            channel: Some(channel.clone()),
            old_level: old,
            new_level: level,
            timestamp,
        });
        self.channel_overrides.insert(channel, level);
    }

    /// Remove a channel override so it falls back to global.
    pub fn clear_channel(&mut self, channel: &str) {
        self.channel_overrides.remove(channel);
    }

    /// Effective level for a channel (override or global).
    pub fn effective_level(&self, channel: &str) -> LogLevel {
        self.channel_overrides
            .get(channel)
            .copied()
            .unwrap_or(self.global_level)
    }

    /// Whether a message at `level` on `channel` should be logged.
    pub fn should_log(&self, channel: &str, level: LogLevel) -> bool {
        if level == LogLevel::Off {
            return false;
        }
        let effective = self.effective_level(channel);
        if effective == LogLevel::Off {
            return false;
        }
        level >= effective
    }

    pub fn global_level(&self) -> LogLevel { self.global_level }
    pub fn overrides(&self) -> &HashMap<String, LogLevel> { &self.channel_overrides }
    pub fn change_count(&self) -> usize { self.change_log.len() }
    pub fn changes(&self) -> &[LevelChangeRecord] { &self.change_log }

    /// Reset all channel overrides.
    pub fn reset_all_overrides(&mut self) {
        self.channel_overrides.clear();
    }
}

// --- LogBufferRing: ring buffer for recent logs ---

pub struct LogBufferRing {
    entries: Vec<LogEntry>,
    capacity: usize,
}

impl LogBufferRing {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::with_capacity(capacity), capacity: capacity.max(1) }
    }

    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn drain(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut self.entries)
    }

    pub fn peek(&self) -> Option<&LogEntry> { self.entries.last() }
    pub fn oldest(&self) -> Option<&LogEntry> { self.entries.first() }
    pub fn newest(&self) -> Option<&LogEntry> { self.entries.last() }
    pub fn capacity(&self) -> usize { self.capacity }
    pub fn is_full(&self) -> bool { self.entries.len() >= self.capacity }
    pub fn clear(&mut self) { self.entries.clear(); }
    pub fn len(&self) -> usize { self.entries.len() }

    pub fn count_by_level(&self, level: LogLevel) -> usize {
        self.entries.iter().filter(|e| e.level == level).count()
    }
}

// --- LogFilterV2 ---

pub struct LogFilterV2 {
    min_level: Option<LogLevel>,
    module_pattern: Option<String>,
    message_pattern: Option<String>,
}

impl LogFilterV2 {
    pub fn new() -> Self {
        Self { min_level: None, module_pattern: None, message_pattern: None }
    }

    pub fn with_level(mut self, level: LogLevel) -> Self { self.min_level = Some(level); self }
    pub fn with_channel(mut self, pattern: &str) -> Self { self.module_pattern = Some(pattern.to_string()); self }
    pub fn with_message(mut self, pattern: &str) -> Self { self.message_pattern = Some(pattern.to_string()); self }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        if let Some(min) = self.min_level {
            if entry.level < min { return false; }
        }
        if let Some(ref mod_pat) = self.module_pattern {
            if !entry.channel.contains(mod_pat.as_str()) { return false; }
        }
        if let Some(ref msg_pat) = self.message_pattern {
            if !entry.message.contains(msg_pat.as_str()) { return false; }
        }
        true
    }

    pub fn parse_filter_string(s: &str) -> Self {
        let mut f = Self::new();
        for part in s.split_whitespace() {
            if let Some(lvl) = part.strip_prefix("level:") {
                match lvl {
                    "trace" => f.min_level = Some(LogLevel::Trace),
                    "debug" => f.min_level = Some(LogLevel::Debug),
                    "info" => f.min_level = Some(LogLevel::Info),
                    "warn" => f.min_level = Some(LogLevel::Warning),
                    "error" => f.min_level = Some(LogLevel::Error),
                    _ => {}
                }
            } else if let Some(m) = part.strip_prefix("module:") {
                f.module_pattern = Some(m.to_string());
            } else if let Some(m) = part.strip_prefix("msg:") {
                f.message_pattern = Some(m.to_string());
            }
        }
        f
    }
}

// --- LogFormatterV2 ---

pub struct LogFormatterV2;

impl LogFormatterV2 {
    pub fn compact(entry: &LogEntry) -> String {
        format!("[{}] {}: {}", Self::level_char(entry.level), entry.channel, entry.message)
    }

    pub fn json(entry: &LogEntry) -> String {
        format!(
            r#"{{"timestamp":{},"level":"{}","channel":"{}","message":"{}"}}"#,
            entry.timestamp, Self::level_str(entry.level), entry.channel, entry.message
        )
    }

    pub fn colored_indicator(level: LogLevel) -> &'static str {
        match level {
            LogLevel::Trace => "⚪",
            LogLevel::Debug => "🔵",
            LogLevel::Info => "🟢",
            LogLevel::Warning => "🟡",
            LogLevel::Error => "🔴",
            LogLevel::Off => " ",
        }
    }

    fn level_char(level: LogLevel) -> char {
        match level {
            LogLevel::Trace => 'T',
            LogLevel::Debug => 'D',
            LogLevel::Info => 'I',
            LogLevel::Warning => 'W',
            LogLevel::Error => 'E',
            LogLevel::Off => '-',
        }
    }

    fn level_str(level: LogLevel) -> &'static str {
        match level {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warning => "warn",
            LogLevel::Error => "error",
            LogLevel::Off => "off",
        }
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

    #[test]
    fn structured_log_entry_fields() {
        let entry = StructuredLogEntry::new(LogLevel::Info, "app", "request received")
            .with_field("method", "GET")
            .with_field("path", "/api/health");

        assert_eq!(entry.get_field("method"), Some("GET"));
        assert_eq!(entry.get_field("path"), Some("/api/health"));
        assert!(entry.get_field("missing").is_none());
    }

    #[test]
    fn structured_log_entry_to_json() {
        let entry = StructuredLogEntry::new(LogLevel::Warning, "db", "slow query")
            .with_field("duration_ms", "500");
        let json = entry.to_json();
        assert!(json.contains(r#""level":"warning""#));
        assert!(json.contains(r#""message":"slow query""#));
        assert!(json.contains(r#""duration_ms":"500""#));
    }

    #[test]
    fn structured_log_entry_to_log_entry() {
        let structured = StructuredLogEntry::new(LogLevel::Error, "ch", "fail")
            .with_field("code", "42");
        let plain = structured.to_log_entry();
        assert_eq!(plain.level, LogLevel::Error);
        assert_eq!(plain.message, "fail");
        assert!(plain.data.is_some());
        assert_eq!(plain.data.unwrap().get("code").unwrap(), "42");
    }

    #[test]
    fn log_exporter_text() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "app", "started"),
            LogEntry::new(LogLevel::Error, "app", "crashed"),
        ];
        let text = LogExporter::to_text(&entries);
        assert!(text.contains("[INFO] app: started"));
        assert!(text.contains("[ERROR] app: crashed"));
    }

    #[test]
    fn log_exporter_json() {
        let entries = vec![LogEntry::new(LogLevel::Debug, "test", "hello")];
        let json = LogExporter::to_json(&entries);
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains(r#""level":"debug""#));
    }

    #[test]
    fn log_exporter_csv() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "msg1"),
            LogEntry::new(LogLevel::Warning, "ch", "msg2"),
        ];
        let csv = LogExporter::to_csv(&entries);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "level,channel,message,timestamp");
        assert!(lines[1].starts_with("info,ch,msg1,"));
        assert!(lines[2].starts_with("warning,ch,msg2,"));
    }

    #[test]
    fn log_aggregator_by_channel() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "app", "hello"),
            LogEntry::new(LogLevel::Info, "db", "query"),
            LogEntry::new(LogLevel::Error, "app", "fail"),
        ];
        let agg = LogAggregator::group_by_channel(&entries);
        assert_eq!(agg.group_count(), 2);
        assert_eq!(agg.total_entries(), 3);
        assert_eq!(agg.get_group("app").unwrap().len(), 2);
        assert_eq!(agg.get_group("db").unwrap().len(), 1);
    }

    #[test]
    fn log_aggregator_by_level() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "a"),
            LogEntry::new(LogLevel::Info, "ch", "b"),
            LogEntry::new(LogLevel::Error, "ch", "c"),
        ];
        let agg = LogAggregator::group_by_level(&entries);
        assert_eq!(agg.get_group("info").unwrap().len(), 2);
        assert_eq!(agg.get_group("error").unwrap().len(), 1);
    }

    #[test]
    fn log_viewer_search() {
        let mut viewer = LogViewer::new("test");
        viewer.push(LogEntry::new(LogLevel::Info, "ch", "user logged in"));
        viewer.push(LogEntry::new(LogLevel::Info, "ch", "file saved"));
        viewer.push(LogEntry::new(LogLevel::Error, "ch", "user not found"));

        let results = viewer.search("user");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn count_at_level_basic() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "msg1"),
            LogEntry::new(LogLevel::Error, "ch", "msg2"),
            LogEntry::new(LogLevel::Info, "ch", "msg3"),
        ];
        assert_eq!(count_at_level(&entries, LogLevel::Info), 2);
        assert_eq!(count_at_level(&entries, LogLevel::Error), 1);
        assert_eq!(count_at_level(&entries, LogLevel::Warning), 0);
    }

    #[test]
    fn unique_channels_deduplicates() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "app", "msg"),
            LogEntry::new(LogLevel::Info, "db", "msg"),
            LogEntry::new(LogLevel::Info, "app", "msg2"),
        ];
        let channels = unique_channels(&entries);
        assert_eq!(channels, vec!["app", "db"]);
    }

    #[test]
    fn unique_channels_empty() {
        let entries: Vec<LogEntry> = vec![];
        assert!(unique_channels(&entries).is_empty());
    }

    #[test]
    fn level_summary_counts() {
        let entries = vec![
            LogEntry::new(LogLevel::Trace, "ch", "t"),
            LogEntry::new(LogLevel::Debug, "ch", "d"),
            LogEntry::new(LogLevel::Info, "ch", "i"),
            LogEntry::new(LogLevel::Warning, "ch", "w"),
            LogEntry::new(LogLevel::Error, "ch", "e"),
        ];
        let (t, d, i, w, e) = level_summary(&entries);
        assert_eq!((t, d, i, w, e), (1, 1, 1, 1, 1));
    }

    #[test]
    fn has_errors_true() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "ok"),
            LogEntry::new(LogLevel::Error, "ch", "bad"),
        ];
        assert!(has_errors(&entries));
    }

    #[test]
    fn has_errors_false() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "ok"),
        ];
        assert!(!has_errors(&entries));
    }

    #[test]
    fn has_message_containing_found() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "user logged in"),
            LogEntry::new(LogLevel::Info, "ch", "file saved"),
        ];
        assert!(has_message_containing(&entries, "logged"));
    }

    #[test]
    fn has_message_containing_not_found() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "hello"),
        ];
        assert!(!has_message_containing(&entries, "goodbye"));
    }

    #[test]
    fn group_by_channel_groups() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "app", "msg1"),
            LogEntry::new(LogLevel::Info, "db", "msg2"),
            LogEntry::new(LogLevel::Info, "app", "msg3"),
        ];
        let groups = group_by_channel(&entries);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["app"].len(), 2);
        assert_eq!(groups["db"].len(), 1);
    }

    #[test]
    fn sort_by_time_orders_correctly() {
        let mut entries = vec![
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "late".into(), timestamp: 200, source: None, data: None },
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "early".into(), timestamp: 100, source: None, data: None },
        ];
        sort_by_time(&mut entries);
        assert_eq!(entries[0].message, "early");
        assert_eq!(entries[1].message, "late");
    }

    #[test]
    fn most_recent_entry_finds_latest() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "old".into(), timestamp: 10, source: None, data: None },
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "new".into(), timestamp: 99, source: None, data: None },
        ];
        let recent = most_recent_entry(&entries).unwrap();
        assert_eq!(recent.message, "new");
    }

    #[test]
    fn most_recent_entry_empty() {
        let entries: Vec<LogEntry> = vec![];
        assert!(most_recent_entry(&entries).is_none());
    }

    // ===== New tests for added functionality =====

    #[test]
    fn log_rate_limiter_allows_first_and_suppresses_fast() {
        let limiter = LogRateLimiter::new(1000); // 1-second window
        assert!(limiter.allow("key1", 100));
        assert!(!limiter.allow("key1", 500)); // only 400ms later
        assert!(limiter.allow("key1", 1200)); // 1100ms after first
        assert_eq!(limiter.tracked_key_count(), 1);
    }

    #[test]
    fn log_rate_limiter_independent_keys() {
        let limiter = LogRateLimiter::new(500);
        assert!(limiter.allow("a", 0));
        assert!(limiter.allow("b", 0));
        assert!(!limiter.allow("a", 100));
        assert!(!limiter.allow("b", 100));
        assert_eq!(limiter.tracked_key_count(), 2);
        limiter.reset();
        assert_eq!(limiter.tracked_key_count(), 0);
        assert!(limiter.allow("a", 100)); // allowed again after reset
    }

    #[test]
    fn log_buffer_respects_capacity() {
        let mut buf = LogBuffer::new(3);
        for i in 0..5 {
            buf.push(LogEntry::new(LogLevel::Info, "ch", format!("m{i}")));
        }
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.capacity(), 3);
        assert_eq!(buf.total_pushed(), 5);
        assert_eq!(buf.evicted_count(), 2);
        assert_eq!(buf.oldest().unwrap().message, "m2");
        assert_eq!(buf.newest().unwrap().message, "m4");
    }

    #[test]
    fn log_buffer_drain_and_clear() {
        let mut buf = LogBuffer::new(10);
        buf.push(LogEntry::new(LogLevel::Info, "ch", "a"));
        buf.push(LogEntry::new(LogLevel::Info, "ch", "b"));
        let drained = buf.drain();
        assert_eq!(drained.len(), 2);
        assert!(buf.is_empty());

        buf.push(LogEntry::new(LogLevel::Info, "ch", "c"));
        buf.clear();
        assert!(buf.is_empty());
    }

    #[test]
    fn log_entry_builder_with_data_and_source() {
        let entry = LogEntry::new(LogLevel::Info, "app", "request")
            .with_source("http_handler")
            .with_data("method", "GET")
            .with_data("path", "/health");

        assert_eq!(entry.source.as_deref(), Some("http_handler"));
        assert_eq!(entry.get_data("method"), Some("GET"));
        assert_eq!(entry.get_data("path"), Some("/health"));
        assert!(entry.get_data("missing").is_none());
        assert_eq!(entry.data_keys(), vec!["method", "path"]);
    }

    #[test]
    fn log_entry_message_contains_ci() {
        let entry = LogEntry::new(LogLevel::Error, "ch", "Connection REFUSED");
        assert!(entry.message_contains_ci("connection"));
        assert!(entry.message_contains_ci("REFUSED"));
        assert!(!entry.message_contains_ci("timeout"));
    }

    #[test]
    fn compact_formatter_output() {
        let fmt = CompactFormatter;
        let entry = LogEntry::new(LogLevel::Warning, "net", "timeout");
        assert_eq!(fmt.format(&entry), "WARNING net timeout");
    }

    #[test]
    fn pretty_formatter_output() {
        let fmt = PrettyFormatter;
        let entry = LogEntry::new(LogLevel::Error, "db", "query failed")
            .with_source("pg_pool")
            .with_data("table", "users");
        let output = fmt.format(&entry);
        assert!(output.contains("--- ERROR ---"));
        assert!(output.contains("channel: db"));
        assert!(output.contains("message: query failed"));
        assert!(output.contains("source:  pg_pool"));
        assert!(output.contains("table: users"));
    }

    #[test]
    fn log_level_from_str_ci() {
        assert_eq!(LogLevel::from_str_ci("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str_ci("warn"), Some(LogLevel::Warning));
        assert_eq!(LogLevel::from_str_ci("Warning"), Some(LogLevel::Warning));
        assert_eq!(LogLevel::from_str_ci("ERROR"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str_ci("TRACE"), Some(LogLevel::Trace));
        assert_eq!(LogLevel::from_str_ci("bogus"), None);
    }

    #[test]
    fn log_level_loggable_excludes_off() {
        let levels = LogLevel::loggable();
        assert_eq!(levels.len(), 5);
        assert!(!levels.contains(&LogLevel::Off));
    }

    #[test]
    fn dedup_consecutive_entries() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "retry"),
            LogEntry::new(LogLevel::Info, "ch", "retry"),
            LogEntry::new(LogLevel::Info, "ch", "retry"),
            LogEntry::new(LogLevel::Error, "ch", "failed"),
            LogEntry::new(LogLevel::Info, "ch", "retry"),
        ];
        let deduped = dedup_consecutive(&entries);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].message, "retry");
        assert_eq!(deduped[1].message, "failed");
        assert_eq!(deduped[2].message, "retry");
    }

    #[test]
    fn entries_matching_any_patterns() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "user login"),
            LogEntry::new(LogLevel::Info, "ch", "file saved"),
            LogEntry::new(LogLevel::Error, "ch", "disk full"),
            LogEntry::new(LogLevel::Info, "ch", "user logout"),
        ];
        let found = entries_matching_any(&entries, &["user", "disk"]);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn partition_by_filter_splits() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "app", "ok"),
            LogEntry::new(LogLevel::Error, "app", "fail"),
            LogEntry::new(LogLevel::Warning, "db", "slow"),
        ];
        let filter = LogFilter::new().with_level(LogLevel::Warning);
        let (matched, rest) = partition_by_filter(&entries, &filter);
        assert_eq!(matched.len(), 2); // Error + Warning
        assert_eq!(rest.len(), 1);    // Info
    }

    #[test]
    fn channel_histogram_counts() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "app", "a"),
            LogEntry::new(LogLevel::Info, "db", "b"),
            LogEntry::new(LogLevel::Info, "app", "c"),
            LogEntry::new(LogLevel::Info, "app", "d"),
        ];
        let hist = channel_histogram(&entries);
        assert_eq!(hist["app"], 3);
        assert_eq!(hist["db"], 1);
    }

    #[test]
    fn earliest_entry_finds_oldest() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "b".into(), timestamp: 200, source: None, data: None },
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "a".into(), timestamp: 50, source: None, data: None },
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "c".into(), timestamp: 150, source: None, data: None },
        ];
        assert_eq!(earliest_entry(&entries).unwrap().message, "a");
    }

    #[test]
    fn time_span_returns_range() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "x".into(), timestamp: 100, source: None, data: None },
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "y".into(), timestamp: 500, source: None, data: None },
            LogEntry { level: LogLevel::Info, channel: "ch".into(), message: "z".into(), timestamp: 300, source: None, data: None },
        ];
        assert_eq!(time_span(&entries), Some((100, 500)));
        let empty: Vec<LogEntry> = vec![];
        assert_eq!(time_span(&empty), None);
    }

    #[test]
    fn all_data_keys_extracts_and_deduplicates() {
        let entries = vec![
            LogEntry::new(LogLevel::Info, "ch", "a")
                .with_data("method", "GET")
                .with_data("path", "/"),
            LogEntry::new(LogLevel::Info, "ch", "b")
                .with_data("method", "POST")
                .with_data("status", "200"),
            LogEntry::new(LogLevel::Info, "ch", "c"), // no data
        ];
        let keys = all_data_keys(&entries);
        assert_eq!(keys, vec!["method", "path", "status"]);
    }

    #[test]
    fn rotation_policy_new() {
        let p = RotationPolicy::new(1024, 5);
        assert_eq!(p.max_bytes, 1024);
        assert_eq!(p.max_files, 5);
        assert_eq!(p.suffix_style, RotationSuffixStyle::Numeric);
    }

    #[test]
    fn rotator_record_write_no_rotation() {
        let mut r = LogOutputRotator::new(RotationPolicy::new(100, 3));
        assert!(!r.record_write(50));
        assert_eq!(r.current_size(), 50);
    }

    #[test]
    fn rotator_needs_rotation() {
        let mut r = LogOutputRotator::new(RotationPolicy::new(100, 3));
        r.record_write(101);
        assert!(r.needs_rotation());
    }

    #[test]
    fn rotator_rotate() {
        let mut r = LogOutputRotator::new(RotationPolicy::new(100, 3));
        r.record_write(150);
        let name = r.rotate("app.log");
        assert_eq!(name, Some("app.log.1".to_string()));
        assert_eq!(r.current_size(), 0);
        assert_eq!(r.rotation_count(), 1);
    }

    #[test]
    fn rotator_rotate_not_needed() {
        let mut r = LogOutputRotator::new(RotationPolicy::new(100, 3));
        r.record_write(50);
        assert!(r.rotate("app.log").is_none());
    }

    #[test]
    fn rotator_timestamp_suffix() {
        let policy = RotationPolicy::new(10, 5).with_suffix_style(RotationSuffixStyle::Timestamp);
        let mut r = LogOutputRotator::new(policy);
        r.record_write(20);
        let name = r.rotate("out.log").unwrap();
        assert!(name.starts_with("out.log.ts"));
    }

    #[test]
    fn rotator_files_to_prune() {
        let mut r = LogOutputRotator::new(RotationPolicy::new(10, 2));
        for _ in 0..4 {
            r.record_write(20);
            r.rotate("x.log");
        }
        let prune = r.files_to_prune();
        assert_eq!(prune.len(), 2);
        assert_eq!(prune[0], "x.log.1");
    }

    #[test]
    fn rotator_history() {
        let mut r = LogOutputRotator::new(RotationPolicy::new(10, 5));
        r.record_write(15);
        r.rotate("a.log");
        assert_eq!(r.history().len(), 1);
        assert_eq!(r.history()[0].sequence, 1);
    }

    #[test]
    fn adjuster_global_level() {
        let mut adj = LogLevelRuntimeAdjuster::new(LogLevel::Info);
        assert_eq!(adj.global_level(), LogLevel::Info);
        adj.set_global(LogLevel::Debug, 100);
        assert_eq!(adj.global_level(), LogLevel::Debug);
        assert_eq!(adj.change_count(), 1);
    }

    #[test]
    fn adjuster_channel_override() {
        let mut adj = LogLevelRuntimeAdjuster::new(LogLevel::Warning);
        adj.set_channel("net", LogLevel::Trace, 1);
        assert_eq!(adj.effective_level("net"), LogLevel::Trace);
        assert_eq!(adj.effective_level("other"), LogLevel::Warning);
    }

    #[test]
    fn adjuster_should_log() {
        let mut adj = LogLevelRuntimeAdjuster::new(LogLevel::Warning);
        assert!(adj.should_log("ch", LogLevel::Error));
        assert!(!adj.should_log("ch", LogLevel::Debug));
        adj.set_channel("ch", LogLevel::Debug, 1);
        assert!(adj.should_log("ch", LogLevel::Debug));
    }

    #[test]
    fn adjuster_clear_channel() {
        let mut adj = LogLevelRuntimeAdjuster::new(LogLevel::Info);
        adj.set_channel("x", LogLevel::Trace, 1);
        adj.clear_channel("x");
        assert_eq!(adj.effective_level("x"), LogLevel::Info);
    }

    #[test]
    fn adjuster_reset_all() {
        let mut adj = LogLevelRuntimeAdjuster::new(LogLevel::Info);
        adj.set_channel("a", LogLevel::Trace, 1);
        adj.set_channel("b", LogLevel::Error, 2);
        adj.reset_all_overrides();
        assert!(adj.overrides().is_empty());
    }

    #[test]
    fn adjuster_off_never_logs() {
        let adj = LogLevelRuntimeAdjuster::new(LogLevel::Trace);
        assert!(!adj.should_log("ch", LogLevel::Off));
    }

    fn make_log_entry(level: LogLevel, channel: &str, message: &str) -> LogEntry {
        LogEntry { timestamp: 0, level, channel: channel.into(), message: message.into(), source: None, data: None }
    }

    #[test]
    fn log_buffer_ring_push_and_len() {
        let mut buf = LogBufferRing::new(5);
        buf.push(make_log_entry(LogLevel::Info, "mod", "msg"));
        assert_eq!(buf.len(), 1);
    }

    #[test]
    fn log_buffer_ring_capacity_eviction() {
        let mut buf = LogBufferRing::new(2);
        buf.push(make_log_entry(LogLevel::Info, "a", "first"));
        buf.push(make_log_entry(LogLevel::Info, "b", "second"));
        buf.push(make_log_entry(LogLevel::Info, "c", "third"));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.oldest().unwrap().channel, "b");
    }

    #[test]
    fn log_buffer_ring_drain() {
        let mut buf = LogBufferRing::new(10);
        buf.push(make_log_entry(LogLevel::Debug, "m", "msg"));
        let drained = buf.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn log_buffer_ring_count_by_level() {
        let mut buf = LogBufferRing::new(10);
        buf.push(make_log_entry(LogLevel::Info, "m", "a"));
        buf.push(make_log_entry(LogLevel::Error, "m", "b"));
        buf.push(make_log_entry(LogLevel::Info, "m", "c"));
        assert_eq!(buf.count_by_level(LogLevel::Info), 2);
        assert_eq!(buf.count_by_level(LogLevel::Error), 1);
    }

    #[test]
    fn log_filter_v2_by_level() {
        let f = LogFilterV2::new().with_level(LogLevel::Warning);
        assert!(!f.matches(&make_log_entry(LogLevel::Info, "m", "msg")));
        assert!(f.matches(&make_log_entry(LogLevel::Error, "m", "msg")));
    }

    #[test]
    fn log_filter_v2_by_channel() {
        let f = LogFilterV2::new().with_channel("auth");
        assert!(f.matches(&make_log_entry(LogLevel::Info, "auth::login", "ok")));
        assert!(!f.matches(&make_log_entry(LogLevel::Info, "db", "ok")));
    }

    #[test]
    fn log_filter_v2_by_message() {
        let f = LogFilterV2::new().with_message("fail");
        assert!(f.matches(&make_log_entry(LogLevel::Error, "m", "connection failed")));
        assert!(!f.matches(&make_log_entry(LogLevel::Error, "m", "success")));
    }

    #[test]
    fn log_filter_v2_parse() {
        let f = LogFilterV2::parse_filter_string("level:error module:auth");
        assert_eq!(f.min_level, Some(LogLevel::Error));
        assert_eq!(f.module_pattern, Some("auth".into()));
    }

    #[test]
    fn log_formatter_v2_compact() {
        let e = make_log_entry(LogLevel::Info, "app", "started");
        let s = LogFormatterV2::compact(&e);
        assert!(s.contains("[I]"));
        assert!(s.contains("app"));
    }

    #[test]
    fn log_formatter_v2_json() {
        let e = make_log_entry(LogLevel::Error, "srv", "crash");
        let s = LogFormatterV2::json(&e);
        assert!(s.contains(r#""level":"error""#));
        assert!(s.contains(r#""channel":"srv""#));
    }

    #[test]
    fn log_formatter_v2_colored_indicator() {
        assert_eq!(LogFormatterV2::colored_indicator(LogLevel::Error), "🔴");
        assert_eq!(LogFormatterV2::colored_indicator(LogLevel::Info), "🟢");
    }

    #[test]
    fn log_buffer_ring_is_full() {
        let mut buf = LogBufferRing::new(1);
        assert!(!buf.is_full());
        buf.push(make_log_entry(LogLevel::Trace, "m", "x"));
        assert!(buf.is_full());
    }

}
