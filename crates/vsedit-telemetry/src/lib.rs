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


// ---------------------------------------------------------------------------
// vsedit-telemetry: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl TelemetryXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for TelemetryXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct TelemetryXRegistry {
    entries: Vec<TelemetryXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl TelemetryXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: TelemetryXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&TelemetryXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut TelemetryXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<TelemetryXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&TelemetryXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&TelemetryXConfig> {
        let mut sorted: Vec<&TelemetryXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&TelemetryXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> TelemetryXIterator<'_> {
        TelemetryXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct TelemetryXIterator<'a> {
    inner: std::slice::Iter<'a, TelemetryXConfig>,
}

impl<'a> Iterator for TelemetryXIterator<'a> {
    type Item = &'a TelemetryXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct TelemetryXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl TelemetryXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
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

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct TelemetryXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl TelemetryXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &TelemetryXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &TelemetryXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &TelemetryXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for TelemetryXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct TelemetryXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl TelemetryXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &TelemetryXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &TelemetryXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for TelemetryXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 175
// ---------------------------------------------------------------------------

/// Generic object pool `Xc175Pool<T>`.
pub struct Xc175Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc175Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc175PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc175Pool<T> {
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
    pub fn stats(&self) -> Xc175PoolStats {
        Xc175PoolStats {
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

impl<T> Default for Xc175Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc175Scheduler`.
pub struct Xc175Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc175Scheduler {
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

impl Default for Xc175Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_175 hash for the given byte slice.
pub fn xc_175_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_175 convention.
pub fn xc_175_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_14 deepening: state machine + event bus ---

/// States for the Xd14 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd14State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd14State {
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
pub struct Xd14Transition {
    pub from: Xd14State,
    pub to: Xd14State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd14StateMachine {
    current: Xd14State,
    history: Vec<Xd14Transition>,
    step_counter: usize,
}

impl Xd14StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd14State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd14State {
        self.current
    }

    pub fn history(&self) -> &[Xd14Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd14State) -> Result<Xd14State, String> {
        let allowed = match (self.current, target) {
            (Xd14State::Idle, Xd14State::Running) => true,
            (Xd14State::Running, Xd14State::Paused) => true,
            (Xd14State::Running, Xd14State::Done) => true,
            (Xd14State::Paused, Xd14State::Running) => true,
            (Xd14State::Paused, Xd14State::Done) => true,
            (Xd14State::Done, Xd14State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_14: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd14Transition {
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
            "Xd14SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd14State> {
        let prefix = "Xd14SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd14State::Idle),
            "Running" => Some(Xd14State::Running),
            "Paused" => Some(Xd14State::Paused),
            "Done" => Some(Xd14State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd14State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd14 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd14Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd14Event {
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

type Xd14HandlerFn = Box<dyn Fn(&Xd14Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd14EventBus {
    handlers: Vec<(usize, Option<String>, Xd14HandlerFn)>,
    next_id: usize,
    published: Vec<Xd14Event>,
}

impl Xd14EventBus {
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
        F: Fn(&Xd14Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd14Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd14Event) {
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

    pub fn published_events(&self) -> &[Xd14Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #12
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf12Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf12TrieNode {
    children: std::collections::HashMap<char, Xf12TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf12Trie {
    root: Xf12TrieNode,
    count: usize,
}

impl Xf12Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf12TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf12TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf12TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf12BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf12BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 174).
pub struct Xh174SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh174SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 216 as u64,
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

/// A compact bit set supporting boolean operations (variant 174).
pub struct Xh174BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh174BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 174).
pub struct Xi174Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi174Deque<T> {
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
pub struct Xi174Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi174Interval {
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

/// A simple interval tree (variant 174).
pub struct Xi174IntervalTree {
    xi_intervals: Vec<Xi174Interval>,
}

impl Xi174IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi174Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi174Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi174Interval) -> Vec<&Xi174Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi174Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi174Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi174Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi174Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi174Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi174Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 174) ---

/// Disjoint set / union-find for crate 174.
pub struct Xj174UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj174UnionFind {
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

const XJ174_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 174.
pub struct Xj174BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj174BTreeNode<K, V>>>,
    len: usize,
}

struct Xj174BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj174BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj174BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ174_BTREE_ORDER - 1
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
        let mid = XJ174_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj174BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj174BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj174BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj174BTreeNode::xj_new_leaf();
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


// --- xk_174 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk174SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk174SegmentTree {
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
pub struct Xk174DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk174DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_174).
#[derive(Debug, Clone)]
pub struct Xl174Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl174Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_174).
#[derive(Debug, Clone)]
pub struct Xl174SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl174SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
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


    #[test]
    fn telemetry_x_config_new() {
        let c = TelemetryXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn telemetry_x_config_builder() {
        let c = TelemetryXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn telemetry_x_config_display() {
        let c = TelemetryXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn telemetry_x_registry_insert_get() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn telemetry_x_registry_duplicate() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a")).unwrap();
        assert!(reg.insert(TelemetryXConfig::new("a")).is_err());
    }

    #[test]
    fn telemetry_x_registry_remove() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a")).unwrap();
        reg.insert(TelemetryXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn telemetry_x_registry_active_entries() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a")).unwrap();
        reg.insert(TelemetryXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn telemetry_x_registry_by_weight() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(TelemetryXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn telemetry_x_registry_tags() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(TelemetryXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn telemetry_x_registry_total_weight() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(TelemetryXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn telemetry_x_registry_iterator() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a")).unwrap();
        reg.insert(TelemetryXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn telemetry_x_cache_put_get() {
        let mut cache = TelemetryXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn telemetry_x_cache_eviction() {
        let mut cache = TelemetryXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn telemetry_x_cache_lru_order() {
        let mut cache = TelemetryXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn telemetry_x_cache_most_least_recent() {
        let mut cache = TelemetryXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn telemetry_x_formatter_entry() {
        let e = TelemetryXConfig::new("k").with_value("v");
        let fmt = TelemetryXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn telemetry_x_formatter_summary() {
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("a").with_weight(5)).unwrap();
        let fmt = TelemetryXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn telemetry_x_validator_valid() {
        let v = TelemetryXValidator::new();
        let c = TelemetryXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn telemetry_x_validator_empty_key() {
        let v = TelemetryXValidator::new();
        let c = TelemetryXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn telemetry_x_validator_require_value() {
        let v = TelemetryXValidator::new().require_value(true);
        let c = TelemetryXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn telemetry_x_validator_allowed_tags() {
        let v = TelemetryXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = TelemetryXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn telemetry_x_validator_validate_all() {
        let v = TelemetryXValidator::new();
        let mut reg = TelemetryXRegistry::new();
        reg.insert(TelemetryXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // ---- xc_ pool / scheduler tests – block 175 ----

    #[test]
    fn xc_175_pool_new_empty() {
        let pool: super::Xc175Pool<i32> = super::Xc175Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_175_pool_release_acquire() {
        let mut pool = super::Xc175Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_175_pool_acquire_empty() {
        let mut pool: super::Xc175Pool<i32> = super::Xc175Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_175_pool_full() {
        let mut pool = super::Xc175Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_175_pool_drain() {
        let mut pool = super::Xc175Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_175_pool_stats() {
        let mut pool = super::Xc175Pool::new(8);
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
    fn xc_175_pool_clear() {
        let mut pool = super::Xc175Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_175_pool_shrink() {
        let mut pool = super::Xc175Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_175_pool_default() {
        let pool: super::Xc175Pool<String> = super::Xc175Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_175_pool_extend() {
        let mut pool = super::Xc175Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_175_pool_retain() {
        let mut pool = super::Xc175Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_175_scheduler_round_robin() {
        let mut sched = super::Xc175Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_175_scheduler_empty() {
        let mut sched = super::Xc175Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_175_scheduler_reset() {
        let mut sched = super::Xc175Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_175_scheduler_add_remove() {
        let mut sched = super::Xc175Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_175_scheduler_targets() {
        let sched = super::Xc175Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_175_hash_empty() {
        assert_eq!(super::xc_175_hash(b""), 5381);
    }

    #[test]
    fn xc_175_hash_data() {
        let h = super::xc_175_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_175_hash(b"hello"), h);
    }

    #[test]
    fn xc_175_reverse_str() {
        assert_eq!(super::xc_175_reverse("abc"), "cba");
        assert_eq!(super::xc_175_reverse(""), "");
    }


    // --- xd_14 deepening tests ---

    #[test]
    fn xd_14_sm_initial_state() {
        let sm = Xd14StateMachine::new();
        assert_eq!(sm.current_state(), Xd14State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_14_sm_valid_idle_to_running() {
        let mut sm = Xd14StateMachine::new();
        assert!(sm.transition(Xd14State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd14State::Running);
    }

    #[test]
    fn xd_14_sm_valid_running_to_paused() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        assert!(sm.transition(Xd14State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd14State::Paused);
    }

    #[test]
    fn xd_14_sm_valid_running_to_done() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        assert!(sm.transition(Xd14State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd14State::Done);
    }

    #[test]
    fn xd_14_sm_valid_paused_to_running() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        sm.transition(Xd14State::Paused).unwrap();
        assert!(sm.transition(Xd14State::Running).is_ok());
    }

    #[test]
    fn xd_14_sm_valid_done_to_idle() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        sm.transition(Xd14State::Done).unwrap();
        assert!(sm.transition(Xd14State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd14State::Idle);
    }

    #[test]
    fn xd_14_sm_invalid_idle_to_done() {
        let mut sm = Xd14StateMachine::new();
        assert!(sm.transition(Xd14State::Done).is_err());
    }

    #[test]
    fn xd_14_sm_invalid_idle_to_paused() {
        let mut sm = Xd14StateMachine::new();
        assert!(sm.transition(Xd14State::Paused).is_err());
    }

    #[test]
    fn xd_14_sm_history_tracking() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        sm.transition(Xd14State::Paused).unwrap();
        sm.transition(Xd14State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd14State::Idle);
        assert_eq!(sm.history()[0].to, Xd14State::Running);
        assert_eq!(sm.history()[1].from, Xd14State::Running);
        assert_eq!(sm.history()[2].to, Xd14State::Done);
    }

    #[test]
    fn xd_14_sm_serialize_deserialize() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd14StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd14State::Running));
    }

    #[test]
    fn xd_14_sm_deserialize_invalid() {
        assert_eq!(Xd14StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_14_sm_reset() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd14State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_14_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd14EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd14Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_14_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd14EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd14Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd14Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_14_bus_unsubscribe() {
        let mut bus = Xd14EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_14_event_kind_and_payload() {
        let e = Xd14Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd14Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_14_bus_clear_history() {
        let mut bus = Xd14EventBus::new();
        bus.publish(Xd14Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_14_sm_step_counter_increments() {
        let mut sm = Xd14StateMachine::new();
        sm.transition(Xd14State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd14State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #12 --

    #[test]
    fn xf12_trie_insert_search() {
        let mut t = Xf12Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf12_trie_starts_with() {
        let mut t = Xf12Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf12_trie_remove() {
        let mut t = Xf12Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf12_trie_word_count() {
        let mut t = Xf12Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf12_trie_longest_prefix() {
        let mut t = Xf12Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf12_trie_all_words() {
        let mut t = Xf12Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf12_trie_autocomplete() {
        let mut t = Xf12Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf12_trie_empty_search() {
        let t = Xf12Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf12_bloom_add_contains() {
        let mut bf = Xf12BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf12_bloom_probably_absent() {
        let bf = Xf12BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf12_bloom_false_positive_rate() {
        let mut bf = Xf12BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf12_bloom_clear() {
        let mut bf = Xf12BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf12_bloom_union() {
        let mut a = Xf12BloomFilter::xf_new(512, 2);
        let mut b = Xf12BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf12_bloom_intersection_estimate() {
        let mut a = Xf12BloomFilter::xf_new(512, 2);
        let mut b = Xf12BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf12_bloom_union_size_mismatch() {
        let a = Xf12BloomFilter::xf_new(256, 2);
        let b = Xf12BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh174_skip_insert_contains() {
        let mut sl = super::Xh174SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh174_skip_remove() {
        let mut sl = super::Xh174SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh174_skip_len() {
        let mut sl = super::Xh174SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh174_skip_range_query() {
        let mut sl = super::Xh174SkipList::xh_new(4);
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
    fn xh174_skip_floor_ceiling() {
        let mut sl = super::Xh174SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh174_skip_rank() {
        let mut sl = super::Xh174SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh174_skip_empty() {
        let sl = super::Xh174SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh174_skip_duplicates() {
        let mut sl = super::Xh174SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh174_bitset_set_test() {
        let mut bs = super::Xh174BitSet::xh_new(256);
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
    fn xh174_bitset_clear_count() {
        let mut bs = super::Xh174BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh174_bitset_and_or_xor() {
        let mut a = super::Xh174BitSet::xh_new(128);
        let mut b = super::Xh174BitSet::xh_new(128);
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
    fn xh174_bitset_iter_ones() {
        let mut bs = super::Xh174BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh174_bitset_first_last() {
        let mut bs = super::Xh174BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh174_bitset_empty() {
        let bs = super::Xh174BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi174_deque_push_pop_back() {
        let mut dq = super::Xi174Deque::xi_new(4);
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
    fn xi174_deque_push_pop_front() {
        let mut dq = super::Xi174Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi174_deque_mixed_ops() {
        let mut dq = super::Xi174Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi174_deque_get_and_split() {
        let mut dq = super::Xi174Deque::xi_new(8);
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
    fn xi174_deque_rotate_left() {
        let mut dq = super::Xi174Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi174_deque_rotate_right() {
        let mut dq = super::Xi174Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi174_deque_grow() {
        let mut dq = super::Xi174Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi174_deque_empty() {
        let dq = super::Xi174Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi174_interval_tree_insert_query() {
        let mut tree = super::Xi174IntervalTree::xi_new();
        tree.xi_insert(super::Xi174Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi174Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi174Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi174_interval_tree_overlap() {
        let mut tree = super::Xi174IntervalTree::xi_new();
        tree.xi_insert(super::Xi174Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi174Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi174Interval::xi_new(12, 20));
        let q = super::Xi174Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi174_interval_tree_remove() {
        let mut tree = super::Xi174IntervalTree::xi_new();
        tree.xi_insert(super::Xi174Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi174Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi174_interval_tree_gaps() {
        let mut tree = super::Xi174IntervalTree::xi_new();
        tree.xi_insert(super::Xi174Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi174Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi174Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi174Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi174Interval::xi_new(8, 10));
    }

    #[test]
    fn xi174_interval_tree_merge() {
        let mut tree = super::Xi174IntervalTree::xi_new();
        tree.xi_insert(super::Xi174Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi174Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi174Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi174Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi174Interval::xi_new(10, 15));
    }

    #[test]
    fn xi174_interval_tree_all() {
        let mut tree = super::Xi174IntervalTree::xi_new();
        tree.xi_insert(super::Xi174Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi174Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi174_interval_tree_empty() {
        let tree = super::Xi174IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi174_interval_tree_contains_point() {
        let iv = super::Xi174Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 174) ---

    #[test]
    fn xj_174_uf_make_and_find() {
        let mut uf = super::Xj174UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_174_uf_union_connected() {
        let mut uf = super::Xj174UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_174_uf_component_count() {
        let mut uf = super::Xj174UnionFind::xj_new();
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
    fn xj_174_uf_component_size() {
        let mut uf = super::Xj174UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_174_uf_largest_component() {
        let mut uf = super::Xj174UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_174_uf_many_elements() {
        let mut uf = super::Xj174UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_174_uf_separate_components() {
        let mut uf = super::Xj174UnionFind::xj_new();
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
    fn xj_174_uf_path_compression() {
        let mut uf = super::Xj174UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_174_bt_insert_get() {
        let mut bt = super::Xj174BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_174_bt_contains_len() {
        let mut bt = super::Xj174BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_174_bt_replace() {
        let mut bt = super::Xj174BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_174_bt_remove() {
        let mut bt = super::Xj174BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_174_bt_keys_values() {
        let mut bt = super::Xj174BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_174_bt_range() {
        let mut bt = super::Xj174BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_174_bt_min_max() {
        let mut bt = super::Xj174BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_174_bt_many_inserts() {
        let mut bt = super::Xj174BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_174 segment tree tests ---

    #[test]
    fn xk_174_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk174SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_174_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk174SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_174_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk174SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_174_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk174SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_174_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk174SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_174_st_single_element() {
        let data = vec![42];
        let st = super::Xk174SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_174_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk174SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_174_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk174SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_174 disjoint intervals tests ---

    #[test]
    fn xk_174_di_add_and_count() {
        let mut di = super::Xk174DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_174_di_merge_overlap() {
        let mut di = super::Xk174DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_174_di_contains() {
        let mut di = super::Xk174DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_174_di_remove() {
        let mut di = super::Xk174DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_174_di_covered_length() {
        let mut di = super::Xk174DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_174_di_gaps() {
        let mut di = super::Xk174DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_174_di_merge_adjacent() {
        let mut di = super::Xk174DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_174_di_empty() {
        let di = super::Xk174DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_174_rope_new_empty() {
        let rope = super::Xl174Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_174_rope_from_str() {
        let rope = super::Xl174Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_174_rope_insert_at() {
        let mut rope = super::Xl174Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_174_rope_delete_range() {
        let mut rope = super::Xl174Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_174_rope_char_at() {
        let rope = super::Xl174Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_174_rope_split_concat() {
        let rope = super::Xl174Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_174_rope_line_count() {
        let rope = super::Xl174Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_174_rope_line_at() {
        let rope = super::Xl174Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_174_sa_build_and_search() {
        let sa = super::Xl174SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_174_sa_count() {
        let sa = super::Xl174SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_174_sa_longest_repeated() {
        let sa = super::Xl174SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_174_sa_all_positions() {
        let sa = super::Xl174SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_174_sa_len() {
        let sa = super::Xl174SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_174_sa_empty() {
        let sa = super::Xl174SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_174_rope_slice() {
        let rope = super::Xl174Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_174_sa_search_start() {
        let sa = super::Xl174SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
