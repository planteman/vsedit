//! Performance monitoring.

use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A timestamped performance mark.
#[derive(Debug, Clone, PartialEq)]
pub struct PerfMark {
    pub label: String,
    pub start_ns: u64,
    pub end_ns: Option<u64>,
}

/// A completed performance measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct PerfEntry {
    pub name: String,
    pub duration_ms: f64,
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Collects and queries performance marks and entries.
#[derive(Debug)]
pub struct PerfService {
    marks: Vec<PerfMark>,
    entries: Vec<PerfEntry>,
    enabled: bool,
}

impl PerfService {
    pub fn new() -> Self {
        Self {
            marks: Vec::new(),
            entries: Vec::new(),
            enabled: true,
        }
    }

    /// Records a performance mark with the current timestamp.
    pub fn mark(&mut self, label: impl Into<String>) {
        if !self.enabled {
            return;
        }
        self.marks.push(PerfMark {
            label: label.into(),
            start_ns: now_ns(),
            end_ns: None,
        });
    }

    /// Finds the most recent mark matching `label`, stamps its end time, and
    /// returns the elapsed duration in milliseconds.
    pub fn measure(&mut self, label: &str) -> Option<f64> {
        if !self.enabled {
            return None;
        }
        let now = now_ns();
        let mark = self.marks.iter_mut().rev().find(|m| m.label == label)?;
        mark.end_ns = Some(now);
        let duration_ms = (now.saturating_sub(mark.start_ns)) as f64 / 1_000_000.0;
        self.entries.push(PerfEntry {
            name: label.to_string(),
            duration_ms,
            timestamp: now,
        });
        Some(duration_ms)
    }

    /// Manually adds a named performance entry.
    pub fn add_entry(&mut self, name: impl Into<String>, duration_ms: f64) {
        self.entries.push(PerfEntry {
            name: name.into(),
            duration_ms,
            timestamp: now_ns(),
        });
    }

    /// Returns all recorded entries.
    pub fn get_entries(&self) -> &[PerfEntry] {
        &self.entries
    }

    /// Clears all marks and entries.
    pub fn clear(&mut self) {
        self.marks.clear();
        self.entries.clear();
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Computes the average duration across all entries with the given name.
    pub fn average_duration(&self, name: &str) -> Option<f64> {
        let (sum, count) = self
            .entries
            .iter()
            .filter(|e| e.name == name)
            .fold((0.0, 0u64), |(s, c), e| (s + e.duration_ms, c + 1));
        if count == 0 { None } else { Some(sum / count as f64) }
    }
}

impl Default for PerfService {
    fn default() -> Self {
        Self::new()
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_and_measure() {
        let mut svc = PerfService::new();
        svc.mark("load");
        let dur = svc.measure("load");
        assert!(dur.is_some());
        assert!(dur.unwrap() >= 0.0);
        assert_eq!(svc.get_entries().len(), 1);
        assert_eq!(svc.get_entries()[0].name, "load");
    }

    #[test]
    fn measure_unknown_returns_none() {
        let mut svc = PerfService::new();
        assert!(svc.measure("nope").is_none());
    }

    #[test]
    fn average_duration_works() {
        let mut svc = PerfService::new();
        svc.add_entry("render", 10.0);
        svc.add_entry("render", 20.0);
        svc.add_entry("other", 100.0);
        let avg = svc.average_duration("render").unwrap();
        assert!((avg - 15.0).abs() < f64::EPSILON);
        assert!(svc.average_duration("missing").is_none());
    }

    #[test]
    fn disabled_service_skips_marks() {
        let mut svc = PerfService::new();
        svc.set_enabled(false);
        assert!(!svc.is_enabled());
        svc.mark("x");
        assert!(svc.marks.is_empty());
        assert!(svc.measure("x").is_none());
    }

    #[test]
    fn clear_removes_everything() {
        let mut svc = PerfService::new();
        svc.mark("a");
        svc.add_entry("b", 5.0);
        svc.clear();
        assert!(svc.marks.is_empty());
        assert!(svc.get_entries().is_empty());
    }
}
