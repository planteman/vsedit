//! Diagnostic markers service

use std::collections::HashMap;
use std::sync::Mutex;

use vsedit_events::{Emitter, Event};
use vsedit_uri::VsUri;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Severity of a diagnostic marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerSeverity {
    Hint,
    Info,
    Warning,
    Error,
}

/// Code attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerCode {
    String(String),
    Number(i32),
}

/// Tag that modifies how a diagnostic is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerTag {
    Unnecessary,
    Deprecated,
}

/// Information related to a diagnostic in another resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedInformation {
    pub uri: VsUri,
    pub message: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// A single diagnostic marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerData {
    pub severity: MarkerSeverity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<MarkerCode>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub related_information: Vec<RelatedInformation>,
    pub tags: Vec<MarkerTag>,
}

// ---------------------------------------------------------------------------
// Filter & Statistics
// ---------------------------------------------------------------------------

/// Filter for querying markers.
pub struct MarkerFilter {
    pub owner: Option<String>,
    pub uri: Option<VsUri>,
    pub severities: Option<Vec<MarkerSeverity>>,
    pub take: Option<usize>,
}

/// Aggregate counts by severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerStatistics {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

// ---------------------------------------------------------------------------
// MarkerService
// ---------------------------------------------------------------------------

/// Manages diagnostic markers per (owner, resource) pair.
pub struct MarkerService {
    markers: Mutex<HashMap<(String, VsUri), Vec<MarkerData>>>,
    on_marker_changed: Emitter<Vec<VsUri>>,
}

impl MarkerService {
    pub fn new() -> Self {
        Self {
            markers: Mutex::new(HashMap::new()),
            on_marker_changed: Emitter::new(),
        }
    }

    /// Set markers for a single resource owned by `owner`.
    pub fn change_one(&self, owner: &str, uri: &VsUri, markers: Vec<MarkerData>) {
        let key = (owner.to_string(), uri.clone());
        let mut map = self.markers.lock().unwrap();
        if markers.is_empty() {
            map.remove(&key);
        } else {
            map.insert(key, markers);
        }
        drop(map);
        self.on_marker_changed.fire(&vec![uri.clone()]);
    }

    /// Set markers for multiple resources owned by `owner`, firing a single event.
    pub fn change_all(&self, owner: &str, markers: Vec<(VsUri, Vec<MarkerData>)>) {
        let mut changed: Vec<VsUri> = Vec::new();
        let mut map = self.markers.lock().unwrap();
        for (uri, data) in markers {
            let key = (owner.to_string(), uri.clone());
            if data.is_empty() {
                map.remove(&key);
            } else {
                map.insert(key, data);
            }
            changed.push(uri);
        }
        drop(map);
        if !changed.is_empty() {
            self.on_marker_changed.fire(&changed);
        }
    }

    /// Read markers matching a filter. Returns `(uri, marker)` pairs.
    pub fn read(&self, filter: &MarkerFilter) -> Vec<(VsUri, MarkerData)> {
        let map = self.markers.lock().unwrap();
        let mut results: Vec<(VsUri, MarkerData)> = Vec::new();

        for ((owner, uri), data) in map.iter() {
            if let Some(ref fo) = filter.owner {
                if owner != fo {
                    continue;
                }
            }
            if let Some(ref fu) = filter.uri {
                if uri != fu {
                    continue;
                }
            }
            for marker in data {
                if let Some(ref sevs) = filter.severities {
                    if !sevs.contains(&marker.severity) {
                        continue;
                    }
                }
                results.push((uri.clone(), marker.clone()));
                if let Some(take) = filter.take {
                    if results.len() >= take {
                        return results;
                    }
                }
            }
        }
        results
    }

    /// Remove all markers for `owner` on the given URIs.
    pub fn remove(&self, owner: &str, uris: &[VsUri]) {
        let mut map = self.markers.lock().unwrap();
        let mut changed: Vec<VsUri> = Vec::new();
        for uri in uris {
            let key = (owner.to_string(), uri.clone());
            if map.remove(&key).is_some() {
                changed.push(uri.clone());
            }
        }
        drop(map);
        if !changed.is_empty() {
            self.on_marker_changed.fire(&changed);
        }
    }

    /// Subscribe to marker-change events.
    pub fn on_marker_changed(&self) -> Event<Vec<VsUri>> {
        self.on_marker_changed.event()
    }

    /// Aggregate statistics across all stored markers.
    pub fn get_statistics(&self) -> MarkerStatistics {
        let map = self.markers.lock().unwrap();
        let mut stats = MarkerStatistics {
            errors: 0,
            warnings: 0,
            infos: 0,
            hints: 0,
        };
        for data in map.values() {
            for m in data {
                match m.severity {
                    MarkerSeverity::Error => stats.errors += 1,
                    MarkerSeverity::Warning => stats.warnings += 1,
                    MarkerSeverity::Info => stats.infos += 1,
                    MarkerSeverity::Hint => stats.hints += 1,
                }
            }
        }
        stats
    }
}

