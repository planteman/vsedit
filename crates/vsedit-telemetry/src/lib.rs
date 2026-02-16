//! Telemetry service.

use std::fmt;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryLevel {
    Off,
    Crash,
    Error,
    Usage,
}

impl fmt::Display for TelemetryLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelemetryLevel::Off => write!(f, "Off"),
            TelemetryLevel::Crash => write!(f, "Crash"),
            TelemetryLevel::Error => write!(f, "Error"),
            TelemetryLevel::Usage => write!(f, "Usage"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryEventType {
    Event,
    Error,
    Exception,
    Metric,
}

impl fmt::Display for TelemetryEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelemetryEventType::Event => write!(f, "Event"),
            TelemetryEventType::Error => write!(f, "Error"),
            TelemetryEventType::Exception => write!(f, "Exception"),
            TelemetryEventType::Metric => write!(f, "Metric"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TelemetryEvent {
    pub name: String,
    pub event_type: TelemetryEventType,
    pub properties: Vec<(String, String)>,
    pub measurements: Vec<(String, f64)>,
    pub timestamp: u64,
}

pub struct TelemetryService {
    events: Vec<TelemetryEvent>,
    level: TelemetryLevel,
    enabled: bool,
}

impl TelemetryService {
    pub fn new(level: TelemetryLevel) -> Self {
        let enabled = level != TelemetryLevel::Off;
        Self {
            events: Vec::new(),
            level,
            enabled,
        }
    }

    pub fn log_event(
        &mut self,
        name: impl Into<String>,
        properties: Vec<(String, String)>,
        measurements: Vec<(String, f64)>,
    ) {
        if !self.enabled {
            return;
        }
        self.events.push(TelemetryEvent {
            name: name.into(),
            event_type: TelemetryEventType::Event,
            properties,
            measurements,
            timestamp: 0,
        });
    }

    pub fn log_error(
        &mut self,
        name: impl Into<String>,
        message: impl Into<String>,
        stack_trace: Option<String>,
    ) {
        if !self.enabled {
            return;
        }
        let mut properties = vec![("message".to_string(), message.into())];
        if let Some(trace) = stack_trace {
            properties.push(("stack_trace".to_string(), trace));
        }
        self.events.push(TelemetryEvent {
            name: name.into(),
            event_type: TelemetryEventType::Error,
            properties,
            measurements: vec![],
            timestamp: 0,
        });
    }

    pub fn log_exception(
        &mut self,
        name: impl Into<String>,
        message: impl Into<String>,
    ) {
        if !self.enabled {
            return;
        }
        let properties = vec![("message".to_string(), message.into())];
        self.events.push(TelemetryEvent {
            name: name.into(),
            event_type: TelemetryEventType::Exception,
            properties,
            measurements: vec![],
            timestamp: 0,
        });
    }

    pub fn log_metric(&mut self, name: impl Into<String>, value: f64) {
        if !self.enabled {
            return;
        }
        self.events.push(TelemetryEvent {
            name: name.into(),
            event_type: TelemetryEventType::Metric,
            properties: vec![],
            measurements: vec![("value".to_string(), value)],
            timestamp: 0,
        });
    }

    pub fn set_level(&mut self, level: TelemetryLevel) {
        self.enabled = level != TelemetryLevel::Off;
        self.level = level;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn get_events(&self) -> &[TelemetryEvent] {
        &self.events
    }

    pub fn get_events_by_type(&self, event_type: &TelemetryEventType) -> Vec<&TelemetryEvent> {
        self.events.iter().filter(|e| &e.event_type == event_type).collect()
    }

    /// Returns whether the current telemetry level permits logging the given event type.
    pub fn should_log(&self, event_type: &TelemetryEventType) -> bool {
        match self.level {
            TelemetryLevel::Off => false,
            TelemetryLevel::Crash => matches!(event_type, TelemetryEventType::Exception),
            TelemetryLevel::Error => matches!(
                event_type,
                TelemetryEventType::Error | TelemetryEventType::Exception
            ),
            TelemetryLevel::Usage => true,
        }
    }

    pub fn flush(&mut self) -> Vec<TelemetryEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

// --- Error types ---

/// Errors that can occur during telemetry operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryError {
    /// The event name was empty or invalid.
    InvalidEventName(String),
    /// A measurement value was NaN or infinite.
    InvalidMeasurement { key: String, value: f64 },
    /// The service is disabled.
    ServiceDisabled,
    /// The event exceeds the maximum allowed property count.
    TooManyProperties { count: usize, max: usize },
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelemetryError::InvalidEventName(name) => {
                write!(f, "invalid event name: '{}'", name)
            }
            TelemetryError::InvalidMeasurement { key, value } => {
                write!(f, "invalid measurement '{}': {}", key, value)
            }
            TelemetryError::ServiceDisabled => write!(f, "telemetry service is disabled"),
            TelemetryError::TooManyProperties { count, max } => {
                write!(f, "too many properties: {} (max {})", count, max)
            }
        }
    }
}

impl std::error::Error for TelemetryError {}

// --- PartialEq for TelemetryEvent ---

impl PartialEq for TelemetryEvent {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.event_type == other.event_type
            && self.properties == other.properties
            && self.timestamp == other.timestamp
    }
}

