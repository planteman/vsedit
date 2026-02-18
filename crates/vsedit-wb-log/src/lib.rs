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

// ── Log Rotation Management ──

/// Configuration for log rotation based on entry count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationConfig {
    /// Maximum number of entries before rotation.
    pub max_entries: usize,
    /// Number of entries to keep after rotation (most recent).
    pub keep_entries: usize,
}

impl RotationConfig {
    pub fn new(max_entries: usize, keep_entries: usize) -> Self {
        Self {
            max_entries,
            keep_entries: keep_entries.min(max_entries),
        }
    }
}

/// Apply rotation to a log service: if entries exceed `config.max_entries`,
/// keep only the most recent `config.keep_entries`.
pub fn rotate_log(service: &mut LogService, config: &RotationConfig) -> usize {
    let len = service.entries.len();
    if len <= config.max_entries {
        return 0;
    }
    let remove_count = len - config.keep_entries;
    service.entries.drain(..remove_count);
    remove_count
}

// ── Log Level Filtering Utilities ──

/// Parse a log level from a case-insensitive string.
pub fn parse_log_level(s: &str) -> Result<LogLevel, LogError> {
    match s.to_uppercase().as_str() {
        "TRACE" => Ok(LogLevel::Trace),
        "DEBUG" => Ok(LogLevel::Debug),
        "INFO" => Ok(LogLevel::Info),
        "WARN" | "WARNING" => Ok(LogLevel::Warning),
        "ERROR" => Ok(LogLevel::Error),
        "CRITICAL" | "FATAL" => Ok(LogLevel::Critical),
        _ => Err(LogError::InvalidLevel(s.to_string())),
    }
}

// ── Structured Log Entry Formatting ──

/// Format a slice of log entries as a newline-delimited JSON string.
pub fn format_entries_ndjson(entries: &[LogEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&LogFormatter::format_json(entry));
        out.push('\n');
    }
    out
}

// ── Log Search / Grep ──

/// Search entries whose message matches a simple case-insensitive substring.
pub fn grep_entries<'a>(entries: &'a [LogEntry], pattern: &str) -> Vec<&'a LogEntry> {
    let lower = pattern.to_lowercase();
    entries
        .iter()
        .filter(|e| e.message.to_lowercase().contains(&lower))
        .collect()
}

/// Search entries whose message or source matches a substring (case-insensitive).
pub fn grep_entries_all_fields<'a>(entries: &'a [LogEntry], pattern: &str) -> Vec<&'a LogEntry> {
    let lower = pattern.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            e.message.to_lowercase().contains(&lower)
                || e.source
                    .as_ref()
                    .map_or(false, |s| s.to_lowercase().contains(&lower))
        })
        .collect()
}

/// Filter criteria for selecting log entries by level.
#[derive(Debug, Clone, PartialEq)]
pub enum LogFilterLevel {
    /// Match all entries regardless of level.
    All,
    /// Match entries at exactly this level.
    AtLevel(LogLevel),
    /// Match entries strictly above this level.
    AboveLevel(LogLevel),
    /// Match entries strictly below this level.
    BelowLevel(LogLevel),
    /// Match entries within an inclusive range.
    Range { min: LogLevel, max: LogLevel },
}

impl fmt::Display for LogFilterLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogFilterLevel::All => write!(f, "All"),
            LogFilterLevel::AtLevel(l) => write!(f, "AtLevel({l})"),
            LogFilterLevel::AboveLevel(l) => write!(f, "AboveLevel({l})"),
            LogFilterLevel::BelowLevel(l) => write!(f, "BelowLevel({l})"),
            LogFilterLevel::Range { min, max } => write!(f, "Range({min}..{max})"),
        }
    }
}

/// Filter `entries` according to a [`LogFilterLevel`] criterion.
pub fn apply_filter<'a>(entries: &'a [LogEntry], filter: &LogFilterLevel) -> Vec<&'a LogEntry> {
    entries
        .iter()
        .filter(|e| match filter {
            LogFilterLevel::All => true,
            LogFilterLevel::AtLevel(l) => e.level == *l,
            LogFilterLevel::AboveLevel(l) => e.level > *l,
            LogFilterLevel::BelowLevel(l) => e.level < *l,
            LogFilterLevel::Range { min, max } => e.level >= *min && e.level <= *max,
        })
        .collect()
}

/// Which fields [`log_search`] should inspect.
#[derive(Debug, Clone, PartialEq)]
pub enum LogSearchFields {
    /// Search only the message field.
    Message,
    /// Search only the source field.
    Source,
    /// Search both message and source fields.
    All,
}

/// Search `entries` for `query` in the specified `fields`.
pub fn log_search<'a>(
    entries: &'a [LogEntry],
    query: &str,
    case_sensitive: bool,
    fields: LogSearchFields,
) -> Vec<&'a LogEntry> {
    let matches = |haystack: &str| -> bool {
        if case_sensitive {
            haystack.contains(query)
        } else {
            haystack.to_lowercase().contains(&query.to_lowercase())
        }
    };

    entries
        .iter()
        .filter(|e| match &fields {
            LogSearchFields::Message => matches(&e.message),
            LogSearchFields::Source => e.source.as_ref().map_or(false, |s| matches(s)),
            LogSearchFields::All => {
                matches(&e.message)
                    || e.source.as_ref().map_or(false, |s| matches(s))
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// log_rotate — manage log sizes
// ---------------------------------------------------------------------------

/// Configuration for log rotation.
#[derive(Debug, Clone)]
pub struct LogRotateConfig {
    /// Maximum number of entries before rotation.
    pub max_entries: usize,
    /// Number of entries to keep after rotation (most recent).
    pub keep_entries: usize,
    /// Minimum log level to retain during rotation (entries below are always dropped).
    pub min_retain_level: LogLevel,
}

impl Default for LogRotateConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            keep_entries: 5_000,
            min_retain_level: LogLevel::Trace,
        }
    }
}

/// Result of a log rotation operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRotateResult {
    pub entries_before: usize,
    pub entries_after: usize,
    pub entries_dropped: usize,
}

impl LogRotateResult {
    pub fn did_rotate(&self) -> bool {
        self.entries_dropped > 0
    }
}

impl fmt::Display for LogRotateResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rotated: {} -> {} (dropped {})",
            self.entries_before, self.entries_after, self.entries_dropped,
        )
    }
}

/// Rotate log entries in-place: if entries exceed `config.max_entries`,
/// keep only the most recent `config.keep_entries` that meet the minimum level.
pub fn log_rotate(entries: &mut Vec<LogEntry>, config: &LogRotateConfig) -> LogRotateResult {
    let entries_before = entries.len();
    if entries_before <= config.max_entries {
        return LogRotateResult {
            entries_before,
            entries_after: entries_before,
            entries_dropped: 0,
        };
    }
    // Filter by minimum level, then keep the tail.
    let filtered: Vec<LogEntry> = entries
        .drain(..)
        .filter(|e| e.level >= config.min_retain_level)
        .collect();
    let start = filtered.len().saturating_sub(config.keep_entries);
    *entries = filtered.into_iter().skip(start).collect();
    let entries_after = entries.len();
    LogRotateResult {
        entries_before,
        entries_after,
        entries_dropped: entries_before - entries_after,
    }
}

/// Check whether a log rotation is needed given current entry count and config.
pub fn log_needs_rotation(entry_count: usize, config: &LogRotateConfig) -> bool {
    entry_count > config.max_entries
}

// ---------------------------------------------------------------------------
// Additional helpers
// ---------------------------------------------------------------------------

impl LogService {
    /// Returns the last error-level entry, if any.
    pub fn last_error(&self) -> Option<&LogEntry> {
        self.entries.iter().rev().find(|e| e.is_error())
    }

    /// Returns the last warning-level entry, if any.
    pub fn last_warning(&self) -> Option<&LogEntry> {
        self.entries.iter().rev().find(|e| e.is_warning())
    }

    /// Returns a human-readable summary of the log state.
    pub fn summary(&self) -> String {
        let total = self.entries.len();
        let errors = self.error_count();
        let warnings = self.warning_count();
        let sources = self.unique_sources();
        format!(
            "{total} entries ({errors} errors, {warnings} warnings, {} sources)",
            sources.len()
        )
    }
}

impl fmt::Display for LogService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LogService({} entries, min_level={}, max={})",
            self.entries.len(),
            self.min_level,
            self.max_entries
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unlimited".to_string()),
        )
    }
}

impl LogLevel {
    /// Returns `true` if `self` is at least as severe as `other`.
    pub fn is_at_least(&self, other: &LogLevel) -> bool {
        self >= other
    }

    /// Returns a numeric rank (0=Trace .. 5=Critical).
    pub fn rank(&self) -> u8 {
        match self {
            Self::Trace => 0,
            Self::Debug => 1,
            Self::Info => 2,
            Self::Warning => 3,
            Self::Error => 4,
            Self::Critical => 5,
        }
    }
}

impl LogEntry {
    /// Returns `true` if this entry has a source set.
    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }

    /// Returns the length of the message string.
    pub fn message_length(&self) -> usize {
        self.message.len()
    }
}

impl LogFilter {
    /// Returns `true` if no filter constraints have been set.
    pub fn is_empty(&self) -> bool {
        self.level.is_none() && self.source.is_none() && self.message_contains.is_none()
    }
}