impl Default for MarkerService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    fn error_marker(msg: &str, line: u32) -> MarkerData {
        MarkerData {
            severity: MarkerSeverity::Error,
            message: msg.to_string(),
            source: None,
            code: None,
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 1,
            related_information: vec![],
            tags: vec![],
        }
    }

    fn warning_marker(msg: &str, line: u32) -> MarkerData {
        MarkerData {
            severity: MarkerSeverity::Warning,
            message: msg.to_string(),
            source: None,
            code: None,
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 1,
            related_information: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn add_and_read_markers() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/foo.rs");

        svc.change_one("rust", &uri, vec![error_marker("E1", 1), error_marker("E2", 5)]);

        let results = svc.read(&MarkerFilter {
            owner: Some("rust".into()),
            uri: Some(uri.clone()),
            severities: None,
            take: None,
        });
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1.message, "E1");
        assert_eq!(results[1].1.message, "E2");
    }

    #[test]
    fn read_with_severity_filter() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/bar.rs");

        svc.change_one(
            "lint",
            &uri,
            vec![
                error_marker("err", 1),
                warning_marker("warn", 2),
            ],
        );

        let results = svc.read(&MarkerFilter {
            owner: None,
            uri: None,
            severities: Some(vec![MarkerSeverity::Warning]),
            take: None,
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.severity, MarkerSeverity::Warning);
    }

    #[test]
    fn read_with_take_limit() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/baz.rs");

        svc.change_one(
            "owner",
            &uri,
            vec![error_marker("a", 1), error_marker("b", 2), error_marker("c", 3)],
        );

        let results = svc.read(&MarkerFilter {
            owner: None,
            uri: None,
            severities: None,
            take: Some(2),
        });
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn remove_by_owner() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/rem.rs");

        svc.change_one("owner_a", &uri, vec![error_marker("a", 1)]);
        svc.change_one("owner_b", &uri, vec![error_marker("b", 1)]);

        svc.remove("owner_a", &[uri.clone()]);

        let all = svc.read(&MarkerFilter {
            owner: None,
            uri: Some(uri),
            severities: None,
            take: None,
        });
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].1.message, "b");
    }

    #[test]
    fn statistics() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/stats.rs");

        svc.change_one(
            "test",
            &uri,
            vec![
                error_marker("e1", 1),
                error_marker("e2", 2),
                warning_marker("w1", 3),
                MarkerData {
                    severity: MarkerSeverity::Info,
                    message: "i1".into(),
                    source: None,
                    code: None,
                    start_line: 4,
                    start_column: 1,
                    end_line: 4,
                    end_column: 1,
                    related_information: vec![],
                    tags: vec![],
                },
            ],
        );

        let stats = svc.get_statistics();
        assert_eq!(stats.errors, 2);
        assert_eq!(stats.warnings, 1);
        assert_eq!(stats.infos, 1);
        assert_eq!(stats.hints, 0);
    }

    #[test]
    fn change_events_fire() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/evt.rs");

        let fired: Arc<StdMutex<Vec<Vec<VsUri>>>> = Arc::new(StdMutex::new(Vec::new()));
        let fired_clone = Arc::clone(&fired);

        let _handle = svc.on_marker_changed().on(move |uris: &Vec<VsUri>| {
            fired_clone.lock().unwrap().push(uris.clone());
        });

        svc.change_one("o", &uri, vec![error_marker("x", 1)]);

        let events = fired.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], vec![uri.clone()]);
    }

    #[test]
    fn change_all_fires_single_event() {
        let svc = MarkerService::new();
        let u1 = VsUri::file("/a.rs");
        let u2 = VsUri::file("/b.rs");

        let fired: Arc<StdMutex<Vec<Vec<VsUri>>>> = Arc::new(StdMutex::new(Vec::new()));
        let fired_clone = Arc::clone(&fired);

        let _handle = svc.on_marker_changed().on(move |uris: &Vec<VsUri>| {
            fired_clone.lock().unwrap().push(uris.clone());
        });

        svc.change_all(
            "o",
            vec![
                (u1.clone(), vec![error_marker("a", 1)]),
                (u2.clone(), vec![warning_marker("b", 2)]),
            ],
        );

        let events = fired.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].len(), 2);
    }

    #[test]
    fn empty_markers_removes_entry() {
        let svc = MarkerService::new();
        let uri = VsUri::file("/empty.rs");

        svc.change_one("o", &uri, vec![error_marker("x", 1)]);
        assert_eq!(svc.get_statistics().errors, 1);

        svc.change_one("o", &uri, vec![]);
        assert_eq!(svc.get_statistics().errors, 0);
    }
}