// --- Display for TelemetryEvent ---

impl fmt::Display for TelemetryEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} (props={}, measurements={})",
            self.event_type,
            self.name,
            self.properties.len(),
            self.measurements.len(),
        )
    }
}

// --- TelemetryEventBuilder ---

/// Builder for constructing [`TelemetryEvent`] instances with validation.
#[derive(Debug, Clone)]
pub struct TelemetryEventBuilder {
    name: Option<String>,
    event_type: TelemetryEventType,
    properties: Vec<(String, String)>,
    measurements: Vec<(String, f64)>,
    timestamp: u64,
}

const MAX_PROPERTIES: usize = 64;

impl TelemetryEventBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            event_type: TelemetryEventType::Event,
            properties: Vec::new(),
            measurements: Vec::new(),
            timestamp: 0,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn event_type(mut self, event_type: TelemetryEventType) -> Self {
        self.event_type = event_type;
        self
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push((key.into(), value.into()));
        self
    }

    pub fn measurement(mut self, key: impl Into<String>, value: f64) -> Self {
        self.measurements.push((key.into(), value));
        self
    }

    pub fn timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    /// Validate and build the event, returning an error if any field is invalid.
    pub fn build(self) -> Result<TelemetryEvent, TelemetryError> {
        let name = self.name.unwrap_or_default();
        if name.is_empty() {
            return Err(TelemetryError::InvalidEventName(name));
        }
        if self.properties.len() > MAX_PROPERTIES {
            return Err(TelemetryError::TooManyProperties {
                count: self.properties.len(),
                max: MAX_PROPERTIES,
            });
        }
        for (key, value) in &self.measurements {
            if value.is_nan() || value.is_infinite() {
                return Err(TelemetryError::InvalidMeasurement {
                    key: key.clone(),
                    value: *value,
                });
            }
        }
        Ok(TelemetryEvent {
            name,
            event_type: self.event_type,
            properties: self.properties,
            measurements: self.measurements,
            timestamp: self.timestamp,
        })
    }
}

impl Default for TelemetryEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// --- TelemetrySummary ---

/// Aggregated summary statistics for a batch of telemetry events.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetrySummary {
    pub total_events: usize,
    pub counts_by_type: HashMap<String, usize>,
    pub measurement_sums: HashMap<String, f64>,
    pub measurement_counts: HashMap<String, usize>,
}

impl TelemetrySummary {
    /// Compute a summary from a slice of events.
    pub fn from_events(events: &[TelemetryEvent]) -> Self {
        let mut counts_by_type: HashMap<String, usize> = HashMap::new();
        let mut measurement_sums: HashMap<String, f64> = HashMap::new();
        let mut measurement_counts: HashMap<String, usize> = HashMap::new();

        for event in events {
            *counts_by_type
                .entry(event.event_type.to_string())
                .or_insert(0) += 1;
            for (key, value) in &event.measurements {
                *measurement_sums.entry(key.clone()).or_insert(0.0) += value;
                *measurement_counts.entry(key.clone()).or_insert(0) += 1;
            }
        }

        Self {
            total_events: events.len(),
            counts_by_type,
            measurement_sums,
            measurement_counts,
        }
    }

