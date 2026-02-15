//! Structured logging service for vsedit.
//!
//! Equivalent to VS Code's `vs/platform/log/common/log.ts`.
//! Wraps the `tracing` crate to provide VS Code-compatible log levels and output channels.

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
}
