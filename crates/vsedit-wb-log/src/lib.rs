//! Workbench logging.

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

/// A single log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
    pub timestamp: u64,
}

/// Service for log workbench functionality.
#[derive(Debug)]
pub struct LogService {
    pub entries: Vec<LogEntry>,
    pub min_level: LogLevel,
}

impl LogService {
    pub fn new(min_level: LogLevel) -> Self {
        Self {
            entries: Vec::new(),
            min_level,
        }
    }

    pub fn log(&mut self, level: LogLevel, message: impl Into<String>, source: Option<String>) {
        if level >= self.min_level {
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

    pub fn get_entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn get_entries_by_level(&self, level: LogLevel) -> Vec<&LogEntry> {
        self.entries.iter().filter(|e| e.level == level).collect()
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
}