    /// Returns the average value for a given measurement key, or `None` if absent.
    pub fn measurement_avg(&self, key: &str) -> Option<f64> {
        let sum = self.measurement_sums.get(key)?;
        let count = self.measurement_counts.get(key)?;
        if *count == 0 {
            return None;
        }
        Some(sum / *count as f64)
    }
}

impl fmt::Display for TelemetrySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TelemetrySummary({} events", self.total_events)?;
        for (k, v) in &self.counts_by_type {
            write!(f, ", {}={}", k, v)?;
        }
        write!(f, ")")
    }
}

// --- Additional TelemetryService methods ---

impl TelemetryService {
    /// Validated event logging that returns errors on invalid input.
    pub fn log_event_validated(
        &mut self,
        name: impl Into<String>,
        properties: Vec<(String, String)>,
        measurements: Vec<(String, f64)>,
    ) -> Result<(), TelemetryError> {
        if !self.enabled {
            return Err(TelemetryError::ServiceDisabled);
        }
        let event = TelemetryEventBuilder::new()
            .name(name)
            .event_type(TelemetryEventType::Event)
            .build()
            .map(|mut e| {
                e.properties = properties;
                e.measurements = measurements;
                e
            })?;
        // Validate measurements on the final event
        for (key, value) in &event.measurements {
            if value.is_nan() || value.is_infinite() {
                return Err(TelemetryError::InvalidMeasurement {
                    key: key.clone(),
                    value: *value,
                });
            }
        }
        self.events.push(event);
        Ok(())
    }

    /// Returns a summary of all currently buffered events.
    pub fn summarize(&self) -> TelemetrySummary {
        TelemetrySummary::from_events(&self.events)
    }

    /// Returns the current telemetry level.
    pub fn level(&self) -> &TelemetryLevel {
        &self.level
    }

    /// Clears all buffered events without returning them.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Returns events whose name contains the given substring.
    pub fn search_events(&self, substring: &str) -> Vec<&TelemetryEvent> {
        self.events
            .iter()
            .filter(|e| e.name.contains(substring))
            .collect()
    }
}

// --- Debug for TelemetryService ---

impl fmt::Debug for TelemetryService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelemetryService")
            .field("level", &self.level)
            .field("enabled", &self.enabled)
            .field("buffered_events", &self.events.len())
            .finish()
    }
}

