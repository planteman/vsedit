//! Workbench logging.

use std::fmt;

/// Severity level for log entries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warning => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Errors that may occur in the logging service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogError {
    /// The log service has reached its maximum capacity.
    ServiceFull,
    /// An invalid log level was specified.
    InvalidLevel(String),
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogError::ServiceFull => write!(f, "log service is at maximum capacity"),
            LogError::InvalidLevel(s) => write!(f, "invalid log level: {s}"),
        }
    }
}

/// A single log entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
    pub timestamp: u64,
}

impl LogEntry {
    /// Returns `true` if this entry is Error or Critical.
    pub fn is_error(&self) -> bool {
        self.level >= LogLevel::Error
    }

    /// Returns `true` if this entry is Warning.
    pub fn is_warning(&self) -> bool {
        self.level == LogLevel::Warning
    }
}

impl fmt::Display for LogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            Some(src) => write!(f, "[{}] [{}] {}", self.level, src, self.message),
            None => write!(f, "[{}] {}", self.level, self.message),
        }
    }
}

/// Service for log workbench functionality.
#[derive(Debug)]
pub struct LogService {
    pub entries: Vec<LogEntry>,
    pub min_level: LogLevel,
    pub max_entries: Option<usize>,
}

impl LogService {
    pub fn new(min_level: LogLevel) -> Self {
        Self {
            entries: Vec::new(),
            min_level,
            max_entries: None,
        }
    }

    /// Create a service with a maximum entry cap.
    pub fn with_max_entries(min_level: LogLevel, max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            min_level,
            max_entries: Some(max_entries),
        }
    }

    pub fn log(&mut self, level: LogLevel, message: impl Into<String>, source: Option<String>) {
        if level >= self.min_level {
            // Evict oldest entry when at capacity.
            if let Some(max) = self.max_entries {
                if self.entries.len() >= max {
                    self.entries.remove(0);
                }
            }
            self.entries.push(LogEntry {
                level,
                message: message.into(),
                source,
                timestamp: 0,
            });
        }
    }

    pub fn trace(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Trace, message, None);
    }

    pub fn debug(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message, None);
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Info, message, None);
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Warning, message, None);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Error, message, None);
    }

    /// Log a message with an explicit source.
    pub fn log_with_source(
        &mut self,
        level: LogLevel,
        message: impl Into<String>,
        source: impl Into<String>,
    ) {
        self.log(level, message, Some(source.into()));
    }

    pub fn get_entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn get_entries_by_level(&self, level: LogLevel) -> Vec<&LogEntry> {
        self.entries.iter().filter(|e| e.level == level).collect()
    }

    /// Return entries at or above the given level.
    pub fn get_entries_above_level(&self, level: LogLevel) -> Vec<&LogEntry> {
        self.entries.iter().filter(|e| e.level >= level).collect()
    }

    /// Search entries whose message contains the given substring.
    pub fn search(&self, substring: &str) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.message.contains(substring))
            .collect()
    }

    /// Return the last `n` entries (or fewer if the log is shorter).
    pub fn last_n(&self, n: usize) -> &[LogEntry] {
        let len = self.entries.len();
        if n >= len {
            &self.entries
        } else {
            &self.entries[len - n..]
        }
    }

    /// Returns `true` if any entry is Error or Critical.
    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|e| e.is_error())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for LogService {
    fn default() -> Self {
        Self::new(LogLevel::Info)
    }
}

/// Filter for selecting log entries by various criteria.
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    level: Option<LogLevel>,
    source: Option<String>,
    message_contains: Option<String>,
}

