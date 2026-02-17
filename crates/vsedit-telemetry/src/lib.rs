//! Telemetry service.

use std::fmt;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryLevel {
    Off,
    Crash,
    Error,
    /// All telemetry events (also known as "Usage" level).
    Usage,
}

impl TelemetryLevel {
    /// Alias for `Usage` matching the VS Code "All" telemetry level.
    pub fn all() -> Self {
        Self::Usage
    }
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
            timestamp: now_epoch_ms(),
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
            timestamp: now_epoch_ms(),
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
            timestamp: now_epoch_ms(),
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
            timestamp: now_epoch_ms(),
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
    pub error_count: usize,
    pub exception_count: usize,
    pub metric_count: usize,
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
            error_count: events.iter().filter(|e| e.event_type == TelemetryEventType::Error).count(),
            exception_count: events.iter().filter(|e| e.event_type == TelemetryEventType::Exception).count(),
            metric_count: events.iter().filter(|e| e.event_type == TelemetryEventType::Metric).count(),
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

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

// --- get_events_since ---

impl TelemetryService {
    /// Returns events recorded since the given timestamp (milliseconds since epoch).
    pub fn get_events_since(&self, since_ms: u64) -> Vec<&TelemetryEvent> {
        self.events
            .iter()
            .filter(|e| e.timestamp >= since_ms)
            .collect()
    }
}

// --- ErrorTelemetry ---

/// Aggregated error telemetry entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorTelemetry {
    pub error_name: String,
    pub message: String,
    pub stack: Option<String>,
    pub count: usize,
    pub first_seen: u64,
    pub last_seen: u64,
}

impl fmt::Display for ErrorTelemetry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ErrorTelemetry({}: {} x{})",
            self.error_name, self.message, self.count
        )
    }
}

impl TelemetryService {
    /// Aggregate error events by name, returning a summary of each distinct error.
    pub fn get_error_summary(&self) -> Vec<ErrorTelemetry> {
        let mut map: HashMap<String, ErrorTelemetry> = HashMap::new();

        for event in &self.events {
            if event.event_type != TelemetryEventType::Error
                && event.event_type != TelemetryEventType::Exception
            {
                continue;
            }
            let message = event
                .properties
                .iter()
                .find(|(k, _)| k == "message")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let stack = event
                .properties
                .iter()
                .find(|(k, _)| k == "stack_trace")
                .map(|(_, v)| v.clone());

            let entry = map.entry(event.name.clone()).or_insert_with(|| ErrorTelemetry {
                error_name: event.name.clone(),
                message: message.clone(),
                stack: stack.clone(),
                count: 0,
                first_seen: event.timestamp,
                last_seen: event.timestamp,
            });
            entry.count += 1;
            if event.timestamp < entry.first_seen {
                entry.first_seen = event.timestamp;
            }
            if event.timestamp > entry.last_seen {
                entry.last_seen = event.timestamp;
            }
        }

        let mut result: Vec<ErrorTelemetry> = map.into_values().collect();
        result.sort_by(|a, b| b.count.cmp(&a.count));
        result
    }
}

// --- TelemetryAggregator ---

/// Collects events and produces summaries.
pub struct TelemetryAggregator {
    pub events: Vec<TelemetryEvent>,
}

impl TelemetryAggregator {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn add_event(&mut self, event: TelemetryEvent) {
        self.events.push(event);
    }

    pub fn add_events(&mut self, events: Vec<TelemetryEvent>) {
        self.events.extend(events);
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub fn counts_by_type(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for event in &self.events {
            *map.entry(event.event_type.to_string()).or_insert(0) += 1;
        }
        map
    }

    pub fn counts_by_name(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for event in &self.events {
            *map.entry(event.name.clone()).or_insert(0) += 1;
        }
        map
    }

    pub fn average_duration(&self, measurement_key: &str) -> Option<f64> {
        let mut sum = 0.0;
        let mut count = 0usize;
        for event in &self.events {
            for (key, value) in &event.measurements {
                if key == measurement_key {
                    sum += value;
                    count += 1;
                }
            }
        }
        if count == 0 {
            None
        } else {
            Some(sum / count as f64)
        }
    }

    pub fn summarize(&self) -> TelemetrySummary {
        TelemetrySummary::from_events(&self.events)
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn drain(&mut self) -> Vec<TelemetryEvent> {
        std::mem::take(&mut self.events)
    }
}

// --- TelemetryLevel::is_more_permissive_than ---

impl TelemetryLevel {
    /// Returns `true` if `self` permits strictly more event types than `other`.
    ///
    /// Permissiveness order: Off < Crash < Error < Usage.
    pub fn is_more_permissive_than(&self, other: &TelemetryLevel) -> bool {
        self.ordinal() > other.ordinal()
    }

    fn ordinal(&self) -> u8 {
        match self {
            TelemetryLevel::Off => 0,
            TelemetryLevel::Crash => 1,
            TelemetryLevel::Error => 2,
            TelemetryLevel::Usage => 3,
        }
    }
}

// --- TelemetryEventType::is_error_type ---

impl TelemetryEventType {
    /// Returns `true` for `Error` and `Exception` variants.
    pub fn is_error_type(&self) -> bool {
        matches!(self, TelemetryEventType::Error | TelemetryEventType::Exception)
    }
}

// --- Additional TelemetryService query methods ---

impl TelemetryService {
    /// Returns the number of events with type `Error`.
    pub fn error_count(&self) -> usize {
        self.events.iter().filter(|e| e.event_type == TelemetryEventType::Error).count()
    }

    /// Returns the number of events with type `Exception`.
    pub fn exception_count(&self) -> usize {
        self.events.iter().filter(|e| e.event_type == TelemetryEventType::Exception).count()
    }

    /// Returns events whose timestamp is >= `timestamp`.
    pub fn events_since(&self, timestamp: u64) -> Vec<&TelemetryEvent> {
        self.events.iter().filter(|e| e.timestamp >= timestamp).collect()
    }

    /// Returns a reference to the most recently recorded event, if any.
    pub fn last_event(&self) -> Option<&TelemetryEvent> {
        self.events.last()
    }
}

// --- TelemetrySummary::from_service ---

impl TelemetrySummary {
    /// Build a summary from a `TelemetryService`, including convenience counts.
    pub fn from_service(service: &TelemetryService) -> Self {
        let mut summary = Self::from_events(service.get_events());
        // Ensure error_count / exception_count / metric_count are present in counts_by_type
        summary.error_count = service.error_count();
        summary.exception_count = service.exception_count();
        summary.metric_count = service
            .get_events()
            .iter()
            .filter(|e| e.event_type == TelemetryEventType::Metric)
            .count();
        summary
    }
}

// --- TelemetryFilter ---

/// Suppresses events by criteria.
pub struct TelemetryFilter {
    pub suppressed_types: Vec<TelemetryEventType>,
    pub suppressed_names: Vec<String>,
    pub min_level: Option<TelemetryLevel>,
}

impl TelemetryFilter {
    pub fn new() -> Self {
        Self {
            suppressed_types: Vec::new(),
            suppressed_names: Vec::new(),
            min_level: None,
        }
    }

    pub fn suppress_type(mut self, event_type: TelemetryEventType) -> Self {
        self.suppressed_types.push(event_type);
        self
    }

    pub fn suppress_name(mut self, name: impl Into<String>) -> Self {
        self.suppressed_names.push(name.into());
        self
    }

    pub fn should_allow(&self, event: &TelemetryEvent) -> bool {
        if self.suppressed_types.contains(&event.event_type) {
            return false;
        }
        if self.suppressed_names.contains(&event.name) {
            return false;
        }
        true
    }

    pub fn filter_events<'a>(&self, events: &'a [TelemetryEvent]) -> Vec<&'a TelemetryEvent> {
        events.iter().filter(|e| self.should_allow(e)).collect()
    }
}