impl fmt::Display for TelemetryService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TelemetryService(level={}, enabled={}, events={})",
            self.level,
            self.enabled,
            self.events.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_when_enabled() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        assert!(svc.is_enabled());
        svc.log_event("open_file", vec![], vec![]);
        assert_eq!(svc.event_count(), 1);
    }

    #[test]
    fn skip_when_off() {
        let mut svc = TelemetryService::new(TelemetryLevel::Off);
        assert!(!svc.is_enabled());
        svc.log_event("open_file", vec![], vec![]);
        assert_eq!(svc.event_count(), 0);
    }

    #[test]
    fn flush_drains_events() {
        let mut svc = TelemetryService::new(TelemetryLevel::Error);
        svc.log_event("err1", vec![], vec![]);
        svc.log_event("err2", vec![], vec![]);
        let flushed = svc.flush();
        assert_eq!(flushed.len(), 2);
        assert_eq!(svc.event_count(), 0);
    }

    #[test]
    fn log_error_with_stack_trace() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_error("io_error", "file not found", Some("at main:10".to_string()));
        assert_eq!(svc.event_count(), 1);
        let ev = &svc.get_events()[0];
        assert_eq!(ev.event_type, TelemetryEventType::Error);
        assert_eq!(ev.properties.len(), 2);
        assert_eq!(ev.properties[1].1, "at main:10");
    }

    #[test]
    fn log_error_without_stack_trace() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_error("io_error", "permission denied", None);
        let ev = &svc.get_events()[0];
        assert_eq!(ev.properties.len(), 1);
    }

    #[test]
    fn log_exception_records_correctly() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_exception("panic", "index out of bounds");
        assert_eq!(svc.event_count(), 1);
        let ev = &svc.get_events()[0];
        assert_eq!(ev.event_type, TelemetryEventType::Exception);
        assert_eq!(ev.properties[0].1, "index out of bounds");
    }

    #[test]
    fn log_metric_records_value() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_metric("latency_ms", 42.5);
        assert_eq!(svc.event_count(), 1);
        let ev = &svc.get_events()[0];
        assert_eq!(ev.event_type, TelemetryEventType::Metric);
        assert!((ev.measurements[0].1 - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn get_events_by_type_filters() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("evt1", vec![], vec![]);
        svc.log_error("err1", "oops", None);
        svc.log_metric("m1", 1.0);
        svc.log_event("evt2", vec![], vec![]);
        assert_eq!(svc.get_events_by_type(&TelemetryEventType::Event).len(), 2);
        assert_eq!(svc.get_events_by_type(&TelemetryEventType::Error).len(), 1);
        assert_eq!(svc.get_events_by_type(&TelemetryEventType::Metric).len(), 1);
        assert_eq!(svc.get_events_by_type(&TelemetryEventType::Exception).len(), 0);
    }

    #[test]
    fn should_log_respects_level() {
        let svc_off = TelemetryService::new(TelemetryLevel::Off);
        assert!(!svc_off.should_log(&TelemetryEventType::Event));

        let svc_crash = TelemetryService::new(TelemetryLevel::Crash);
        assert!(svc_crash.should_log(&TelemetryEventType::Exception));
        assert!(!svc_crash.should_log(&TelemetryEventType::Error));
        assert!(!svc_crash.should_log(&TelemetryEventType::Event));

        let svc_error = TelemetryService::new(TelemetryLevel::Error);
        assert!(svc_error.should_log(&TelemetryEventType::Error));
        assert!(svc_error.should_log(&TelemetryEventType::Exception));
        assert!(!svc_error.should_log(&TelemetryEventType::Event));

        let svc_usage = TelemetryService::new(TelemetryLevel::Usage);
        assert!(svc_usage.should_log(&TelemetryEventType::Event));
        assert!(svc_usage.should_log(&TelemetryEventType::Metric));
    }

    #[test]
    fn display_impls() {
        assert_eq!(TelemetryLevel::Off.to_string(), "Off");
        assert_eq!(TelemetryLevel::Crash.to_string(), "Crash");
        assert_eq!(TelemetryLevel::Error.to_string(), "Error");
        assert_eq!(TelemetryLevel::Usage.to_string(), "Usage");
        assert_eq!(TelemetryEventType::Event.to_string(), "Event");
        assert_eq!(TelemetryEventType::Error.to_string(), "Error");
        assert_eq!(TelemetryEventType::Exception.to_string(), "Exception");
        assert_eq!(TelemetryEventType::Metric.to_string(), "Metric");
    }

    // --- New tests ---

    #[test]
    fn builder_creates_valid_event() {
        let event = TelemetryEventBuilder::new()
            .name("test_event")
            .event_type(TelemetryEventType::Metric)
            .property("env", "staging")
            .measurement("latency", 12.5)
            .timestamp(1000)
            .build()
            .unwrap();
        assert_eq!(event.name, "test_event");
        assert_eq!(event.event_type, TelemetryEventType::Metric);
        assert_eq!(event.properties.len(), 1);
        assert_eq!(event.measurements.len(), 1);
        assert_eq!(event.timestamp, 1000);
    }

    #[test]
    fn builder_rejects_empty_name() {
        let result = TelemetryEventBuilder::new().build();
        assert_eq!(
            result.unwrap_err(),
            TelemetryError::InvalidEventName(String::new())
        );
    }

    #[test]
    fn builder_rejects_nan_measurement() {
        let result = TelemetryEventBuilder::new()
            .name("evt")
            .measurement("bad", f64::NAN)
            .build();
        assert!(matches!(
            result,
            Err(TelemetryError::InvalidMeasurement { .. })
        ));
    }

    #[test]
    fn builder_rejects_infinite_measurement() {
        let result = TelemetryEventBuilder::new()
            .name("evt")
            .measurement("bad", f64::INFINITY)
            .build();
        assert!(matches!(
            result,
            Err(TelemetryError::InvalidMeasurement { .. })
        ));
    }

    #[test]
    fn telemetry_error_display() {
        let err = TelemetryError::ServiceDisabled;
        assert_eq!(err.to_string(), "telemetry service is disabled");

        let err2 = TelemetryError::TooManyProperties { count: 100, max: 64 };
        assert!(err2.to_string().contains("100"));
    }

    #[test]
    fn event_display_impl() {
        let event = TelemetryEventBuilder::new()
            .name("startup")
            .property("version", "1.0")
            .measurement("boot_ms", 320.0)
            .build()
            .unwrap();
        let display = event.to_string();
        assert!(display.contains("startup"));
        assert!(display.contains("Event"));
    }

    #[test]
    fn event_partial_eq() {
        let a = TelemetryEventBuilder::new()
            .name("evt")
            .timestamp(5)
            .build()
            .unwrap();
        let b = TelemetryEventBuilder::new()
            .name("evt")
            .timestamp(5)
            .build()
            .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn log_event_validated_rejects_when_disabled() {
        let mut svc = TelemetryService::new(TelemetryLevel::Off);
        let result = svc.log_event_validated("evt", vec![], vec![]);
        assert_eq!(result.unwrap_err(), TelemetryError::ServiceDisabled);
    }

    #[test]
    fn log_event_validated_rejects_nan() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        let result = svc.log_event_validated(
            "evt",
            vec![],
            vec![("bad".to_string(), f64::NAN)],
        );
        assert!(matches!(
            result,
            Err(TelemetryError::InvalidMeasurement { .. })
        ));
        assert_eq!(svc.event_count(), 0);
    }

    #[test]
    fn summarize_computes_counts_and_sums() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_metric("latency", 10.0);
        svc.log_metric("latency", 20.0);
        svc.log_event("click", vec![], vec![]);
        let summary = svc.summarize();
        assert_eq!(summary.total_events, 3);
        assert_eq!(summary.counts_by_type.get("Metric"), Some(&2));
        assert_eq!(summary.counts_by_type.get("Event"), Some(&1));
        let avg = summary.measurement_avg("value").unwrap();
        assert!((avg - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_avg_returns_none_for_missing_key() {
        let summary = TelemetrySummary::from_events(&[]);
        assert!(summary.measurement_avg("nonexistent").is_none());
    }

    #[test]
    fn search_events_filters_by_name() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("file.open", vec![], vec![]);
        svc.log_event("file.close", vec![], vec![]);
        svc.log_event("editor.save", vec![], vec![]);
        assert_eq!(svc.search_events("file").len(), 2);
        assert_eq!(svc.search_events("save").len(), 1);
        assert_eq!(svc.search_events("missing").len(), 0);
    }

    #[test]
    fn clear_removes_all_events() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("a", vec![], vec![]);
        svc.log_event("b", vec![], vec![]);
        assert_eq!(svc.event_count(), 2);
        svc.clear();
        assert_eq!(svc.event_count(), 0);
    }

    #[test]
    fn service_debug_and_display() {
        let svc = TelemetryService::new(TelemetryLevel::Crash);
        let debug = format!("{:?}", svc);
        assert!(debug.contains("TelemetryService"));
        assert!(debug.contains("Crash"));
        let display = svc.to_string();
        assert!(display.contains("Crash"));
        assert!(display.contains("enabled=true"));
    }

    #[test]
    fn level_accessor() {
        let svc = TelemetryService::new(TelemetryLevel::Error);
        assert_eq!(*svc.level(), TelemetryLevel::Error);
    }
}