// ---------------------------------------------------------------------------
// Log level filtering with timestamp range
// ---------------------------------------------------------------------------

/// Filter log entries by a combination of level, source, and timestamp range.
pub fn filter_entries<'a>(
    entries: &'a [LogEntry],
    min_level: Option<&LogLevel>,
    source: Option<&str>,
    after_timestamp: Option<u64>,
    before_timestamp: Option<u64>,
) -> Vec<&'a LogEntry> {
    entries
        .iter()
        .filter(|e| {
            if let Some(ml) = min_level {
                if e.level < *ml {
                    return false;
                }
            }
            if let Some(src) = source {
                if e.source.as_deref() != Some(src) {
                    return false;
                }
            }
            if let Some(after) = after_timestamp {
                if e.timestamp < after {
                    return false;
                }
            }
            if let Some(before) = before_timestamp {
                if e.timestamp > before {
                    return false;
                }
            }
            true
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Log message formatting templates
// ---------------------------------------------------------------------------

/// Format a log entry with a custom template.
///
/// Supported placeholders: `{level}`, `{message}`, `{source}`, `{timestamp}`.
pub fn format_with_template(entry: &LogEntry, template: &str) -> String {
    let src = entry.source.as_deref().unwrap_or("");
    template
        .replace("{level}", &entry.level.to_string())
        .replace("{message}", &entry.message)
        .replace("{source}", src)
        .replace("{timestamp}", &entry.timestamp.to_string())
}

// ---------------------------------------------------------------------------
// Log rotation tracking
// ---------------------------------------------------------------------------

/// Tracks the history of log rotations.
#[derive(Debug, Clone, Default)]
pub struct RotationTracker {
    rotations: Vec<RotationRecord>,
}

/// Record of a single log rotation event.
#[derive(Debug, Clone)]
pub struct RotationRecord {
    pub timestamp: u64,
    pub entries_dropped: usize,
    pub entries_retained: usize,
}

impl RotationTracker {
    pub fn new() -> Self {
        Self { rotations: Vec::new() }
    }

    /// Record a rotation event.
    pub fn record(&mut self, timestamp: u64, entries_dropped: usize, entries_retained: usize) {
        self.rotations.push(RotationRecord {
            timestamp,
            entries_dropped,
            entries_retained,
        });
    }

    /// Total number of entries dropped across all rotations.
    pub fn total_dropped(&self) -> usize {
        self.rotations.iter().map(|r| r.entries_dropped).sum()
    }

    /// Number of rotations that have occurred.
    pub fn rotation_count(&self) -> usize {
        self.rotations.len()
    }

    /// Most recent rotation, if any.
    pub fn last_rotation(&self) -> Option<&RotationRecord> {
        self.rotations.last()
    }
}

// ---------------------------------------------------------------------------
// Structured log field extraction
// ---------------------------------------------------------------------------

/// Extract key=value fields from a structured log message.
///
/// Scans for patterns like `key=value` or `key="quoted value"` in the message.
pub fn extract_fields(message: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let chars: Vec<char> = message.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // look for key=
        if let Some(eq) = chars[i..].iter().position(|&c| c == '=') {
            let key_start = {
                let mut s = i + eq;
                while s > i && !chars[s - 1].is_whitespace() {
                    s -= 1;
                }
                s
            };
            let key: String = chars[key_start..i + eq].iter().collect();
            if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                i = i + eq + 1;
                continue;
            }
            let val_start = i + eq + 1;
            if val_start >= chars.len() {
                break;
            }
            let value;
            if chars[val_start] == '"' {
                // quoted value
                let end = chars[val_start + 1..]
                    .iter()
                    .position(|&c| c == '"')
                    .map(|p| val_start + 1 + p)
                    .unwrap_or(chars.len());
                value = chars[val_start + 1..end].iter().collect();
                i = end + 1;
            } else {
                let end = chars[val_start..]
                    .iter()
                    .position(|&c| c.is_whitespace())
                    .map(|p| val_start + p)
                    .unwrap_or(chars.len());
                value = chars[val_start..end].iter().collect();
                i = end;
            }
            fields.push((key, value));
        } else {
            break;
        }
    }
    fields
}

impl LogService {
    /// Return the most recent entry, if any.
    pub fn last_entry(&self) -> Option<&LogEntry> {
        self.entries.last()
    }

    /// Return entries whose message starts with the given prefix.
    pub fn search_prefix(&self, prefix: &str) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.message.starts_with(prefix))
            .collect()
    }

    /// Return a breakdown of entry counts by level.
    pub fn level_counts(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for entry in &self.entries {
            *map.entry(format!("{}", entry.level)).or_insert(0) += 1;
        }
        map
    }

    /// Return the average message length across all entries.
    pub fn average_message_length(&self) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        let total: usize = self.entries.iter().map(|e| e.message.len()).sum();
        total / self.entries.len()
    }

    /// Return the longest message entry, if any.
    pub fn longest_message(&self) -> Option<&LogEntry> {
        self.entries.iter().max_by_key(|e| e.message.len())
    }
}

impl LogFilter {
    /// Return true if this filter has no criteria set.
    pub fn is_empty_filter(&self) -> bool {
        self.level.is_none() && self.source.is_none() && self.message_contains.is_none()
    }

    /// Return a human-readable description of the active filters.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref level) = self.level {
            parts.push(format!("level={}", level));
        }
        if let Some(ref src) = self.source {
            parts.push(format!("source={}", src));
        }
        if let Some(ref msg) = self.message_contains {
            parts.push(format!("contains={}", msg));
        }
        if parts.is_empty() {
            "no filters".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Count entries per source, returning source name and count pairs.
pub fn count_by_source(entries: &[LogEntry]) -> Vec<(String, usize)> {
    let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in entries {
        let src = e.source.clone().unwrap_or_else(|| "<none>".to_string());
        *map.entry(src).or_insert(0) += 1;
    }
    let mut result: Vec<_> = map.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    result
}

// ---------------------------------------------------------------------------
// Output Channel Management
// ---------------------------------------------------------------------------

/// Represents a named output channel that collects log entries independently.
#[derive(Debug, Clone)]
pub struct OutputChannel {
    pub name: String,
    pub entries: Vec<LogEntry>,
    pub min_level: LogLevel,
    pub visible: bool,
}

impl OutputChannel {
    pub fn new(name: impl Into<String>, min_level: LogLevel) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
            min_level,
            visible: true,
        }
    }

    /// Append a log entry if it meets the channel's minimum level.
    pub fn append(&mut self, entry: LogEntry) {
        if entry.level >= self.min_level {
            self.entries.push(entry);
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl fmt::Display for OutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Channel({}, {} entries, {})",
            self.name,
            self.entries.len(),
            if self.visible { "visible" } else { "hidden" },
        )
    }
}