// ── TelemetryBatchExporter ──

/// Batch and export telemetry events.
pub struct TelemetryBatchExporter {
    batch: Vec<TelemetryEvent>,
    max_batch_size: usize,
    exported_count: usize,
}

impl TelemetryBatchExporter {
    pub fn new(max_batch_size: usize) -> Self {
        Self {
            batch: Vec::new(),
            max_batch_size,
            exported_count: 0,
        }
    }

    /// Add an event to the batch. Returns `true` if the batch is now full.
    pub fn add(&mut self, event: TelemetryEvent) -> bool {
        self.batch.push(event);
        self.batch.len() >= self.max_batch_size
    }

    /// Drain the current batch, returning all queued events.
    pub fn drain(&mut self) -> Vec<TelemetryEvent> {
        self.exported_count += self.batch.len();
        std::mem::take(&mut self.batch)
    }

    /// Returns the number of events in the current batch.
    pub fn pending_count(&self) -> usize {
        self.batch.len()
    }

    /// Returns the total number of events exported (drained) so far.
    pub fn total_exported(&self) -> usize {
        self.exported_count
    }

    /// Returns true if the batch is at capacity.
    pub fn is_full(&self) -> bool {
        self.batch.len() >= self.max_batch_size
    }
}

// ── TelemetryRateLimiter ──

/// Rate limiter for telemetry events using a simple sliding window.
pub struct TelemetryRateLimiter {
    window_ms: u64,
    max_events: usize,
    timestamps: Vec<u64>,
}

impl TelemetryRateLimiter {
    pub fn new(window_ms: u64, max_events: usize) -> Self {
        Self {
            window_ms,
            max_events,
            timestamps: Vec::new(),
        }
    }

    /// Check if an event at the given timestamp should be allowed.
    pub fn should_allow(&mut self, timestamp_ms: u64) -> bool {
        // Remove expired timestamps
        let cutoff = timestamp_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&ts| ts > cutoff);
        if self.timestamps.len() >= self.max_events {
            return false;
        }
        self.timestamps.push(timestamp_ms);
        true
    }

    /// Returns how many events have been recorded in the current window.
    pub fn current_count(&self) -> usize {
        self.timestamps.len()
    }

    /// Returns the number of remaining events allowed in the current window.
    pub fn remaining(&self) -> usize {
        self.max_events.saturating_sub(self.timestamps.len())
    }

    /// Reset the rate limiter.
    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

// ── TelemetryMetricsBucket ──

/// Time-bucketed metrics accumulator.
#[derive(Debug, Clone)]
pub struct TelemetryMetricsBucket {
    bucket_duration_ms: u64,
    buckets: HashMap<u64, Vec<f64>>,
}

impl TelemetryMetricsBucket {
    pub fn new(bucket_duration_ms: u64) -> Self {
        Self {
            bucket_duration_ms: bucket_duration_ms.max(1),
            buckets: HashMap::new(),
        }
    }

    /// Record a value at the given timestamp.
    pub fn record(&mut self, timestamp_ms: u64, value: f64) {
        let bucket_key = timestamp_ms / self.bucket_duration_ms;
        self.buckets.entry(bucket_key).or_default().push(value);
    }

    /// Get the average value for a specific bucket.
    pub fn bucket_avg(&self, timestamp_ms: u64) -> Option<f64> {
        let bucket_key = timestamp_ms / self.bucket_duration_ms;
        self.buckets.get(&bucket_key).map(|values| {
            values.iter().sum::<f64>() / values.len() as f64
        })
    }

    /// Get the sum of all values across all buckets.
    pub fn total_sum(&self) -> f64 {
        self.buckets.values().flat_map(|v| v.iter()).sum()
    }

    /// Get the total count of all recorded values.
    pub fn total_count(&self) -> usize {
        self.buckets.values().map(|v| v.len()).sum()
    }

    /// Returns the number of time buckets.
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    /// Get min and max values across all buckets.
    pub fn min_max(&self) -> Option<(f64, f64)> {
        let all: Vec<f64> = self.buckets.values().flat_map(|v| v.iter().copied()).collect();
        if all.is_empty() {
            return None;
        }
        let min = all.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = all.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Some((min, max))
    }
}

// ── Statistical functions on TelemetryAggregator ──

impl TelemetryAggregator {
    /// Compute the standard deviation of a measurement across aggregated events.
    pub fn measurement_stddev(&self, key: &str) -> Option<f64> {
        let values: Vec<f64> = self
            .events
            .iter()
            .flat_map(|e| e.measurements.iter())
            .filter(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .collect();
        if values.len() < 2 {
            return None;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        Some(variance.sqrt())
    }

    /// Compute the median of a measurement across aggregated events.
    pub fn measurement_median(&self, key: &str) -> Option<f64> {
        let mut values: Vec<f64> = self
            .events
            .iter()
            .flat_map(|e| e.measurements.iter())
            .filter(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .collect();
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = values.len() / 2;
        if values.len() % 2 == 0 {
            Some((values[mid - 1] + values[mid]) / 2.0)
        } else {
            Some(values[mid])
        }
    }

    /// Count how many events have a specific property key.
    pub fn count_with_property(&self, key: &str) -> usize {
        self.events
            .iter()
            .filter(|e| e.properties.iter().any(|(k, _)| k == key))
            .count()
    }
}

// ---------------------------------------------------------------------------
// Telemetry utility functions
// ---------------------------------------------------------------------------

/// Returns the names of all events in the service, in order.
pub fn event_names(svc: &TelemetryService) -> Vec<&str> {
    svc.get_events().iter().map(|e| e.name.as_str()).collect()
}

/// Returns only the events whose name starts with the given prefix.
pub fn events_with_prefix<'a>(
    events: &'a [TelemetryEvent],
    prefix: &str,
) -> Vec<&'a TelemetryEvent> {
    events
        .iter()
        .filter(|e| e.name.starts_with(prefix))
        .collect()
}

/// Returns the total of a named measurement across all events.
pub fn sum_measurement(events: &[TelemetryEvent], key: &str) -> f64 {
    events
        .iter()
        .flat_map(|e| &e.measurements)
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v)
        .sum()
}

