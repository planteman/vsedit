//! Telemetry service.

use std::fmt;

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
}
