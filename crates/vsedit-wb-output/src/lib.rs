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
}