impl LogFilter {
    /// Create a new empty filter that matches all entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Only keep entries with this exact level.
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = Some(level);
        self
    }

    /// Only keep entries whose source matches exactly.
    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    /// Only keep entries whose message contains the substring.
    pub fn with_message_contains(mut self, substring: String) -> Self {
        self.message_contains = Some(substring);
        self
    }

    /// Apply all configured filter criteria and return matching entries.
    pub fn apply<'a>(&self, entries: &'a [LogEntry]) -> Vec<&'a LogEntry> {
        entries
            .iter()
            .filter(|e| {
                if let Some(ref lvl) = self.level {
                    if e.level != *lvl {
                        return false;
                    }
                }
                if let Some(ref src) = self.source {
                    match &e.source {
                        Some(s) if s == src => {}
                        _ => return false,
                    }
                }
                if let Some(ref sub) = self.message_contains {
                    if !e.message.contains(sub.as_str()) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

/// Aggregate statistics about a collection of log entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogStats {
    pub total: usize,
    pub trace_count: usize,
    pub debug_count: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub critical_count: usize,
    pub unique_sources: usize,
}

impl LogStats {
    /// Compute statistics from a slice of log entries.
    pub fn from_entries(entries: &[LogEntry]) -> Self {
        let mut trace_count = 0;
        let mut debug_count = 0;
        let mut info_count = 0;
        let mut warning_count = 0;
        let mut error_count = 0;
        let mut critical_count = 0;
        let mut sources = std::collections::HashSet::new();

        for entry in entries {
            match entry.level {
                LogLevel::Trace => trace_count += 1,
                LogLevel::Debug => debug_count += 1,
                LogLevel::Info => info_count += 1,
                LogLevel::Warning => warning_count += 1,
                LogLevel::Error => error_count += 1,
                LogLevel::Critical => critical_count += 1,
            }
            if let Some(ref src) = entry.source {
                sources.insert(src.clone());
            }
        }

        Self {
            total: entries.len(),
            trace_count,
            debug_count,
            info_count,
            warning_count,
            error_count,
            critical_count,
            unique_sources: sources.len(),
        }
    }
}

/// Utilities for formatting log entries in various styles.
pub struct LogFormatter;

impl LogFormatter {
    /// Compact format: `[LEVEL] msg`
    pub fn format_compact(entry: &LogEntry) -> String {
        format!("[{}] {}", entry.level, entry.message)
    }

    /// Detailed format: `[LEVEL] [source|unknown] (ts) msg`
    pub fn format_detailed(entry: &LogEntry) -> String {
        let src = entry.source.as_deref().unwrap_or("unknown");
        format!(
            "[{}] [{}] ({}) {}",
            entry.level, src, entry.timestamp, entry.message
        )
    }

    /// JSON format: `{"level":"X","message":"Y","source":"Z","timestamp":N}`
    pub fn format_json(entry: &LogEntry) -> String {
        let src = entry.source.as_deref().unwrap_or("");
        format!(
            r#"{{"level":"{}","message":"{}","source":"{}","timestamp":{}}}"#,
            entry.level, entry.message, src, entry.timestamp
        )
    }
}

impl LogService {
    /// Return entries whose source matches exactly.
    pub fn get_entries_by_source(&self, source: &str) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.source.as_deref() == Some(source))
            .collect()
    }

    /// Return a sorted, deduplicated list of all sources present in the log.
    pub fn unique_sources(&self) -> Vec<String> {
        let mut sources: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| e.source.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        sources.sort();
        sources
    }

    /// Count entries at Error or Critical level.
    pub fn error_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_error()).count()
    }

    /// Count entries at Warning level.
    pub fn warning_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_warning()).count()
    }

    /// Compute aggregate statistics for all current entries.
    pub fn stats(&self) -> LogStats {
        LogStats::from_entries(&self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_at_or_above_min_level() {
        let mut svc = LogService::new(LogLevel::Info);
        svc.debug("should be filtered");
        svc.info("visible");
        svc.error("also visible");
        assert_eq!(svc.entry_count(), 2);
        assert_eq!(svc.get_entries()[0].message, "visible");
        assert_eq!(svc.get_entries()[1].message, "also visible");
    }

    #[test]
    fn get_entries_by_level_filters_correctly() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("info1");
        svc.warn("warn1");
        svc.info("info2");
        let infos = svc.get_entries_by_level(LogLevel::Info);
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].message, "info1");
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.trace("a");
        svc.debug("b");
        assert_eq!(svc.entry_count(), 2);
        svc.clear();
        assert_eq!(svc.entry_count(), 0);
        assert!(svc.get_entries().is_empty());
    }

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warning.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
        assert_eq!(LogLevel::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn log_error_display() {
        let full = LogError::ServiceFull;
        assert_eq!(full.to_string(), "log service is at maximum capacity");
        let invalid = LogError::InvalidLevel("foo".into());
        assert_eq!(invalid.to_string(), "invalid log level: foo");
    }

    #[test]
    fn log_entry_display_with_and_without_source() {
        let entry_no_src = LogEntry {
            level: LogLevel::Info,
            message: "hello".into(),
            source: None,
            timestamp: 0,
        };
        assert_eq!(entry_no_src.to_string(), "[INFO] hello");

        let entry_src = LogEntry {
            level: LogLevel::Error,
            message: "oops".into(),
            source: Some("parser".into()),
            timestamp: 0,
        };
        assert_eq!(entry_src.to_string(), "[ERROR] [parser] oops");
    }

    #[test]
    fn log_entry_is_error_and_is_warning() {
        let warning = LogEntry {
            level: LogLevel::Warning,
            message: "w".into(),
            source: None,
            timestamp: 0,
        };
        assert!(warning.is_warning());
        assert!(!warning.is_error());

        let error = LogEntry {
            level: LogLevel::Error,
            message: "e".into(),
            source: None,
            timestamp: 0,
        };
        assert!(error.is_error());
        assert!(!error.is_warning());

        let critical = LogEntry {
            level: LogLevel::Critical,
            message: "c".into(),
            source: None,
            timestamp: 0,
        };
        assert!(critical.is_error());
    }

    #[test]
    fn get_entries_above_level() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.trace("t");
        svc.info("i");
        svc.warn("w");
        svc.error("e");
        let above_warn = svc.get_entries_above_level(LogLevel::Warning);
        assert_eq!(above_warn.len(), 2);
        assert_eq!(above_warn[0].message, "w");
        assert_eq!(above_warn[1].message, "e");
    }

    #[test]
    fn search_entries() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("connection opened");
        svc.info("data received");
        svc.warn("connection lost");
        let results = svc.search("connection");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].message, "connection opened");
        assert_eq!(results[1].message, "connection lost");
        assert!(svc.search("missing").is_empty());
    }

    #[test]
    fn last_n_entries() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("a");
        svc.info("b");
        svc.info("c");
        svc.info("d");
        let last2 = svc.last_n(2);
        assert_eq!(last2.len(), 2);
        assert_eq!(last2[0].message, "c");
        assert_eq!(last2[1].message, "d");

        // Requesting more than available returns all.
        let all = svc.last_n(100);
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn log_with_source() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.log_with_source(LogLevel::Info, "hello", "mymod");
        assert_eq!(svc.entry_count(), 1);
        let entry = &svc.get_entries()[0];
        assert_eq!(entry.source.as_deref(), Some("mymod"));
        assert_eq!(entry.message, "hello");
    }

    #[test]
    fn has_errors_detects_error_and_critical() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("ok");
        svc.warn("hmm");
        assert!(!svc.has_errors());
        svc.error("bad");
        assert!(svc.has_errors());

        let mut svc2 = LogService::new(LogLevel::Trace);
        svc2.log(LogLevel::Critical, "fatal", None);
        assert!(svc2.has_errors());
    }

    #[test]
    fn max_entries_evicts_oldest() {
        let mut svc = LogService::with_max_entries(LogLevel::Trace, 3);
        svc.info("a");
        svc.info("b");
        svc.info("c");
        assert_eq!(svc.entry_count(), 3);
        svc.info("d");
        assert_eq!(svc.entry_count(), 3);
        assert_eq!(svc.get_entries()[0].message, "b");
        assert_eq!(svc.get_entries()[2].message, "d");
    }

    #[test]
    fn default_service_has_info_min_level() {
        let svc = LogService::default();
        assert_eq!(svc.min_level, LogLevel::Info);
        assert!(svc.max_entries.is_none());
        assert_eq!(svc.entry_count(), 0);
    }

    // --- new tests ---

    fn sample_entries() -> Vec<LogEntry> {
        vec![
            LogEntry { level: LogLevel::Info, message: "started".into(), source: Some("app".into()), timestamp: 1 },
            LogEntry { level: LogLevel::Warning, message: "disk low".into(), source: Some("storage".into()), timestamp: 2 },
            LogEntry { level: LogLevel::Error, message: "connection failed".into(), source: Some("network".into()), timestamp: 3 },
            LogEntry { level: LogLevel::Info, message: "connected".into(), source: Some("network".into()), timestamp: 4 },
            LogEntry { level: LogLevel::Debug, message: "tick".into(), source: None, timestamp: 5 },
            LogEntry { level: LogLevel::Critical, message: "out of memory".into(), source: Some("app".into()), timestamp: 6 },
        ]
    }

    #[test]
    fn test_log_filter_by_level() {
        let entries = sample_entries();
        let filtered = LogFilter::new().with_level(LogLevel::Info).apply(&entries);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].message, "started");
        assert_eq!(filtered[1].message, "connected");
    }

    #[test]
    fn test_log_filter_by_source() {
        let entries = sample_entries();
        let filtered = LogFilter::new().with_source("network".into()).apply(&entries);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].message, "connection failed");
        assert_eq!(filtered[1].message, "connected");
    }

    #[test]
    fn test_log_filter_by_message() {
        let entries = sample_entries();
        let filtered = LogFilter::new()
            .with_message_contains("connect".into())
            .apply(&entries);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].message, "connection failed");
        assert_eq!(filtered[1].message, "connected");
    }

    #[test]
    fn test_log_filter_combined() {
        let entries = sample_entries();
        let filtered = LogFilter::new()
            .with_level(LogLevel::Info)
            .with_source("network".into())
            .apply(&entries);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "connected");
    }

    #[test]
    fn test_log_stats() {
        let entries = sample_entries();
        let stats = LogStats::from_entries(&entries);
        assert_eq!(stats.total, 6);
        assert_eq!(stats.trace_count, 0);
        assert_eq!(stats.debug_count, 1);
        assert_eq!(stats.info_count, 2);
        assert_eq!(stats.warning_count, 1);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.critical_count, 1);
        assert_eq!(stats.unique_sources, 3);
    }

    #[test]
    fn test_format_compact() {
        let entry = LogEntry {
            level: LogLevel::Warning,
            message: "low disk".into(),
            source: Some("storage".into()),
            timestamp: 42,
        };
        assert_eq!(LogFormatter::format_compact(&entry), "[WARN] low disk");
    }

    #[test]
    fn test_format_detailed() {
        let entry = LogEntry {
            level: LogLevel::Error,
            message: "fail".into(),
            source: Some("net".into()),
            timestamp: 99,
        };
        assert_eq!(
            LogFormatter::format_detailed(&entry),
            "[ERROR] [net] (99) fail"
        );

        let entry_no_src = LogEntry {
            level: LogLevel::Info,
            message: "hi".into(),
            source: None,
            timestamp: 10,
        };
        assert_eq!(
            LogFormatter::format_detailed(&entry_no_src),
            "[INFO] [unknown] (10) hi"
        );
    }

    #[test]
    fn test_format_json() {
        let entry = LogEntry {
            level: LogLevel::Info,
            message: "boot".into(),
            source: Some("sys".into()),
            timestamp: 7,
        };
        assert_eq!(
            LogFormatter::format_json(&entry),
            r#"{"level":"INFO","message":"boot","source":"sys","timestamp":7}"#
        );

        let entry_no_src = LogEntry {
            level: LogLevel::Debug,
            message: "x".into(),
            source: None,
            timestamp: 0,
        };
        assert_eq!(
            LogFormatter::format_json(&entry_no_src),
            r#"{"level":"DEBUG","message":"x","source":"","timestamp":0}"#
        );
    }

    #[test]
    fn test_get_entries_by_source() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.log_with_source(LogLevel::Info, "a", "mod1");
        svc.log_with_source(LogLevel::Warning, "b", "mod2");
        svc.log_with_source(LogLevel::Error, "c", "mod1");
        svc.info("no source");
        let by_mod1 = svc.get_entries_by_source("mod1");
        assert_eq!(by_mod1.len(), 2);
        assert_eq!(by_mod1[0].message, "a");
        assert_eq!(by_mod1[1].message, "c");
        assert!(svc.get_entries_by_source("missing").is_empty());
    }

    #[test]
    fn test_unique_sources() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.log_with_source(LogLevel::Info, "a", "beta");
        svc.log_with_source(LogLevel::Info, "b", "alpha");
        svc.log_with_source(LogLevel::Info, "c", "beta");
        svc.info("no source");
        let sources = svc.unique_sources();
        assert_eq!(sources, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn test_error_count() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("ok");
        svc.error("e1");
        svc.warn("w");
        svc.error("e2");
        svc.log(LogLevel::Critical, "c1", None);
        assert_eq!(svc.error_count(), 3);
    }

    #[test]
    fn test_warning_count() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("ok");
        svc.warn("w1");
        svc.warn("w2");
        svc.error("e");
        assert_eq!(svc.warning_count(), 2);
    }

    #[test]
    fn test_service_stats() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.trace("t");
        svc.debug("d");
        svc.info("i");
        svc.warn("w");
        svc.error("e");
        svc.log(LogLevel::Critical, "c", None);
        svc.log_with_source(LogLevel::Info, "i2", "src1");
        svc.log_with_source(LogLevel::Info, "i3", "src2");
        let stats = svc.stats();
        assert_eq!(stats.total, 8);
        assert_eq!(stats.trace_count, 1);
        assert_eq!(stats.debug_count, 1);
        assert_eq!(stats.info_count, 3);
        assert_eq!(stats.warning_count, 1);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.critical_count, 1);
        assert_eq!(stats.unique_sources, 2);
    }
}