/// Returns a de-duplicated sorted list of all property keys across events.
pub fn all_property_keys(events: &[TelemetryEvent]) -> Vec<String> {
    let mut keys: Vec<String> = events
        .iter()
        .flat_map(|e| e.properties.iter().map(|(k, _)| k.clone()))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Returns `true` if any event has the given property key with the given value.
pub fn has_property_value(events: &[TelemetryEvent], key: &str, value: &str) -> bool {
    events
        .iter()
        .any(|e| e.properties.iter().any(|(k, v)| k == key && v == value))
}

/// Returns the count of events per event type.
pub fn count_by_type(events: &[TelemetryEvent]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for e in events {
        *map.entry(format!("{}", e.event_type)).or_insert(0) += 1;
    }
    map
}

/// Returns the most recent event (highest timestamp), or `None` if empty.
pub fn most_recent_event(events: &[TelemetryEvent]) -> Option<&TelemetryEvent> {
    events.iter().max_by_key(|e| e.timestamp)
}

/// Group events by name, returning a map of event name to count.
pub fn events_grouped_by_name(events: &[TelemetryEvent]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for e in events { *map.entry(e.name.clone()).or_insert(0) += 1; }
    map
}

/// Return events within a given timestamp range (inclusive).
pub fn events_in_time_range<'a>(events: &'a [TelemetryEvent], start_ms: u64, end_ms: u64) -> Vec<&'a TelemetryEvent> {
    events.iter().filter(|e| e.timestamp >= start_ms && e.timestamp <= end_ms).collect()
}

/// Return the time span (in ms) between earliest and latest events.
pub fn event_time_span(events: &[TelemetryEvent]) -> u64 {
    if events.len() < 2 { return 0; }
    let min_ts = events.iter().map(|e| e.timestamp).min().unwrap_or(0);
    let max_ts = events.iter().map(|e| e.timestamp).max().unwrap_or(0);
    max_ts.saturating_sub(min_ts)
}

/// Return the average measurement value for a given key.
pub fn avg_measurement(events: &[TelemetryEvent], key: &str) -> Option<f64> {
    let values: Vec<f64> = events.iter().flat_map(|e| &e.measurements).filter(|(k, _)| k == key).map(|(_, v)| *v).collect();
    if values.is_empty() { return None; }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Return the min and max measurement values for a given key.
pub fn measurement_min_max(events: &[TelemetryEvent], key: &str) -> Option<(f64, f64)> {
    let values: Vec<f64> = events.iter().flat_map(|e| &e.measurements).filter(|(k, _)| k == key).map(|(_, v)| *v).collect();
    if values.is_empty() { return None; }
    Some((values.iter().cloned().fold(f64::INFINITY, f64::min), values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)))
}

/// Return all distinct event names sorted alphabetically.
pub fn distinct_event_names(events: &[TelemetryEvent]) -> Vec<String> {
    let mut names: Vec<String> = events.iter().map(|e| e.name.clone()).collect();
    names.sort(); names.dedup(); names
}

/// Return events that have at least one measurement.
pub fn events_with_measurements<'a>(events: &'a [TelemetryEvent]) -> Vec<&'a TelemetryEvent> {
    events.iter().filter(|e| !e.measurements.is_empty()).collect()
}

/// Return events that have a specific property key.
pub fn events_with_property_key<'a>(events: &'a [TelemetryEvent], key: &str) -> Vec<&'a TelemetryEvent> {
    events.iter().filter(|e| e.properties.iter().any(|(k, _)| k == key)).collect()
}

// ---------------------------------------------------------------------------
// TelemetryBatcher – batches events before sending
// ---------------------------------------------------------------------------

/// Batches telemetry events and flushes when the batch reaches capacity.
pub struct TelemetryBatcher {
    batch: Vec<TelemetryEvent>,
    capacity: usize,
    flushed_batches: Vec<Vec<TelemetryEvent>>,
}

impl TelemetryBatcher {
    /// Create a new batcher with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            batch: Vec::new(),
            capacity: capacity.max(1),
            flushed_batches: Vec::new(),
        }
    }

    /// Push an event. Returns `true` if a flush was triggered.
    pub fn push(&mut self, event: TelemetryEvent) -> bool {
        self.batch.push(event);
        if self.batch.len() >= self.capacity {
            self.flush();
            return true;
        }
        false
    }

    /// Force-flush the current batch.
    pub fn flush(&mut self) {
        if !self.batch.is_empty() {
            let batch = std::mem::take(&mut self.batch);
            self.flushed_batches.push(batch);
        }
    }

    /// Number of events in the current (unflushed) batch.
    pub fn pending_count(&self) -> usize {
        self.batch.len()
    }

    /// Number of batches that have been flushed so far.
    pub fn flushed_batch_count(&self) -> usize {
        self.flushed_batches.len()
    }

    /// Drain all flushed batches.
    pub fn drain_flushed(&mut self) -> Vec<Vec<TelemetryEvent>> {
        std::mem::take(&mut self.flushed_batches)
    }
}

// ---------------------------------------------------------------------------
// PiiScrubber – privacy-aware PII scrubbing
// ---------------------------------------------------------------------------

/// Rule describing a property key whose value should be redacted.
#[derive(Debug, Clone)]
pub struct PiiRule {
    /// Property key to match (case-insensitive).
    pub key_pattern: String,
    /// Replacement text.
    pub replacement: String,
}

/// Scrubs PII from telemetry event properties based on configurable rules.
pub struct PiiScrubber {
    rules: Vec<PiiRule>,
}

impl PiiScrubber {
    /// Create a scrubber with default PII rules (email, password, token, secret).
    pub fn with_defaults() -> Self {
        let defaults = ["email", "password", "token", "secret", "api_key"];
        let rules = defaults
            .iter()
            .map(|k| PiiRule {
                key_pattern: k.to_string(),
                replacement: "[REDACTED]".to_string(),
            })
            .collect();
        Self { rules }
    }

    /// Create an empty scrubber (no rules).
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a custom PII rule.
    pub fn add_rule(&mut self, key_pattern: impl Into<String>, replacement: impl Into<String>) {
        self.rules.push(PiiRule {
            key_pattern: key_pattern.into(),
            replacement: replacement.into(),
        });
    }

    /// Scrub a single event, returning a new event with PII values replaced.
    pub fn scrub(&self, event: &TelemetryEvent) -> TelemetryEvent {
        let mut scrubbed = event.clone();
        for (key, value) in scrubbed.properties.iter_mut() {
            for rule in &self.rules {
                if key.to_lowercase().contains(&rule.key_pattern.to_lowercase()) {
                    *value = rule.replacement.clone();
                }
            }
        }
        scrubbed
    }

    /// Scrub a batch of events.
    pub fn scrub_all(&self, events: &[TelemetryEvent]) -> Vec<TelemetryEvent> {
        events.iter().map(|e| self.scrub(e)).collect()
    }