/// Manages multiple named output channels.
#[derive(Debug, Default)]
pub struct ChannelManager {
    channels: Vec<OutputChannel>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self { channels: Vec::new() }
    }

    /// Create a new channel and return its index.
    pub fn create_channel(&mut self, name: impl Into<String>, min_level: LogLevel) -> usize {
        let idx = self.channels.len();
        self.channels.push(OutputChannel::new(name, min_level));
        idx
    }

    /// Get a channel by index.
    pub fn get_channel(&self, index: usize) -> Option<&OutputChannel> {
        self.channels.get(index)
    }

    /// Get a mutable channel by index.
    pub fn get_channel_mut(&mut self, index: usize) -> Option<&mut OutputChannel> {
        self.channels.get_mut(index)
    }

    /// Find a channel by name.
    pub fn find_by_name(&self, name: &str) -> Option<(usize, &OutputChannel)> {
        self.channels
            .iter()
            .enumerate()
            .find(|(_, ch)| ch.name == name)
    }

    /// Broadcast a log entry to all channels.
    pub fn broadcast(&mut self, entry: &LogEntry) {
        for ch in &mut self.channels {
            ch.append(entry.clone());
        }
    }

    /// Return names of all visible channels.
    pub fn visible_channels(&self) -> Vec<&str> {
        self.channels
            .iter()
            .filter(|ch| ch.visible)
            .map(|ch| ch.name.as_str())
            .collect()
    }

    /// Total entry count across all channels.
    pub fn total_entries(&self) -> usize {
        self.channels.iter().map(|ch| ch.entry_count()).sum()
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

// ---------------------------------------------------------------------------
// Ring Buffer for bounded log storage
// ---------------------------------------------------------------------------

/// A fixed-capacity ring buffer for log entries.
///
/// When full, the oldest entry is overwritten. Entries are returned in
/// insertion order (oldest first) via [`Self::entries`].
#[derive(Debug)]
pub struct LogRingBuffer {
    buf: Vec<Option<LogEntry>>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl LogRingBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ring buffer capacity must be > 0");
        Self {
            buf: (0..capacity).map(|_| None).collect(),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push an entry, overwriting the oldest if at capacity.
    pub fn push(&mut self, entry: LogEntry) {
        self.buf[self.head] = Some(entry);
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Return all entries in insertion order (oldest first).
    pub fn entries(&self) -> Vec<&LogEntry> {
        let mut result = Vec::with_capacity(self.len);
        if self.len < self.capacity {
            // haven't wrapped yet
            for slot in &self.buf[..self.len] {
                if let Some(e) = slot {
                    result.push(e);
                }
            }
        } else {
            // wrapped: oldest is at head, read head..cap then 0..head
            for i in 0..self.capacity {
                let idx = (self.head + i) % self.capacity;
                if let Some(ref e) = self.buf[idx] {
                    result.push(e);
                }
            }
        }
        result
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        for slot in &mut self.buf {
            *slot = None;
        }
        self.head = 0;
        self.len = 0;
    }
}

// ---------------------------------------------------------------------------
// Search with context lines
// ---------------------------------------------------------------------------

/// Result of a contextual search: the matching entry index plus surrounding
/// context entries.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Index of the matching entry in the original slice.
    pub index: usize,
    /// Indices of context entries (before and after) included with this hit.
    pub context_indices: Vec<usize>,
}

/// Search entries for `query` (case-insensitive in message) and return hits
/// with up to `context` entries before and after each match.
pub fn search_with_context(
    entries: &[LogEntry],
    query: &str,
    context: usize,
) -> Vec<SearchHit> {
    let lower = query.to_lowercase();
    let mut hits = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        if entry.message.to_lowercase().contains(&lower) {
            let start = i.saturating_sub(context);
            let end = (i + context + 1).min(entries.len());
            let context_indices: Vec<usize> = (start..end).filter(|&idx| idx != i).collect();
            hits.push(SearchHit {
                index: i,
                context_indices,
            });
        }
    }
    hits
}

// ---------------------------------------------------------------------------
// Log entry grouping by source
// ---------------------------------------------------------------------------

/// Group entries by their source, preserving insertion order within each group.
pub fn group_by_source(entries: &[LogEntry]) -> Vec<(String, Vec<&LogEntry>)> {
    let mut map: std::collections::HashMap<String, Vec<&LogEntry>> =
        std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for entry in entries {
        let key = entry.source.clone().unwrap_or_else(|| "<none>".to_string());
        if !map.contains_key(&key) {
            order.push(key.clone());
        }
        map.entry(key).or_default().push(entry);
    }

    order.into_iter().map(|k| {
        let v = map.remove(&k).unwrap();
        (k, v)
    }).collect()
}

// ---------------------------------------------------------------------------
// Timestamp formatting helpers
// ---------------------------------------------------------------------------

/// Format a raw u64 timestamp as `HH:MM:SS` assuming the value represents
/// seconds since midnight.
pub fn format_timestamp_hms(ts: u64) -> String {
    let h = (ts / 3600) % 24;
    let m = (ts % 3600) / 60;
    let s = ts % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Format a raw u64 timestamp as `HH:MM:SS.mmm` assuming the value represents
/// milliseconds since midnight.
pub fn format_timestamp_hms_millis(ts_millis: u64) -> String {
    let total_secs = ts_millis / 1000;
    let millis = ts_millis % 1000;
    let h = (total_secs / 3600) % 24;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}.{millis:03}")
}

// ---------------------------------------------------------------------------
// Performance log tracking
// ---------------------------------------------------------------------------

/// Tracks named operations with start/end timestamps to compute durations.
#[derive(Debug, Default)]
pub struct PerfTracker {
    records: Vec<PerfRecord>,
}

/// A single performance measurement.
#[derive(Debug, Clone)]
pub struct PerfRecord {
    pub name: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl PerfRecord {
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

impl fmt::Display for PerfRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}ms", self.name, self.duration_ms())
    }
}

impl PerfTracker {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    /// Record a completed operation.
    pub fn record(&mut self, name: impl Into<String>, start_ms: u64, end_ms: u64) {
        self.records.push(PerfRecord {
            name: name.into(),
            start_ms,
            end_ms,
        });
    }

    /// Return all records.
    pub fn records(&self) -> &[PerfRecord] {
        &self.records
    }

    /// Return the slowest recorded operation, if any.
    pub fn slowest(&self) -> Option<&PerfRecord> {
        self.records.iter().max_by_key(|r| r.duration_ms())
    }

    /// Return the fastest recorded operation, if any.
    pub fn fastest(&self) -> Option<&PerfRecord> {
        self.records.iter().min_by_key(|r| r.duration_ms())
    }

    /// Average duration across all records, in milliseconds.
    pub fn average_ms(&self) -> u64 {
        if self.records.is_empty() {
            return 0;
        }
        let total: u64 = self.records.iter().map(|r| r.duration_ms()).sum();
        total / self.records.len() as u64
    }

    /// Return records whose duration exceeds the given threshold.
    pub fn slower_than(&self, threshold_ms: u64) -> Vec<&PerfRecord> {
        self.records
            .iter()
            .filter(|r| r.duration_ms() > threshold_ms)
            .collect()
    }

    /// Total number of recorded operations.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// Format all records as a summary report string.
    pub fn summary_report(&self) -> String {
        if self.records.is_empty() {
            return "No performance records.".to_string();
        }
        let mut lines = Vec::new();
        lines.push(format!("Performance: {} operations", self.records.len()));
        lines.push(format!("  avg: {}ms", self.average_ms()));
        if let Some(s) = self.slowest() {
            lines.push(format!("  slowest: {} ({}ms)", s.name, s.duration_ms()));
        }
        if let Some(f) = self.fastest() {
            lines.push(format!("  fastest: {} ({}ms)", f.name, f.duration_ms()));
        }
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Log export formatting
// ---------------------------------------------------------------------------

/// Export entries as CSV (level,source,timestamp,message).
pub fn export_csv(entries: &[LogEntry]) -> String {
    let mut out = String::from("level,source,timestamp,message\n");
    for e in entries {
        let src = e.source.as_deref().unwrap_or("");
        // Escape quotes in message for CSV
        let msg = e.message.replace('"', "\"\"");
        out.push_str(&format!(
            "{},{},{},\"{}\"\n",
            e.level, src, e.timestamp, msg
        ));
    }
    out
}

/// Export entries as a plain-text table with aligned columns.
pub fn export_table(entries: &[LogEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let header = format!(
        "{:<10} {:<15} {:<12} {}",
        "LEVEL", "SOURCE", "TIMESTAMP", "MESSAGE"
    );
    let mut lines = vec![header];
    lines.push("-".repeat(60));
    for e in entries {
        let src = e.source.as_deref().unwrap_or("-");
        lines.push(format!(
            "{:<10} {:<15} {:<12} {}",
            e.level.to_string(),
            src,
            e.timestamp,
            e.message,
        ));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Log level aggregation across time windows
// ---------------------------------------------------------------------------

/// Aggregate entry counts into fixed-width timestamp windows.
///
/// Each bucket covers `window_size` timestamp units. Returns a sorted vec of
/// `(window_start, count)` pairs.
pub fn aggregate_by_time_window(
    entries: &[LogEntry],
    window_size: u64,
) -> Vec<(u64, usize)> {
    if window_size == 0 || entries.is_empty() {
        return Vec::new();
    }
    let mut map: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    for e in entries {
        let bucket = (e.timestamp / window_size) * window_size;
        *map.entry(bucket).or_insert(0) += 1;
    }
    let mut result: Vec<_> = map.into_iter().collect();
    result.sort_by_key(|&(k, _)| k);
    result
}

/// Compute a per-level breakdown within each time window.
pub fn aggregate_levels_by_window(
    entries: &[LogEntry],
    window_size: u64,
) -> Vec<(u64, LogStats)> {
    if window_size == 0 || entries.is_empty() {
        return Vec::new();
    }
    let mut buckets: std::collections::HashMap<u64, Vec<&LogEntry>> =
        std::collections::HashMap::new();
    for e in entries {
        let bucket = (e.timestamp / window_size) * window_size;
        buckets.entry(bucket).or_default().push(e);
    }
    let mut result: Vec<_> = buckets
        .into_iter()
        .map(|(bucket, refs)| {
            let owned: Vec<LogEntry> = refs.into_iter().cloned().collect();
            (bucket, LogStats::from_entries(&owned))
        })
        .collect();
    result.sort_by_key(|&(k, _)| k);
    result
}

// ---------------------------------------------------------------------------
// LogChannelManager
// ---------------------------------------------------------------------------

/// Manage named log channels.
#[derive(Debug)]
pub struct LogChannelManager {
    channels: Vec<(String, LogLevel)>,
}

impl LogChannelManager {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    pub fn create_channel(&mut self, name: &str, level: LogLevel) {
        if !self.channels.iter().any(|(n, _)| n == name) {
            self.channels.push((name.to_string(), level));
        }
    }

    pub fn get_channel_level(&self, name: &str) -> Option<&LogLevel> {
        self.channels.iter().find(|(n, _)| n == name).map(|(_, l)| l)
    }

    pub fn set_channel_level(&mut self, name: &str, level: LogLevel) {
        if let Some(ch) = self.channels.iter_mut().find(|(n, _)| n == name) {
            ch.1 = level;
        }
    }

    pub fn remove_channel(&mut self, name: &str) -> bool {
        let len = self.channels.len();
        self.channels.retain(|(n, _)| n != name);
        self.channels.len() < len
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    pub fn channel_names(&self) -> Vec<&str> {
        self.channels.iter().map(|(n, _)| n.as_str()).collect()
    }

    pub fn list_channels(&self) -> &[(String, LogLevel)] {
        &self.channels
    }
}

// ---------------------------------------------------------------------------
// LogOutputFormatter
// ---------------------------------------------------------------------------

/// Format log entries for display.
pub struct LogOutputFormatter {
    pub max_line_width: usize,
    pub show_timestamp: bool,
    pub show_line_number: bool,
}

impl LogOutputFormatter {
    pub fn new(max_line_width: usize) -> Self {
        Self {
            max_line_width,
            show_timestamp: true,
            show_line_number: true,
        }
    }

    pub fn format_line(&self, line_num: usize, timestamp: &str, level: &str, message: &str) -> String {
        let mut parts = Vec::new();
        if self.show_line_number {
            parts.push(format!("{line_num:>4}"));
        }
        if self.show_timestamp {
            parts.push(format!("[{timestamp}]"));
        }
        parts.push(format!("[{level}]"));
        parts.push(message.to_string());
        parts.join(" ")
    }

    pub fn truncate_long_message(message: &str, max_len: usize) -> String {
        if message.len() <= max_len {
            message.to_string()
        } else {
            format!("{}...", &message[..max_len.saturating_sub(3)])
        }
    }

    pub fn word_wrap(text: &str, width: usize) -> Vec<String> {
        if width == 0 || text.is_empty() {
            return vec![text.to_string()];
        }
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            if !current.is_empty() && current.len() + 1 + word.len() > width {
                lines.push(current);
                current = String::new();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }
}

// ---------------------------------------------------------------------------
// LogExportConfig
// ---------------------------------------------------------------------------

/// Configuration for exporting logs.
#[derive(Debug, Clone)]
pub struct LogExportConfig {
    pub format: LogExportFormat,
    pub include_levels: Vec<LogLevel>,
    pub max_entries: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogExportFormat {
    Json,
    Text,
    Csv,
}

impl LogExportConfig {
    pub fn new(format: LogExportFormat) -> Self {
        Self {
            format,
            include_levels: Vec::new(),
            max_entries: None,
        }
    }

    pub fn with_levels(mut self, levels: Vec<LogLevel>) -> Self {
        self.include_levels = levels;
        self
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = Some(max);
        self
    }

    pub fn should_include(&self, level: &LogLevel) -> bool {
        self.include_levels.is_empty() || self.include_levels.contains(level)
    }

    pub fn estimated_size(&self, entry_count: usize) -> usize {
        let per_entry = match self.format {
            LogExportFormat::Json => 200,
            LogExportFormat::Text => 100,
            LogExportFormat::Csv => 80,
        };
        let count = self.max_entries.unwrap_or(entry_count).min(entry_count);
        count * per_entry
    }
}


/// Workbench log configuration manager.
#[derive(Debug, Clone)]
pub struct WbLogConfig {
    entries: Vec<WbLogEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench log entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbLogEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbLogEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbLogConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbLogEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbLogEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbLogEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbLogEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbLogEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbLogEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbLogEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Workbench output log channels — extended utilities (qb)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_log operations.
#[derive(Debug, Clone)]
pub struct QbMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QbMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for wb_log.
#[derive(Debug, Clone)]
pub struct QbRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QbRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for wb_log lookups.
#[derive(Debug, Clone)]
pub struct QbLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QbLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_log
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbLogRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbLogRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaWbLogCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbLogCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaWbLogCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 214
// ---------------------------------------------------------------------------

/// Generic object pool `Xc214Pool<T>`.
pub struct Xc214Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc214Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc214PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc214Pool<T> {
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
    pub fn stats(&self) -> Xc214PoolStats {
        Xc214PoolStats {
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

impl<T> Default for Xc214Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc214Scheduler`.
pub struct Xc214Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc214Scheduler {
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

impl Default for Xc214Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_214 hash for the given byte slice.
pub fn xc_214_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_214 convention.
pub fn xc_214_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_116 deepening: state machine + event bus ---

/// States for the Xd116 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd116State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd116State {
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
pub struct Xd116Transition {
    pub from: Xd116State,
    pub to: Xd116State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd116StateMachine {
    current: Xd116State,
    history: Vec<Xd116Transition>,
    step_counter: usize,
}

impl Xd116StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd116State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd116State {
        self.current
    }

    pub fn history(&self) -> &[Xd116Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd116State) -> Result<Xd116State, String> {
        let allowed = match (self.current, target) {
            (Xd116State::Idle, Xd116State::Running) => true,
            (Xd116State::Running, Xd116State::Paused) => true,
            (Xd116State::Running, Xd116State::Done) => true,
            (Xd116State::Paused, Xd116State::Running) => true,
            (Xd116State::Paused, Xd116State::Done) => true,
            (Xd116State::Done, Xd116State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_116: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd116Transition {
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
            "Xd116SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd116State> {
        let prefix = "Xd116SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd116State::Idle),
            "Running" => Some(Xd116State::Running),
            "Paused" => Some(Xd116State::Paused),
            "Done" => Some(Xd116State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd116State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd116 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd116Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd116Event {
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

type Xd116HandlerFn = Box<dyn Fn(&Xd116Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd116EventBus {
    handlers: Vec<(usize, Option<String>, Xd116HandlerFn)>,
    next_id: usize,
    published: Vec<Xd116Event>,
}

impl Xd116EventBus {
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
        F: Fn(&Xd116Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd116Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd116Event) {
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

    pub fn published_events(&self) -> &[Xd116Event] {
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
// xg_42: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg42Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg42Graph {
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

impl Default for Xg42Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_42: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg42Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg42Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg42Heap<T>) {
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

impl<T: Ord> Default for Xg42Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 213).
pub struct Xh213SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh213SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 255 as u64,
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

/// A compact bit set supporting boolean operations (variant 213).
pub struct Xh213BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh213BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 213).
pub struct Xi213Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi213Deque<T> {
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
pub struct Xi213Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi213Interval {
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

/// A simple interval tree (variant 213).
pub struct Xi213IntervalTree {
    xi_intervals: Vec<Xi213Interval>,
}

impl Xi213IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi213Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi213Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi213Interval) -> Vec<&Xi213Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi213Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi213Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi213Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi213Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi213Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi213Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 213) ---

/// Disjoint set / union-find for crate 213.
pub struct Xj213UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj213UnionFind {
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

const XJ213_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 213.
pub struct Xj213BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj213BTreeNode<K, V>>>,
    len: usize,
}

struct Xj213BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj213BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj213BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ213_BTREE_ORDER - 1
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
        let mid = XJ213_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj213BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj213BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj213BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj213BTreeNode::xj_new_leaf();
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


// --- xk_213 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk213SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk213SegmentTree {
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
pub struct Xk213DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk213DisjointIntervals {
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

    #[test]
    fn test_rotate_log_removes_old_entries() {
        let mut svc = LogService::new(LogLevel::Trace);
        for i in 0..10 {
            svc.info(format!("msg{}", i));
        }
        let config = RotationConfig::new(5, 3);
        let removed = rotate_log(&mut svc, &config);
        assert_eq!(removed, 7);
        assert_eq!(svc.entry_count(), 3);
        assert_eq!(svc.get_entries()[0].message, "msg7");
    }

    #[test]
    fn test_rotate_log_no_op_when_under_limit() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("a");
        svc.info("b");
        let config = RotationConfig::new(10, 5);
        let removed = rotate_log(&mut svc, &config);
        assert_eq!(removed, 0);
        assert_eq!(svc.entry_count(), 2);
    }

    #[test]
    fn test_parse_log_level_valid() {
        assert_eq!(parse_log_level("trace").unwrap(), LogLevel::Trace);
        assert_eq!(parse_log_level("DEBUG").unwrap(), LogLevel::Debug);
        assert_eq!(parse_log_level("Info").unwrap(), LogLevel::Info);
        assert_eq!(parse_log_level("WARN").unwrap(), LogLevel::Warning);
        assert_eq!(parse_log_level("warning").unwrap(), LogLevel::Warning);
        assert_eq!(parse_log_level("error").unwrap(), LogLevel::Error);
        assert_eq!(parse_log_level("CRITICAL").unwrap(), LogLevel::Critical);
        assert_eq!(parse_log_level("fatal").unwrap(), LogLevel::Critical);
    }

    #[test]
    fn test_parse_log_level_invalid() {
        assert!(parse_log_level("unknown").is_err());
    }

    #[test]
    fn test_format_entries_ndjson() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "a".into(), source: None, timestamp: 1 },
            LogEntry { level: LogLevel::Error, message: "b".into(), source: Some("x".into()), timestamp: 2 },
        ];
        let ndjson = format_entries_ndjson(&entries);
        let lines: Vec<&str> = ndjson.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"level\":\"INFO\""));
        assert!(lines[1].contains("\"level\":\"ERROR\""));
    }

    #[test]
    fn test_grep_entries_case_insensitive() {
        let entries = sample_entries();
        let results = grep_entries(&entries, "CONNECT");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].message, "connection failed");
        assert_eq!(results[1].message, "connected");
    }

    #[test]
    fn test_grep_entries_all_fields() {
        let entries = sample_entries();
        let results = grep_entries_all_fields(&entries, "NETWORK");
        assert_eq!(results.len(), 2); // both entries with source "network"
    }

    #[test]
    fn test_apply_filter_all() {
        let entries = sample_entries();
        let results = apply_filter(&entries, &LogFilterLevel::All);
        assert_eq!(results.len(), entries.len());
    }

    #[test]
    fn test_apply_filter_at_level() {
        let entries = sample_entries();
        let results = apply_filter(&entries, &LogFilterLevel::AtLevel(LogLevel::Info));
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.level == LogLevel::Info));
    }

    #[test]
    fn test_apply_filter_above_level() {
        let entries = sample_entries();
        // Above Warning => Error, Critical
        let results = apply_filter(&entries, &LogFilterLevel::AboveLevel(LogLevel::Warning));
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.level > LogLevel::Warning));
    }

    #[test]
    fn test_apply_filter_below_level() {
        let entries = sample_entries();
        // Below Warning => Trace, Debug, Info
        let results = apply_filter(&entries, &LogFilterLevel::BelowLevel(LogLevel::Warning));
        assert_eq!(results.len(), 3); // Info("started"), Info("connected"), Debug("tick")
        assert!(results.iter().all(|e| e.level < LogLevel::Warning));
    }

    #[test]
    fn test_apply_filter_range() {
        let entries = sample_entries();
        // Info..=Error inclusive
        let results = apply_filter(
            &entries,
            &LogFilterLevel::Range { min: LogLevel::Info, max: LogLevel::Error },
        );
        assert_eq!(results.len(), 4); // 2×Info + Warning + Error
        assert!(results.iter().all(|e| e.level >= LogLevel::Info && e.level <= LogLevel::Error));
    }

    #[test]
    fn test_log_search_case_insensitive_message() {
        let entries = sample_entries();
        let results = log_search(&entries, "STARTED", false, LogSearchFields::Message);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "started");
    }

    #[test]
    fn test_log_search_case_sensitive_message() {
        let entries = sample_entries();
        // "STARTED" should NOT match "started" when case-sensitive
        let results = log_search(&entries, "STARTED", true, LogSearchFields::Message);
        assert!(results.is_empty());

        let results = log_search(&entries, "started", true, LogSearchFields::Message);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_log_search_source_field() {
        let entries = sample_entries();
        let results = log_search(&entries, "network", false, LogSearchFields::Source);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_log_search_all_fields() {
        let entries = sample_entries();
        // "app" appears as source on "started" and "out of memory"
        let results = log_search(&entries, "app", false, LogSearchFields::All);
        assert_eq!(results.len(), 2);
    }

    // -- log_rotate tests ---------------------------------------------------

    #[test]
    fn rotate_not_needed() {
        let mut entries = vec![
            LogEntry { level: LogLevel::Info, message: "a".into(), source: None, timestamp: 1 },
        ];
        let config = LogRotateConfig { max_entries: 10, keep_entries: 5, min_retain_level: LogLevel::Trace };
        let result = log_rotate(&mut entries, &config);
        assert!(!result.did_rotate());
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn rotate_trims_to_keep_entries() {
        let mut entries: Vec<LogEntry> = (0..20)
            .map(|i| LogEntry { level: LogLevel::Info, message: format!("msg{i}"), source: None, timestamp: i })
            .collect();
        let config = LogRotateConfig { max_entries: 10, keep_entries: 5, min_retain_level: LogLevel::Trace };
        let result = log_rotate(&mut entries, &config);
        assert!(result.did_rotate());
        assert_eq!(result.entries_before, 20);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].message, "msg15");
    }

    #[test]
    fn rotate_filters_by_level() {
        let mut entries: Vec<LogEntry> = (0..20)
            .map(|i| LogEntry {
                level: if i % 2 == 0 { LogLevel::Debug } else { LogLevel::Error },
                message: format!("msg{i}"),
                source: None,
                timestamp: i,
            })
            .collect();
        let config = LogRotateConfig { max_entries: 10, keep_entries: 100, min_retain_level: LogLevel::Error };
        let result = log_rotate(&mut entries, &config);
        assert!(result.did_rotate());
        assert!(entries.iter().all(|e| e.level >= LogLevel::Error));
    }

    #[test]
    fn rotate_result_display() {
        let result = LogRotateResult { entries_before: 100, entries_after: 50, entries_dropped: 50 };
        let s = format!("{result}");
        assert!(s.contains("100 -> 50"));
        assert!(s.contains("dropped 50"));
    }

    #[test]
    fn needs_rotation() {
        let config = LogRotateConfig::default();
        assert!(!log_needs_rotation(5_000, &config));
        assert!(log_needs_rotation(10_001, &config));
    }

    #[test]
    fn rotate_config_default() {
        let config = LogRotateConfig::default();
        assert_eq!(config.max_entries, 10_000);
        assert_eq!(config.keep_entries, 5_000);
        assert_eq!(config.min_retain_level, LogLevel::Trace);
    }

    #[test]
    fn last_error_returns_most_recent() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("ok");
        svc.error("err1");
        svc.info("ok2");
        svc.error("err2");
        let last = svc.last_error().unwrap();
        assert_eq!(last.message, "err2");
    }

    #[test]
    fn last_warning_returns_most_recent() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.warn("w1");
        svc.info("ok");
        svc.warn("w2");
        let last = svc.last_warning().unwrap();
        assert_eq!(last.message, "w2");
    }

    #[test]
    fn last_error_returns_none_when_no_errors() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.info("ok");
        assert!(svc.last_error().is_none());
    }

    #[test]
    fn log_service_summary() {
        let mut svc = LogService::new(LogLevel::Trace);
        svc.error("e1");
        svc.warn("w1");
        svc.info("i1");
        let s = svc.summary();
        assert!(s.contains("3 entries"));
        assert!(s.contains("1 errors"));
        assert!(s.contains("1 warnings"));
    }

    #[test]
    fn log_service_display() {
        let svc = LogService::new(LogLevel::Info);
        let s = format!("{svc}");
        assert!(s.contains("0 entries"));
        assert!(s.contains("min_level=INFO"));
    }

    #[test]
    fn log_level_is_at_least() {
        assert!(LogLevel::Error.is_at_least(&LogLevel::Warning));
        assert!(LogLevel::Warning.is_at_least(&LogLevel::Warning));
        assert!(!LogLevel::Info.is_at_least(&LogLevel::Warning));
    }

    #[test]
    fn log_level_rank() {
        assert_eq!(LogLevel::Trace.rank(), 0);
        assert_eq!(LogLevel::Critical.rank(), 5);
        assert!(LogLevel::Error.rank() > LogLevel::Info.rank());
    }

    #[test]
    fn log_entry_has_source_and_message_length() {
        let entry = LogEntry {
            level: LogLevel::Info,
            message: "hello".into(),
            source: Some("test".into()),
            timestamp: 0,
        };
        assert!(entry.has_source());
        assert_eq!(entry.message_length(), 5);

        let entry2 = LogEntry {
            level: LogLevel::Info,
            message: "".into(),
            source: None,
            timestamp: 0,
        };
        assert!(!entry2.has_source());
        assert_eq!(entry2.message_length(), 0);
    }

    #[test]
    fn log_filter_is_empty() {
        let f = LogFilter::new();
        assert!(f.is_empty());
        let f2 = LogFilter::new().with_level(LogLevel::Error);
        assert!(!f2.is_empty());
    }

    #[test]
    fn filter_entries_by_level_and_timestamp() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "a".into(), source: Some("s1".into()), timestamp: 100 },
            LogEntry { level: LogLevel::Error, message: "b".into(), source: Some("s1".into()), timestamp: 200 },
            LogEntry { level: LogLevel::Warning, message: "c".into(), source: Some("s2".into()), timestamp: 300 },
        ];
        let result = filter_entries(&entries, Some(&LogLevel::Warning), None, Some(150), None);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].message, "b");
        assert_eq!(result[1].message, "c");
    }

    #[test]
    fn filter_entries_by_source() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "x".into(), source: Some("app".into()), timestamp: 0 },
            LogEntry { level: LogLevel::Info, message: "y".into(), source: Some("db".into()), timestamp: 0 },
        ];
        let result = filter_entries(&entries, None, Some("db"), None, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message, "y");
    }

    #[test]
    fn format_with_template_replaces_placeholders() {
        let entry = LogEntry {
            level: LogLevel::Error,
            message: "disk full".into(),
            source: Some("storage".into()),
            timestamp: 42,
        };
        let result = format_with_template(&entry, "{level}: {message} ({source}) @{timestamp}");
        assert_eq!(result, "ERROR: disk full (storage) @42");
    }

    #[test]
    fn rotation_tracker_records_events() {
        let mut tracker = RotationTracker::new();
        tracker.record(100, 500, 1000);
        tracker.record(200, 300, 1000);
        assert_eq!(tracker.rotation_count(), 2);
        assert_eq!(tracker.total_dropped(), 800);
        assert_eq!(tracker.last_rotation().unwrap().entries_dropped, 300);
    }

    #[test]
    fn extract_fields_simple() {
        let fields = extract_fields("user=alice status=200 path=/api");
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0], ("user".into(), "alice".into()));
        assert_eq!(fields[1], ("status".into(), "200".into()));
        assert_eq!(fields[2], ("path".into(), "/api".into()));
    }

    #[test]
    fn extract_fields_quoted_value() {
        let fields = extract_fields(r#"msg="hello world" code=42"#);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0], ("msg".into(), "hello world".into()));
        assert_eq!(fields[1], ("code".into(), "42".into()));
    }

    #[test]
    fn last_entry_returns_most_recent() {
        let mut svc = LogService::new(LogLevel::Info);
        svc.info("first");
        svc.info("second");
        assert_eq!(svc.last_entry().unwrap().message, "second");
    }

    #[test]
    fn last_entry_empty_service() {
        let svc = LogService::new(LogLevel::Info);
        assert!(svc.last_entry().is_none());
    }

    #[test]
    fn search_prefix_finds_matching() {
        let mut svc = LogService::new(LogLevel::Info);
        svc.info("auth: login success");
        svc.info("db: query done");
        svc.info("auth: logout");
        let results = svc.search_prefix("auth:");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_prefix_no_match() {
        let mut svc = LogService::new(LogLevel::Info);
        svc.info("hello");
        assert!(svc.search_prefix("xyz").is_empty());
    }

    #[test]
    fn level_counts_breakdown() {
        let mut svc = LogService::new(LogLevel::Debug);
        svc.info("a");
        svc.info("b");
        svc.error("c");
        let counts = svc.level_counts();
        assert_eq!(counts.get("INFO"), Some(&2));
        assert_eq!(counts.get("ERROR"), Some(&1));
    }

    #[test]
    fn average_message_length_computed() {
        let mut svc = LogService::new(LogLevel::Info);
        svc.info("ab");
        svc.info("abcd");
        assert_eq!(svc.average_message_length(), 3);
    }

    #[test]
    fn average_message_length_empty() {
        let svc = LogService::new(LogLevel::Info);
        assert_eq!(svc.average_message_length(), 0);
    }

    #[test]
    fn longest_message_found() {
        let mut svc = LogService::new(LogLevel::Info);
        svc.info("short");
        svc.info("a longer message here");
        assert_eq!(svc.longest_message().unwrap().message, "a longer message here");
    }

    #[test]
    fn filter_is_empty_when_no_criteria() {
        let f = LogFilter::new();
        assert!(f.is_empty_filter());
    }

    #[test]
    fn filter_is_not_empty_with_level() {
        let f = LogFilter::new().with_level(LogLevel::Error);
        assert!(!f.is_empty_filter());
    }

    #[test]
    fn filter_describe_no_criteria() {
        let f = LogFilter::new();
        assert_eq!(f.describe(), "no filters");
    }

    #[test]
    fn filter_describe_with_criteria() {
        let f = LogFilter::new()
            .with_level(LogLevel::Error)
            .with_source("db".to_string());
        let desc = f.describe();
        assert!(desc.contains("level="));
        assert!(desc.contains("source=db"));
    }

    #[test]
    fn count_by_source_groups() {
        let entries = vec![
            LogEntry {
                level: LogLevel::Info,
                message: "a".into(),
                source: Some("db".into()),
                timestamp: 0,
            },
            LogEntry {
                level: LogLevel::Info,
                message: "b".into(),
                source: Some("db".into()),
                timestamp: 1,
            },
            LogEntry {
                level: LogLevel::Error,
                message: "c".into(),
                source: Some("api".into()),
                timestamp: 2,
            },
        ];
        let counts = count_by_source(&entries);
        assert_eq!(counts[0], ("db".into(), 2));
        assert_eq!(counts[1], ("api".into(), 1));
    }

    #[test]
    fn count_by_source_empty() {
        let counts = count_by_source(&[]);
        assert!(counts.is_empty());
    }

    // ── Output Channel Management ──

    #[test]
    fn output_channel_filters_by_min_level() {
        let mut ch = OutputChannel::new("test-ch", LogLevel::Warning);
        ch.append(LogEntry {
            level: LogLevel::Info,
            message: "below threshold".into(),
            source: None,
            timestamp: 0,
        });
        ch.append(LogEntry {
            level: LogLevel::Error,
            message: "above threshold".into(),
            source: None,
            timestamp: 1,
        });
        assert_eq!(ch.entry_count(), 1);
        assert_eq!(ch.entries[0].message, "above threshold");
    }

    #[test]
    fn channel_manager_broadcast_and_visibility() {
        let mut mgr = ChannelManager::new();
        let a = mgr.create_channel("alpha", LogLevel::Trace);
        let b = mgr.create_channel("beta", LogLevel::Error);

        let entry = LogEntry {
            level: LogLevel::Info,
            message: "hello".into(),
            source: None,
            timestamp: 0,
        };
        mgr.broadcast(&entry);

        // alpha accepts Info, beta requires Error
        assert_eq!(mgr.get_channel(a).unwrap().entry_count(), 1);
        assert_eq!(mgr.get_channel(b).unwrap().entry_count(), 0);
        assert_eq!(mgr.channel_count(), 2);
        assert_eq!(mgr.total_entries(), 1);

        mgr.get_channel_mut(a).unwrap().set_visible(false);
        assert_eq!(mgr.visible_channels(), vec!["beta"]);
    }

    #[test]
    fn channel_manager_find_by_name() {
        let mut mgr = ChannelManager::new();
        mgr.create_channel("output", LogLevel::Info);
        mgr.create_channel("debug", LogLevel::Debug);
        let (idx, ch) = mgr.find_by_name("debug").unwrap();
        assert_eq!(idx, 1);
        assert_eq!(ch.name, "debug");
        assert!(mgr.find_by_name("missing").is_none());
    }

    // ── Ring Buffer ──

    #[test]
    fn ring_buffer_overwrites_oldest() {
        let mut ring = LogRingBuffer::new(3);
        for i in 0..5u64 {
            ring.push(LogEntry {
                level: LogLevel::Info,
                message: format!("msg{i}"),
                source: None,
                timestamp: i,
            });
        }
        assert_eq!(ring.len(), 3);
        assert!(ring.is_full());
        let entries = ring.entries();
        // oldest surviving should be msg2
        assert_eq!(entries[0].message, "msg2");
        assert_eq!(entries[1].message, "msg3");
        assert_eq!(entries[2].message, "msg4");
    }

    #[test]
    fn ring_buffer_clear() {
        let mut ring = LogRingBuffer::new(5);
        ring.push(LogEntry {
            level: LogLevel::Info,
            message: "a".into(),
            source: None,
            timestamp: 0,
        });
        assert!(!ring.is_empty());
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert_eq!(ring.capacity(), 5);
    }

    // ── Search with context ──

    #[test]
    fn search_with_context_returns_surrounding_indices() {
        let entries: Vec<LogEntry> = (0..10)
            .map(|i| LogEntry {
                level: LogLevel::Info,
                message: if i == 5 { "TARGET".into() } else { format!("line{i}") },
                source: None,
                timestamp: i,
            })
            .collect();

        let hits = search_with_context(&entries, "target", 2);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].index, 5);
        assert_eq!(hits[0].context_indices, vec![3, 4, 6, 7]);
    }

    // ── Group by source ──

    #[test]
    fn group_by_source_preserves_order() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "a".into(), source: Some("db".into()), timestamp: 0 },
            LogEntry { level: LogLevel::Info, message: "b".into(), source: Some("api".into()), timestamp: 1 },
            LogEntry { level: LogLevel::Info, message: "c".into(), source: Some("db".into()), timestamp: 2 },
            LogEntry { level: LogLevel::Info, message: "d".into(), source: None, timestamp: 3 },
        ];
        let groups = group_by_source(&entries);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, "db");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "api");
        assert_eq!(groups[2].0, "<none>");
    }

    // ── Timestamp formatting ──

    #[test]
    fn format_timestamp_hms_basic() {
        assert_eq!(format_timestamp_hms(0), "00:00:00");
        assert_eq!(format_timestamp_hms(3661), "01:01:01");
        assert_eq!(format_timestamp_hms(86399), "23:59:59");
    }

    #[test]
    fn format_timestamp_hms_millis_basic() {
        assert_eq!(format_timestamp_hms_millis(0), "00:00:00.000");
        assert_eq!(format_timestamp_hms_millis(3_661_123), "01:01:01.123");
    }

    // ── Performance tracker ──

    #[test]
    fn perf_tracker_stats() {
        let mut pt = PerfTracker::new();
        pt.record("fast_op", 100, 110);
        pt.record("slow_op", 200, 350);
        pt.record("mid_op", 300, 370);

        assert_eq!(pt.count(), 3);
        assert_eq!(pt.fastest().unwrap().name, "fast_op");
        assert_eq!(pt.slowest().unwrap().name, "slow_op"); // 150ms
        assert_eq!(pt.average_ms(), (10 + 150 + 70) / 3);

        let slow = pt.slower_than(50);
        assert_eq!(slow.len(), 2);

        let report = pt.summary_report();
        assert!(report.contains("3 operations"));
        assert!(report.contains("slowest: slow_op"));
    }

    #[test]
    fn perf_record_display() {
        let r = PerfRecord { name: "compile".into(), start_ms: 0, end_ms: 42 };
        assert_eq!(r.to_string(), "compile: 42ms");
    }

    // ── Export CSV ──

    #[test]
    fn export_csv_format() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "hello".into(), source: Some("app".into()), timestamp: 100 },
            LogEntry { level: LogLevel::Error, message: "fail".into(), source: None, timestamp: 200 },
        ];
        let csv = export_csv(&entries);
        assert!(csv.starts_with("level,source,timestamp,message\n"));
        assert!(csv.contains("INFO,app,100,\"hello\""));
        assert!(csv.contains("ERROR,,200,\"fail\""));
    }

    // ── Export table ──

    #[test]
    fn export_table_includes_header() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "test".into(), source: Some("src".into()), timestamp: 0 },
        ];
        let table = export_table(&entries);
        assert!(table.contains("LEVEL"));
        assert!(table.contains("SOURCE"));
        assert!(table.contains("MESSAGE"));
        assert!(table.contains("test"));
    }

    #[test]
    fn export_table_empty() {
        assert!(export_table(&[]).is_empty());
    }

    // ── Time window aggregation ──

    #[test]
    fn aggregate_by_time_window_groups() {
        let entries: Vec<LogEntry> = vec![0, 1, 2, 5, 6, 10]
            .into_iter()
            .map(|ts| LogEntry {
                level: LogLevel::Info,
                message: "x".into(),
                source: None,
                timestamp: ts,
            })
            .collect();

        let buckets = aggregate_by_time_window(&entries, 5);
        assert_eq!(buckets, vec![(0, 3), (5, 2), (10, 1)]);
    }

    #[test]
    fn aggregate_levels_by_window_counts() {
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "a".into(), source: None, timestamp: 0 },
            LogEntry { level: LogLevel::Error, message: "b".into(), source: None, timestamp: 1 },
            LogEntry { level: LogLevel::Info, message: "c".into(), source: None, timestamp: 10 },
        ];
        let result = aggregate_levels_by_window(&entries, 5);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, 0);
        assert_eq!(result[0].1.info_count, 1);
        assert_eq!(result[0].1.error_count, 1);
        assert_eq!(result[1].0, 10);
        assert_eq!(result[1].1.info_count, 1);
    }

    #[test]
    fn aggregate_by_time_window_empty_and_zero() {
        assert!(aggregate_by_time_window(&[], 5).is_empty());
        let entries = vec![
            LogEntry { level: LogLevel::Info, message: "a".into(), source: None, timestamp: 0 },
        ];
        assert!(aggregate_by_time_window(&entries, 0).is_empty());
    }

    // ── OutputChannel display ──

    #[test]
    fn output_channel_display_format() {
        let ch = OutputChannel::new("logs", LogLevel::Info);
        let s = ch.to_string();
        assert!(s.contains("logs"));
        assert!(s.contains("0 entries"));
        assert!(s.contains("visible"));
    }

    // -- LogChannelManager -------------------------------------------------

    #[test]
    fn channel_manager_create() {
        let mut mgr = LogChannelManager::new();
        mgr.create_channel("output", LogLevel::Info);
        assert_eq!(mgr.channel_count(), 1);
        assert_eq!(mgr.channel_names(), vec!["output"]);
    }

    #[test]
    fn channel_manager_no_duplicates() {
        let mut mgr = LogChannelManager::new();
        mgr.create_channel("main", LogLevel::Info);
        mgr.create_channel("main", LogLevel::Debug);
        assert_eq!(mgr.channel_count(), 1);
    }

    #[test]
    fn channel_manager_set_level() {
        let mut mgr = LogChannelManager::new();
        mgr.create_channel("ch", LogLevel::Info);
        mgr.set_channel_level("ch", LogLevel::Debug);
        assert_eq!(mgr.get_channel_level("ch"), Some(&LogLevel::Debug));
    }

    #[test]
    fn channel_manager_remove() {
        let mut mgr = LogChannelManager::new();
        mgr.create_channel("tmp", LogLevel::Trace);
        assert!(mgr.remove_channel("tmp"));
        assert_eq!(mgr.channel_count(), 0);
    }

    // -- LogOutputFormatter ------------------------------------------------

    #[test]
    fn formatter_format_line() {
        let fmt = LogOutputFormatter::new(120);
        let line = fmt.format_line(1, "12:00:00", "INFO", "Hello world");
        assert!(line.contains("[12:00:00]"));
        assert!(line.contains("[INFO]"));
        assert!(line.contains("Hello world"));
    }

    #[test]
    fn formatter_truncate() {
        let result = LogOutputFormatter::truncate_long_message("hello world this is long", 10);
        assert!(result.len() <= 10);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn formatter_word_wrap() {
        let lines = LogOutputFormatter::word_wrap("hello world foo bar", 11);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "hello world");
    }

    #[test]
    fn formatter_no_truncate_short() {
        assert_eq!(LogOutputFormatter::truncate_long_message("short", 100), "short");
    }

    // -- LogExportConfig ---------------------------------------------------

    #[test]
    fn export_config_should_include() {
        let cfg = LogExportConfig::new(LogExportFormat::Json)
            .with_levels(vec![LogLevel::Error, LogLevel::Warning]);
        assert!(cfg.should_include(&LogLevel::Error));
        assert!(!cfg.should_include(&LogLevel::Info));
    }

    #[test]
    fn export_config_empty_levels_includes_all() {
        let cfg = LogExportConfig::new(LogExportFormat::Text);
        assert!(cfg.should_include(&LogLevel::Trace));
    }

    #[test]
    fn export_config_estimated_size() {
        let cfg = LogExportConfig::new(LogExportFormat::Csv).with_max_entries(10);
        assert_eq!(cfg.estimated_size(100), 800);
    }

    #[test]
    fn export_config_format() {
        let cfg = LogExportConfig::new(LogExportFormat::Json);
        assert_eq!(cfg.format, LogExportFormat::Json);
    }


    #[test]
    fn wb_log_entry_creation() {
        let e = WbLogEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_log_entry_with_priority() {
        let e = WbLogEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_log_entry_metadata() {
        let e = WbLogEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_log_entry_remove_meta() {
        let mut e = WbLogEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_log_entry_activate_deactivate() {
        let mut e = WbLogEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_log_config_add_sorted() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("lo", "Lo").with_priority(1));
        c.add(WbLogEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_log_config_capacity() {
        let mut c = WbLogConfig::new(1);
        assert!(c.add(WbLogEntry::new("a", "A")));
        assert!(!c.add(WbLogEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_log_config_remove() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_log_config_get() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_log_config_active_entries() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "A"));
        c.add(WbLogEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_log_config_enable_disable() {
        let mut c = WbLogConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_log_config_clear() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_log_config_find_by_label() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_log_config_top_n() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "A").with_priority(1));
        c.add(WbLogEntry::new("b", "B").with_priority(2));
        c.add(WbLogEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_log_config_deactivate_activate_all() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "A"));
        c.add(WbLogEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_log_config_highest_priority() {
        let mut c = WbLogConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbLogEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_log_config_contains() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_log_config_labels() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "Alpha"));
        c.add(WbLogEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_log_config_drain_inactive() {
        let mut c = WbLogConfig::new(10);
        c.add(WbLogEntry::new("a", "A"));
        c.add(WbLogEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qb_metrics_empty() {
        let m = QbMetrics::new("wb_log");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qb_metrics_record_and_mean() {
        let mut m = QbMetrics::new("wb_log");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qb_metrics_min_max() {
        let mut m = QbMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qb_metrics_variance_and_std() {
        let mut m = QbMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qb_metrics_percentile() {
        let mut m = QbMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qb_metrics_merge() {
        let mut a = QbMetrics::new("a");
        a.record(1.0);
        let mut b = QbMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qb_metrics_reset() {
        let mut m = QbMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qb_rate_window_empty() {
        let rw = QbRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qb_rate_window_tick_and_rate() {
        let mut rw = QbRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qb_lru_cache_basic() {
        let mut c = QbLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qb_lru_cache_contains_and_keys() {
        let mut c = QbLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qb_lru_cache_remove() {
        let mut c = QbLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qb_metrics_sum() {
        let mut m = QbMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qb_metrics_label() {
        let m = QbMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qb_lru_cache_clear() {
        let mut c = QbLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_log
    #[test]
    fn xa_wb_log_ring_new() {
        let rb = super::XaWbLogRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_log_ring_push_len() {
        let mut rb = super::XaWbLogRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_log_ring_wrap() {
        let mut rb = super::XaWbLogRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_log_ring_mean_empty() {
        let rb = super::XaWbLogRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_log_ring_mean_values() {
        let mut rb = super::XaWbLogRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_log_ring_min_max() {
        let mut rb = super::XaWbLogRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_log_ring_iter() {
        let mut rb = super::XaWbLogRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_log_counter_new() {
        let c = super::XaWbLogCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_log_counter_inc() {
        let mut c = super::XaWbLogCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_log_counter_inc_by() {
        let mut c = super::XaWbLogCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_log_counter_reset() {
        let mut c = super::XaWbLogCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_log_counter_clear() {
        let mut c = super::XaWbLogCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_log_counter_default() {
        let c = super::XaWbLogCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 214 ----

    #[test]
    fn xc_214_pool_new_empty() {
        let pool: super::Xc214Pool<i32> = super::Xc214Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_214_pool_release_acquire() {
        let mut pool = super::Xc214Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_214_pool_acquire_empty() {
        let mut pool: super::Xc214Pool<i32> = super::Xc214Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_214_pool_full() {
        let mut pool = super::Xc214Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_214_pool_drain() {
        let mut pool = super::Xc214Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_214_pool_stats() {
        let mut pool = super::Xc214Pool::new(8);
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
    fn xc_214_pool_clear() {
        let mut pool = super::Xc214Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_214_pool_shrink() {
        let mut pool = super::Xc214Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_214_pool_default() {
        let pool: super::Xc214Pool<String> = super::Xc214Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_214_pool_extend() {
        let mut pool = super::Xc214Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_214_pool_retain() {
        let mut pool = super::Xc214Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_214_scheduler_round_robin() {
        let mut sched = super::Xc214Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_214_scheduler_empty() {
        let mut sched = super::Xc214Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_214_scheduler_reset() {
        let mut sched = super::Xc214Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_214_scheduler_add_remove() {
        let mut sched = super::Xc214Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_214_scheduler_targets() {
        let sched = super::Xc214Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_214_hash_empty() {
        assert_eq!(super::xc_214_hash(b""), 5381);
    }

    #[test]
    fn xc_214_hash_data() {
        let h = super::xc_214_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_214_hash(b"hello"), h);
    }

    #[test]
    fn xc_214_reverse_str() {
        assert_eq!(super::xc_214_reverse("abc"), "cba");
        assert_eq!(super::xc_214_reverse(""), "");
    }


    // --- xd_116 deepening tests ---

    #[test]
    fn xd_116_sm_initial_state() {
        let sm = Xd116StateMachine::new();
        assert_eq!(sm.current_state(), Xd116State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_116_sm_valid_idle_to_running() {
        let mut sm = Xd116StateMachine::new();
        assert!(sm.transition(Xd116State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd116State::Running);
    }

    #[test]
    fn xd_116_sm_valid_running_to_paused() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        assert!(sm.transition(Xd116State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd116State::Paused);
    }

    #[test]
    fn xd_116_sm_valid_running_to_done() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        assert!(sm.transition(Xd116State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd116State::Done);
    }

    #[test]
    fn xd_116_sm_valid_paused_to_running() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        sm.transition(Xd116State::Paused).unwrap();
        assert!(sm.transition(Xd116State::Running).is_ok());
    }

    #[test]
    fn xd_116_sm_valid_done_to_idle() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        sm.transition(Xd116State::Done).unwrap();
        assert!(sm.transition(Xd116State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd116State::Idle);
    }

    #[test]
    fn xd_116_sm_invalid_idle_to_done() {
        let mut sm = Xd116StateMachine::new();
        assert!(sm.transition(Xd116State::Done).is_err());
    }

    #[test]
    fn xd_116_sm_invalid_idle_to_paused() {
        let mut sm = Xd116StateMachine::new();
        assert!(sm.transition(Xd116State::Paused).is_err());
    }

    #[test]
    fn xd_116_sm_history_tracking() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        sm.transition(Xd116State::Paused).unwrap();
        sm.transition(Xd116State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd116State::Idle);
        assert_eq!(sm.history()[0].to, Xd116State::Running);
        assert_eq!(sm.history()[1].from, Xd116State::Running);
        assert_eq!(sm.history()[2].to, Xd116State::Done);
    }

    #[test]
    fn xd_116_sm_serialize_deserialize() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd116StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd116State::Running));
    }

    #[test]
    fn xd_116_sm_deserialize_invalid() {
        assert_eq!(Xd116StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_116_sm_reset() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd116State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_116_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd116EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd116Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_116_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd116EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd116Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd116Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_116_bus_unsubscribe() {
        let mut bus = Xd116EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_116_event_kind_and_payload() {
        let e = Xd116Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd116Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_116_bus_clear_history() {
        let mut bus = Xd116EventBus::new();
        bus.publish(Xd116Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_116_sm_step_counter_increments() {
        let mut sm = Xd116StateMachine::new();
        sm.transition(Xd116State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd116State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_42 graph tests ------------------------------------------------

    #[test]
    fn xg_42_graph_empty() {
        let g = super::Xg42Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_42_graph_add_node() {
        let mut g = super::Xg42Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_42_graph_add_edge() {
        let mut g = super::Xg42Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_42_graph_neighbors() {
        let mut g = super::Xg42Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_42_graph_has_path() {
        let mut g = super::Xg42Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_42_graph_self_path() {
        let g = super::Xg42Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_42_graph_topo_sort() {
        let mut g = super::Xg42Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_42_graph_cycle_detect_false() {
        let mut g = super::Xg42Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_42_graph_cycle_detect_true() {
        let mut g = super::Xg42Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_42 heap tests -------------------------------------------------

    #[test]
    fn xg_42_heap_empty() {
        let h: super::Xg42Heap<i32> = super::Xg42Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_42_heap_push_pop() {
        let mut h = super::Xg42Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_42_heap_peek() {
        let mut h = super::Xg42Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_42_heap_drain_sorted() {
        let mut h = super::Xg42Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_42_heap_merge() {
        let mut a = super::Xg42Heap::new();
        let mut b = super::Xg42Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_42_heap_default() {
        let h: super::Xg42Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_42_graph_default() {
        let g: super::Xg42Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh213_skip_insert_contains() {
        let mut sl = super::Xh213SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh213_skip_remove() {
        let mut sl = super::Xh213SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh213_skip_len() {
        let mut sl = super::Xh213SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh213_skip_range_query() {
        let mut sl = super::Xh213SkipList::xh_new(4);
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
    fn xh213_skip_floor_ceiling() {
        let mut sl = super::Xh213SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh213_skip_rank() {
        let mut sl = super::Xh213SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh213_skip_empty() {
        let sl = super::Xh213SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh213_skip_duplicates() {
        let mut sl = super::Xh213SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh213_bitset_set_test() {
        let mut bs = super::Xh213BitSet::xh_new(256);
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
    fn xh213_bitset_clear_count() {
        let mut bs = super::Xh213BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh213_bitset_and_or_xor() {
        let mut a = super::Xh213BitSet::xh_new(128);
        let mut b = super::Xh213BitSet::xh_new(128);
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
    fn xh213_bitset_iter_ones() {
        let mut bs = super::Xh213BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh213_bitset_first_last() {
        let mut bs = super::Xh213BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh213_bitset_empty() {
        let bs = super::Xh213BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi213_deque_push_pop_back() {
        let mut dq = super::Xi213Deque::xi_new(4);
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
    fn xi213_deque_push_pop_front() {
        let mut dq = super::Xi213Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi213_deque_mixed_ops() {
        let mut dq = super::Xi213Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi213_deque_get_and_split() {
        let mut dq = super::Xi213Deque::xi_new(8);
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
    fn xi213_deque_rotate_left() {
        let mut dq = super::Xi213Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi213_deque_rotate_right() {
        let mut dq = super::Xi213Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi213_deque_grow() {
        let mut dq = super::Xi213Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi213_deque_empty() {
        let dq = super::Xi213Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi213_interval_tree_insert_query() {
        let mut tree = super::Xi213IntervalTree::xi_new();
        tree.xi_insert(super::Xi213Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi213Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi213Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi213_interval_tree_overlap() {
        let mut tree = super::Xi213IntervalTree::xi_new();
        tree.xi_insert(super::Xi213Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi213Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi213Interval::xi_new(12, 20));
        let q = super::Xi213Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi213_interval_tree_remove() {
        let mut tree = super::Xi213IntervalTree::xi_new();
        tree.xi_insert(super::Xi213Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi213Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi213_interval_tree_gaps() {
        let mut tree = super::Xi213IntervalTree::xi_new();
        tree.xi_insert(super::Xi213Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi213Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi213Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi213Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi213Interval::xi_new(8, 10));
    }

    #[test]
    fn xi213_interval_tree_merge() {
        let mut tree = super::Xi213IntervalTree::xi_new();
        tree.xi_insert(super::Xi213Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi213Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi213Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi213Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi213Interval::xi_new(10, 15));
    }

    #[test]
    fn xi213_interval_tree_all() {
        let mut tree = super::Xi213IntervalTree::xi_new();
        tree.xi_insert(super::Xi213Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi213Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi213_interval_tree_empty() {
        let tree = super::Xi213IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi213_interval_tree_contains_point() {
        let iv = super::Xi213Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 213) ---

    #[test]
    fn xj_213_uf_make_and_find() {
        let mut uf = super::Xj213UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_213_uf_union_connected() {
        let mut uf = super::Xj213UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_213_uf_component_count() {
        let mut uf = super::Xj213UnionFind::xj_new();
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
    fn xj_213_uf_component_size() {
        let mut uf = super::Xj213UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_213_uf_largest_component() {
        let mut uf = super::Xj213UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_213_uf_many_elements() {
        let mut uf = super::Xj213UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_213_uf_separate_components() {
        let mut uf = super::Xj213UnionFind::xj_new();
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
    fn xj_213_uf_path_compression() {
        let mut uf = super::Xj213UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_213_bt_insert_get() {
        let mut bt = super::Xj213BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_213_bt_contains_len() {
        let mut bt = super::Xj213BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_213_bt_replace() {
        let mut bt = super::Xj213BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_213_bt_remove() {
        let mut bt = super::Xj213BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_213_bt_keys_values() {
        let mut bt = super::Xj213BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_213_bt_range() {
        let mut bt = super::Xj213BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_213_bt_min_max() {
        let mut bt = super::Xj213BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_213_bt_many_inserts() {
        let mut bt = super::Xj213BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_213 segment tree tests ---

    #[test]
    fn xk_213_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk213SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_213_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk213SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_213_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk213SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_213_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk213SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_213_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk213SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_213_st_single_element() {
        let data = vec![42];
        let st = super::Xk213SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_213_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk213SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_213_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk213SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_213 disjoint intervals tests ---

    #[test]
    fn xk_213_di_add_and_count() {
        let mut di = super::Xk213DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_213_di_merge_overlap() {
        let mut di = super::Xk213DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_213_di_contains() {
        let mut di = super::Xk213DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_213_di_remove() {
        let mut di = super::Xk213DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_213_di_covered_length() {
        let mut di = super::Xk213DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_213_di_gaps() {
        let mut di = super::Xk213DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_213_di_merge_adjacent() {
        let mut di = super::Xk213DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_213_di_empty() {
        let di = super::Xk213DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