    /// Number of rules registered.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

// ---------------------------------------------------------------------------
// MetricStore – histogram, counter, gauge collection
// ---------------------------------------------------------------------------

/// A simple counter metric.
#[derive(Debug, Clone)]
pub struct MetricCounter {
    pub name: String,
    pub value: u64,
}

/// A gauge metric (can go up and down).
#[derive(Debug, Clone)]
pub struct MetricGauge {
    pub name: String,
    pub value: f64,
}

/// A histogram metric that collects sample values.
#[derive(Debug, Clone)]
pub struct MetricHistogram {
    pub name: String,
    pub samples: Vec<f64>,
}

impl MetricHistogram {
    /// Create a new empty histogram.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), samples: Vec::new() }
    }

    /// Record a sample.
    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    /// Compute the mean of all samples.
    pub fn mean(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        Some(self.samples.iter().sum::<f64>() / self.samples.len() as f64)
    }

    /// Compute a percentile of all samples.
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    /// Minimum sample value.
    pub fn min_value(&self) -> Option<f64> {
        self.samples.iter().cloned().reduce(f64::min)
    }

    /// Maximum sample value.
    pub fn max_value(&self) -> Option<f64> {
        self.samples.iter().cloned().reduce(f64::max)
    }
}

/// Collects counters, gauges, and histograms.
pub struct MetricStore {
    counters: HashMap<String, u64>,
    gauges: HashMap<String, f64>,
    histograms: HashMap<String, MetricHistogram>,
}

impl MetricStore {
    /// Create a new empty metrics store.
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            gauges: HashMap::new(),
            histograms: HashMap::new(),
        }
    }

    /// Increment a counter by the given amount.
    pub fn increment_counter(&mut self, name: &str, amount: u64) {
        *self.counters.entry(name.to_string()).or_insert(0) += amount;
    }

    /// Get the current value of a counter.
    pub fn counter_value(&self, name: &str) -> u64 {
        self.counters.get(name).copied().unwrap_or(0)
    }

    /// Set a gauge to the given value.
    pub fn set_gauge(&mut self, name: &str, value: f64) {
        self.gauges.insert(name.to_string(), value);
    }

    /// Get the current value of a gauge.
    pub fn gauge_value(&self, name: &str) -> Option<f64> {
        self.gauges.get(name).copied()
    }

    /// Record a sample in a histogram.
    pub fn record_histogram(&mut self, name: &str, value: f64) {
        self.histograms
            .entry(name.to_string())
            .or_insert_with(|| MetricHistogram::new(name))
            .record(value);
    }

    /// Get a histogram by name.
    pub fn get_histogram(&self, name: &str) -> Option<&MetricHistogram> {
        self.histograms.get(name)
    }

    /// Total number of distinct metric names across all types.
    pub fn total_metric_count(&self) -> usize {
        self.counters.len() + self.gauges.len() + self.histograms.len()
    }
}

// ---------------------------------------------------------------------------
// SessionDurationTracker
// ---------------------------------------------------------------------------

/// Tracks session start/end times and computes durations.
pub struct SessionDurationTracker {
    sessions: HashMap<String, (u64, Option<u64>)>,
}

impl SessionDurationTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        Self { sessions: HashMap::new() }
    }

    /// Start a session at the given timestamp (ms).
    pub fn start_session(&mut self, id: impl Into<String>, start_ms: u64) {
        self.sessions.insert(id.into(), (start_ms, None));
    }

    /// End a session at the given timestamp (ms).
    pub fn end_session(&mut self, id: &str, end_ms: u64) -> Option<u64> {
        if let Some(entry) = self.sessions.get_mut(id) {
            entry.1 = Some(end_ms);
            Some(end_ms.saturating_sub(entry.0))
        } else {
            None
        }
    }

    /// Get the duration of a completed session.
    pub fn duration_ms(&self, id: &str) -> Option<u64> {
        self.sessions.get(id).and_then(|(start, end)| end.map(|e| e.saturating_sub(*start)))
    }

    /// Return the average duration of all completed sessions.
    pub fn average_duration_ms(&self) -> Option<f64> {
        let completed: Vec<u64> = self.sessions.values()
            .filter_map(|(start, end)| end.map(|e| e.saturating_sub(*start)))
            .collect();
        if completed.is_empty() {
            return None;
        }
        Some(completed.iter().sum::<u64>() as f64 / completed.len() as f64)
    }

    /// Number of active (not yet ended) sessions.
    pub fn active_count(&self) -> usize {
        self.sessions.values().filter(|(_, end)| end.is_none()).count()
    }

    /// Number of completed sessions.
    pub fn completed_count(&self) -> usize {
        self.sessions.values().filter(|(_, end)| end.is_some()).count()
    }
}


// === Telemetry Consent Manager ===

/// Telemetry Consent Manager implementation.
#[derive(Debug, Clone)]
pub struct TelemetryConsentManager {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TelemetryConsentManagerStats,
}

/// Statistics for TelemetryConsentManager.
#[derive(Debug, Clone, Default)]
pub struct TelemetryConsentManagerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TelemetryConsentManagerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TelemetryConsentManager {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TelemetryConsentManagerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TelemetryConsentManagerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TelemetryConsentManager {
    fn default() -> Self {
        Self::new()
    }
}

// === Telemetry Error Classifier ===

/// Priority level for TelemetryErrorClassifier items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TelemetryErrorClassifierPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TelemetryErrorClassifierPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TelemetryErrorClassifierPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Telemetry Error Classifier implementation.
#[derive(Debug, Clone)]
pub struct TelemetryErrorClassifier {
    items: Vec<TelemetryErrorClassifierItem>,
    max_items: usize,
    default_priority: TelemetryErrorClassifierPriority,
}

/// A single item in TelemetryErrorClassifier.
#[derive(Debug, Clone)]
pub struct TelemetryErrorClassifierItem {
    pub id: String,
    pub label: String,
    pub priority: TelemetryErrorClassifierPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TelemetryErrorClassifierItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TelemetryErrorClassifierPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TelemetryErrorClassifierPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TelemetryErrorClassifier {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TelemetryErrorClassifierPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TelemetryErrorClassifierItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TelemetryErrorClassifierItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TelemetryErrorClassifierItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TelemetryErrorClassifierPriority) -> Vec<&TelemetryErrorClassifierItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TelemetryErrorClassifierItem> {
        let mut sorted: Vec<&TelemetryErrorClassifierItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TelemetryErrorClassifierItem> {
        let mut sorted: Vec<&TelemetryErrorClassifierItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TelemetryErrorClassifierItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TelemetryErrorClassifierPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TelemetryErrorClassifierPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TelemetryErrorClassifierItem> {
        self.items.iter()
    }
}

impl Default for TelemetryErrorClassifier {
    fn default() -> Self {
        Self::new()
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

    // --- New feature tests ---

    #[test]
    fn telemetry_level_all_alias() {
        assert_eq!(TelemetryLevel::all(), TelemetryLevel::Usage);
    }

    #[test]
    fn events_have_timestamps() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("evt", vec![], vec![]);
        assert!(svc.get_events()[0].timestamp > 0);
    }

    #[test]
    fn get_events_since_filters_by_time() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("evt1", vec![], vec![]);
        let after_first = now_epoch_ms() + 1;
        // Manually push an event with a future timestamp
        svc.events.push(TelemetryEvent {
            name: "evt2".to_string(),
            event_type: TelemetryEventType::Event,
            properties: vec![],
            measurements: vec![],
            timestamp: after_first + 1000,
        });
        let recent = svc.get_events_since(after_first);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "evt2");
    }

    #[test]
    fn get_error_summary_aggregates() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_error("io_error", "file not found", None);
        svc.log_error("io_error", "file not found", None);
        svc.log_error("parse_error", "invalid syntax", Some("at line 5".to_string()));
        svc.log_event("normal_event", vec![], vec![]);

        let summary = svc.get_error_summary();
        assert_eq!(summary.len(), 2);
        // io_error has count 2, should be first (sorted by count desc)
        assert_eq!(summary[0].error_name, "io_error");
        assert_eq!(summary[0].count, 2);
        assert_eq!(summary[1].error_name, "parse_error");
        assert_eq!(summary[1].count, 1);
        assert_eq!(summary[1].stack.as_deref(), Some("at line 5"));
    }

    #[test]
    fn error_telemetry_display() {
        let et = ErrorTelemetry {
            error_name: "test_err".to_string(),
            message: "oops".to_string(),
            stack: None,
            count: 3,
            first_seen: 100,
            last_seen: 300,
        };
        let s = et.to_string();
        assert!(s.contains("test_err"));
        assert!(s.contains("x3"));
    }

    #[test]
    fn error_summary_includes_exceptions() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_exception("panic", "index out of bounds");
        let summary = svc.get_error_summary();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].error_name, "panic");
    }

    #[test]
    fn get_events_since_empty() {
        let svc = TelemetryService::new(TelemetryLevel::Usage);
        assert!(svc.get_events_since(0).is_empty());
    }

    #[test]
    fn error_summary_empty_when_no_errors() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("click", vec![], vec![]);
        assert!(svc.get_error_summary().is_empty());
    }

    // --- TelemetryAggregator tests ---

    fn make_event(name: &str, event_type: TelemetryEventType, measurements: Vec<(String, f64)>) -> TelemetryEvent {
        TelemetryEvent {
            name: name.to_string(),
            event_type,
            properties: vec![],
            measurements,
            timestamp: 1000,
        }
    }

    #[test]
    fn aggregator_add_and_count() {
        let mut agg = TelemetryAggregator::new();
        assert_eq!(agg.event_count(), 0);
        agg.add_event(make_event("a", TelemetryEventType::Event, vec![]));
        assert_eq!(agg.event_count(), 1);
        agg.add_events(vec![
            make_event("b", TelemetryEventType::Error, vec![]),
            make_event("c", TelemetryEventType::Metric, vec![]),
        ]);
        assert_eq!(agg.event_count(), 3);
    }

    #[test]
    fn aggregator_counts_by_type() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(make_event("a", TelemetryEventType::Event, vec![]));
        agg.add_event(make_event("b", TelemetryEventType::Event, vec![]));
        agg.add_event(make_event("c", TelemetryEventType::Error, vec![]));
        let counts = agg.counts_by_type();
        assert_eq!(counts.get("Event"), Some(&2));
        assert_eq!(counts.get("Error"), Some(&1));
    }

    #[test]
    fn aggregator_counts_by_name() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(make_event("click", TelemetryEventType::Event, vec![]));
        agg.add_event(make_event("click", TelemetryEventType::Event, vec![]));
        agg.add_event(make_event("scroll", TelemetryEventType::Event, vec![]));
        let counts = agg.counts_by_name();
        assert_eq!(counts.get("click"), Some(&2));
        assert_eq!(counts.get("scroll"), Some(&1));
    }

    #[test]
    fn aggregator_average_duration() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(make_event("a", TelemetryEventType::Metric, vec![("duration".to_string(), 10.0)]));
        agg.add_event(make_event("b", TelemetryEventType::Metric, vec![("duration".to_string(), 30.0)]));
        agg.add_event(make_event("c", TelemetryEventType::Event, vec![]));
        assert_eq!(agg.average_duration("duration"), Some(20.0));
        assert_eq!(agg.average_duration("missing"), None);
    }

    #[test]
    fn aggregator_summarize() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(make_event("a", TelemetryEventType::Metric, vec![("latency".to_string(), 5.0)]));
        agg.add_event(make_event("b", TelemetryEventType::Event, vec![]));
        let summary = agg.summarize();
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.counts_by_type.get("Metric"), Some(&1));
        assert_eq!(summary.counts_by_type.get("Event"), Some(&1));
        assert_eq!(summary.measurement_avg("latency"), Some(5.0));
    }

    #[test]
    fn aggregator_drain() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(make_event("a", TelemetryEventType::Event, vec![]));
        agg.add_event(make_event("b", TelemetryEventType::Event, vec![]));
        let drained = agg.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(agg.event_count(), 0);
    }

    // --- TelemetryFilter tests ---

    #[test]
    fn filter_suppress_type() {
        let filter = TelemetryFilter::new()
            .suppress_type(TelemetryEventType::Error);
        let allowed = make_event("a", TelemetryEventType::Event, vec![]);
        let blocked = make_event("b", TelemetryEventType::Error, vec![]);
        assert!(filter.should_allow(&allowed));
        assert!(!filter.should_allow(&blocked));
    }

    #[test]
    fn filter_suppress_name() {
        let filter = TelemetryFilter::new()
            .suppress_name("debug_ping");
        let allowed = make_event("click", TelemetryEventType::Event, vec![]);
        let blocked = make_event("debug_ping", TelemetryEventType::Event, vec![]);
        assert!(filter.should_allow(&allowed));
        assert!(!filter.should_allow(&blocked));
    }

    #[test]
    fn filter_events_combined() {
        let filter = TelemetryFilter::new()
            .suppress_type(TelemetryEventType::Exception)
            .suppress_name("noisy");
        let events = vec![
            make_event("ok", TelemetryEventType::Event, vec![]),
            make_event("crash", TelemetryEventType::Exception, vec![]),
            make_event("noisy", TelemetryEventType::Event, vec![]),
            make_event("metric", TelemetryEventType::Metric, vec![]),
        ];
        let filtered = filter.filter_events(&events);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "ok");
        assert_eq!(filtered[1].name, "metric");
    }

    // --- Tests for newly added functionality ---

    #[test]
    fn error_count_returns_only_errors() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_error("err1", "msg", None);
        svc.log_error("err2", "msg", None);
        svc.log_exception("exc1", "msg");
        svc.log_event("evt1", vec![], vec![]);
        assert_eq!(svc.error_count(), 2);
    }

    #[test]
    fn exception_count_returns_only_exceptions() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_exception("exc1", "msg");
        svc.log_exception("exc2", "msg");
        svc.log_error("err1", "msg", None);
        svc.log_event("evt1", vec![], vec![]);
        assert_eq!(svc.exception_count(), 2);
    }

    #[test]
    fn events_since_filters_by_timestamp() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.events.push(TelemetryEvent {
            name: "old".to_string(),
            event_type: TelemetryEventType::Event,
            properties: vec![],
            measurements: vec![],
            timestamp: 100,
        });
        svc.events.push(TelemetryEvent {
            name: "new".to_string(),
            event_type: TelemetryEventType::Event,
            properties: vec![],
            measurements: vec![],
            timestamp: 500,
        });
        let recent = svc.events_since(200);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "new");
    }

    #[test]
    fn last_event_returns_most_recent() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        assert!(svc.last_event().is_none());
        svc.log_event("first", vec![], vec![]);
        svc.log_event("second", vec![], vec![]);
        assert_eq!(svc.last_event().unwrap().name, "second");
    }

    #[test]
    fn telemetry_level_is_more_permissive_than() {
        assert!(TelemetryLevel::Usage.is_more_permissive_than(&TelemetryLevel::Error));
        assert!(TelemetryLevel::Error.is_more_permissive_than(&TelemetryLevel::Crash));
        assert!(TelemetryLevel::Crash.is_more_permissive_than(&TelemetryLevel::Off));
        assert!(!TelemetryLevel::Off.is_more_permissive_than(&TelemetryLevel::Off));
        assert!(!TelemetryLevel::Error.is_more_permissive_than(&TelemetryLevel::Usage));
    }

    #[test]
    fn event_type_is_error_type() {
        assert!(TelemetryEventType::Error.is_error_type());
        assert!(TelemetryEventType::Exception.is_error_type());
        assert!(!TelemetryEventType::Event.is_error_type());
        assert!(!TelemetryEventType::Metric.is_error_type());
    }

    #[test]
    fn telemetry_summary_from_service() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_error("e1", "msg", None);
        svc.log_error("e2", "msg", None);
        svc.log_exception("ex1", "msg");
        svc.log_metric("m1", 5.0);
        svc.log_metric("m2", 10.0);
        svc.log_event("ev1", vec![], vec![]);

        let summary = TelemetrySummary::from_service(&svc);
        assert_eq!(summary.total_events, 6);
        assert_eq!(summary.error_count, 2);
        assert_eq!(summary.exception_count, 1);
        assert_eq!(summary.metric_count, 2);
    }

    // ── New tests ──

    #[test]
    fn batch_exporter_add_and_drain() {
        let mut exporter = TelemetryBatchExporter::new(3);
        let event = || TelemetryEventBuilder::new().name("e").build().unwrap();
        assert!(!exporter.add(event()));
        assert!(!exporter.add(event()));
        assert!(exporter.add(event())); // batch is full
        assert_eq!(exporter.pending_count(), 3);
        assert!(exporter.is_full());
        let drained = exporter.drain();
        assert_eq!(drained.len(), 3);
        assert_eq!(exporter.pending_count(), 0);
        assert_eq!(exporter.total_exported(), 3);
    }

    #[test]
    fn rate_limiter_allows_within_limit() {
        let mut limiter = TelemetryRateLimiter::new(1000, 3);
        assert!(limiter.should_allow(100));
        assert!(limiter.should_allow(200));
        assert!(limiter.should_allow(300));
        assert!(!limiter.should_allow(400)); // exceeded
        assert_eq!(limiter.remaining(), 0);
    }

    #[test]
    fn rate_limiter_window_expiry() {
        let mut limiter = TelemetryRateLimiter::new(100, 2);
        assert!(limiter.should_allow(10));
        assert!(limiter.should_allow(20));
        assert!(!limiter.should_allow(30));
        // After window expires (all old timestamps <= 100 are removed)
        assert!(limiter.should_allow(200));
        assert_eq!(limiter.current_count(), 1);
    }

    #[test]
    fn metrics_bucket_record_and_avg() {
        let mut bucket = TelemetryMetricsBucket::new(1000);
        bucket.record(100, 10.0);
        bucket.record(200, 20.0);
        bucket.record(1500, 30.0); // different bucket
        assert_eq!(bucket.bucket_count(), 2);
        assert_eq!(bucket.bucket_avg(100), Some(15.0));
        assert_eq!(bucket.bucket_avg(1500), Some(30.0));
        assert_eq!(bucket.total_count(), 3);
        assert!((bucket.total_sum() - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn metrics_bucket_min_max() {
        let mut bucket = TelemetryMetricsBucket::new(1000);
        bucket.record(0, 5.0);
        bucket.record(0, 25.0);
        bucket.record(0, 15.0);
        let (min, max) = bucket.min_max().unwrap();
        assert!((min - 5.0).abs() < f64::EPSILON);
        assert!((max - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregator_measurement_stddev() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(TelemetryEventBuilder::new()
            .name("m1").measurement("latency", 10.0).build().unwrap());
        agg.add_event(TelemetryEventBuilder::new()
            .name("m2").measurement("latency", 20.0).build().unwrap());
        agg.add_event(TelemetryEventBuilder::new()
            .name("m3").measurement("latency", 30.0).build().unwrap());
        let sd = agg.measurement_stddev("latency").unwrap();
        // stddev of [10, 20, 30] = sqrt(200/3) ≈ 8.165
        assert!((sd - 8.165).abs() < 0.01);
    }

    #[test]
    fn aggregator_measurement_median() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(TelemetryEventBuilder::new()
            .name("a").measurement("val", 3.0).build().unwrap());
        agg.add_event(TelemetryEventBuilder::new()
            .name("b").measurement("val", 1.0).build().unwrap());
        agg.add_event(TelemetryEventBuilder::new()
            .name("c").measurement("val", 2.0).build().unwrap());
        assert_eq!(agg.measurement_median("val"), Some(2.0));
        assert_eq!(agg.measurement_median("nonexistent"), None);
    }

    #[test]
    fn aggregator_count_with_property() {
        let mut agg = TelemetryAggregator::new();
        agg.add_event(TelemetryEventBuilder::new()
            .name("a").property("source", "ui").build().unwrap());
        agg.add_event(TelemetryEventBuilder::new()
            .name("b").property("source", "api").build().unwrap());
        agg.add_event(TelemetryEventBuilder::new()
            .name("c").build().unwrap());
        assert_eq!(agg.count_with_property("source"), 2);
        assert_eq!(agg.count_with_property("missing"), 0);
    }

    // --- new tests ---

    #[test]
    fn test_event_names_empty() {
        let svc = TelemetryService::new(TelemetryLevel::Usage);
        assert!(event_names(&svc).is_empty());
    }

    #[test]
    fn test_event_names_populated() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("open", vec![], vec![]);
        svc.log_event("close", vec![], vec![]);
        assert_eq!(event_names(&svc), vec!["open", "close"]);
    }

    #[test]
    fn test_events_with_prefix() {
        let events = vec![
            TelemetryEventBuilder::new().name("editor.open").build().unwrap(),
            TelemetryEventBuilder::new().name("editor.close").build().unwrap(),
            TelemetryEventBuilder::new().name("terminal.open").build().unwrap(),
        ];
        let filtered = events_with_prefix(&events, "editor.");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_sum_measurement() {
        let events = vec![
            TelemetryEventBuilder::new().name("a").measurement("dur", 10.0).build().unwrap(),
            TelemetryEventBuilder::new().name("b").measurement("dur", 20.0).build().unwrap(),
            TelemetryEventBuilder::new().name("c").measurement("other", 99.0).build().unwrap(),
        ];
        assert!((sum_measurement(&events, "dur") - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sum_measurement_missing() {
        let events: Vec<TelemetryEvent> = vec![];
        assert!((sum_measurement(&events, "dur") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_all_property_keys() {
        let events = vec![
            TelemetryEventBuilder::new().name("a").property("src", "ui").build().unwrap(),
            TelemetryEventBuilder::new().name("b").property("src", "api").property("lang", "en").build().unwrap(),
        ];
        let keys = all_property_keys(&events);
        assert_eq!(keys, vec!["lang", "src"]);
    }

    #[test]
    fn test_has_property_value_true() {
        let events = vec![
            TelemetryEventBuilder::new().name("x").property("env", "prod").build().unwrap(),
        ];
        assert!(has_property_value(&events, "env", "prod"));
        assert!(!has_property_value(&events, "env", "dev"));
    }

    #[test]
    fn test_count_by_type() {
        let mut svc = TelemetryService::new(TelemetryLevel::Usage);
        svc.log_event("a", vec![], vec![]);
        svc.log_error("b", "err", None);
        svc.log_event("c", vec![], vec![]);
        let counts = count_by_type(svc.get_events());
        assert_eq!(counts.get("Event"), Some(&2));
        assert_eq!(counts.get("Error"), Some(&1));
    }

    #[test]
    fn test_most_recent_event() {
        let events: Vec<TelemetryEvent> = vec![];
        assert!(most_recent_event(&events).is_none());
    }

    #[test]
    fn events_grouped_by_name_counts() {
        let events = vec![
            TelemetryEventBuilder::new().name("open").build().unwrap(),
            TelemetryEventBuilder::new().name("save").build().unwrap(),
            TelemetryEventBuilder::new().name("open").build().unwrap(),
        ];
        let grouped = events_grouped_by_name(&events);
        assert_eq!(grouped.get("open"), Some(&2));
        assert_eq!(grouped.get("save"), Some(&1));
    }

    #[test]
    fn events_in_time_range_filters() {
        let e1 = TelemetryEventBuilder::new().name("a").timestamp(100).build().unwrap();
        let e2 = TelemetryEventBuilder::new().name("b").timestamp(200).build().unwrap();
        let e3 = TelemetryEventBuilder::new().name("c").timestamp(300).build().unwrap();
        let events = vec![e1, e2, e3];
        assert_eq!(events_in_time_range(&events, 150, 250).len(), 1);
    }

    #[test]
    fn event_time_span_computes() {
        let e1 = TelemetryEventBuilder::new().name("a").timestamp(100).build().unwrap();
        let e2 = TelemetryEventBuilder::new().name("b").timestamp(500).build().unwrap();
        assert_eq!(event_time_span(&[e1, e2]), 400);
        let e3 = TelemetryEventBuilder::new().name("c").timestamp(100).build().unwrap();
        assert_eq!(event_time_span(&[e3]), 0);
    }

    #[test]
    fn avg_measurement_computes() {
        let e1 = TelemetryEventBuilder::new().name("a").measurement("dur", 10.0).build().unwrap();
        let e2 = TelemetryEventBuilder::new().name("b").measurement("dur", 20.0).build().unwrap();
        assert!((avg_measurement(&[e1, e2], "dur").unwrap() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn measurement_min_max_computes() {
        let e1 = TelemetryEventBuilder::new().name("a").measurement("dur", 5.0).build().unwrap();
        let e2 = TelemetryEventBuilder::new().name("b").measurement("dur", 25.0).build().unwrap();
        let (min, max) = measurement_min_max(&[e1, e2], "dur").unwrap();
        assert!((min - 5.0).abs() < f64::EPSILON);
        assert!((max - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distinct_event_names_sorts() {
        let events = vec![
            TelemetryEventBuilder::new().name("open").build().unwrap(),
            TelemetryEventBuilder::new().name("save").build().unwrap(),
            TelemetryEventBuilder::new().name("open").build().unwrap(),
        ];
        assert_eq!(distinct_event_names(&events), vec!["open", "save"]);
    }

    #[test]
    fn events_with_measurements_filters() {
        let e1 = TelemetryEventBuilder::new().name("a").measurement("dur", 10.0).build().unwrap();
        let e2 = TelemetryEventBuilder::new().name("b").build().unwrap();
        let events = vec![e1, e2];
        assert_eq!(events_with_measurements(&events).len(), 1);
    }

    #[test]
    fn events_with_property_key_filters() {
        let e1 = TelemetryEventBuilder::new().name("a").property("env", "prod").build().unwrap();
        let e2 = TelemetryEventBuilder::new().name("b").property("src", "ui").build().unwrap();
        let events = vec![e1, e2];
        assert_eq!(events_with_property_key(&events, "env").len(), 1);
    }

    // -- TelemetryBatcher tests --

    #[test]
    fn batcher_flushes_at_capacity() {
        let mut batcher = TelemetryBatcher::new(2);
        let e = TelemetryEventBuilder::new().name("a").build().unwrap();
        assert!(!batcher.push(e.clone()));
        assert!(batcher.push(e.clone()));
        assert_eq!(batcher.flushed_batch_count(), 1);
        assert_eq!(batcher.pending_count(), 0);
    }

    #[test]
    fn batcher_manual_flush() {
        let mut batcher = TelemetryBatcher::new(10);
        let e = TelemetryEventBuilder::new().name("a").build().unwrap();
        batcher.push(e);
        assert_eq!(batcher.pending_count(), 1);
        batcher.flush();
        assert_eq!(batcher.pending_count(), 0);
        assert_eq!(batcher.flushed_batch_count(), 1);
    }

    #[test]
    fn batcher_drain_returns_batches() {
        let mut batcher = TelemetryBatcher::new(1);
        let e = TelemetryEventBuilder::new().name("a").build().unwrap();
        batcher.push(e.clone());
        batcher.push(e);
        let batches = batcher.drain_flushed();
        assert_eq!(batches.len(), 2);
        assert_eq!(batcher.flushed_batch_count(), 0);
    }

    // -- PiiScrubber tests --

    #[test]
    fn filter_scrubs_pii_keys() {
        let filter = PiiScrubber::with_defaults();
        let e = TelemetryEventBuilder::new()
            .name("login")
            .property("user_email", "alice@example.com")
            .property("action", "click")
            .build()
            .unwrap();
        let scrubbed = filter.scrub(&e);
        let email_val = scrubbed.properties.iter().find(|(k, _)| k == "user_email").unwrap();
        assert_eq!(email_val.1, "[REDACTED]");
        let action_val = scrubbed.properties.iter().find(|(k, _)| k == "action").unwrap();
        assert_eq!(action_val.1, "click");
    }

    #[test]
    fn filter_empty_leaves_intact() {
        let filter = PiiScrubber::empty();
        let e = TelemetryEventBuilder::new().name("x").property("password", "hunter2").build().unwrap();
        let scrubbed = filter.scrub(&e);
        let pw = scrubbed.properties.iter().find(|(k, _)| k == "password").unwrap();
        assert_eq!(pw.1, "hunter2");
    }

    #[test]
    fn filter_custom_rule() {
        let mut filter = PiiScrubber::empty();
        filter.add_rule("ssn", "***");
        assert_eq!(filter.rule_count(), 1);
        let e = TelemetryEventBuilder::new().name("x").property("user_ssn", "123").build().unwrap();
        let scrubbed = filter.scrub(&e);
        let v = scrubbed.properties.iter().find(|(k, _)| k == "user_ssn").unwrap();
        assert_eq!(v.1, "***");
    }

    // -- MetricStore tests --

    #[test]
    fn metrics_counter() {
        let mut m = MetricStore::new();
        m.increment_counter("requests", 5);
        m.increment_counter("requests", 3);
        assert_eq!(m.counter_value("requests"), 8);
        assert_eq!(m.counter_value("missing"), 0);
    }

    #[test]
    fn metrics_gauge() {
        let mut m = MetricStore::new();
        m.set_gauge("cpu", 0.75);
        assert_eq!(m.gauge_value("cpu"), Some(0.75));
        m.set_gauge("cpu", 0.50);
        assert_eq!(m.gauge_value("cpu"), Some(0.50));
    }

    #[test]
    fn metrics_histogram_stats() {
        let mut m = MetricStore::new();
        for v in [10.0, 20.0, 30.0, 40.0, 50.0] {
            m.record_histogram("latency", v);
        }
        let h = m.get_histogram("latency").unwrap();
        assert_eq!(h.mean(), Some(30.0));
        assert_eq!(h.min_value(), Some(10.0));
        assert_eq!(h.max_value(), Some(50.0));
        assert_eq!(h.percentile(50.0), Some(30.0));
    }

    #[test]
    fn metrics_total_count() {
        let mut m = MetricStore::new();
        m.increment_counter("a", 1);
        m.set_gauge("b", 1.0);
        m.record_histogram("c", 1.0);
        assert_eq!(m.total_metric_count(), 3);
    }

    // -- SessionDurationTracker tests --

    #[test]
    fn session_tracker_basic() {
        let mut t = SessionDurationTracker::new();
        t.start_session("s1", 100);
        assert_eq!(t.active_count(), 1);
        assert_eq!(t.completed_count(), 0);
        let dur = t.end_session("s1", 350);
        assert_eq!(dur, Some(250));
        assert_eq!(t.duration_ms("s1"), Some(250));
        assert_eq!(t.active_count(), 0);
        assert_eq!(t.completed_count(), 1);
    }

    #[test]
    fn session_tracker_average() {
        let mut t = SessionDurationTracker::new();
        t.start_session("a", 0);
        t.start_session("b", 0);
        t.end_session("a", 100);
        t.end_session("b", 200);
        assert_eq!(t.average_duration_ms(), Some(150.0));
    }

    #[test]
    fn session_tracker_end_unknown() {
        let mut t = SessionDurationTracker::new();
        assert_eq!(t.end_session("nope", 100), None);
        assert_eq!(t.average_duration_ms(), None);
    }

    #[test]
    fn telemetryConsentManager_new() {
        let s = TelemetryConsentManager::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn telemetryConsentManager_add_contains() {
        let mut s = TelemetryConsentManager::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn telemetryConsentManager_add_duplicate() {
        let mut s = TelemetryConsentManager::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn telemetryConsentManager_remove() {
        let mut s = TelemetryConsentManager::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn telemetryConsentManager_capacity() {
        let s = TelemetryConsentManager::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn telemetryConsentManager_search() {
        let mut s = TelemetryConsentManager::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn telemetryConsentManager_stats() {
        let mut s = TelemetryConsentManager::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn telemetryErrorClassifier_new() {
        let m = TelemetryErrorClassifier::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn telemetryErrorClassifier_add_find() {
        let mut m = TelemetryErrorClassifier::new();
        m.add(TelemetryErrorClassifierItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn telemetryErrorClassifier_priority_filter() {
        let mut m = TelemetryErrorClassifier::new();
        m.add(TelemetryErrorClassifierItem::new("a", "A").with_priority(TelemetryErrorClassifierPriority::High));
        m.add(TelemetryErrorClassifierItem::new("b", "B").with_priority(TelemetryErrorClassifierPriority::Low));
        m.add(TelemetryErrorClassifierItem::new("c", "C").with_priority(TelemetryErrorClassifierPriority::High));
        assert_eq!(m.by_priority(TelemetryErrorClassifierPriority::High).len(), 2);
    }

    #[test]
    fn telemetryErrorClassifier_remove() {
        let mut m = TelemetryErrorClassifier::new();
        m.add(TelemetryErrorClassifierItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn telemetryErrorClassifier_search() {
        let mut m = TelemetryErrorClassifier::new();
        m.add(TelemetryErrorClassifierItem::new("id1", "Hello World"));
        m.add(TelemetryErrorClassifierItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn telemetryErrorClassifier_total_weight() {
        let mut m = TelemetryErrorClassifier::new();
        m.add(TelemetryErrorClassifierItem::new("a", "A").with_priority(TelemetryErrorClassifierPriority::Critical));
        m.add(TelemetryErrorClassifierItem::new("b", "B").with_priority(TelemetryErrorClassifierPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn telemetryErrorClassifier_capacity_limit() {
        let mut m = TelemetryErrorClassifier::new().with_max_items(2);
        m.add(TelemetryErrorClassifierItem::new("1", "one"));
        m.add(TelemetryErrorClassifierItem::new("2", "two"));
        assert!(!m.add(TelemetryErrorClassifierItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn telemetryErrorClassifier_sorted_by_priority() {
        let mut m = TelemetryErrorClassifier::new();
        m.add(TelemetryErrorClassifierItem::new("lo", "Low").with_priority(TelemetryErrorClassifierPriority::Low));
        m.add(TelemetryErrorClassifierItem::new("hi", "High").with_priority(TelemetryErrorClassifierPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn telemetryErrorClassifier_item_metadata() {
        let mut item = TelemetryErrorClassifierItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn telemetryConsentManager_enabled_toggle() {
        let mut s = TelemetryConsentManager::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn telemetryErrorClassifier_priority_display() {
        assert_eq!(format!("{}", TelemetryErrorClassifierPriority::High), "high");
        assert_eq!(format!("{}", TelemetryErrorClassifierPriority::Low), "low");
    }

}
